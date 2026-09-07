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

/// In-process cache for the Groq key.
///
/// `docs/tracing.md` says of `keychain.api_key`: "a keychain round-trip
/// sitting between the user's last word and the Whisper call, and it is
/// unbounded". It was, and it was the only keychain read left on the
/// dictation path that had neither a cache nor a warm-up — the usage and
/// licence secrets got both; this one was missed because nothing had yet
/// been observed to be slow through it.
///
/// That is not a reason to leave it. The same securityd that took 62,304 ms
/// on `usage_hmac_secret` serves `groq_api_key`, from the same service, under
/// the same ACL, and a slow read here does not merely delay bookkeeping — it
/// delays the transcription itself.
///
/// Correctness of caching: unlike the HMAC secrets, this value *can* change —
/// the user can paste a new key in Settings, or clear it. Both paths go
/// through `set_groq_api_key` / `delete_groq_api_key` below, and both drop
/// the cache entry. Nothing else on the machine can change it under us: the
/// legacy-file migration happens on the same read that populates the cache.
fn key_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<String>>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Forget the cached key. Called after any write that changes it, so a user
/// who pastes a new key does not keep dictating with the old one.
fn invalidate_key_cache() {
    if let Ok(mut cache) = key_cache().lock() {
        cache.remove(GROQ_KEY_ACCOUNT);
    }
}

/// Pay the keychain's ACL evaluation at launch, on a thread nobody is waiting
/// on, rather than between the user's last word and the Whisper call.
/// Mirrors `keychain::warm_caches`; nothing waits on it.
pub fn warm_key_cache() {
    let started = std::time::Instant::now();
    let found = read_groq_key().is_some();
    crate::trace::event(
        "keychain.warmed",
        serde_json::json!({
            "account": GROQ_KEY_ACCOUNT,
            "ms": started.elapsed().as_millis() as u64,
            // Never the key, and never its length. Only whether one exists,
            // which is what the first dictation is about to ask.
            "found": found,
        }),
    );
}

/// Read the Groq API key, migrating from the legacy JSON file on first call
/// after upgrade. Order: env var → keychain → legacy JSON (migrated).
///
/// At most one keychain round-trip per process, and at most one across
/// however many callers arrive together — see [`crate::keychain::read_once`].
fn read_groq_key() -> Option<String> {
    // The env override is checked on every call, before the cache: it costs
    // nothing, and a test or CI run that exports GROQ_API_KEY expects it to
    // win regardless of what a previous read cached.
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }
    crate::keychain::read_once(key_cache(), GROQ_KEY_ACCOUNT, read_groq_key_uncached).value
}

/// The keychain round-trip itself, kept separate so the caching wrapper above
/// stays readable and the interaction lives in one place.
fn read_groq_key_uncached() -> Option<String> {
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
    // Before returning, not after: the next dictation must not be able to
    // read a stale key between the write landing and the cache being dropped.
    invalidate_key_cache();
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
    invalidate_key_cache();
    delete_legacy_file();
    Ok(())
}

/// Validate a Groq API key by making a lightweight GET request to the models endpoint.
/// Returns Ok(()) if the key is valid, or Err(code) with a translation key
/// from `error.api_*`. The frontend resolves the key via i18next; dynamic
/// detail (network error text, HTTP body) is logged via `log_error` for
/// developer debugging but never surfaced to the user — the generic localized
/// message is enough.
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
                "error.api_timeout".to_string()
            } else {
                crate::logging::log_error(&format!("API network error: {}", e));
                "error.api_network".to_string()
            }
        })?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else if status.as_u16() == 401 {
        Err("error.api_invalid_key".to_string())
    } else if status.as_u16() == 403 {
        Err("error.api_forbidden".to_string())
    } else {
        let body = response.text().await.unwrap_or_default();
        crate::logging::log_error(&format!("Groq API error {}: {}", status.as_u16(), body));
        Err("error.api_generic".to_string())
    }
}
