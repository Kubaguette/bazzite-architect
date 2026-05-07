use crate::commands::logs;
use crate::core::environment::{
    self, CreateEnvironmentParams, EnvironmentManifest, ProgressKind, ProgressUpdate,
};
use crate::core::util::{build_host_command, build_host_command_async};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

const CREATE_ENV_EVENT: &str = "creation-progress";

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum ProgressLevel {
    Info,
    Error,
}

#[derive(Serialize, Clone)]
struct ProgressPayload {
    stage: String,
    message: String,
    level: ProgressLevel,
    done: bool,
    success: Option<bool>,
}

impl ProgressPayload {
    fn from_update(update: ProgressUpdate) -> Self {
        Self {
            stage: update.stage.to_string(),
            message: update.message,
            level: match update.kind {
                ProgressKind::Info => ProgressLevel::Info,
                ProgressKind::Error => ProgressLevel::Error,
            },
            done: false,
            success: None,
        }
    }

    fn completion(
        stage: &'static str,
        message: String,
        level: ProgressLevel,
        success: Option<bool>,
    ) -> Self {
        Self {
            stage: stage.to_string(),
            message,
            level,
            done: true,
            success,
        }
    }
}

#[derive(Deserialize)]
/// Request payload for creating an environment from the frontend.
///
/// Why: mirrors the minimal fields the UI needs to request environment
/// creation. The `home_mount` field is optional so the backend can apply
/// sensible defaults and perform central validation.
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub template: String,
    #[serde(rename = "homeMount", alias = "home_mount")]
    pub home_mount: Option<String>,
}

#[derive(Serialize)]
/// Public description of an environment container returned to the UI.
///
/// Why: returns a compact row-like representation suitable for listing views
/// without transmitting full container metadata.
pub struct EnvironmentInfo {
    pub name: String,
    pub image: String,
    pub status: String,
    pub container_id: String,
}

#[tauri::command]
/// List available distrobox environments (containers) on the host.
///
/// Why: abstracts parsing of distrobox CLI output into a structured format
/// consumed by the UI. Parsing tolerates common table and fenced output
/// formats to be resilient across versions.
///
/// # Errors
/// Returns Err when the underlying distrobox CLI cannot be executed or
/// returns an error status.
pub fn list_environments(app: tauri::AppHandle) -> Result<Vec<EnvironmentInfo>, String> {
    let output = build_host_command("distrobox")
        .args(["list", "--no-color"])
        .output()
        .map_err(|e| format!("Failed to execute 'distrobox list': {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut environments = Vec::new();

    for line in text.lines() {
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
            if parts.len() >= 4 {
                environments.push(EnvironmentInfo {
                    container_id: parts[0].clone(),
                    name: parts[1].clone(),
                    status: parts[2].clone(),
                    image: parts[3].clone(),
                });
                continue;
            }
        }

        let ws: Vec<&str> = trimmed.split_whitespace().collect();
        if ws.len() >= 4 {
            environments.push(EnvironmentInfo {
                container_id: ws[0].to_string(),
                name: ws[1].to_string(),
                status: ws[2].to_string(),
                image: ws[3].to_string(),
            });
        }
    }

    logs::info(
        &app,
        "env",
        format!("Listed {} environments", environments.len()),
    );
    Ok(environments)
}

/// Entry point invoked by the frontend to create an environment. This command
/// schedules the heavyweight creation work on the background runtime and
/// immediately returns control to the UI.
///
/// Why: creating an environment involves long-running external calls and file
/// scaffolding. Scheduling the work asynchronously prevents blocking the
/// main invoke thread and allows progress to be streamed back via events.
///
/// # Errors
/// Returns Err only if the request payload validation fails synchronously
/// (e.g. empty name). Runtime failures during creation are reported via
/// progress events and notifications emitted from the background task.
#[tauri::command]
pub async fn create_environment(
    app: tauri::AppHandle,
    request: CreateEnvironmentRequest,
) -> Result<(), String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }
    let template = request.template.trim().to_string();
    let home_mount = request
        .home_mount
        .map(|hm| hm.trim().to_string())
        .filter(|value| !value.is_empty());

    let params = CreateEnvironmentParams {
        name: name.clone(),
        template: template.clone(),
        home_mount,
    };

    logs::info(
        &app,
        "env",
        format!("Create start: name='{}' template='{}'", name, template),
    );

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let emit_progress = |update: ProgressUpdate| {
            let payload = ProgressPayload::from_update(update.clone());
            let kind = update.kind.clone();
            let message = update.message.clone();
            let _ = app_handle.emit(CREATE_ENV_EVENT, &payload);
            match kind {
                ProgressKind::Info => logs::info(&app_handle, "env", message),
                ProgressKind::Error => logs::error(&app_handle, "env", message),
            }
        };

        match environment::create_environment(params, |update| emit_progress(update)).await {
            Ok(result) => {
                let payload = ProgressPayload::completion(
                    "complete",
                    result.message.clone(),
                    ProgressLevel::Info,
                    Some(true),
                );
                let _ = app_handle.emit(CREATE_ENV_EVENT, &payload);
                logs::info(&app_handle, "env", result.message.clone());
                let _ = app_handle.emit(
                    "app-notification",
                    serde_json::json!({ "message": "Environment created", "type": "success" }),
                );
            }
            Err(err) => {
                let payload = ProgressPayload::completion(
                    "error",
                    err.clone(),
                    ProgressLevel::Error,
                    Some(false),
                );
                let _ = app_handle.emit(CREATE_ENV_EVENT, &payload);
                logs::error(&app_handle, "env", err.clone());
                let _ = app_handle.emit(
                    "app-notification",
                    serde_json::json!({ "message": "Environment creation failed", "type": "error" })
                );
            }
        }
    });

    Ok(())
}

