// TTP - Keyboard simulation
// Two strategies:
//   * `simulate_paste()` — Cmd+V (macOS) / Ctrl+V (Windows). Requires text to
//     already be on the clipboard. Used for long transcriptions where typing
//     character-by-character would be too slow.
//   * `simulate_typing(text)` — direct unicode keystroke injection.
//     Does NOT touch the clipboard. Eliminates the NSPasteboard read/restore
//     race that bites slow Electron targets (Slack, Notion, Mail) when we
//     restore the user's pre-record clipboard before the target has finished
//     reading our transcription. Used for short transcriptions (the common
//     case for voice-to-text).
//
// Both paths require Accessibility permission on macOS — CGEvent posting is
// gated by AX trust.
//
// ── Why macOS typing is hand-rolled instead of enigo ────────────────────────
//
// enigo's macOS backend builds its CGEventSource from
// `CGEventSourceStateID::CombinedSessionState` and never calls
// `CGEventSetFlags` on the events it posts (see enigo 0.2.1
// `macos_impl.rs::fast_text`). `CGEventCreateKeyboardEvent` seeds a new
// event's flags from the *current modifier state of that source* — i.e. from
// whatever modifier keys the window server believes are held right now.
//
// TTP's default hotkey is the physical Fn/Globe key. When the Fn key-up is
// missed (a swallowed flagsChanged, another app grabbing the event, an
// overlay latching the flag) the Globe bit stays set in the session state.
// Every character we then inject arrives carrying
// `kCGEventFlagMaskSecondaryFn`, so macOS routes it to the Globe shortcut
// layer instead of the text input system: Globe+Q opens a Quick Note,
// Globe+E the emoji picker, Globe+C Control Centre, Globe+N Notification
// Centre. The user sees "I dictated, nothing was written, and a new note
// opened" — and it keeps happening on every subsequent dictation until the
// stale flag clears, which is what made the failure look permanent and
// random.
//
// The fix is to own the event construction: build the source from
// `CGEventSourceStateID::Private` (a state that carries no hardware modifier
// history) and *additionally* set the flags explicitly on every event. Either
// one alone would do; both together mean no modifier the user happens to be
// holding — stuck or real — can ever be merged into our synthetic keystrokes.

use std::thread;
use std::time::Duration;

/// Focus-settle delay before the first synthetic event, so the keystroke
/// isn't swallowed by an app that has just regained foreground.
const FOCUS_SETTLE_MS: u64 = 100;

#[cfg(target_os = "macos")]
mod mac {
    use std::thread;
    use std::time::{Duration, Instant};

    /// `CGEventSourceStateID::CombinedSessionState` — the state whose modifier
    /// flags get merged into synthetic events, and therefore the one we have to
    /// inspect to know whether an injection is about to be contaminated.
    pub const COMBINED_SESSION_STATE: i32 = 0;

    /// Modifier bits that turn an injected keystroke into a system shortcut
    /// rather than text.
    ///
    /// CapsLock (`0x00010000`) and NumericPad (`0x00200000`) are deliberately
    /// excluded: they are steady states, not "the user is holding a key down",
    /// and neither changes how a unicode-string event is interpreted. Waiting
    /// on CapsLock would stall every injection for a user who leaves it on.
    pub const HOSTILE_MODIFIERS: u64 = 0x0002_0000   // Shift
        | 0x0004_0000                                // Control
        | 0x0008_0000                                // Alternate / Option
        | 0x0010_0000                                // Command
        | 0x0080_0000; // SecondaryFn — the Globe/Fn key, TTP's own hotkey

    /// How long to wait for a held modifier to clear before injecting anyway.
    ///
    /// Short on purpose. This is a courtesy pause for the real case (the user
    /// is still physically holding Fn as we start typing), not a fix for the
    /// stuck-flag case — the explicit `set_flags` below already handles that,
    /// and a stuck flag never clears on its own, so a long wait would only add
    /// latency to every dictation.
    pub const MODIFIER_SETTLE_MAX_MS: u64 = 250;

    /// Poll interval while waiting for modifiers to clear.
    const MODIFIER_POLL_MS: u64 = 10;

