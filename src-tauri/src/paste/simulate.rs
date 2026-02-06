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
        use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        // Key code for 'v' on macOS
        const KEY_V: CGKeyCode = 9;

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "Failed to create event source")?;

        // Create key down event for 'v' with Command modifier
        let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
            .map_err(|_| "Failed to create key down event")?;
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);

        // Create key up event for 'v' with Command modifier
        let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
            .map_err(|_| "Failed to create key up event")?;
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);

        // Post the events to the annotated session (current user session)
        key_down.post(CGEventTapLocation::AnnotatedSession);
        thread::sleep(Duration::from_millis(10));
        key_up.post(CGEventTapLocation::AnnotatedSession);
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