#[derive(Deserialize)]
/// Request payload for deleting an environment. delete_project controls whether
/// the associated project directory should also be removed (subject to safety
/// checks).
pub struct DeleteEnvironmentRequest {
    pub name: String,
    #[serde(rename = "deleteProject")]
    pub delete_project: bool,
}

/// Resolve a probable host project path for the named environment using a
/// sequence of heuristics (findmnt, podman inspect, /var.home mapping, and an
/// inferred $HOME-based path).
///
/// Why: this central helper encapsulates platform-specific heuristics so other
/// commands can reuse a single, testable implementation.
pub fn resolve_host_project_path(env_name: &str) -> Option<String> {
    let home_out = build_host_command("distrobox")
        .args([
            "enter",
            env_name,
            "--",
            "bash",
            "-lc",
            "printf %s \"$HOME\"",
        ])
        .output()
        .ok()?;
    if !home_out.status.success() {
        return None;
    }
    let container_home = String::from_utf8_lossy(&home_out.stdout).trim().to_string();
    if container_home.is_empty() {
        return None;
    }

    let findmnt_out = build_host_command("distrobox")
        .args([
            "enter",
            env_name,
            "--",
            "bash",
            "-lc",
            "if command -v findmnt >/dev/null 2>&1; then findmnt -n -o SOURCE --target \"$HOME\"; fi",
        ])
        .output()
        .ok();
    let mut findmnt_src_path = String::new();
    if let Some(out) = &findmnt_out {
        if out.status.success() {
            let out_s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            for token in out_s.split_whitespace() {
                if let (Some(lb), Some(rb)) = (token.find('['), token.find(']')) {
                    if rb > lb + 1 {
                        let inner = &token[lb + 1..rb];
                        if inner.starts_with('/') {
                            findmnt_src_path = inner.to_string();
                            break;
                        }
                    }
                }
            }
            if findmnt_src_path.is_empty() && out_s.starts_with('/') {
                findmnt_src_path = out_s;
            }
        }
    }
    if !findmnt_src_path.is_empty() && std::path::Path::new(&findmnt_src_path).exists() {
        return Some(findmnt_src_path);
    }

    if container_home.starts_with("/home/") {
        let suffix = &container_home["/home/".len()..];
        let candidate = format!("/var/home/{}", suffix);
        if std::path::Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }

    let host_home = std::env::var("HOME").unwrap_or_else(|_| String::from("/home"));
    let inferred = format!("{}/{}", host_home, env_name);
    if std::path::Path::new(&inferred).exists() {
        return Some(inferred);
    }

    None
}

/// Ensure the named environment is started by invoking a no-op command inside
/// the container (distrobox enter ... true).
///
/// Why: this is a lightweight start operation that verifies the container
/// runtime can enter the environment without performing additional side
/// effects. Errors are returned with diagnostic output from the distrobox
/// invocation.
///
/// # Errors
/// Returns Err when the environment name is empty or when the distrobox
/// invocation fails.
#[tauri::command]
pub fn start_environment(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let env_name = name.trim();
    if env_name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }

    logs::info(&app, "env", format!("Start requested: '{}'", env_name));

    let output = build_host_command("distrobox")
        .args(["enter", env_name, "--", "bash", "-lc", "true"])
        .output()
        .map_err(|e| {
            let msg = format!("Failed to execute 'distrobox enter ...': {}", e);
            logs::error(
                &app,
                "env",
                format!("start_environment failed for '{}': {}", env_name, msg),
            );
            msg
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let code = output.status.code().unwrap_or(-1);
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("distrobox enter (start) failed (exit code {})", code)
        };
        logs::error(
            &app,
            "env",
            format!("start_environment failed for '{}': {}", env_name, &msg),
        );
        return Err(msg);
    }

    let msg = format!("✅ Environment '{}' was started.", env_name);
    logs::info(&app, "env", format!("Start ok: '{}'", env_name));
    Ok(msg)
}

