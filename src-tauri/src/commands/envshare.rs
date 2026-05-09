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

#[tauri::command]
pub async fn share_environment(project_path: String) -> Result<String, String> {
    // Retrieve PAT from store
    let store = store_path().ok_or_else(|| "Could not determine config directory".to_string())?;
    if !store.exists() {
        return Err("No GitHub PAT found. Please configure it in Settings.".to_string());
    }
    let content = fs::read_to_string(&store).map_err(|e| format!("Failed to read store file: {}", e))?;
    let v: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("Invalid store JSON: {}", e))?;
    let pat = v.get("github_pat").and_then(|s| s.as_str()).ok_or_else(|| "No GitHub PAT found. Please configure it in Settings.".to_string())?;

    // Read project config file
    let cfg_path = PathBuf::from(&project_path).join(".envstation.json");
    if !cfg_path.exists() {
        return Err("Project configuration file not found: .envstation.json".to_string());
    }
    let cfg_content = fs::read_to_string(&cfg_path).map_err(|e| format!("Failed to read project config: {}", e))?;

    // Build payload
    let payload = serde_json::json!({
        "public": true,
        "files": {
            "envstation-export.json": { "content": cfg_content }
        }
    });

    // Prepare client and headers
    let client = reqwest::Client::new();
    let url = "https://api.github.com/gists";
    let resp = client
        .post(url)
        .header(reqwest::header::USER_AGENT, "EnvStation")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", pat))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status();
    let resp_json: serde_json::Value = resp.json().await.map_err(|e| format!("Failed to parse GitHub response: {}", e))?;
    if !status.is_success() {
        let msg = resp_json.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown GitHub API error");
        return Err(format!("GitHub API error ({}): {}", status.as_u16(), msg));
    }

    let html_url = resp_json
        .get("html_url")
        .and_then(|h| h.as_str())
        .ok_or_else(|| "GitHub response did not include html_url".to_string())?;

    Ok(html_url.to_string())
}

#[tauri::command]
pub async fn import_environment(app: tauri::AppHandle, gist_url: String, target_dir: String) -> Result<(), String> {
    // Extract gist id from URL
    let trimmed = gist_url.trim().trim_end_matches('/');
    let gist_id = trimmed.split('/').last().ok_or_else(|| "Invalid Gist URL".to_string())?.to_string();
    if gist_id.is_empty() {
        return Err("Invalid Gist URL".to_string());
    }

    let url = format!("https://api.github.com/gists/{}", gist_id);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "EnvStation")
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status().as_u16()));
    }

    let resp_json: serde_json::Value = resp.json().await.map_err(|e| format!("Failed to parse GitHub response: {}", e))?;
    let files = resp_json.get("files").and_then(|f| f.as_object()).ok_or_else(|| "Malformed Gist response".to_string())?;
    let file_val = files.get("envstation-export.json").ok_or_else(|| "Gist does not contain envstation-export.json".to_string())?;
    let content = file_val.get("content").and_then(|c| c.as_str()).ok_or_else(|| "Gist file missing content".to_string())?;

    // Expand target_dir (~ and $HOME)
    let expanded = if target_dir.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}{}", home.trim_end_matches('/'), target_dir.trim_start_matches('~'))
        } else {
            target_dir.clone()
        }
    } else if target_dir.contains("$HOME") {
        if let Ok(home) = std::env::var("HOME") {
            target_dir.replace("$HOME", &home)
        } else {
            target_dir.clone()
        }
    } else {
        target_dir.clone()
    };

    std::fs::create_dir_all(&expanded).map_err(|e| format!("Failed to create target dir: {}", e))?;

    let manifest_path = PathBuf::from(&expanded).join(".envstation.json");
    fs::write(&manifest_path, content).map_err(|e| format!("Failed to write manifest: {}", e))?;

    // Parse manifest to extract metadata
    let manifest: crate::core::environment::EnvironmentManifest = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse manifest JSON: {}", e))?;

    // Prepare create_environment request
    let req = crate::commands::env::CreateEnvironmentRequest {
        name: manifest.name.clone(),
        template: manifest.stack.clone(),
        home_mount: Some(expanded.clone()),
    };

    // Schedule environment creation using existing command
    crate::commands::env::create_environment(app, req).await.map_err(|e| format!("create_environment failed: {}", e))?;

    Ok(())
}
