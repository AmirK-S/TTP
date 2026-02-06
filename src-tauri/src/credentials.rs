// TTP - Talk To Paste
// API key storage using simple JSON file (no keychain)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeys {
    pub openai: Option<String>,
    pub groq: Option<String>,
    pub gladia: Option<String>,
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

// ============ OpenAI ============

/// Internal function to get OpenAI API key
/// Priority: 1. Environment variable, 2. Config file
pub fn get_api_key_internal(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().openai)
}

#[tauri::command]
pub async fn get_api_key(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().openai)
}

#[tauri::command]
pub async fn set_api_key(_app: tauri::AppHandle, key: String) -> Result<(), String> {
    let mut keys = load_keys();
    keys.openai = Some(key);
    save_keys(&keys)
}

#[tauri::command]
pub async fn has_api_key(_app: tauri::AppHandle) -> Result<bool, String> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Ok(true);
        }
    }
    Ok(load_keys().openai.is_some())
}

#[tauri::command]
pub async fn delete_api_key(_app: tauri::AppHandle) -> Result<(), String> {
    let mut keys = load_keys();
    keys.openai = None;
    save_keys(&keys)
}

// ============ Groq ============

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

// ============ Gladia ============

pub fn get_gladia_api_key_internal(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("GLADIA_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().gladia)
}

#[tauri::command]
pub async fn get_gladia_api_key(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var("GLADIA_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    Ok(load_keys().gladia)
}

#[tauri::command]
pub async fn set_gladia_api_key(_app: tauri::AppHandle, key: String) -> Result<(), String> {
    let mut keys = load_keys();
    keys.gladia = Some(key);
    save_keys(&keys)
}

#[tauri::command]
pub async fn has_gladia_api_key(_app: tauri::AppHandle) -> Result<bool, String> {
    if let Ok(key) = std::env::var("GLADIA_API_KEY") {
        if !key.is_empty() {
            return Ok(true);
        }
    }
    Ok(load_keys().gladia.is_some())
}

#[tauri::command]
pub async fn delete_gladia_api_key(_app: tauri::AppHandle) -> Result<(), String> {
    let mut keys = load_keys();
    keys.gladia = None;
    save_keys(&keys)
}
