use std::path::Path;
use std::process::Command;

/// Build a Command that executes the requested binary on the host when running
/// inside a distrobox/container environment, or directly otherwise.
///
/// Why: when the process itself runs inside a container, many host-facing
/// tooling commands are proxied via distrobox-host-exec to reach the host
/// environment. Centralizing this logic avoids sprinkling container-detection
/// throughout the codebase and ensures consistent behavior across sync/async
/// command paths.
pub fn build_host_command(base_cmd: &str) -> Command {
    // If running inside Flatpak, use flatpak-spawn --host to execute on the
    // host system. Otherwise, if inside a distrobox/container environment,
    // proxy via distrobox-host-exec. Fall back to direct execution when
    // running natively.
    if Path::new("/.flatpak-info").exists() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(base_cmd);
        cmd
    } else if Path::new("/run/.containerenv").exists() {
        let mut cmd = Command::new("distrobox-host-exec");
        cmd.arg(base_cmd);
        cmd
    } else {
        Command::new(base_cmd)
    }
}

/// Async variant of build_host_command for use with tokio::process.
///
/// Why: mirrors build_host_command behavior but returns the async-friendly
/// Command type to avoid spawning blocking subprocess management code in the
/// runtime.
pub fn build_host_command_async(base_cmd: &str) -> tokio::process::Command {
    // Async variant mirrors the sync behavior but returns a tokio Command.
    if Path::new("/.flatpak-info").exists() {
        let mut cmd = tokio::process::Command::new("flatpak-spawn");
        cmd.arg("--host").arg(base_cmd);
        cmd
    } else if Path::new("/run/.containerenv").exists() {
        let mut cmd = tokio::process::Command::new("distrobox-host-exec");
        cmd.arg(base_cmd);
        cmd
    } else {
        tokio::process::Command::new(base_cmd)
    }
}

/// Normalize host paths for presentation by converting common Fedora/Silverblue
/// /var/home mappings to /home and trimming trailing slashes.
///
/// Why: presents a consistent path to users across systems where HOME may be
/// mounted at /var/home while the usual developer mental model expects /home.
pub fn normalize_home_path(path: &str) -> String {
    path.replace("/var/home", "/home")
        .trim_end_matches('/')
        .to_string()
}
