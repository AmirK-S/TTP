// TTP - Talk To Paste
// API key storage for Groq

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeys {
    pub groq: Option<String>,
}

/// Get the API keys file path (~/.config/ttp/api-keys.json on macOS)
fn get_keys_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("api-keys.json"))
}

/// Load API keys from file
fn load_keys() -> ApiKeys {
    let Some(path) = get_keys_path() else {
        return ApiKeys::default();
    };

    if !path.exists() {
        return ApiKeys::default();
    }

    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ApiKeys::default(),
    }
}

/// Save API keys to file
fn save_keys(keys: &ApiKeys) -> Result<(), String> {
    let path = get_keys_path().ok_or("Could not determine config directory")?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(keys)
        .map_err(|e| format!("Failed to serialize keys: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write keys file: {}", e))?;

    Ok(())
}

pub fn get_groq_api_key_internal(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().groq)
}

#[tauri::command]
pub async fn get_groq_api_key(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().groq)
}

#[tauri::command]
pub async fn set_groq_api_key(_app: tauri::AppHandle, key: String) -> Result<(), String> {
    let mut keys = load_keys();
    keys.groq = Some(key);
    save_keys(&keys)
}

#[tauri::command]
pub async fn has_groq_api_key(_app: tauri::AppHandle) -> Result<bool, String> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Ok(true);
        }
    }
    Ok(load_keys().groq.is_some())
}

#[tauri::command]
pub async fn delete_groq_api_key(_app: tauri::AppHandle) -> Result<(), String> {
    let mut keys = load_keys();
    keys.groq = None;
    save_keys(&keys)
}

/// Validate a Groq API key by making a lightweight GET request to the models endpoint.
/// Returns Ok(()) if the key is valid, or Err(message) with a user-friendly error.
#[tauri::command]
pub async fn validate_groq_api_key(key: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get("https://api.groq.com/openai/v1/models")
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Request timed out — check your internet connection".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else if status.as_u16() == 401 {
        Err("Invalid API key".to_string())
    } else if status.as_u16() == 403 {
        Err("API key does not have access — check your Groq account".to_string())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(format!("Groq API error ({}): {}", status.as_u16(), body))
    }
}
