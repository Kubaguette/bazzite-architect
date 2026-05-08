use chrono::Local;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

static LOGS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
const MAX_LOG_LINES: usize = 1000;

fn with_logs_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut VecDeque<String>) -> R,
{
    let mutex = LOGS.get_or_init(|| Mutex::new(VecDeque::new()));
    // SAFETY: LOGS is initialized above with a Mutex<Void>. Acquiring the lock
    // only panics if a poisoning scenario occurs (thread panicked while holding
    // the lock). The application tolerates poisoning as a signal of higher-level
    // process failure; recovering with unwrap here is acceptable because logging
    // is not expected to be used across threads that intentionally panic while
    // holding the lock. If poisoned behavior is later required, change to
    // lock().unwrap_or_else(|g| g.into_inner()).
    let mut guard = mutex.lock().expect("LOGS mutex lock failed: initialized in OnceLock and expected unpoisoned because logging is an in-process buffer and no thread should have panicked while holding this lock");
    f(&mut guard)
}

/// Log an informational message and emit it to the frontend.
///
/// Why: this wrapper centralizes the 'INFO' level and ensures all information
/// messages follow the same emission path (in-memory ring + UI event). The
/// in-memory ring bounds memory usage and reduces expensive disk I/O for
/// frequent logs.
pub fn info(app: &tauri::AppHandle, source: &str, message: impl Into<String>) {
    append(app, source, "INFO", &message.into());
}

/// Log an error-level message and emit it to the frontend.
///
/// Why: error-level messages are surfaced to both the in-memory buffer and the
/// UI so that diagnostics can be shown to users promptly while still keeping a
/// limited history for retrieval.
pub fn error(app: &tauri::AppHandle, source: &str, message: impl Into<String>) {
    append(app, source, "ERROR", &message.into());
}

/// Append a raw log line to the in-memory buffer and emit an event to the UI.
///
/// Why: central operation for all logging. The function keeps a bounded buffer
/// (MAX_LOG_LINES) to protect memory usage on long-running sessions, and emits
/// each line via tauri events so the frontend can update incrementally without
/// reloading the full buffer.
pub fn append(app: &tauri::AppHandle, source: &str, level: &str, message: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let line = format!("[{}] [{}] [{}] {}", ts, source, level, message);
    with_logs_mut(|logs| {
        logs.push_back(line.clone());
        while logs.len() > MAX_LOG_LINES {
            logs.pop_front();
        }
    });
    let _ = app.emit("app-log", line);
}

/// Return the current log buffer as a single string.
///
/// Why: the UI sometimes needs the full history (e.g. when opening a logs
/// viewer). Exposing a single text blob simplifies the frontend implementation
/// while keeping the authoritative state in-process.
#[tauri::command]
pub fn get_logs_text() -> Result<String, String> {
    let text = with_logs_mut(|logs| logs.iter().cloned().collect::<Vec<_>>().join("\n"));
    Ok(text)
}

/// Clear the in-memory log buffer.
///
/// Why: provide a low-cost way for the frontend to reset visible diagnostics
/// without performing any filesystem operations. This only affects the in-memory
/// buffer; persistent logs (if added later) would be separate.
#[tauri::command]
pub fn clear_logs() -> Result<(), String> {
    with_logs_mut(|logs| logs.clear());
    Ok(())
}

/// Accept a log line from the frontend (client) and forward it to the
/// centralized log buffer.
///
/// Why: instrumentation/events from the renderer can be useful for debugging
/// issues that only surface in the UI. The command normalizes missing levels to
/// INFO and routes messages to the same bounded buffer and event path.
#[tauri::command]
pub fn client_log(
    app: tauri::AppHandle,
    source: String,
    level: Option<String>,
    message: String,
) -> Result<(), String> {
    let lvl = level.unwrap_or_else(|| "INFO".to_string());
    append(&app, &source, &lvl, &message);
    Ok(())
}
