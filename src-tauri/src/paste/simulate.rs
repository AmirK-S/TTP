// TTP - Keyboard simulation
// Simulates Cmd+V paste keystroke on macOS, Ctrl+V on Windows

use std::thread;
use std::time::Duration;

/// Simulate a paste keystroke (Cmd+V on macOS, Ctrl+V on Windows)
pub fn simulate_paste() -> Result<(), String> {
    // Small delay to ensure target app has focus
    thread::sleep(Duration::from_millis(100));

    #[cfg(target_os = "macos")]
    {
        // Use AppleScript for reliable paste on macOS
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("AppleScript paste failed: {}", stderr));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Direction::{Click, Press, Release}, Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to create Enigo: {}", e))?;

        enigo.key(Key::Control, Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}