    /// Max UTF-16 code units per synthetic event.
    ///
    /// `CGEventKeyboardSetUnicodeString` truncates long strings
    /// (enigo-rs#68); 20 chars is the commonly cited safe bound. We budget in
    /// UTF-16 units rather than chars so an emoji-heavy transcription (2 units
    /// per codepoint) can't silently blow past it, and leave headroom for the
    /// zero-width-space prefix below.
    const CHUNK_UTF16_UNITS: usize = 16;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        /// Current modifier flags for a given event-source state. Not exposed
        /// by the `core-graphics` crate, so declared here.
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
    }

    /// Read the modifier bits currently held in the combined session state,
    /// masked to the ones that would corrupt an injection.
    pub fn held_modifiers() -> u64 {
        unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) & HOSTILE_MODIFIERS }
    }

    /// Wait (bounded) for held modifiers to clear. Returns whatever is still
    /// held when we give up — zero means the coast is clear.
    pub fn wait_for_modifiers_release(max_wait: Duration) -> u64 {
        let deadline = Instant::now() + max_wait;
        loop {
            let held = held_modifiers();
            if held == 0 {
                return 0;
            }
            if Instant::now() >= deadline {
                return held;
            }
            thread::sleep(Duration::from_millis(MODIFIER_POLL_MS));
        }
    }

    /// Human-readable modifier list for the log line.
    pub fn describe_modifiers(bits: u64) -> String {
        let mut names = Vec::new();
        if bits & 0x0002_0000 != 0 {
            names.push("Shift");
        }
        if bits & 0x0004_0000 != 0 {
            names.push("Control");
        }
        if bits & 0x0008_0000 != 0 {
            names.push("Option");
        }
        if bits & 0x0010_0000 != 0 {
            names.push("Command");
        }
        if bits & 0x0080_0000 != 0 {
            names.push("Fn/Globe");
        }
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join("+")
        }
    }

    /// Split `text` into pieces small enough for one
    /// `CGEventKeyboardSetUnicodeString` call, never splitting a codepoint.
    ///
    /// A chunk that *starts* with `\n`, `\r` or `\t` is silently dropped in
    /// its entirety by `CGEventKeyboardSetUnicodeString` (enigo-rs#260), so we
    /// prefix those with a zero-width space. Emitting a real Return/Tab
    /// keycode instead would be visually cleaner but changes behaviour: Return
    /// sends the message in Slack and iMessage, and Tab moves focus out of the
    /// field. A ZWSP keeps the newline a literal character, which is what the
    /// user dictated.
    pub fn injection_chunks(text: &str, max_units: usize) -> Vec<String> {
        let mut chunks: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut units = 0usize;

        for ch in text.chars() {
            let width = ch.len_utf16();
            if units + width > max_units && !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                units = 0;
            }
            current.push(ch);
            units += width;
        }
        if !current.is_empty() {
            chunks.push(current);
        }

        for chunk in chunks.iter_mut() {
            if chunk.starts_with(|c| matches!(c, '\n' | '\r' | '\t')) {
                chunk.insert(0, '\u{200B}');
            }
        }
        chunks
    }

    pub fn chunk_budget() -> usize {
        CHUNK_UTF16_UNITS
    }
}

/// Pause before injecting: let the target app settle into the foreground, then
/// give any physically-held modifier a brief chance to come up.
///
/// Returns the modifier bits still held when we proceed (macOS only; always 0
/// elsewhere). Injection goes ahead regardless — the callers below pin the
/// flags on every event they post, so a stuck modifier can no longer corrupt
/// the output. We log it because it is the single most useful breadcrumb when
/// a user reports "I dictated and nothing appeared".
fn settle_before_injection() -> u64 {
    thread::sleep(Duration::from_millis(FOCUS_SETTLE_MS));

    #[cfg(target_os = "macos")]
    let stuck = {
        let stuck = mac::wait_for_modifiers_release(Duration::from_millis(
            mac::MODIFIER_SETTLE_MAX_MS,
        ));
        if stuck != 0 {
            // WARN, not INFO: release builds filter at Warn, and this is
            // exactly the state we need to see in a user-submitted log.
            crate::logging::log_warn(&format!(
                "[Paste] Modifiers still held at injection time ({} / 0x{:06X}) — \
                 posting with flags pinned to null anyway",
                mac::describe_modifiers(stuck),
                stuck
            ));
            sentry::add_breadcrumb(sentry::Breadcrumb {
                category: Some("paste".to_string()),
                message: Some(format!(
                    "stuck modifiers at injection: {}",
                    mac::describe_modifiers(stuck)
                )),
                level: sentry::Level::Warning,
                ..Default::default()
            });
            // Also into the dictation trace, so it sits chronologically next
            // to the `paste.verify` line that will show whether this dictation
            // actually landed.
            crate::trace::event(
                "paste.modifiers",
                serde_json::json!({
                    "held": mac::describe_modifiers(stuck),
                    "bits": format!("0x{:06X}", stuck),
                }),
            );
        }
        stuck
    };

    #[cfg(not(target_os = "macos"))]
    let stuck: u64 = 0;

    stuck
}

