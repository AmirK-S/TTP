// TTP - Talk To Paste
// Permission checking module for onboarding flow

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::command;

/// Microphone permission status
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum PermissionStatus {
    Granted,
    Denied,
    Undetermined,
}

/// Get the app config directory path
fn get_config_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ttp");
    
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    
    config_dir
}

/// Get the first launch marker file path
fn get_first_launch_file() -> PathBuf {
    get_config_dir().join("first_launch_complete")
}

/// Check if this is the first launch of the app
/// Returns true if the app has never been launched or onboarding hasn't been completed
pub fn is_first_launch() -> bool {
    !get_first_launch_file().exists()
}

/// Tauri command to check if this is the first launch
#[command]
pub fn is_first_launch_cmd() -> bool {
    is_first_launch()
}

/// Mark the first launch as complete
/// This should be called after onboarding successfully completes
pub fn mark_first_launch_complete() -> Result<(), String> {
    let file_path = get_first_launch_file();
    
    // Create the marker file with a timestamp
    let content = format!(
        "first_launch_complete={}",
        chrono::Utc::now().to_rfc3339()
    );
    
    fs::write(&file_path, content)
        .map_err(|e| format!("Failed to mark first launch complete: {}", e))
}

/// Tauri command to mark first launch complete
#[command]
pub fn mark_first_launch_complete_cmd() -> Result<(), String> {
    mark_first_launch_complete()
}

/// Check the current microphone permission status
/// On macOS, uses AVFoundation to check the microphone authorization status
#[cfg(target_os = "macos")]
pub fn check_microphone_permission_impl() -> PermissionStatus {
    use std::process::Command;
    
    // Use macOS system_profiler to check microphone permission status
    // Alternative: Use AppleScript to check System Preferences > Security & Privacy > Microphone
    let output = Command::new("system_profiler")
        .args(["SPMicrophoneDataType", "-json"])
        .output();
    
    match output {
        Ok(output) => {
            if !output.status.success() {
                return PermissionStatus::Undetermined;
            }
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Parse JSON output to check if TTP has microphone access
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(data) = json.get("SPMicrophoneDataType").and_then(|v| v.as_array()) {
                    for item in data {
                        if let Some(name) = item.get("_name").and_then(|v| v.as_str()) {
                            if name.contains("TTP") || name.contains("Talk To Paste") {
                                // Check if the app has access
                                if let Some(media) = item.get("media").and_then(|v| v.as_array()) {
                                    if !media.is_empty() {
                                        return PermissionStatus::Granted;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // If we can't find TTP in the list, permission is either denied or undetermined
            // For a more accurate check, we'd need to use CoreFoundation
            // For now, we'll check the TCC database directly
            check_tcc_microphone_permission_impl()
        }
        Err(_) => PermissionStatus::Undetermined,
    }
}

/// Check TCC (Transparency, Control, and Consent) database directly for microphone access
#[cfg(target_os = "macos")]
fn check_tcc_microphone_permission_impl() -> PermissionStatus {
    use std::process::Command;
    
    // Query TCC database for microphone access for our app
    let bundle_id = "com.talktopaste.app";
    
    let output = Command::new("sqlite3")
        .args([
            "/Library/Application Support/com.apple.TCC/TCC.db",
            &format!(
                "SELECT allowed FROM access WHERE client='{}' AND service='com.apple.security.device.microphone'",
                bundle_id
            ),
        ])
        .output();
    
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match stdout.as_str() {
                "1" => PermissionStatus::Granted,
                "0" => PermissionStatus::Denied,
                _ => PermissionStatus::Undetermined,
            }
        }
        Err(_) => {
            // Try user TCC database
            let home = dirs::home_dir().unwrap_or_default();
            let tcc_db = home.join("Library/Application Support/com.apple.TCC/TCC.db");
            
            if tcc_db.exists() {
                let output = Command::new("sqlite3")
                    .args([
                        tcc_db.to_str().unwrap_or(""),
                        &format!(
                            "SELECT allowed FROM access WHERE client='{}' AND service='com.apple.security.device.microphone'",
                            bundle_id
                        ),
                    ])
                    .output();
                
                match output {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        match stdout.as_str() {
                            "1" => PermissionStatus::Granted,
                            "0" => PermissionStatus::Denied,
                            _ => PermissionStatus::Undetermined,
                        }
                    }
                    Err(_) => PermissionStatus::Undetermined,
                }
            } else {
                PermissionStatus::Undetermined
            }
        }
    }
}

/// Check microphone permission on non-macOS platforms (simplified)
#[cfg(not(target_os = "macos"))]
pub fn check_microphone_permission_impl() -> PermissionStatus {
    // On non-macOS platforms, assume permission is granted or check platform-specific
    // For now, return Granted as most desktop platforms don't have the same permission model
    PermissionStatus::Granted
}

/// Tauri command to check microphone permission status
#[command]
pub fn check_microphone_permission() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        check_microphone_permission_impl()
    }
    #[cfg(not(target_os = "macos"))]
    {
        check_microphone_permission_impl()
    }
}

/// Get a human-readable message for the permission status
pub fn get_permission_message(status: &PermissionStatus) -> String {
    match status {
        PermissionStatus::Granted => "Microphone access is granted. You're all set!".to_string(),
        PermissionStatus::Denied => {
            "Microphone access is denied. Please enable it in System Settings.".to_string()
        }
        PermissionStatus::Undetermined => {
            "Microphone permission has not been requested yet.".to_string()
        }
    }
}

/// Get instructions for enabling microphone permission in System Settings
pub fn get_permission_instructions(status: &PermissionStatus) -> String {
    match status {
        PermissionStatus::Granted => String::new(),
        PermissionStatus::Denied => {
            "1. Open System Settings\n2. Go to Privacy & Security\n3. Click on Microphone\n4. Enable TTP (Talk To Paste)".to_string()
        }
        PermissionStatus::Undetermined => {
            "1. Open System Settings\n2. Go to Privacy & Security\n3. Click on Microphone\n4. Enable TTP to allow microphone access".to_string()
        }
    }
}