/// Stop the named environment using `distrobox stop --yes`.
///
/// Why: exposes a controlled stop operation that surfaces CLI error output
/// back to the caller and logs failures for diagnostics.
///
/// # Errors
/// Returns Err when the environment name is empty or when the distrobox stop
/// command fails.
#[tauri::command]
pub fn stop_environment(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let env_name = name.trim();
    if env_name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }

    logs::info(&app, "env", format!("Stop requested: '{}'", env_name));

    let output = build_host_command("distrobox")
        .args(["stop", "--yes", env_name])
        .output()
        .map_err(|e| {
            let msg = format!("Failed to execute 'distrobox stop --yes': {}", e);
            logs::error(
                &app,
                "env",
                format!("stop_environment failed for '{}': {}", env_name, msg),
            );
            msg
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let code = output.status.code().unwrap_or(-1);
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("distrobox stop failed (exit code {})", code)
        };
        logs::error(
            &app,
            "env",
            format!("stop_environment failed for '{}': {}", env_name, &msg),
        );
        return Err(msg);
    }

    let msg = format!("✅ Environment '{}' was stopped.", env_name);
    logs::info(&app, "env", format!("Stop ok: '{}'", env_name));
    Ok(msg)
}

/// Delete an environment and optionally its associated host project folder.
///
/// Architectural intent / Why:
/// - The command removes the distrobox environment and then attempts to clean
///   up orphaned podman containers that VS Code may have created for Dev
///   Containers. These cleanup steps are best-effort and do not make deletion
///   fail to avoid leaving users in inconsistent states.
/// - When deleting a project folder, conservative safety checks prevent
///   accidental deletion of user home directories.
///
/// # Errors
/// Returns Err when the initial `distrobox rm` fails or when the provided name
/// is empty. Other cleanup failures are logged and returned as informational
/// strings appended to the success message.
#[tauri::command]
pub async fn delete_environment(
    app: tauri::AppHandle,
    request: DeleteEnvironmentRequest,
) -> Result<String, String> {
    let env_name = request.name.trim();
    if env_name.is_empty() {
        return Err("Environment name is empty.".to_string());
    }

    let project_path = if request.delete_project {
        resolve_host_project_path(env_name)
    } else {
        None
    };

    let output = build_host_command("distrobox")
        .args(["rm", "-f", env_name])
        .output()
        .map_err(|e| format!("Failed to execute 'distrobox rm': {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    // Attempt to clean up any orphaned DevContainer podman containers labeled with the project path.
    // Fedora Silverblue / Bazzite often has /home -> /var/home symlink. VS Code may label using either path.
    // We'll query podman for both path variants and remove any matching containers. This must NOT fail
    // the overall deletion; we only log failures and proceed.
    let mut extra = String::new();
    if let Some(pp) = &project_path {
        // Strip any trailing slashes to match VS Code labels which don't include a trailing slash
        let clean_pp = pp.trim_end_matches('/').to_string();

        // prepare candidate path variants and deduplicate while preserving order
        let mut variants: Vec<String> = Vec::new();
        let s = clean_pp.as_str();
        if s.starts_with("/var/home/") {
            variants.push(s.replacen("/var/home/", "/home/", 1));
            variants.push(s.to_string());
        } else if s.starts_with("/home/") {
            variants.push(s.to_string());
            variants.push(s.replacen("/home/", "/var/home/", 1));
        } else {
            variants.push(s.to_string());
            variants.push(s.replacen("/var/home/", "/home/", 1));
            variants.push(s.replacen("/home/", "/var/home/", 1));
        }
        variants.dedup();

        // collect unique container ids found across all variants
        let mut ids_set: std::collections::HashSet<String> = std::collections::HashSet::new();

        for v in &variants {
        let filter_arg = format!("label=devcontainer.local_folder={}", v);
        let mut ps_cmd = build_host_command_async("podman");
        ps_cmd.args(["ps", "-a", "-q", "--filter", &filter_arg]);
        match ps_cmd.output().await {
            Ok(ps_out) => {

                    if ps_out.status.success() {
                        let ids = String::from_utf8_lossy(&ps_out.stdout).trim().to_string();
                        if !ids.is_empty() {
                            for id in ids.split_whitespace() {
                                ids_set.insert(id.to_string());
                            }
                            logs::info(&app, "env", format!("Found podman containers for project label variant {}: {}", v, ids));
                        } else {
                            logs::info(&app, "env", format!("No podman containers found for project label variant {}", v));
                        }
                    } else {
                        let stderr = String::from_utf8_lossy(&ps_out.stderr).trim().to_string();
                        logs::error(&app, "env", format!("podman ps failed for project variant {}: {}", v, stderr));
                    }
                }
                Err(e) => {
                    logs::error(&app, "env", format!("Failed to execute 'podman ps' for variant {}: {}", v, e));
                }
            }
        }

        if !ids_set.is_empty() {
        // Build rm command with unique ids
        let mut rm_cmd = build_host_command_async("podman");
        rm_cmd.arg("rm").arg("-f");
        for id in ids_set.iter() { rm_cmd.arg(id); }
        match rm_cmd.output().await {
            Ok(rm_out) => {

                    if rm_out.status.success() {
                        let id_list: Vec<String> = ids_set.iter().cloned().collect();
                        logs::info(&app, "env", format!("Removed {} orphaned podman container(s) for project path {}", id_list.len(), pp));
                        extra.push_str(&format!("\n🧹 Removed orphaned DevContainer(s): {}", id_list.join(", ")));
                    } else {
                        let stderr = String::from_utf8_lossy(&rm_out.stderr).trim().to_string();
                        logs::error(&app, "env", format!("podman rm failed for project {}: {}", pp, stderr));
                    }
                }
                Err(e) => {
                    logs::error(&app, "env", format!("Failed to execute 'podman rm': {}", e));
                }
            }
        }
    }

    if let Some(pp) = project_path {
        let p = std::path::Path::new(&pp);
        if p.exists() && p.is_dir() {
            let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/home"));
            let home_candidates = vec![
                String::from("/home"),
                String::from("/var/home"),
                home.clone(),
                format!("/var{}", home),
            ];
            let is_dangerous = home_candidates.iter().any(|hc| hc == &pp);
            let looks_like_project = p.join(".devcontainer").join("devcontainer.json").exists();

            if !is_dangerous && looks_like_project {
                if let Err(e) = std::fs::remove_dir_all(&p) {
                    extra.push_str(&format!("\n⚠️ Project folder could not be deleted: {} ({})", pp, e));
                } else {
                    extra.push_str(&format!("\n🧹 Project folder deleted: {}", pp));
                }
            } else {
                extra.push_str(&format!("\nℹ️ Project folder not deleted (safety guard): {}", pp));
            }
        }
    }

    let ok = format!("✅ Environment '{}' was deleted.{}", env_name, extra);

    let _ = app.emit(
        "app-notification",
        serde_json::json!({ "message": "Environment deleted", "type": "success" }),
    );

    Ok(ok)
}

/// Read the on-disk EnvironmentManifest for a given project path.
///
/// Why: reading a small, predictable manifest avoids expensive project
/// introspection and provides a stable contract for other commands that need
/// to mutate or query per-project metadata.
///
/// # Errors
/// Returns Err when the manifest cannot be read or parsed.
#[tauri::command]
pub fn get_environment_manifest(project_path: String) -> Result<EnvironmentManifest, String> {
    use std::fs;
    let manifest_path = std::path::Path::new(&project_path).join(".envstation.json");
    let manifest_content = fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "Failed to read manifest ({}): {}",
            manifest_path.display(),
            e
        )
    })?;
    let manifest: EnvironmentManifest = serde_json::from_str(&manifest_content).map_err(|e| {
        format!(
            "Failed to parse manifest ({}): {}",
            manifest_path.display(),
            e
        )
    })?;
    Ok(manifest)
}


