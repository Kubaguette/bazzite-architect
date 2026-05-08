use crate::commands::logs;
use crate::core::util::{build_host_command, normalize_home_path};
use serde::Serialize;
use std::path::PathBuf;
use sysinfo::Disks;
use tauri::Emitter;

#[derive(Serialize)]
/// Information about a candidate storage drive suitable for storing container
/// data.
///
/// Why: provides a compact, frontend-friendly representation with human-meaningful
/// units (GB) so the UI can present capacity and suitability heuristics without
/// invoking low-level OS APIs directly.
pub struct DriveInfo {
    pub name: String,
    pub file_system: String,
    pub mount_point: String,
    pub available_gb: u64,
    pub total_gb: u64,
}

/// Determine Podman's active storage GraphRoot, normalized for UI use.
///
/// Why: Podman's storage location may be configured independently from HOME. The
/// command canonicalizes the path and normalizes /var.home -> /home to present a
/// consistent view to the user.
///
/// # Errors
/// Returns Err when the podman command fails or returns no GraphRoot.
#[tauri::command]
pub fn get_active_storage_path(app: tauri::AppHandle) -> Result<String, String> {
    let output = build_host_command("podman")
        .args(["info", "--format", "{{.Store.GraphRoot}}"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if raw.is_empty() {
                Err("Podman did not return a GraphRoot path.".to_string())
            } else {
                let canonicalized =
                    std::fs::canonicalize(&raw).unwrap_or_else(|_| PathBuf::from(&raw));
                let path = normalize_home_path(&canonicalized.to_string_lossy());
                logs::info(&app, "storage", format!("active storage: {}", path));
                Ok(path)
            }
        }
        Ok(o) => Err(format!(
            "Error in 'podman info': {}",
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => {
            logs::error(&app, "storage", format!("podman info failed: {}", e));
            Err(format!("Failed to execute 'podman info': {}", e))
        }
    }
}

/// Scan the system for drives suitable for container storage (home/system/external).
///
/// Architectural intent / Why:
/// - Filters out overlay/ghost mounts and unsupported filesystems to avoid
///   suggesting locations that will lead to runtime errors or poor performance.
/// - Uses heuristics to include the user's home and common external mounts but
///   keeps the list small so the UI can present a concise set of options.
///
/// # Errors
/// This function currently never returns Err for scanning logic; failures are
/// surfaced via logging and by returning an empty set.
#[tauri::command]
pub fn scan_drives(app: tauri::AppHandle) -> Result<Vec<DriveInfo>, String> {
    let disks = Disks::new_with_refreshed_list();
    let mut result: Vec<DriveInfo> = Vec::new();
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());

    for disk in disks.list() {
        let mount_point = disk.mount_point().to_string_lossy().to_string();
        let is_external = mount_point.starts_with("/run/media");
        let is_system = mount_point == "/";
        let is_home = mount_point == "/home" || mount_point == "/var/home";

        if !is_external && !is_system && !is_home {
            continue;
        }

        let fs_string = disk.file_system().to_string_lossy().to_string();
        let fs_lower = fs_string.to_lowercase();

        let is_ghost_mount = fs_lower.contains("overlay")
            || mount_point.contains("podman")
            || mount_point.contains("containers");

        let is_supported_linux_fs = matches!(fs_lower.as_str(), "ext4" | "btrfs" | "xfs");

        if is_external && (is_ghost_mount || !is_supported_linux_fs) {
            continue;
        }

        let resolved_mount_point = if is_system || is_home {
            home_dir.clone()
        } else {
            mount_point
        };

        let total_space_gb = disk.total_space() / 1024 / 1024 / 1024;
        if total_space_gb <= 5 {
            continue;
        }

        if result.iter().any(|d| d.mount_point == resolved_mount_point) {
            continue;
        }

        result.push(DriveInfo {
            name: disk.name().to_string_lossy().to_string(),
            file_system: fs_string,
            mount_point: resolved_mount_point,
            available_gb: disk.available_space() / 1024 / 1024 / 1024,
            total_gb: total_space_gb,
        });
    }

    logs::info(
        &app,
        "storage",
        format!("scan_drives: {} entries", result.len()),
    );

    let _ = app.emit(
        "app-notification",
        serde_json::json!({ "message": "Drives detected", "type": "success" }),
    );

    Ok(result)
}

/// Configure Podman to use a specific storage graphroot by writing
/// ~/.config/containers/storage.conf and restarting user podman sockets.
///
/// Why: changing Podman's graphroot is potentially disruptive; this function
/// performs safe checks, writes a minimal overlay-based configuration, and
/// restarts the user podman socket to pick up changes. It also writes a small
/// tracker ignore file to avoid desktop indexers scanning large storage trees.
///
/// # Errors
/// Returns Err on filesystem errors (writing/removing the config) or when HOME
/// cannot be determined.
#[tauri::command]
pub fn apply_storage_setup(app: tauri::AppHandle, target_path: String) -> Result<String, String> {
    let home_dir = std::env::var("HOME").map_err(|_| "Failed to read HOME".to_string())?;
    let normalized_target = normalize_home_path(&target_path);
    let normalized_home = normalize_home_path(&home_dir);

    let config_path = PathBuf::from(format!("{}/.config/containers/storage.conf", home_dir));
    let final_graphroot = if normalized_target == normalized_home {
        if config_path.exists() {
            std::fs::remove_file(&config_path)
                .map_err(|e| format!("Failed to remove storage.conf: {}", e))?;
        }
        None
    } else {
        let graphroot = format!("{}/podman-data", target_path.trim_end_matches('/'));
        let parent = config_path.parent().ok_or("Invalid storage.conf path")?;
        std::fs::create_dir_all(parent).map_err(|e| {
            logs::error(&app, "storage", format!("mkdir config failed: {}", e));
            format!("Failed to create config directory: {}", e)
        })?;
        let content = format!(
            "[storage]\ndriver = \"overlay\"\ngraphroot = \"{}\"\n",
            graphroot
        );
        std::fs::write(&config_path, content).map_err(|e| {
            logs::error(&app, "storage", format!("write storage.conf failed: {}", e));
            format!("Failed to write storage.conf: {}", e)
        })?;
        Some(graphroot)
    };

    let _ = build_host_command("systemctl")
        .args(["--user", "stop", "podman.socket", "podman.service"])
        .output();
    let _ = build_host_command("systemctl")
        .args(["--user", "start", "podman.socket"])
        .output();

    // If we've set a custom GraphRoot, try to create the directory and
    // place .trackerignore/.nomedia to prevent desktop indexers from
    // crawling heavy container storage locations.
    if let Some(ref p) = final_graphroot {
        // attempt to create the graphroot directory
        if let Err(e) = std::fs::create_dir_all(p) {
            logs::error(&app, "storage", format!("Failed to create graphroot {}: {}", p, e));
        } else {
            // create .trackerignore
            let tracker_path = PathBuf::from(&p).join(".trackerignore");
            if let Err(e) = std::fs::write(&tracker_path, "") {
                logs::error(&app, "storage", format!("Could not write {}: {}", tracker_path.display(), e));
            } else {
                logs::info(&app, "storage", format!("Wrote {}", tracker_path.display()));
            }
            // optional .nomedia
            let nomedia_path = PathBuf::from(&p).join(".nomedia");
            if let Err(e) = std::fs::write(&nomedia_path, "") {
                logs::error(&app, "storage", format!("Could not write {}: {}", nomedia_path.display(), e));
            } else {
                logs::info(&app, "storage", format!("Wrote {}", nomedia_path.display()));
            }
        }
    }

    let msg = match final_graphroot {
        Some(p) => format!(
            "✅ Podman storage configured: {}\nNote: Existing images/containers are not migrated automatically (MVP).",
            normalize_home_path(&p)
        ),
        None => "✅ Podman storage reset to default in HOME.".to_string(),
    };

    logs::info(&app, "storage", msg.clone());
    Ok(msg)
}
