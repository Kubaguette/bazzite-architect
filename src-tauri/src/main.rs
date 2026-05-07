// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // If we're running on Wayland, override environment variables
    // to bypass AppImage hooks that force GDK_BACKEND=x11 and break WebKitGTK.
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        if session.to_lowercase() == "wayland" {
            // Force Wayland backend for GDK
            std::env::set_var("GDK_BACKEND", "wayland");

            // Prefer GLES2 renderer for WebKitGTK to improve stability with
            // fractional-scaling and NVIDIA drivers. This is less likely to
            // flicker when moving between monitors with different DPI.
            std::env::set_var("WEBKIT_USE_GL_RENDERER", "gles2");

            // Optionally disable DMABUF renderer if flickering persists.
            // Keep commented by default as this may affect performance on some systems.
            // std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

            // Preserve the previous compositing-disable fallback for older setups
            // where WebKit's compositing path causes issues. Left enabled only
            // when explicitly required by troubleshooting.
            // std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    envstation_lib::run()
}
