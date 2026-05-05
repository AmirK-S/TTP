// TTP - Talk To Paste
// Audio recording module - manages recording paths and context

use std::path::PathBuf;
use tauri::Manager;

/// Context for tracking current recording state
pub struct RecordingContext {
    pub current_file: Option<PathBuf>,
}

impl Default for RecordingContext {
    fn default() -> Self {
        Self { current_file: None }
    }
}

/// Get the directory where recordings are stored
pub fn get_recording_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("recordings"))
        .map_err(|e| format!("Failed to get app data dir: {}", e))
}

/// Generate a unique path for a new recording with timestamp
pub fn generate_recording_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = get_recording_dir(app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create recordings directory: {}", e))?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    Ok(dir.join(format!("recording_{}.wav", timestamp)))
}

/// Get the path to the most recent recording (for debugging/testing)
#[tauri::command]
pub fn get_recordings_dir(app: tauri::AppHandle) -> Result<String, String> {
    let dir = get_recording_dir(&app)?;
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".to_string())
}