/// Detect packages installed inside the running environment that are not recorded
/// in the project's manifest (drift) and packages declared in the
/// .devcontainer/devcontainer.json that are not recorded in the manifest.
///
/// Returns an object with two arrays:
/// - new_in_container: packages found in Distrobox but missing from manifest
/// - new_in_devcontainer: packages found in devcontainer.json install commands but missing from manifest
#[derive(Serialize)]
pub struct DriftScanResult {
    pub new_in_container: Vec<String>,
    pub new_in_devcontainer: Vec<String>,
    pub baseline_missing: bool,
    pub fallback_used: bool,
}

fn normalize_pkg_name(s: &str) -> String {
    let mut t = s.trim().trim_matches(|c: char| c == ',' || c == '"' || c == '\'');
    // remove leading epoch like "1:pkg"
    if let Some(colon) = t.find(':') {
        if t[..colon].chars().all(|c| c.is_ascii_digit()) {
            t = &t[colon + 1..];
        }
    }
    // remove trailing :arch (dpkg style) e.g. pkg:amd64 -> pkg
    if let Some(colon) = t.rfind(':') {
        if colon + 1 < t.len() && t[colon + 1..].chars().all(|c| c.is_ascii_alphanumeric()) {
            t = &t[..colon];
        }
    }
    // remove .arch suffix (rpm style)
    if let Some(dot) = t.rfind('.') {
        let suffix = &t[dot + 1..];
        let arches = ["x86_64", "noarch", "i686", "aarch64", "armv7hl", "ppc64le"];
        if arches.contains(&suffix) {
            t = &t[..dot];
        }
    }
    // remove trailing version if it looks like -<digit>
    if let Some(dash) = t.rfind('-') {
        let after = &t[dash + 1..];
        if after.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            t = &t[..dash];
        }
    }
    t.to_lowercase()
}

