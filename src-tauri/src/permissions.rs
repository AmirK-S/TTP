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

/// Request microphone permission - triggers the native macOS permission prompt
#[command]
pub fn request_microphone_permission() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};
        use std::sync::mpsc;

        #[link(name = "AVFoundation", kind = "framework")]
        extern "C" {}

        // First check current status
        let current = check_microphone_permission_impl();
        match current {
            PermissionStatus::Undetermined => {
                // Trigger the native permission dialog via AVCaptureDevice.requestAccessForMediaType
                let (tx, rx) = mpsc::channel();
                unsafe {
                    let media_type_audio: cocoa::base::id =
                        msg_send![class!(NSString), stringWithUTF8String: b"soun\0".as_ptr()];

                    let block = block::ConcreteBlock::new(move |granted: bool| {
                        let _ = tx.send(granted);
                    });
                    let block = block.copy();

                    let _: () = msg_send![
                        class!(AVCaptureDevice),
                        requestAccessForMediaType: media_type_audio
                        completionHandler: &*block
                    ];
                }

                // Wait up to 30 seconds for user response
                match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                    Ok(granted) => {
                        if granted {
                            return Ok(PermissionStatus::Granted);
                        } else {
                            return Ok(PermissionStatus::Denied);
                        }
                    }
                    Err(_) => {
                        return Ok(check_microphone_permission_impl());
                    }
                }
            }
            PermissionStatus::Denied => {
                // Already denied - open System Settings so user can toggle it
                use std::process::Command;
                Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
                    .spawn()
                    .map_err(|e| format!("Failed to open microphone settings: {}", e))?;
            }
            PermissionStatus::Granted => {}
        }
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

/// Check accessibility permission status.
///
/// This does a two-level check on macOS:
/// 1. AXIsProcessTrustedWithOptions — checks if the TCC database says we're trusted
/// 2. probe_accessibility() — actually tries an AX API call to detect stale entries
///
/// After an app update, the binary hash changes and macOS may show the app as
/// "enabled" in System Preferences while the actual AX calls fail. The probe
/// detects this stale state so the UI can guide the user to re-grant access.
#[command]
pub fn check_accessibility_permission() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let api_says_trusted = crate::paste::check_accessibility();

        if !api_says_trusted {
            return PermissionStatus::Denied;
        }

        // API says trusted — but verify with a real AX call to catch stale entries
        if crate::paste::probe_accessibility() {
            PermissionStatus::Granted
        } else {
            // Stale trust entry: TCC says yes, but AX calls fail.
            // Return Denied so the UI prompts the user to fix it.
            eprintln!(
                "[Permissions] Accessibility trust is stale (TCC says trusted but AX calls fail). \
                 This typically happens after an app update."
            );
            PermissionStatus::Denied
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionStatus::Granted
    }
}

/// Request accessibility permission.
///
/// Uses AXIsProcessTrustedWithOptions with the prompt flag, which triggers
/// the native macOS dialog asking the user to grant accessibility. If the
/// trust entry is stale (after an app update), it first resets the TCC entry
/// so the user gets a clean prompt instead of seeing a confusing "already enabled"
/// state.
#[command]
pub fn request_accessibility_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let api_says_trusted = crate::paste::check_accessibility();
        let actually_works = crate::paste::probe_accessibility();

        if api_says_trusted && !actually_works {
            // Stale entry detected — reset TCC so the user gets a fresh prompt
            eprintln!("[Permissions] Resetting stale accessibility TCC entry before re-prompting");
            if let Err(e) = crate::paste::reset_accessibility_tcc() {
                eprintln!("[Permissions] Failed to reset TCC entry: {}. Opening System Settings instead.", e);
                // Fall back to opening System Settings
                std::process::Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                    .spawn()
                    .map_err(|e| format!("Failed to open accessibility settings: {}", e))?;
                return Ok(());
            }
        }

        // Prompt the user via the system dialog
        crate::paste::check_accessibility_with_prompt(true);

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(()) // No action needed on other platforms
    }
}

/// Reset stale accessibility trust and re-prompt.
/// Exposed as a Tauri command so the UI can trigger it explicitly.
#[command]
pub fn reset_accessibility_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Reset the TCC entry
        crate::paste::reset_accessibility_tcc()?;

        // Small delay to let TCC process the reset
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Re-prompt
        crate::paste::check_accessibility_with_prompt(true);

        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}
