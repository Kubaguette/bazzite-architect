pub mod commands;
pub mod core;

mod windowcmds {
    /// Request the native window to start a drag operation.
    ///
    /// Why: the UI cannot directly control window drag from the renderer process
    /// on all platforms. Exposing this command provides a minimal bridge so the
    /// frontend can implement custom titlebars without blocking the UI thread.
    #[tauri::command]
    pub async fn drag_window(window: tauri::WebviewWindow) -> Result<(), String> {
        window
            .start_dragging()
            .map_err(|e| format!("start_dragging error: {e:?}"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Initialize and run the Tauri application.
///
/// Why: central application bootstrap that registers plugins and all
/// invoke-handler commands. Keeping this in a single function isolates
/// platform-specific setup (e.g. Wayland diagnostics) and ensures the
/// application initialization path is auditable and testable.
pub fn run() {
    use tauri::Manager;

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Trace).build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // System
            commands::system::system_check,
            // Storage
            commands::storage::scan_drives,
            commands::storage::get_active_storage_path,
            commands::storage::apply_storage_setup,
            // Environments
            commands::env::list_environments,
            commands::env::create_environment,
            commands::env::start_environment,
            commands::env::stop_environment,
            commands::env::delete_environment,
            commands::env::install_system_package,
            commands::env::get_environment_manifest,
            commands::env::detect_environment_drift,
            // VS Code
            commands::vscode::open_in_vscode,
            // Terminal
            commands::terminal::open_in_terminal,
            // Space
            commands::space::get_environment_space,
            commands::space::resolve_project_path,
            commands::space::get_dir_size,
            commands::space::cancel_dir_size_jobs,
            // Logs
            commands::logs::get_logs_text,
            commands::logs::clear_logs,
            commands::logs::client_log,
            // Window
            windowcmds::drag_window,
        ])
        .setup(|app| {
            // Environment diagnostics for Wayland/X11
            let xdg_sess = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".into());
            let wayland_disp = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".into());
            let display = std::env::var("DISPLAY").unwrap_or_else(|_| "<unset>".into());
            let gdk_backend = std::env::var("GDK_BACKEND").unwrap_or_else(|_| "<unset>".into());
            println!(
                "[setup] XDG_SESSION_TYPE={xdg_sess} WAYLAND_DISPLAY={wayland_disp} DISPLAY={display} GDK_BACKEND={gdk_backend}"
            );

            if let Some(window) = app.get_webview_window("main") {
                // Ensure window props are as expected on Linux
                if let Err(e) = window.set_decorations(false) {
                    eprintln!("[setup] set_decorations error: {e:?}");
                }
                if let Err(e) = window.set_resizable(true) {
                    eprintln!("[setup] set_resizable error: {e:?}");
                }
                // Log size/visibility for sanity
                match window.outer_size() {
                    Ok(size) => println!("[setup] window size = {:?}", size),
                    Err(e) => eprintln!("[setup] outer_size error: {e:?}"),
                }
                println!("[setup] window initialized");
            } else {
                eprintln!("[setup] main window not found");
            }
            Ok(())
        });

    // SAFETY: run() returns a Result which will only be Err on startup failure
    // (invalid configuration or platform support issues). It is appropriate to
    // treat failure to run the application as fatal at this point and surface a
    // clear message for diagnostic purposes.
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
