// TTP - Keyboard simulation
// Simulates Cmd+V paste keystroke using enigo
//
// Requires macOS accessibility permission to function.
// If permission is not granted, this will fail silently or return an error.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Simulate a paste keystroke (Cmd+V on macOS)
///
/// This uses the enigo crate to simulate keyboard input.
/// On macOS, this requires Accessibility permission to be granted in
/// System Settings > Privacy & Security > Accessibility.
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create enigo: {}", e))?;

    // Small delay to ensure target app has focus
    // This helps when the floating bar was just hidden
    thread::sleep(Duration::from_millis(50));

    // Press Cmd (Meta) key
    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| format!("Failed to press Meta: {}", e))?;

    // Press and release V while Cmd is held
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Failed to click V: {}", e))?;

    // Release Cmd key
    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|e| format!("Failed to release Meta: {}", e))?;

    Ok(())
}
