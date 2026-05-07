// TTP - Keyboard simulation
// Two strategies:
//   * `simulate_paste()` — Cmd+V (macOS) / Ctrl+V (Windows). Requires text to
//     already be on the clipboard. Used for long transcriptions where typing
//     character-by-character would be too slow.
//   * `simulate_typing(text)` — direct unicode keystroke injection via enigo.
//     Does NOT touch the clipboard. Eliminates the NSPasteboard read/restore
//     race that bites slow Electron targets (Slack, Notion, Mail) when we
//     restore the user's pre-record clipboard before the target has finished
//     reading our transcription. Used for short transcriptions (the common
//     case for voice-to-text).
//
// Both paths require Accessibility permission on macOS — CGEvent posting and
// enigo's macOS backend (CGEventKeyboardSetUnicodeString under the hood) both
// route through the same HID event tap and are gated by AX trust.

use std::thread;
use std::time::Duration;

/// Simulate a paste keystroke (Cmd+V on macOS, Ctrl+V on Windows).
///
/// Caller must have already written the desired text to the clipboard.
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

/// Type `text` directly into the focused application via synthetic keystrokes.
///
/// On macOS this goes through `CGEventKeyboardSetUnicodeString`, so:
///   * Any unicode codepoint is supported (accents, emoji, CJK).
///   * Keyboard layout is irrelevant (no key-code translation).
///   * The clipboard is never touched — no race with slow paste readers.
///
/// On Windows the same `enigo.text()` call uses `SendInput` with VK_PACKET.
pub fn simulate_typing(text: &str) -> Result<(), String> {
    use enigo::{Enigo, Keyboard, Settings};

    // Same focus-settle delay as simulate_paste so the first keystroke isn't
    // swallowed by an app that just gained focus (target app is the
    // foreground process at the moment our pill hands off).
    thread::sleep(Duration::from_millis(100));

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create Enigo: {}", e))?;

    enigo
        .text(text)
        .map_err(|e| format!("Failed to type text: {}", e))?;

    Ok(())
}
