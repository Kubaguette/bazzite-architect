use crate::commands::env::resolve_host_project_path;
use crate::commands::logs;
use crate::core::util::build_host_command;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;
use tauri::Emitter;
use tauri::Manager;

#[derive(Serialize)]
/// Space usage information for a single environment.
///
/// Why: provides a compact payload for the UI to show both project-local
/// storage and optional container writable-layer size. project_bytes is left as
/// None in this iteration to avoid expensive synchronous filesystem scans; the
/// frontend can request directory sizes separately via get_dir_size.
pub struct EnvironmentSpaceInfo {
    pub project_path: Option<String>,
    pub project_bytes: Option<u64>,
    pub container_size_rw: Option<u64>,
}

fn find_container_id_for_env(name: &str) -> Option<String> {
    let out = build_host_command("distrobox")
        .args(["list", "--no-color"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    for line in txt.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("ID") && trimmed.contains("NAME") {
            continue;
        }
        if trimmed.starts_with('-') || trimmed.starts_with('=') {
            continue;
        }
        if trimmed.contains('|') {
            let parts: Vec<String> = trimmed
                .split('|')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 2 && parts[1] == name {
                return Some(parts[0].clone());
            }
        } else {
            let ws: Vec<&str> = trimmed.split_whitespace().collect();
            if ws.len() >= 2 && ws[1] == name {
                return Some(ws[0].to_string());
            }
        }
    }
    None
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct SizeCacheEntry {
    size: u64,
    mtime: u64,
}

#[derive(Default)]
struct SizeCache {
    map: HashMap<String, SizeCacheEntry>,
    file: Option<PathBuf>,
}

static SIZE_CACHE: OnceLock<Mutex<SizeCache>> = OnceLock::new();

fn ensure_cache_file(app: &tauri::AppHandle) -> PathBuf {
    let base = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    let _ = fs::create_dir_all(&base);
    base.join("sizes.json")
}

fn load_cache(app: &tauri::AppHandle) {
    let mut cache = SIZE_CACHE
        .get_or_init(|| Mutex::new(SizeCache::default()))
        // SAFETY: this mutex is initialized above in a single-threaded fashion on
        // first access. Locking may panic only if the mutex is poisoned due to a
        // thread panic while holding it; in this process-level cache that would
        // indicate a serious internal error and it is acceptable to propagate
        // the problem by unwrapping. If poisoned-recovery is required later,
        // replace unwrap() with lock().unwrap_or_else(|g| g.into_inner()).
        .lock()
        .unwrap();
    if cache.file.is_some() {
        return;
    }
    let path = ensure_cache_file(app);
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(map) = serde_json::from_slice::<HashMap<String, SizeCacheEntry>>(&bytes) {
            cache.map = map;
        }
    }
    cache.file = Some(path);
}

fn save_cache() {
    if let Some(m) = SIZE_CACHE.get() {
        if let Ok(cache) = m.lock() {
            if let Some(path) = &cache.file {
                let _ = fs::write(
                    path,
                    serde_json::to_vec_pretty(&cache.map).unwrap_or_default(),
                );
            }
        }
    }
}

static DIR_SIZE_GEN: AtomicU64 = AtomicU64::new(0);

fn dir_size_parallel(path: &Path, gen_at_start: u64) -> u64 {
    let mut total: u64 = 0;
    // IO throttle: limit jwalk to at most 2 threads
    let walker = jwalk::WalkDir::new(path).parallelism(jwalk::Parallelism::RayonNewPool(2));
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if DIR_SIZE_GEN.load(Ordering::Relaxed) != gen_at_start {
            break;
        }
        if let Ok(m) = entry.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            }
        } else if let Ok(m2) = std::fs::metadata(entry.path()) {
            if m2.is_file() {
                total = total.saturating_add(m2.len());
            }
        }
    }
    total
}

#[tauri::command]
/// Cancel any in-progress directory size computations.
///
/// Why: long-running directory walks are cooperative and check a generation
/// counter to abort early. Incrementing the generation ensures currently
/// running workers observe the change and exit without expensive cleanup.
pub fn cancel_dir_size_jobs(app: tauri::AppHandle) {
    // Bump generation; active walkers will observe the change and exit quickly
    DIR_SIZE_GEN.fetch_add(1, Ordering::Relaxed);
    logs::info(&app, "space", "cancel_dir_size_jobs called");
}

