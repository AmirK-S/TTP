// TTP - Talk To Paste
// Persistent error logging to file
//
// Logs API errors, crashes, and failures to a rotating log file
// in the app data directory. Users can share this for debugging.

use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Max log file size before rotation (500KB)
const MAX_LOG_SIZE: u64 = 500_000;

/// Get the log file path in the app data directory
fn log_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.ttp.desktop").join("ttp.log"))
}

/// Get the rotated log file path
fn log_path_old() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.ttp.desktop").join("ttp.log.old"))
}

/// Rotate log file if it's too large
fn rotate_if_needed(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_LOG_SIZE {
            if let Some(old_path) = log_path_old() {
                let _ = fs::rename(path, old_path);
            }
        }
    }
}

/// Write a log entry to the persistent log file
pub fn log_to_file(level: &str, message: &str) {
    let Some(path) = log_path() else { return };

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    rotate_if_needed(&path);

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] [{}] {}\n", timestamp, level, message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(entry.as_bytes());
    }
}

/// Log an error
pub fn log_error(message: &str) {
    log_to_file("ERROR", message);
    eprintln!("[ERROR] {}", message);
}

/// Log a warning
pub fn log_warn(message: &str) {
    log_to_file("WARN", message);
}

/// Log an info event
pub fn log_info(message: &str) {
    log_to_file("INFO", message);
}
