use std::fs;
use std::path::PathBuf;
use tauri::Emitter;

fn store_path() -> Option<PathBuf> {
    // Prefer XDG_CONFIG_HOME if set, otherwise fall back to ~/.config/envstation
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("envstation");
        let _ = std::fs::create_dir_all(&p);
        return Some(p.join("store.json"));
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".config").join("envstation");
        let _ = std::fs::create_dir_all(&p);
        return Some(p.join("store.json"));
    }
    None
}

#[tauri::command]
pub fn get_github_pat() -> Result<Option<String>, String> {
    let path = store_path().ok_or_else(|| "Could not determine config directory".to_string())?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read store file: {}", e))?;
    let v: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("Invalid store JSON: {}", e))?;
    Ok(v.get("github_pat").and_then(|s| s.as_str().map(|s| s.to_string())))
}

#[tauri::command]
pub fn set_github_pat(app: tauri::AppHandle, pat: String) -> Result<(), String> {
    let path = store_path().ok_or_else(|| "Could not determine config directory".to_string())?;
    // Read existing
    let mut obj = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read store file: {}", e))?;
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            .map_err(|e| format!("Invalid store JSON: {}", e))?
    } else {
        serde_json::Map::new()
    };
    let cleared = pat.is_empty();
    if cleared {
        obj.remove("github_pat");
    } else {
        obj.insert("github_pat".to_string(), serde_json::Value::String(pat.clone()));
    }
    let serialized = serde_json::to_string_pretty(&obj).map_err(|e| format!("Failed to serialize store: {}", e))?;
    fs::write(&path, serialized).map_err(|e| format!("Failed to write store file: {}", e))?;

    let msg = if cleared {
        "GitHub token cleared"
    } else {
        "GitHub token saved"
    };

    // Emit a transient UI notification (toaster) and append an entry to the app logs.
    let _ = app.emit("app-notification", serde_json::json!({ "message": msg, "type": "success" }));
    crate::commands::logs::info(&app, "envshare", msg);

    Ok(())
}
