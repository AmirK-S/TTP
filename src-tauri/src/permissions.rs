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

/// Request microphone permission - opens System Settings for the user to grant access
#[command]
pub fn request_microphone_permission() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Open System Settings > Privacy & Security > Microphone
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn()
            .map_err(|e| format!("Failed to open microphone settings: {}", e))?;
    }

    // Return current status
    Ok(check_microphone_permission())
}

/// Check the current microphone permission status
/// On macOS, uses AVCaptureDevice.authorizationStatus(for: .audio) via objc
#[cfg(target_os = "macos")]
pub fn check_microphone_permission_impl() -> PermissionStatus {
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    // AVAuthorizationStatus values:
    // 0 = NotDetermined, 1 = Restricted, 2 = Denied, 3 = Authorized
    // AVMediaType.audio = "soun" (FourCC)
    unsafe {
        let media_type_audio: cocoa::base::id =
            msg_send![class!(NSString), stringWithUTF8String: b"soun\0".as_ptr()];
        let status: i64 =
            msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: media_type_audio];

        match status {
            3 => PermissionStatus::Granted,  // AVAuthorizationStatusAuthorized
            2 | 1 => PermissionStatus::Denied, // Denied or Restricted
            _ => PermissionStatus::Undetermined, // NotDetermined (0) or unknown
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

// ============================================================================
// Accessibility Permission
// ============================================================================

/// Check accessibility permission status
#[command]
pub fn check_accessibility_permission() -> PermissionStatus {
    let granted = crate::paste::check_accessibility();
    if granted {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

/// Request accessibility permission - opens System Settings
#[command]
pub fn request_accessibility_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Open System Settings > Privacy & Security > Accessibility
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| format!("Failed to open accessibility settings: {}", e))?;

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(()) // No action needed on other platforms
    }
}
