// TTP - Talk To Paste
// API key storage for Groq.
//
// Storage strategy: macOS Keychain / Windows Credential Manager via the
// `keyring` crate. The legacy plaintext JSON file at
// ~/.config/ttp/api-keys.json is read once on first run after upgrade and
// migrated into the keychain (then deleted). Brand-new installs never
// touch the JSON path.
//
// Env override: GROQ_API_KEY in the environment always wins, for testing
// and CI scenarios.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "com.ttp.desktop";
const GROQ_KEY_ACCOUNT: &str = "groq_api_key";

/// Used only to deserialize the legacy ~/.config/ttp/api-keys.json file
/// during the one-time migration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LegacyApiKeys {
    pub groq: Option<String>,
}

fn legacy_keys_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("api-keys.json"))
}

fn load_legacy_keys() -> LegacyApiKeys {
    let Some(path) = legacy_keys_path() else {
        return LegacyApiKeys::default();
    };
    if !path.exists() {
        return LegacyApiKeys::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => LegacyApiKeys::default(),
    }
}

fn delete_legacy_file() {
    if let Some(path) = legacy_keys_path() {
        let _ = fs::remove_file(path);
    }
}

fn keychain_entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|e| format!("Failed to open keychain entry: {}", e))
}

fn keychain_get(account: &str) -> Option<String> {
    let entry = keychain_entry(account).ok()?;
    match entry.get_password() {
        Ok(value) => Some(value),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            crate::logging::log_error(&format!("Keychain read failed: {}", e));
            None
        }
    }
}

fn keychain_set(account: &str, value: &str) -> Result<(), String> {
    let entry = keychain_entry(account)?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to write to keychain: {}", e))
}

fn keychain_delete(account: &str) -> Result<(), String> {
    let entry = keychain_entry(account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to delete from keychain: {}", e)),
    }
}

/// Read the Groq API key, migrating from the legacy JSON file on first call
/// after upgrade. Order: env var → keychain → legacy JSON (migrated).
fn read_groq_key() -> Option<String> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }
    if let Some(key) = keychain_get(GROQ_KEY_ACCOUNT) {
        return Some(key);
    }
    // First run after upgrade: take the key from the legacy file, write it
    // to keychain, delete the legacy file. Best-effort — if the keychain
    // write fails we leave the legacy file in place so the user isn't
    // suddenly without a key.
    let legacy = load_legacy_keys();
    if let Some(key) = legacy.groq {
        if keychain_set(GROQ_KEY_ACCOUNT, &key).is_ok() {
            delete_legacy_file();
            crate::logging::log_info("Migrated Groq API key from legacy JSON to keychain");
        }
        return Some(key);
    }
    None
}

pub fn get_groq_api_key_internal(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(read_groq_key())
}

#[tauri::command]
pub async fn get_groq_api_key(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(read_groq_key())
}

#[tauri::command]
pub async fn set_groq_api_key(_app: tauri::AppHandle, key: String) -> Result<(), String> {
    keychain_set(GROQ_KEY_ACCOUNT, &key)?;
    // If the legacy file is still around (e.g. first set after upgrade
    // happened before any read), wipe it now too.
    delete_legacy_file();
    Ok(())
}

#[tauri::command]
pub async fn has_groq_api_key(_app: tauri::AppHandle) -> Result<bool, String> {
    Ok(read_groq_key().is_some())
}

#[tauri::command]
pub async fn delete_groq_api_key(_app: tauri::AppHandle) -> Result<(), String> {
    keychain_delete(GROQ_KEY_ACCOUNT)?;
    delete_legacy_file();
    Ok(())
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