#[tauri::command]
pub async fn detect_environment_drift(
    name: String,
    project_path: String,
) -> Result<DriftScanResult, String> {
    use std::fs;
    use std::collections::HashSet;

    let manifest_path = std::path::Path::new(&project_path).join(".envstation.json");
    let manifest_content = fs::read_to_string(&manifest_path).map_err(|e| {
        format!("Failed to read manifest ({}): {}", manifest_path.display(), e)
    })?;
    let manifest: EnvironmentManifest = serde_json::from_str(&manifest_content).map_err(|e| {
        format!("Failed to parse manifest ({}): {}", manifest_path.display(), e)
    })?;

    let declared: HashSet<String> = manifest.system_packages.into_iter().map(|s| normalize_pkg_name(&s)).collect();

    // --------- Container -> Manifest (existing) ---------
    // Probe package manager inside the container
    let probe = r#"if command -v apt-get >/dev/null 2>&1; then echo apt; \
elif command -v dnf >/dev/null 2>&1; then echo dnf; \
elif command -v apk >/dev/null 2>&1; then echo apk; \
elif command -v pacman >/dev/null 2>&1; then echo pacman; \
else echo unknown; fi"#;
    let probe_out = build_host_command("distrobox")
        .args(["enter", &name, "--", "sh", "-lc", probe])
        .output()
        .map_err(|e| format!("Failed to execute 'distrobox enter' for package-manager probe: {}", e))?;

    let pm = String::from_utf8_lossy(&probe_out.stdout).trim().to_string();
    if pm == "unknown" || pm.is_empty() {
        let stderr = String::from_utf8_lossy(&probe_out.stderr).trim().to_string();
        return Err(format!("Could not detect package manager inside container: {}", stderr));
    }

    // Preferred and fallback listing commands for current installed packages (user-installed where possible)
    let (primary_cmd, fallback_cmd): (&str, Option<&str>) = match pm.as_str() {
        "apt" => ("apt-mark showmanual", Some("dpkg-query -f '${binary:Package}\\n' -W")),
        "dnf" => ("dnf repoquery --userinstalled --qf '%{name}\\n'", Some("rpm -qa --queryformat '%{NAME}\\n'")),
        "apk" => ("apk info", None),
        "pacman" => ("pacman -Qqe", Some("pacman -Qq")),
        _ => ("rpm -qa --queryformat '%{NAME}\\n'", None),
    };

    let mut current_set: HashSet<String> = HashSet::new();
    let mut list_output = String::new();
    let mut used_fallback = false;

    let mut primary_exec = build_host_command_async("distrobox");
    primary_exec.args(["enter", &name, "--", "sh", "-lc", primary_cmd]);
    if let Ok(po) = primary_exec.output().await {
        if po.status.success() {
            list_output = String::from_utf8_lossy(&po.stdout).to_string();
        }
    }

    if list_output.trim().is_empty() {
        if let Some(fb) = fallback_cmd {
            let mut fb_exec = build_host_command_async("distrobox");
            fb_exec.args(["enter", &name, "--", "sh", "-lc", fb]);
            if let Ok(fo) = fb_exec.output().await {
                if fo.status.success() {
                    list_output = String::from_utf8_lossy(&fo.stdout).to_string();
                    used_fallback = true;
                }
            }
        }
    }

    if used_fallback {
        eprintln!("Warning: primary package-listing command failed or returned empty; used fallback for PM='{}'.", pm);
    }

    if list_output.trim().is_empty() {
        eprintln!("detect_environment_drift: no package list could be retrieved");
    } else {
        for line in list_output.lines() {
            let ln = line.trim();
            if ln.is_empty() { continue; }
            for tok in ln.split_whitespace() {
                let norm = normalize_pkg_name(tok);
                if !norm.is_empty() { current_set.insert(norm); }
            }
        }
    }

    // use current_set below by renaming user_installed -> current_set

    // Use baseline diffing: retrieve the baseline snapshot from ~/.bazzite/base_packages.txt
    // and compute (current_packages - baseline_packages) to find packages added since creation.

    let mut baseline_missing = false;
    let mut baseline_set: HashSet<String> = HashSet::new();
    let mut cat_cmd = build_host_command_async("distrobox");
    cat_cmd.args(["enter", &name, "--", "sh", "-lc", "cat ~/.bazzite/base_packages.txt"]);
    match cat_cmd.output().await {
        Ok(cat_out) => {
            if cat_out.status.success() {
                let baseline_s = String::from_utf8_lossy(&cat_out.stdout).to_string();
                for line in baseline_s.lines() {
                    let ln = line.trim();
                    if ln.is_empty() { continue; }
                    let norm = normalize_pkg_name(ln);
                    if !norm.is_empty() { baseline_set.insert(norm); }
                }
            } else {
                baseline_missing = true;
            }
        }
        Err(_) => {
            baseline_missing = true;
        }
    }

    // Compute added = current - baseline
    let mut added: HashSet<String> = HashSet::new();
    for cur in current_set.into_iter() {
        if !baseline_set.contains(&cur) {
            added.insert(cur);
        }
    }

    // Filter out packages already declared in manifest
    let mut new_in_container: Vec<String> = added.into_iter().filter(|p| !declared.contains(p)).collect();
    new_in_container.sort();

    // --------- DevContainer -> Manifest (parse devcontainer.json) ---------
    let mut new_in_devcontainer: Vec<String> = Vec::new();
    let devcontainer_path = std::path::Path::new(&project_path).join(".devcontainer").join("devcontainer.json");
    if devcontainer_path.exists() {
        if let Ok(s) = fs::read_to_string(&devcontainer_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                // collect postCreateCommand and postStartCommand if present
                let mut commands: Vec<String> = Vec::new();
                if let Some(pc) = val.get("postCreateCommand") {
                    if let Some(st) = pc.as_str() {
                        commands.push(st.to_string());
                    }
                }
                if let Some(ps) = val.get("postStartCommand") {
                    if let Some(st) = ps.as_str() {
                        commands.push(st.to_string());
                    }
                }

                // helper to extract package tokens from a command string
                fn extract_pkgs_from_cmd(cmd: &str) -> Vec<String> {
                    let mut out: Vec<String> = Vec::new();
                    let patterns = ["dnf install", "apt-get install", "apt install", "apk add", "pacman -S", "pacman -Sy", "pacman -S --noconfirm"];
                    for pat in patterns.iter() {
                        let mut start = 0usize;
                        while let Some(idx) = cmd[start..].to_lowercase().find(&pat.to_lowercase()) {
                            let abs = start + idx + pat.len();
                            // take substring from abs to next && or ; or end
                            let rest = &cmd[abs..];
                            let end_idx = rest.find("&&").or_else(|| rest.find(";")).unwrap_or(rest.len());
                            let segment = &rest[..end_idx];
                            // split by whitespace and filter tokens
                            for tok in segment.split_whitespace() {
                                let t = tok.trim().trim_matches('"').trim_matches('\'');
                                if t.is_empty() { continue; }
                                // skip flags
                                if t.starts_with('-') { continue; }
                                if t.eq_ignore_ascii_case("sudo") || t.eq_ignore_ascii_case("-y") { continue; }
                                // basic validation: allow alnum and common pkg chars
                                if t.chars().all(|c| c.is_ascii_alphanumeric() || "-+_.:".contains(c)) {
                                    out.push(t.to_string());
                                }
                            }
                            start = abs;
                        }
                    }
                    out
                }

                let mut dev_pkgs_set: HashSet<String> = HashSet::new();
                for c in commands.iter() {
                    let found = extract_pkgs_from_cmd(c);
                    for p in found {
                        let norm = normalize_pkg_name(&p);
                        if !norm.is_empty() {
                            dev_pkgs_set.insert(norm);
                        }
                    }
                }

                new_in_devcontainer = dev_pkgs_set.into_iter().filter(|p| !declared.contains(p)).collect();
                new_in_devcontainer.sort();
            }
        }
    }

    Ok(DriftScanResult { new_in_container, new_in_devcontainer, baseline_missing, fallback_used: used_fallback })
}

