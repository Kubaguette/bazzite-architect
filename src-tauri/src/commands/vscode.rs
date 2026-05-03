use crate::commands::logs;
use crate::core::util::build_host_command;

#[tauri::command]
/// Attempt to open the host project folder for a given environment in VS Code.
///
/// Architectural intent / Why:
/// - Uses multiple heuristics (findmnt, podman inspect, /var/home mapping and
///   a simple inferred-home heuristic) to robustly map a container's $HOME to a
///   host filesystem path across diverse Fedora/Silverblue setups.
/// - Tries multiple launchers (flatpak, system 'code', 'codium') and returns
///   a helpful debug string on failure to aid troubleshooting.
/// - Spawns the editor without awaiting long-running child processes to avoid
///   blocking the invoking thread.
///
/// # Errors
/// Returns Err(String) when no environment name is provided or when all
/// attempted launchers fail; the returned string includes a debug trace useful
/// for diagnostics.
#[tauri::command]
pub fn open_in_vscode(app: tauri::AppHandle, name: String) -> Result<String, String> {
    logs::info(&app, "vscode", format!("open_in_vscode start: '{}'", name));
    if name.trim().is_empty() {
        return Err("Environment name is empty.".to_string());
    }

    let mut dbg = String::new();

    let home_out = build_host_command("distrobox")
        .args(["enter", &name, "--", "bash", "-lc", "printf %s \"$HOME\""])
        .output()
        .map_err(|e| format!("Could not determine HOME in the container: {}", e))?;
    if !home_out.status.success() {
        return Err(format!(
            "Could not determine HOME in the container: {}",
            String::from_utf8_lossy(&home_out.stderr).trim()
        ));
    }
    let container_home = String::from_utf8_lossy(&home_out.stdout).trim().to_string();
    if container_home.is_empty() {
        return Err("Container HOME is empty.".to_string());
    }
    dbg.push_str(&format!("container_home = {}\n", container_home));

    let findmnt_out = build_host_command("distrobox")
        .args([
            "enter",
            &name,
            "--",
            "bash",
            "-lc",
            "if command -v findmnt >/dev/null 2>&1; then findmnt -n -o SOURCE --target \"$HOME\"; fi",
        ])
        .output();
    let mut findmnt_src_raw = String::new();
    let mut findmnt_src_path = String::new();
    if let Ok(out) = &findmnt_out {
        let out_s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let err_s = String::from_utf8_lossy(&out.stderr).trim().to_string();
        dbg.push_str(&format!(
            "findmnt status={} stdout='{}' stderr='{}'\n",
            out.status, out_s, err_s
        ));
        if out.status.success() {
            findmnt_src_raw = out_s.clone();
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
            if findmnt_src_path.is_empty() {
                if out_s.starts_with('/') {
                    findmnt_src_path = out_s;
                }
            }
        }
    } else if let Err(e) = &findmnt_out {
        dbg.push_str(&format!("findmnt call error: {}\n", e));
    }

    let list_out = build_host_command("distrobox")
        .args(["list", "--no-color"])
        .output();
    let mut container_id: Option<String> = None;
    if let Ok(out) = &list_out {
        dbg.push_str(&format!(
            "distrobox list status={} stdout='{}' stderr='{}'\n",
            out.status,
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
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
                    if parts.len() >= 2 && parts[1] == name {
                        container_id = Some(parts[0].clone());
                        break;
                    }
                } else {
                    let ws: Vec<&str> = trimmed.split_whitespace().collect();
                    if ws.len() >= 2 && ws[1] == name {
                        container_id = Some(ws[0].to_string());
                        break;
                    }
                }
            }
        }
    } else if let Err(e) = &list_out {
        dbg.push_str(&format!("distrobox list call error: {}\n", e));
    }
    dbg.push_str(&format!("resolved container_id = {:?}\n", container_id));

    let mut inspect_src = String::new();
    if let Some(cid) = &container_id {
        let tmpl = format!(
            "{{{{range .Mounts}}}}{{{{if eq .Destination \"{}\"}}}}{{{{.Source}}}}{{{{end}}}}{{{{end}}}}",
            container_home
        );
        if let Ok(ins_out) = build_host_command("podman")
            .args(["inspect", cid, "--format", &tmpl])
            .output()
        {
            dbg.push_str(&format!(
                "podman inspect status={} stdout='{}' stderr='{}'\n",
                ins_out.status,
                String::from_utf8_lossy(&ins_out.stdout).trim(),
                String::from_utf8_lossy(&ins_out.stderr).trim()
            ));
            if ins_out.status.success() {
                inspect_src = String::from_utf8_lossy(&ins_out.stdout).trim().to_string();
            }
        }
    }

    let mut host_home_candidate = String::new();
    if !findmnt_src_path.is_empty() {
        host_home_candidate = findmnt_src_path.clone();
        dbg.push_str(&format!(
            "candidate=findmnt -> {} (raw='{}')\n",
            host_home_candidate, findmnt_src_raw
        ));
    } else if !inspect_src.is_empty() {
        host_home_candidate = inspect_src.clone();
        dbg.push_str(&format!("candidate=inspect -> {}\n", host_home_candidate));
    } else if container_home.starts_with("/home/") {
        let suffix = &container_home["/home/".len()..];
        host_home_candidate = format!("/var/home/{}", suffix);
        dbg.push_str(&format!(
            "candidate=mapped /var/home -> {}\n",
            host_home_candidate
        ));
    }

    let host_env_home = std::env::var("HOME").unwrap_or_else(|_| String::from("/home"));
    let exists_candidate = std::path::Path::new(&host_home_candidate).exists();
    let exists_container_home = std::path::Path::new(&container_home).exists();
    let exists_host_env_home = std::path::Path::new(&host_env_home).exists();
    dbg.push_str(&format!(
        "exists: candidate={} container_home={} host_env_home={}\n",
        exists_candidate, exists_container_home, exists_host_env_home
    ));

    let inferred_project = format!("{}/{}", host_env_home, name);
    let exists_inferred = std::path::Path::new(&inferred_project).exists();
    dbg.push_str(&format!(
        "heuristic inferred_project='{}' exists={}\n",
        inferred_project, exists_inferred
    ));

    let target_path = if exists_inferred {
        inferred_project
    } else if !host_home_candidate.is_empty() && exists_candidate {
        host_home_candidate
    } else if exists_container_home {
        container_home.clone()
    } else {
        host_env_home
    };
    dbg.push_str(&format!("selected target_path = {}\n", target_path));

    let mut last_err: Option<String> = None;
    let mut used_launcher: Option<String> = None;
    let mut try_spawn = |bin: &str, args: &[&str]| -> bool {
        let mut cmd = build_host_command(bin);
        for a in args {
            cmd.arg(a);
        }
        let result = cmd.spawn();
        match result {
            Ok(_) => {
                used_launcher = Some(format!("{} {}", bin, args.join(" ")));
                true
            }
            Err(e) => {
                last_err = Some(format!("{}: {}", bin, e));
                false
            }
        }
    };

    let flatpak_list = build_host_command("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
        .ok();
    if let Some(out) = flatpak_list {
        if out.status.success() {
            let apps = String::from_utf8_lossy(&out.stdout);
            let has_code = apps.lines().any(|l| {
                l.trim() == "com.visualstudio.code" || l.trim() == "com.visualstudio.Code"
            });
            let has_codium = apps.lines().any(|l| l.trim() == "com.vscodium.codium");
            dbg.push_str(&format!(
                "flatpak apps detected: code={} codium={}\n",
                has_code, has_codium
            ));
            if has_code {
                if try_spawn(
                    "flatpak",
                    &["run", "com.visualstudio.code", "--new-window", &target_path],
                ) {
                    {
                        let msg = format!("VS Code started: {}\n\nDEBUG\n{}", target_path, dbg);
                        logs::info(
                            &app,
                            "vscode",
                            format!("open_in_vscode ok -> {}", target_path),
                        );
                        return Ok(msg);
                    }
                }
                if try_spawn(
                    "flatpak",
                    &["run", "com.visualstudio.Code", "--new-window", &target_path],
                ) {
                    {
                        let msg = format!("VS Code started: {}\n\nDEBUG\n{}", target_path, dbg);
                        logs::info(
                            &app,
                            "vscode",
                            format!("open_in_vscode ok -> {}", target_path),
                        );
                        return Ok(msg);
                    }
                }
            }
            if has_codium {
                if try_spawn(
                    "flatpak",
                    &["run", "com.vscodium.codium", "--new-window", &target_path],
                ) {
                    {
                        let msg = format!("VS Code started: {}\n\nDEBUG\n{}", target_path, dbg);
                        logs::info(
                            &app,
                            "vscode",
                            format!("open_in_vscode ok -> {}", target_path),
                        );
                        return Ok(msg);
                    }
                }
            }
        }
    }

    let which_code = build_host_command("sh")
        .args(["-lc", "command -v code >/dev/null 2>&1"])
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false);
    dbg.push_str(&format!("which code -> {}\n", which_code));
    if which_code {
        if try_spawn("code", &["-n", &target_path]) {
            {
                let msg = format!("VS Code started: {}\n\nDEBUG\n{}", target_path, dbg);
                logs::info(
                    &app,
                    "vscode",
                    format!("open_in_vscode ok -> {}", target_path),
                );
                return Ok(msg);
            }
        }
    }

    let which_codium = build_host_command("sh")
        .args(["-lc", "command -v codium >/dev/null 2>&1"])
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false);
    dbg.push_str(&format!("which codium -> {}\n", which_codium));
    if which_codium {
        if try_spawn("codium", &["-n", &target_path]) {
            {
                let msg = format!("VS Code started: {}\n\nDEBUG\n{}", target_path, dbg);
                logs::info(
                    &app,
                    "vscode",
                    format!("open_in_vscode ok -> {}", target_path),
                );
                return Ok(msg);
            }
        }
    }

    let err = format!(
        "Could not start VS Code. {}\n\nDEBUG\n{}\nLauncher last error: {:?}",
        last_err
            .clone()
            .unwrap_or_else(|| "No suitable installation found".to_string()),
        dbg,
        used_launcher
    );
    logs::error(&app, "vscode", err.clone());
    Err(err)
}
