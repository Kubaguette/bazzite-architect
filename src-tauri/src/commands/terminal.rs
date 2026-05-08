use crate::commands::logs;
use crate::core::util::build_host_command;
use std::fs::OpenOptions;
use std::io;
use std::process::Stdio;

use std::os::unix::process::CommandExt;

fn shell_escape_single(s: &str) -> String {
    // Escape single quotes for sh/bash: ' -> '\'' (implemented as: '"'"')
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// Open a distrobox environment in a native host terminal emulator.
///
/// Attempts ptyxis, then xdg-terminal-exec, then gnome-terminal and konsole.
/// The spawned terminal is detached from the Tauri process using setsid and
/// stdio redirected to /dev/null so it survives when the parent exits.
#[tauri::command]
pub fn open_in_terminal(app: tauri::AppHandle, env_name: String) -> Result<String, String> {
    logs::info(&app, "terminal", format!("open_in_terminal start: '{}'", env_name));
    if env_name.trim().is_empty() {
        return Err("Environment name is empty.".to_string());
    }

    let candidates = ["ptyxis", "xdg-terminal-exec", "gnome-terminal", "konsole"];
    let mut last_err: Option<String> = None;

    for bin in &candidates {
        // quick existence check
        let is_present = build_host_command("sh")
            .args(["-lc", &format!("command -v {} >/dev/null 2>&1", bin)])
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false);
        if !is_present {
            continue;
        }

        let escaped = shell_escape_single(&env_name);

        let mut cmd = build_host_command(bin);
        match *bin {
            "ptyxis" => {
                cmd.arg("--");
                cmd.arg("distrobox");
                cmd.arg("enter");
                cmd.arg(&env_name);
            }
            "xdg-terminal-exec" => {
                // xdg-terminal-exec typically forwards args as the command to run
                cmd.arg("distrobox");
                cmd.arg("enter");
                cmd.arg(&env_name);
            }
            "gnome-terminal" => {
                // run via bash -lc to ensure a login-like execution
                cmd.arg("--");
                cmd.arg("bash");
                cmd.arg("-lc");
                cmd.arg(format!("distrobox enter {}", escaped));
            }
            "konsole" => {
                // konsole uses -e to execute a command
                cmd.arg("-e");
                cmd.arg("bash");
                cmd.arg("-lc");
                cmd.arg(format!("distrobox enter {}", escaped));
            }
            _ => {}
        }

        // Redirect stdio to /dev/null so the child doesn't keep file descriptors
        // from the parent and can continue after parent exits.
        let devnull = OpenOptions::new().read(true).write(true).open("/dev/null");
        if let Ok(f) = devnull {
            let stdin = Stdio::from(f.try_clone().expect("Cloning /dev/null file descriptor should succeed: file was just opened successfully and is a regular fd"));
            let stdout = Stdio::from(f.try_clone().expect("Cloning /dev/null file descriptor should succeed: file was just opened successfully and is a regular fd"));
            let stderr = Stdio::from(f);
            cmd.stdin(stdin).stdout(stdout).stderr(stderr);
        }

        // Detach from controlling terminal and create new session so the child
        // does not receive SIGHUP when the parent exits.
        unsafe {
            cmd.pre_exec(|| {
                // SAFETY: calling libc::setsid is a well-known POSIX operation to
                // create a new session. We translate a -1 return into an io::Error
                // so the spawn() call fails cleanly.
                let rc = libc::setsid();
                if rc == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }

        match cmd.spawn() {
            Ok(_child) => {
                let msg = format!("Terminal launched using '{}'.", bin);
                logs::info(&app, "terminal", format!("open_in_terminal ok -> {}", bin));
                return Ok(msg);
            }
            Err(e) => {
                last_err = Some(format!("{}: {}", bin, e));
                continue;
            }
        }
    }

    let err = format!(
        "Could not launch terminal. Last error: {:?}",
        last_err
    );
    logs::error(&app, "terminal", err.clone());
    Err(err)
}