#[derive(Serialize, Clone)]
struct SizeUpdatePayload {
    path: String,
    size: u64,
}

#[tauri::command]
/// Compute (or retrieve from cache) the byte size of a directory and emit a
/// size-update event to the provided webview window.
///
/// Architectural intent / Why:
/// - Uses an on-disk cache to avoid repeated expensive IO for stable paths.
/// - Uses podman/podman system df as a fast-path when the path looks like a
///   container graphroot to avoid traversing potentially massive storage trees.
/// - When falling back to walking the filesystem, uses a throttled parallel
///   walker (bounded Rayon thread-pool) to limit IO concurrency and avoid
///   starving the system.
/// - Supports cancellation via a generation counter so the UI can abort
///   long-running operations without blocking.
///
/// # Errors
/// Returns Err(String) if the background task join is cancelled or panics; the
/// returned error contains the join diagnostic.
#[tauri::command]
pub async fn get_dir_size(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    path: String,
) -> Result<(), String> {
    logs::info(&app, "space", format!("dir_size start: {}", path));
    load_cache(&app);
    let p = PathBuf::from(&path);

    // If cached and mtime matches, emit immediately without touching disk heavy IO
    let current_mtime = fs::metadata(&p)
        .and_then(|m| m.modified())
        .and_then(|t| {
            t.duration_since(UNIX_EPOCH)
                .map_err(|_| std::io::ErrorKind::Other.into())
        })
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Some(mutex) = SIZE_CACHE.get() {
        if let Ok(cache) = mutex.lock() {
            if let Some(entry) = cache.map.get(&path) {
                if entry.mtime == current_mtime {
                    let _ = window.emit(
                        "size-update",
                        SizeUpdatePayload {
                            path: path.clone(),
                            size: entry.size,
                        },
                    );
                    return Ok(());
                }
            }
        }
    }

    // Otherwise compute and update cache
    let gen = DIR_SIZE_GEN.load(Ordering::Relaxed);
    let size = tokio::task::spawn_blocking(move || {
        if !p.exists() || !p.is_dir() {
            0
        } else {
            // Fast-path: if this path looks like Podman's storage location, ask
            // Podman for a summary via the CLI instead of walking the filesystem.
            let pstr = p.to_string_lossy().to_string();
            if pstr.contains("podman") || pstr.contains("containers") || pstr.contains("/var/lib/containers") {
                // Try 'podman system df --format json' and sum any numeric fields
                // whose keys contain "size". This avoids a deep recursive walk
                // through the graphroot which can trigger system indexers.
                if let Ok(out) = build_host_command("podman")
                    .args(["system", "df", "--format", "json"]) 
                    .output()
                {
                    if out.status.success() {
                        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                            // Try structured parsing for common Podman JSON fields first
                            // 1) Top-level LayersSize (present in many Podman versions)
                            if let Some(ls) = json_val.get("LayersSize") {
                                if let Some(n) = ls.as_u64() {
                                    return n;
                                }
                            }

                            // 2) Images array: sum known fields like "Size", "VirtualSize", "SizeBytes"
                            if let Some(images) = json_val.get("Images") {
                                if let Some(arr) = images.as_array() {
                                    let mut sum = 0u64;
                                    for img in arr {
                                        if let Some(n) = img.get("Size").and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                        if let Some(n) = img.get("VirtualSize").and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                        if let Some(n) = img.get("SizeBytes").and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                    }
                                    if sum > 0 {
                                        return sum;
                                    }
                                }
                            }

                            // 3) Containers array: sum known keys like "SizeRootFs", "SizeRw"
                            if let Some(conts) = json_val.get("Containers") {
                                if let Some(arr) = conts.as_array() {
                                    let mut sum = 0u64;
                                    for c in arr {
                                        if let Some(n) = c.get("SizeRootFs").and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                        if let Some(n) = c.get("SizeRw").and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                        if let Some(n) = c.get("Size") .and_then(|v| v.as_u64()) {
                                            sum = sum.saturating_add(n);
                                            continue;
                                        }
                                    }
                                    if sum > 0 {
                                        return sum;
                                    }
                                }
                            }

                            // 4) As a last-ditch structured attempt, sum any numeric fields that include "size" (case-insensitive)
                            fn sum_sizes(v: &serde_json::Value) -> u64 {
                                match v {
                                    serde_json::Value::Object(map) => map.iter().map(|(k, vv)| {
                                        let mut s = 0u64;
                                        if k.to_lowercase().contains("size") {
                                            if let Some(n) = vv.as_u64() {
                                                s = s.saturating_add(n);
                                            }
                                        }
                                        s.saturating_add(sum_sizes(vv))
                                    }).sum(),
                                    serde_json::Value::Array(arr) => arr.iter().map(sum_sizes).sum(),
                                    _ => 0u64,
                                }
                            }
                            let total = sum_sizes(&json_val);
                            if total > 0 {
                                return total;
                            }
                        }
                    }
                }
            }

            // Fallback: do a parallel directory walk (throttled).
            dir_size_parallel(&p, gen)
        }
    })
    .await
    .map_err(|e| format!("join error: {e:?}"))?;

    // Update cache and emit
    if let Some(mutex) = SIZE_CACHE.get() {
        if let Ok(mut cache) = mutex.lock() {
            cache.map.insert(
                path.clone(),
                SizeCacheEntry {
                    size,
                    mtime: current_mtime,
                },
            );
        }
    }
    save_cache();

    let _ = window.emit(
        "size-update",
        SizeUpdatePayload {
            path: path.clone(),
            size,
        },
    );

    logs::info(
        &app,
        "space",
        format!("dir_size done: {} -> {} bytes", path, size),
    );
    Ok(())
}

