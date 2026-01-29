// TTP - Keyboard simulation
// Simulates Cmd+V paste keystroke on macOS, Ctrl+V on Windows
//
// Uses enigo for cross-platform keyboard simulation

use enigo::{Direction::{Click, Press, Release}, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Simulate a paste keystroke (Cmd+V on macOS, Ctrl+V on Windows)
///
/// Uses enigo for cross-platform keyboard simulation.
/// Note: On macOS, the system will automatically prompt for Accessibility
/// permission the first time this runs.
pub fn simulate_paste() -> Result<(), String> {
    // Small delay to ensure target app has focus
    thread::sleep(Duration::from_millis(100));

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create Enigo: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        enigo.key(Key::Meta, Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Meta, Release).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        enigo.key(Key::Control, Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}
