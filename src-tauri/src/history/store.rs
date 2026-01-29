// TTP - Talk To Paste
// History store - handles transcription history persistence to JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single history entry representing a past transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The polished/final transcription text
    pub text: String,
    /// Unix timestamp in milliseconds when transcription was created
    pub timestamp: i64,
    /// The raw transcription text before AI polish (if polish was enabled)
    pub raw_text: Option<String>,
}

/// Get the history file path (~/.config/ttp/history.json)
fn get_history_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("history.json"))
}

/// Load history from file, return empty vec if file doesn't exist
/// Returns entries sorted by timestamp, newest first
#[tauri::command]
pub fn get_history() -> Vec<HistoryEntry> {
    let Some(path) = get_history_path() else {
        return Vec::new();
    };

    if !path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            let mut entries: Vec<HistoryEntry> =
                serde_json::from_str(&content).unwrap_or_default();
            // Sort by timestamp descending (newest first)
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            entries
        }
        Err(_) => Vec::new(),
    }
}

/// Add a new entry to history
/// Prepends to existing history (newest first)
pub fn add_history_entry(text: &str, raw_text: Option<&str>) -> Result<(), String> {
    let path = get_history_path().ok_or("Could not determine config directory")?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Load existing history
    let mut entries = get_history();

    // Create new entry with current timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let entry = HistoryEntry {
        text: text.to_string(),
        timestamp,
        raw_text: raw_text.map(|s| s.to_string()),
    };

    // Prepend new entry (will be at start after sort)
    entries.insert(0, entry);

    // Save back to file
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write history file: {}", e))?;

    Ok(())
}

/// Clear all history by deleting the history file
#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    let Some(path) = get_history_path() else {
        return Ok(()); // No config dir, nothing to clear
    };

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete history file: {}", e))?;
    }

    Ok(())
}