#[tauri::command]
/// Resolve a host-side project path for a given environment name.
///
/// Why: this encapsulates the heuristics used to map a container's $HOME to a
/// local filesystem path (findmnt, podman inspect, /var/home mapping and a
/// simple inferred-home heuristic). Exposing this as a command centralizes the
/// logic and makes it easier for the UI to open the right folder.
///
/// # Errors
/// Returns Err if the provided environment name is empty.
#[tauri::command]
pub fn resolve_project_path(name: String) -> Result<Option<String>, String> {
    let env_name = name.trim().to_string();
    if env_name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }
    Ok(resolve_host_project_path(&env_name))
}

#[tauri::command]
/// Retrieve space usage metadata for an environment.
///
/// Why: the command returns opt-in container size information because computing
/// container image/writable-layer sizes may be slow and requires calling out to
/// external tooling. The implementation delegates heavier tasks to a blocking
/// thread pool to avoid blocking the async runtime.
///
/// # Errors
/// Returns Err if the provided environment name is empty or if the blocking
/// join is cancelled or panics.
#[tauri::command]
pub async fn get_environment_space(
    app: tauri::AppHandle,
    name: String,
    include_container_size: Option<bool>,
) -> Result<EnvironmentSpaceInfo, String> {
    let env_name = name.trim().to_string();
    if env_name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }
    let include = include_container_size.unwrap_or(false);

    logs::info(
        &app,
        "space",
        format!(
            "get_environment_space start: name='{}' include_container_size={}",
            env_name, include
        ),
    );

    let env_for_calc = env_name.clone();
    let info = tokio::task::spawn_blocking(move || {
        let mut project_path: Option<String> = None;
        let mut container_size_rw: Option<u64> = None;
        if include {
            project_path = resolve_host_project_path(&env_for_calc);
            if let Some(cid) = find_container_id_for_env(&env_for_calc) {
                let cmd = format!("podman inspect --size {} --format '{{{{.SizeRw}}}}'", cid);
                if let Ok(out) = build_host_command("sh").args(["-lc", &cmd]).output() {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if let Ok(v) = s.parse::<u64>() {
                            container_size_rw = Some(v);
                        }
                    }
                }
            }
        }
        EnvironmentSpaceInfo {
            project_path,
            project_bytes: None,
            container_size_rw,
        }
    })
    .await
    .map_err(|e| format!("join error: {e:?}"))?;

    if let Some(bytes) = info.container_size_rw {
        logs::info(
            &app,
            "space",
            format!(
                "get_environment_space done: name='{}' container_size_rw={} bytes",
                env_name, bytes
            ),
        );
    } else {
        logs::info(
            &app,
            "space",
            format!("get_environment_space done: name='{}' (no size)", env_name),
        );
    }
    Ok(info)
}
