// TTP - Talk To Paste
// Settings store - handles settings persistence to JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Whether to run AI polish on transcriptions (removes filler words, fixes grammar)
    pub ai_polish_enabled: bool,
    /// Global keyboard shortcut for recording (e.g., "Alt+Space", "Ctrl+Shift+R")
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    /// Use Fn key as push-to-talk trigger (macOS only)
    #[serde(default)]
    pub fn_key_enabled: bool,
    /// Telemetry opt-in: controls error reporting (Sentry) and usage analytics (Aptabase)
    /// Default is OFF -- user must explicitly enable
    #[serde(default)]
    pub telemetry_enabled: bool,
    /// Hands-free mode (double-tap to toggle) - persists across app restarts
    #[serde(default)]
    pub hands_free_mode: bool,
    /// Hide the recording indicator pill when not recording
    #[serde(default)]
    pub hide_pill_when_inactive: bool,
    /// Whether to save transcriptions to history. Default ON.
    #[serde(default = "default_true")]
    pub history_enabled: bool,
    /// Whether to subscribe to the beta update channel. Default OFF (stable).
    /// When true, the updater queries `latest-beta.json` instead of `latest.json`.
    #[serde(default)]
    pub use_beta_channel: bool,
}

fn default_true() -> bool {
    true
}

fn default_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        // Fn key is default on macOS
        "FnKey".to_string()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Ctrl+Space on Windows/Linux (no conflicts with system shortcuts)
        "Ctrl+Space".to_string()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_polish_enabled: true,
            shortcut: default_shortcut(),
            #[cfg(target_os = "macos")]
            fn_key_enabled: true,
            #[cfg(not(target_os = "macos"))]
            fn_key_enabled: false,
            telemetry_enabled: false,
            hands_free_mode: false,
            hide_pill_when_inactive: false,
            history_enabled: true,
            use_beta_channel: false,
        }
    }
}

/// Get the settings file path (~/.config/ttp/settings.json)
fn get_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("settings.json"))
}

/// Backup of the most recent valid settings file. Used to recover from a
/// corrupted primary file (e.g. interrupted write, future schema migration
/// gone wrong) instead of silently resetting the user's preferences.
fn get_settings_backup_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("settings.json.bak"))
}

/// Try the .bak file when the primary settings file is unreadable / unparseable.
/// Returns Settings::default() if the backup is also missing or corrupt.
fn recover_from_backup() -> Settings {
    let Some(bak) = get_settings_backup_path() else {
        return Settings::default();
    };
    if !bak.exists() {
        return Settings::default();
    }
    match fs::read_to_string(&bak) {
        Ok(content) => match serde_json::from_str::<Settings>(&content) {
            Ok(s) => {
                crate::logging::log_info("Recovered settings from settings.json.bak");
                s
            }
            Err(_) => Settings::default(),
        },
        Err(_) => Settings::default(),
    }
}

/// Load settings from file, return defaults if file doesn't exist
#[tauri::command]
pub fn get_settings() -> Settings {
    let Some(path) = get_settings_path() else {
        return Settings::default();
    };

    if !path.exists() {
        return Settings::default();
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return recover_from_backup(),
    };

    match serde_json::from_str::<Settings>(&content) {
        Ok(s) => s,
        Err(e) => {
            crate::logging::log_error(&format!(
                "settings.json failed to parse ({}); falling back to .bak",
                e
            ));
            recover_from_backup()
        }
    }
}

/// Save settings to file. Copies the current file to settings.json.bak first
/// so a parse failure on the next load (e.g. a half-written file, or a
/// future field whose schema we got wrong) can recover instead of resetting
/// to defaults.
#[tauri::command]
pub fn set_settings(settings: Settings, app: AppHandle) -> Result<(), String> {
    let path = get_settings_path().ok_or("Could not determine config directory")?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Snapshot the current valid file before overwriting it. Best-effort —
    // if the copy fails (e.g. permission, disk full) we still proceed; we'd
    // rather lose the backup than block the user from changing a setting.
    if path.exists() {
        if let Some(bak) = get_settings_backup_path() {
            let _ = fs::copy(&path, &bak);
        }
    }

    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write settings file: {}", e))?;

    // Emit event to notify all windows of settings change
    app.emit("settings-changed", &settings).ok();

    Ok(())
}

/// Reset settings to defaults by deleting the settings file
#[tauri::command]
pub fn reset_settings() -> Result<(), String> {
    let Some(path) = get_settings_path() else {
        return Ok(()); // No config dir, nothing to reset
    };

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete settings file: {}", e))?;
    }

    // Also clear the backup so a stale recovery doesn't resurrect old settings.
    if let Some(bak) = get_settings_backup_path() {
        if bak.exists() {
            let _ = fs::remove_file(&bak);
        }
    }

    Ok(())
}
