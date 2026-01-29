// TTP - Talk To Paste
// Secure API key storage using system keychain

use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

const SERVICE_NAME: &str = "TTP";
const API_KEY_USER: &str = "openai-api-key";

/// Internal function to get API key (for use within Rust code, not as a command)
pub fn get_api_key_internal(app: &AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Get the stored API key from the system keychain
#[tauri::command]
pub async fn get_api_key(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())
}

/// Store an API key in the system keychain
#[tauri::command]
pub async fn set_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    app.keyring()
        .set_password(SERVICE_NAME, API_KEY_USER, &key)
        .map_err(|e| e.to_string())
}

/// Check if an API key exists in the system keychain
#[tauri::command]
pub async fn has_api_key(app: tauri::AppHandle) -> Result<bool, String> {
    let result = app
        .keyring()
        .get_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())?;
    Ok(result.is_some())
}

/// Delete the API key from the system keychain
#[tauri::command]
pub async fn delete_api_key(app: tauri::AppHandle) -> Result<(), String> {
    app.keyring()
        .delete_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())
}
