// TTP - Accessibility permissions
// Checks if accessibility permission is granted for keyboard simulation
//
// We use AppleScript with System Events which requires Accessibility permission.

use std::process::Command;

/// Check if accessibility permission is granted
///
/// Tests by running a simple AppleScript System Events command.
/// System Events requires Accessibility permission to automate keystrokes.
///
/// Returns:
/// - `true` if permission is granted
/// - `false` if permission is denied or check fails
pub fn check_accessibility() -> bool {
    // Try a simple System Events command
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to return 1")
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