/// Simulate a paste keystroke (Cmd+V on macOS, Ctrl+V on Windows).
///
/// Caller must have already written the desired text to the clipboard.
pub fn simulate_paste() -> Result<(), String> {
    let _stuck = settle_before_injection();

    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        // Key code for 'v' on macOS
        const KEY_V: CGKeyCode = 9;

        // Private state: carries no hardware modifier history, so nothing the
        // user is holding leaks into the event we are about to build.
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "Failed to create event source")?;

        // Command and nothing else. Setting flags explicitly (rather than
        // relying on the source being clean) is what guarantees a stuck Fn
        // can't turn Cmd+V into Globe+Cmd+V.
        let flags = CGEventFlags::CGEventFlagCommand;

        let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
            .map_err(|_| "Failed to create key down event")?;
        key_down.set_flags(flags);

        let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
            .map_err(|_| "Failed to create key up event")?;
        key_up.set_flags(flags);

        // Post the events to the annotated session (current user session)
        key_down.post(CGEventTapLocation::AnnotatedSession);
        thread::sleep(Duration::from_millis(10));
        key_up.post(CGEventTapLocation::AnnotatedSession);
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{
            Direction::{Click, Press, Release},
            Enigo, Key, Keyboard, Settings,
        };

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
///   * Modifier flags are pinned to null on every event, so no held or stuck
///     modifier can reroute the characters into a system shortcut.
///
/// On Windows this uses `enigo.text()` — `SendInput` with VK_PACKET, which
/// carries no modifier state to begin with.
pub fn simulate_typing(text: &str) -> Result<(), String> {
    let _stuck = settle_before_injection();

    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "Failed to create event source")?;

        for chunk in mac::injection_chunks(text, mac::chunk_budget()) {
            // keycode 0 + a unicode string is the layout-independent "insert
            // this text" event; the keycode itself is ignored by the target.
            let event = CGEvent::new_keyboard_event(source.clone(), 0, true)
                .map_err(|_| "Failed to create keyboard event")?;
            event.set_flags(CGEventFlags::CGEventFlagNull);
            event.set_string(&chunk);
            event.post(CGEventTapLocation::HID);
        }

        // Matches enigo's trailing pause — gives the target's input queue a
        // moment to drain before the caller restores the clipboard.
        thread::sleep(Duration::from_millis(2));
    }

    #[cfg(not(target_os = "macos"))]
    {
        use enigo::{Enigo, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to create Enigo: {}", e))?;

        enigo
            .text(text)
            .map_err(|e| format!("Failed to type text: {}", e))?;
    }

    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::mac::{describe_modifiers, injection_chunks, HOSTILE_MODIFIERS};

    fn utf16_len(s: &str) -> usize {
        s.encode_utf16().count()
    }

    #[test]
    fn short_text_is_a_single_chunk() {
        let chunks = injection_chunks("bonjour", 16);
        assert_eq!(chunks, vec!["bonjour".to_string()]);
    }

    #[test]
    fn empty_text_produces_no_events() {
        assert!(injection_chunks("", 16).is_empty());
    }

    #[test]
    fn chunks_never_exceed_the_utf16_budget() {
        let text = "a".repeat(100);
        for chunk in injection_chunks(&text, 16) {
            assert!(utf16_len(&chunk) <= 16);
        }
    }

    #[test]
    fn chunking_is_lossless_for_plain_text() {
        let text = "Le renard brun rapide saute par-dessus le chien paresseux.";
        let joined: String = injection_chunks(text, 16).concat();
        assert_eq!(joined, text);
    }

    #[test]
    fn surrogate_pairs_are_never_split() {
        // Each emoji is 2 UTF-16 units; a naive char-count budget would let a
        // chunk reach 32 units and get truncated by CoreGraphics.
        let text = "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉";
        let chunks = injection_chunks(text, 16);
        for chunk in &chunks {
            assert!(utf16_len(chunk) <= 16, "chunk {:?} too wide", chunk);
            // A split surrogate pair would not round-trip through char().
            assert!(chunk.chars().all(|c| c == '🎉'));
        }
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn accents_survive_chunking() {
        let text = "éàçûö ñ ÿ";
        assert_eq!(injection_chunks(text, 4).concat(), text);
    }

    #[test]
    fn leading_newline_gets_a_zero_width_space() {
        // Without the prefix CoreGraphics drops the whole chunk.
        let chunks = injection_chunks("\nligne", 16);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].starts_with('\u{200B}'));
        assert_eq!(chunks[0], "\u{200B}\nligne");
    }

    #[test]
    fn leading_tab_and_carriage_return_get_the_same_treatment() {
        assert!(injection_chunks("\tindent", 16)[0].starts_with('\u{200B}'));
        assert!(injection_chunks("\rretour", 16)[0].starts_with('\u{200B}'));
    }

    #[test]
    fn interior_newline_is_left_alone() {
        let chunks = injection_chunks("ab\ncd", 16);
        assert_eq!(chunks, vec!["ab\ncd".to_string()]);
    }

    #[test]
    fn newline_landing_at_a_chunk_boundary_is_prefixed() {
        // Budget of 2 forces "\n" to open the second chunk.
        let chunks = injection_chunks("ab\ncd", 2);
        assert!(chunks.iter().any(|c| c.starts_with('\u{200B}')));
    }

    #[test]
    fn fn_globe_is_part_of_the_hostile_mask() {
        // The regression this module exists for: Globe+letter becomes a
        // system shortcut instead of text.
        assert_ne!(HOSTILE_MODIFIERS & 0x0080_0000, 0);
    }

    #[test]
    fn capslock_is_not_treated_as_hostile() {
        // CapsLock is a steady state — waiting on it would stall every
        // dictation for users who leave it on.
        assert_eq!(HOSTILE_MODIFIERS & 0x0001_0000, 0);
    }

    #[test]
    fn describe_modifiers_names_the_globe_key() {
        assert_eq!(describe_modifiers(0x0080_0000), "Fn/Globe");
        assert_eq!(describe_modifiers(0x0010_0000 | 0x0002_0000), "Shift+Command");
        assert_eq!(describe_modifiers(0), "none");
    }
}