#[tauri::command]
/// Initialize or rewrite the baseline snapshot inside a running environment.
/// This captures the current installed package list (normalized) and writes it
/// to ~/.bazzite/base_packages.txt inside the container.
pub async fn initialize_baseline(name: String) -> Result<(), String> {
    // Probe package manager
    let probe = r#"if command -v apt-get >/dev/null 2>&1; then echo apt; \\
elif command -v dnf >/dev/null 2>&1; then echo dnf; \\
elif command -v apk >/dev/null 2>&1; then echo apk; \\
elif command -v pacman >/dev/null 2>&1; then echo pacman; \\
else echo unknown; fi"#;
    let probe_out = build_host_command("distrobox")
        .args(["enter", &name, "--", "sh", "-lc", probe])
        .output()
        .map_err(|e| format!("Failed to execute package-manager probe: {}", e))?;
    let pm = String::from_utf8_lossy(&probe_out.stdout).trim().to_string();

    let (primary_cmd, fallback_cmd): (&str, Option<&str>) = match pm.as_str() {
        "apt" => ("apt-mark showmanual", Some("dpkg-query -f '${binary:Package}\\n' -W")),
        "dnf" => ("dnf repoquery --userinstalled --qf '%{name}\\n'", Some("rpm -qa --queryformat '%{NAME}\\n'")),
        "apk" => ("apk info", None),
        "pacman" => ("pacman -Qqe", Some("pacman -Qq")),
        _ => ("rpm -qa --queryformat '%{NAME}\\n'", None),
    };

    let mut list_output = String::new();
    let mut used_fallback = false;

    let mut primary_exec = build_host_command_async("distrobox");
    primary_exec.args(["enter", &name, "--", "sh", "-lc", primary_cmd]);
    if let Ok(po) = primary_exec.output().await {
        if po.status.success() {
            list_output = String::from_utf8_lossy(&po.stdout).to_string();
        }
    }

    if list_output.trim().is_empty() {
        if let Some(fb) = fallback_cmd {
            let mut fb_exec = build_host_command_async("distrobox");
            fb_exec.args(["enter", &name, "--", "sh", "-lc", fb]);
            if let Ok(fo) = fb_exec.output().await {
                if fo.status.success() {
                    list_output = String::from_utf8_lossy(&fo.stdout).to_string();
                    used_fallback = true;
                }
            }
        }
    }

    if used_fallback {
        eprintln!("Warning: primary package-listing command failed or returned empty; used fallback for PM='{}'.", pm);
    }

    if list_output.trim().is_empty() {
        return Err("Listing packages failed: no output".to_string());
    }

    let s = list_output;
    let mut pkgs: Vec<String> = Vec::new();
    for line in s.lines() {
        let ln = line.trim();
        if ln.is_empty() { continue; }
        for tok in ln.split_whitespace() {
            let norm = normalize_pkg_name(tok);
            if !norm.is_empty() { pkgs.push(norm); }
        }
    }

    // Build a here-doc to avoid quoting complexity
    let mut printf_body = String::new();
    for p in pkgs.iter() {
        printf_body.push_str(p);
        printf_body.push('\n');
    }
    let write_cmd = format!(
        "mkdir -p ~/.bazzite && cat > ~/.bazzite/base_packages.txt <<'EOF'\\n{}EOF\\n",
        printf_body
    );

    let mut write_exec = build_host_command_async("distrobox");
    write_exec.args(["enter", &name, "--", "sh", "-lc", &write_cmd]);
    let wout = write_exec
        .output()
        .await
        .map_err(|e| format!("Failed to write baseline inside container: {}", e))?;
    if !wout.status.success() {
        let stderr = String::from_utf8_lossy(&wout.stderr).trim().to_string();
        return Err(format!("Failed to write baseline file inside container: {}", stderr));
    }

    Ok(())
}

