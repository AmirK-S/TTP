// TTP - Keyboard simulation
// Simulates Cmd+V paste keystroke
//
// Uses AppleScript on macOS for reliability (avoids enigo FFI crashes)

use std::process::Command;
use std::thread;
use std::time::Duration;

/// Simulate a paste keystroke (Cmd+V on macOS)
///
/// Uses AppleScript to send keystroke, which is more reliable than
/// direct keyboard simulation and doesn't require Accessibility permission.
pub fn simulate_paste() -> Result<(), String> {
    // Small delay to ensure target app has focus
    thread::sleep(Duration::from_millis(100));

    // Use AppleScript to simulate Cmd+V
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("osascript failed: {}", stderr))
    }
}
