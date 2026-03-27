// TTP - Talk To Paste
// "What's New" version tracking — shows changelog after app update

use std::fs;
use std::path::PathBuf;
use tauri::command;

/// Get the config directory (same as permissions.rs)
fn get_config_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ttp");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }

    config_dir
}

/// Path to the file that stores the last version the user has seen
fn last_seen_version_path() -> PathBuf {
    get_config_dir().join("last_seen_version")
}

/// Current app version (from Cargo.toml / tauri.conf.json)
fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Hardcoded changelogs per version.
/// Returns None if no changelog is available for that version.
fn changelog_for(version: &str) -> Option<&'static str> {
    match version {
        "1.3.5" => Some(
            "• Left-click on menu bar icon now opens the menu\n\
             • Tray menu shows \"Stop Recording\" during active recording\n\
             • Tutorial text adapts to your language (EN/FR)\n\
             • API key is validated before saving\n\
             • History capped at 500 entries\n\
             • Fixed Windows shortcut conflicts (Ctrl+Space is now default)\n\
             • Removed unused audio codec for faster builds",
        ),
        _ => None,
    }
}

/// Check whether a "What's New" popup should be shown.
/// Returns `Some((version, changelog))` if the app was updated since the user last dismissed,
/// or `None` if the user is already up-to-date.
#[command]
pub fn check_whats_new() -> Option<(String, String)> {
    let version = current_version();

    // Read last seen version (if the file doesn't exist, treat as "never seen")
    let last_seen = fs::read_to_string(last_seen_version_path())
        .ok()
        .map(|s| s.trim().to_string());

    // If the user already saw this version, nothing to show
    if last_seen.as_deref() == Some(version) {
        return None;
    }

    // Return changelog if one exists for the current version
    changelog_for(version).map(|log| (version.to_string(), log.to_string()))
}

/// Dismiss the "What's New" popup by recording the current version.
#[command]
pub fn dismiss_whats_new() -> Result<(), String> {
    let version = current_version();
    fs::write(last_seen_version_path(), version)
        .map_err(|e| format!("Failed to save last_seen_version: {}", e))
}