/// Add a system package to the project's manifest and attempt to install it
/// inside the running environment.
///
/// Why: this operation updates the durable manifest to record requested system
/// packages and updates the devcontainer.json so future DevContainer runs will
/// include the same install command. It also attempts the install immediately
/// inside the container to provide fast feedback.
///
/// # Errors
/// Returns Err if manifest or devcontainer.json cannot be read/parsed/written,
/// or if the in-container installation fails.
#[tauri::command]
pub async fn install_system_package(
    name: String,
    project_path: String,
    package: String,
) -> Result<(), String> {
    use std::fs;

    let manifest_path = std::path::Path::new(&project_path).join(".envstation.json");
    let manifest_content = fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "Failed to read manifest ({}): {}",
            manifest_path.display(),
            e
        )
    })?;
    let mut manifest: EnvironmentManifest =
        serde_json::from_str(&manifest_content).map_err(|e| {
            format!(
                "Failed to parse manifest ({}): {}",
                manifest_path.display(),
                e
            )
        })?;

    let package_norm = normalize_pkg_name(&package);
    if manifest.system_packages.iter().any(|p| normalize_pkg_name(p) == package_norm) {
        return Ok(());
    }

    manifest.system_packages.push(package_norm.clone());
    // Normalize all entries before writing to disk to keep the manifest consistent.
    manifest.system_packages = manifest
        .system_packages
        .into_iter()
        .map(|p| normalize_pkg_name(&p))
        .collect();
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(&manifest_path, manifest_json).map_err(|e| {
        format!(
            "Failed to write manifest ({}): {}",
            manifest_path.display(),
            e
        )
    })?;

    let devcontainer_path = std::path::Path::new(&project_path)
        .join(".devcontainer")
        .join("devcontainer.json");
    let dev_json_str = fs::read_to_string(&devcontainer_path).map_err(|e| {
        format!(
            "Failed to read devcontainer.json ({}): {}",
            devcontainer_path.display(),
            e
        )
    })?;
    let mut dev_val: serde_json::Value = serde_json::from_str(&dev_json_str).map_err(|e| {
        format!(
            "Failed to parse devcontainer.json ({}): {}",
            devcontainer_path.display(),
            e
        )
    })?;

    // Probe the container to detect which package manager is available.
    // We run a small shell snippet inside the distrobox environment that checks
    // for common package managers and returns a short id (apt/dnf/apk/pacman).
    let probe = r#"if command -v apt-get >/dev/null 2>&1; then echo apt; \
