// TTP - Talk To Paste
// Secure API key storage using system keychain

use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

const SERVICE_NAME: &str = "TTP";
const OPENAI_API_KEY_USER: &str = "openai-api-key";
const GROQ_API_KEY_USER: &str = "groq-api-key";

/// Internal function to get OpenAI API key (for use within Rust code, not as a command)
/// Priority: 1. Environment variable OPENAI_API_KEY, 2. System keychain
pub fn get_api_key_internal(app: &AppHandle) -> Result<Option<String>, String> {
    // Check environment variable first (useful for development)
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            println!("OpenAI API key found in environment variable");
            return Ok(Some(key));
        }
    }

    // Fall back to keychain
    app.keyring()
        .get_password(SERVICE_NAME, OPENAI_API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Internal function to get Groq API key (for use within Rust code, not as a command)
/// Priority: 1. Environment variable GROQ_API_KEY, 2. System keychain
pub fn get_groq_api_key_internal(app: &AppHandle) -> Result<Option<String>, String> {
    // Check environment variable first (useful for development)
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            println!("Groq API key found in environment variable");
            return Ok(Some(key));
        }
    }

    // Fall back to keychain
    app.keyring()
        .get_password(SERVICE_NAME, GROQ_API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Get the stored OpenAI API key
/// Priority: 1. Environment variable OPENAI_API_KEY, 2. System keychain
#[tauri::command]
pub async fn get_api_key(app: tauri::AppHandle) -> Result<Option<String>, String> {
    // Check environment variable first
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }

    // Fall back to keychain
    app.keyring()
        .get_password(SERVICE_NAME, OPENAI_API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Store an OpenAI API key in the system keychain
#[tauri::command]
pub async fn set_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    app.keyring()
        .set_password(SERVICE_NAME, OPENAI_API_KEY_USER, &key)
        .map_err(|e| e.to_string())
}

/// Check if an OpenAI API key exists
/// Priority: 1. Environment variable OPENAI_API_KEY, 2. System keychain
#[tauri::command]
pub async fn has_api_key(app: tauri::AppHandle) -> Result<bool, String> {
    // Check environment variable first
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Ok(true);
        }
    }

    // Fall back to keychain
    let result = app
        .keyring()
        .get_password(SERVICE_NAME, OPENAI_API_KEY_USER)
        .map_err(|e| e.to_string())?;
    Ok(result.is_some())
}

/// Delete the OpenAI API key from the system keychain
#[tauri::command]
pub async fn delete_api_key(app: tauri::AppHandle) -> Result<(), String> {
    app.keyring()
        .delete_password(SERVICE_NAME, OPENAI_API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Get the stored Groq API key
/// Priority: 1. Environment variable GROQ_API_KEY, 2. System keychain
#[tauri::command]
pub async fn get_groq_api_key(app: tauri::AppHandle) -> Result<Option<String>, String> {
    // Check environment variable first
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }

    // Fall back to keychain
    app.keyring()
        .get_password(SERVICE_NAME, GROQ_API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Store a Groq API key in the system keychain
#[tauri::command]
pub async fn set_groq_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    app.keyring()
        .set_password(SERVICE_NAME, GROQ_API_KEY_USER, &key)
        .map_err(|e| e.to_string())
}

/// Check if a Groq API key exists
/// Priority: 1. Environment variable GROQ_API_KEY, 2. System keychain
#[tauri::command]
pub async fn has_groq_api_key(app: tauri::AppHandle) -> Result<bool, String> {
    // Check environment variable first
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Ok(true);
        }
    }

    // Fall back to keychain
    let result = app
        .keyring()
        .get_password(SERVICE_NAME, GROQ_API_KEY_USER)
        .map_err(|e| e.to_string())?;
    Ok(result.is_some())
}

/// Delete the Groq API key from the system keychain
#[tauri::command]
pub async fn delete_groq_api_key(app: tauri::AppHandle) -> Result<(), String> {
    app.keyring()
        .delete_password(SERVICE_NAME, GROQ_API_KEY_USER)
        .map_err(|e| e.to_string())
}