elif command -v dnf >/dev/null 2>&1; then echo dnf; \
elif command -v apk >/dev/null 2>&1; then echo apk; \
elif command -v pacman >/dev/null 2>&1; then echo pacman; \
else echo unknown; fi"#;
    let probe_out = build_host_command("distrobox")
        .args(["enter", &name, "--", "sh", "-lc", probe])
        .output()
        .map_err(|e| format!("Failed to execute 'distrobox enter' for package-manager probe: {}", e))?;

    let pm = String::from_utf8_lossy(&probe_out.stdout).trim().to_string();

    if pm == "unknown" || pm.is_empty() {
        let stderr = String::from_utf8_lossy(&probe_out.stderr).trim().to_string();
        return Err(format!("Could not detect package manager inside container: {}", stderr));
    }

    // Build the postCreateCommand fragment and the live install invocation
    // according to the detected package manager.
    let (post_fragment, live_command) = match pm.as_str() {
        "apt" => (
            format!(" && sudo apt-get update && sudo apt-get install -y {}", package),
            format!("sudo apt-get update && sudo apt-get install -y {}", package),
        ),
        "dnf" => (
            format!(" && sudo dnf install -y {}", package),
            format!("sudo dnf install -y {}", package),
        ),
        "apk" => (
            format!(" && sudo apk add --no-cache {}", package),
            format!("sudo apk add --no-cache {}", package),
        ),
        "pacman" => (
            format!(" && sudo pacman -Sy --noconfirm {}", package),
            format!("sudo pacman -Sy --noconfirm {}", package),
        ),
        other => {
            return Err(format!("Unsupported package manager detected: {}", other));
        }
    };

    match dev_val.get_mut("postCreateCommand") {
        Some(v) => {
            if let Some(s) = v.as_str() {
                let mut s_owned = s.to_string();
                s_owned.push_str(&post_fragment);
                *v = serde_json::Value::String(s_owned);
            } else {
                *v = serde_json::Value::String(post_fragment.trim_start_matches(" && ").to_string());
            }
        }
        None => {
            if let Some(obj) = dev_val.as_object_mut() {
                obj.insert(
                    "postCreateCommand".to_string(),
                    serde_json::Value::String(post_fragment.trim_start_matches(" && ").to_string()),
                );
            } else {
                return Err("devcontainer.json does not have an object root".to_string());
            }
        }
    }

    let new_dev_json = serde_json::to_string_pretty(&dev_val)
        .map_err(|e| format!("Failed to serialize devcontainer.json: {}", e))?;
    fs::write(&devcontainer_path, new_dev_json).map_err(|e| {
        format!(
            "Failed to write devcontainer.json ({}): {}",
            devcontainer_path.display(),
            e
        )
    })?;

    // Attempt live installation inside the distrobox using the chosen command.
    let output = build_host_command("distrobox")
        .args(["enter", &name, "--", "sh", "-lc", &live_command])
        .output()
        .map_err(|e| format!("Failed to execute 'distrobox enter' for live install: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("Installation in the container failed: {}", msg));
    }

    Ok(())
}
