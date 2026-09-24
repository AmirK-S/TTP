// TTP - Talk To Paste
// The dictation trigger: which key, modifier or mouse button starts a recording.
//
// Pure logic only — no CoreGraphics calls — so every rule here runs in a unit
// test. `fnkey.rs` owns the event tap and feeds raw event fields in; this
// module answers two questions about them:
//
//   * `held_change`: does this event press or release the configured trigger?
//   * `Capture::feed`: while the user is choosing a trigger ("press the key you
//     want"), which trigger did they just press?
//
// A trigger is one of four shapes, because macOS reports them four ways:
//
//   * `Fn`        the Globe key. A FlagsChanged event with keycode 63.
//   * `Modifier`  a modifier on its own, one side (right ⌘, left ⌥…). A
//                 FlagsChanged event whose keycode names the side; whether it
//                 went down or up is in the device-dependent flag bits.
//   * `Key`       an ordinary key, optionally with modifiers (F13, ⌥Space).
//                 KeyDown / KeyUp.
//   * `Mouse`     a mouse button other than left and right. OtherMouseDown /
//                 OtherMouseUp, with the button number in a field.

use serde::{Deserialize, Serialize};

// CGEventType values.
pub const EV_KEY_DOWN: u32 = 10;
pub const EV_KEY_UP: u32 = 11;
pub const EV_FLAGS_CHANGED: u32 = 12;
pub const EV_OTHER_MOUSE_DOWN: u32 = 25;
pub const EV_OTHER_MOUSE_UP: u32 = 26;
pub const EV_OTHER_MOUSE_DRAGGED: u32 = 27;

// CGEventFlags, device-independent.
pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;
pub const FLAG_FUNCTION: u64 = 0x0080_0000;
/// The modifiers a `Key` trigger may require. Function is excluded: arrow and
/// F-keys carry it on their own, so requiring or forbidding it would make
/// those keys unbindable.
pub const COMBO_MODIFIERS: u64 = FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND;

pub const KEY_ESCAPE: u16 = 53;
pub const KEY_CAPS_LOCK: u16 = 57;
pub const KEY_FN: u16 = 63;

/// The trigger as stored in `settings.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Trigger {
    Fn,
    Modifier { code: u16 },
    Key { code: u16, mods: u64 },
    Mouse { button: u8 },
}

impl Default for Trigger {
    fn default() -> Self {
        Trigger::Fn
    }
}

/// Device-dependent flag bit (`NX_DEVICE*KEYMASK`) that says one specific
/// modifier key is down. `None` for keycodes that are not a sided modifier.
pub fn modifier_side_bit(code: u16) -> Option<u64> {
    Some(match code {
        59 => 0x0000_0001, // left control
        56 => 0x0000_0002, // left shift
        60 => 0x0000_0004, // right shift
        55 => 0x0000_0008, // left command
        54 => 0x0000_0010, // right command
        58 => 0x0000_0020, // left option
        61 => 0x0000_0040, // right option
        62 => 0x0000_2000, // right control
        _ => return None,
    })
}

/// F1–F20 and the navigation keys: the keys that type nothing, and so can be
/// bound on their own without taking a character away from the user.
fn types_nothing(code: u16) -> bool {
    matches!(
        code,
        122 | 120 | 99 | 118 | 96 | 97 | 98 | 100 | 101 | 109 | 103 | 111 // F1–F12
        | 105 | 107 | 113 | 106 | 64 | 79 | 80 | 90 // F13–F20
        | 114 | 115 | 116 | 117 | 119 | 121 // help/insert, home, page up, forward delete, end, page down
        | 71 // keypad clear
    )
}

/// Why a pressed key cannot become the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// A letter, digit or symbol with no modifier: binding it would stop the
    /// user from typing that character anywhere.
    TypesACharacter,
    /// Caps Lock toggles a lock state rather than reporting press and release.
    CapsLock,
}

pub fn validate(trigger: &Trigger) -> Result<(), Rejection> {
    match *trigger {
        Trigger::Key { code, .. } if code == KEY_CAPS_LOCK => Err(Rejection::CapsLock),
        Trigger::Key { code, mods } if mods & COMBO_MODIFIERS == 0 && !types_nothing(code) => {
            Err(Rejection::TypesACharacter)
        }
        Trigger::Mouse { button } if button < 2 => Err(Rejection::TypesACharacter),
        _ => Ok(()),
    }
}

/// Raw fields of one tap event — everything the rules need, nothing else.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawEvent {
    pub event_type: u32,
    pub keycode: u16,
    pub flags: u64,
    pub autorepeat: bool,
    pub button: u8,
}

/// `Some(true)` when the event presses the trigger, `Some(false)` when it
/// releases it, `None` when it has nothing to do with it.
///
/// A `Key` release ignores modifiers on purpose: the user often lets go of ⌥
/// before Space, and a release that required ⌥ would leave the key latched.
pub fn held_change(trigger: &Trigger, ev: &RawEvent) -> Option<bool> {
    match *trigger {
        Trigger::Fn => (ev.event_type == EV_FLAGS_CHANGED && ev.keycode == KEY_FN)
            .then(|| ev.flags & FLAG_FUNCTION != 0),
        Trigger::Modifier { code } => {
            if ev.event_type != EV_FLAGS_CHANGED || ev.keycode != code {
                return None;
            }
            modifier_side_bit(code).map(|bit| ev.flags & bit != 0)
        }
        Trigger::Key { code, mods } => match ev.event_type {
            EV_KEY_DOWN if ev.keycode == code && !ev.autorepeat => {
                (ev.flags & COMBO_MODIFIERS == mods).then_some(true)
            }
            EV_KEY_UP if ev.keycode == code => Some(false),
            _ => None,
        },
        Trigger::Mouse { button } => match ev.event_type {
            EV_OTHER_MOUSE_DOWN if ev.button == button => Some(true),
            EV_OTHER_MOUSE_UP if ev.button == button => Some(false),
            _ => None,
        },
    }
}

/// Whether a modifier-only trigger, currently held, is being used as part of
/// a shortcut instead: any key pressed while it is down. Right ⌥ types `{`,
/// `|`, `~` on French layouts; right ⌘ is half of every ⌘-shortcut. Those
/// presses must cancel the dictation, not start one.
///
/// Fn is left out: its behaviour predates this module and has months of
/// traces behind it.
pub fn interrupts(trigger: &Trigger, ev: &RawEvent) -> bool {
    matches!(trigger, Trigger::Modifier { .. }) && ev.event_type == EV_KEY_DOWN
}

/// Whether the tap should keep this event from reaching the focused app.
///
/// Only keys and mouse buttons are swallowed. A modifier must still reach the
/// system — it is half of every shortcut the user types — and Fn keeps its
/// existing behaviour.
///
/// A key-up is swallowed only if its key-down was (`down_swallowed`). With ⌥Space
/// bound, a plain Space goes through; eating its key-up would leave the app
/// with a key that never came back up.
pub fn swallows(trigger: &Trigger, ev: &RawEvent, down_swallowed: bool) -> bool {
    match *trigger {
        Trigger::Key { code, mods } => match ev.event_type {
            EV_KEY_DOWN => ev.keycode == code && ev.flags & COMBO_MODIFIERS == mods,
            EV_KEY_UP => ev.keycode == code && down_swallowed,
            _ => false,
        },
        Trigger::Mouse { button } => {
            matches!(ev.event_type, EV_OTHER_MOUSE_DOWN | EV_OTHER_MOUSE_UP | EV_OTHER_MOUSE_DRAGGED)
                && ev.button == button
        }
        _ => false,
    }
}

/// Outcome of feeding one event to a capture in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStep {
    /// Keep listening.
    Pending,
    Captured(Trigger),
    Rejected(Rejection),
    /// Escape: the user backed out.
    Cancelled,
}

/// "Press the key you want." A modifier becomes the trigger only if it is
/// released without another key in between; otherwise it is part of a combo
/// and the combo is what gets captured.
#[derive(Debug, Clone, Copy, Default)]
pub struct Capture {
    pending_modifier: Option<u16>,
}

impl Capture {
    pub const fn new() -> Self {
        Self { pending_modifier: None }
    }

    pub fn feed(&mut self, ev: &RawEvent) -> CaptureStep {
        match ev.event_type {
            EV_FLAGS_CHANGED => {
                let down = if ev.keycode == KEY_FN {
                    Some(ev.flags & FLAG_FUNCTION != 0)
                } else {
                    modifier_side_bit(ev.keycode).map(|bit| ev.flags & bit != 0)
                };
                match down {
                    Some(true) => {
                        self.pending_modifier = Some(ev.keycode);
                        CaptureStep::Pending
                    }
                    Some(false) if self.pending_modifier == Some(ev.keycode) => {
                        self.pending_modifier = None;
                        CaptureStep::Captured(if ev.keycode == KEY_FN {
                            Trigger::Fn
                        } else {
                            Trigger::Modifier { code: ev.keycode }
                        })
                    }
                    _ => CaptureStep::Pending,
                }
            }
            EV_KEY_DOWN if ev.autorepeat => CaptureStep::Pending,
            EV_KEY_DOWN => {
                self.pending_modifier = None;
                if ev.keycode == KEY_ESCAPE && ev.flags & COMBO_MODIFIERS == 0 {
                    return CaptureStep::Cancelled;
                }
                let trigger = Trigger::Key { code: ev.keycode, mods: ev.flags & COMBO_MODIFIERS };
                match validate(&trigger) {
                    Ok(()) => CaptureStep::Captured(trigger),
                    Err(r) => CaptureStep::Rejected(r),
                }
            }
            EV_OTHER_MOUSE_DOWN if ev.button >= 2 => {
                self.pending_modifier = None;
                CaptureStep::Captured(Trigger::Mouse { button: ev.button })
            }
            _ => CaptureStep::Pending,
        }
    }
}

/// The trigger for a settings file written before triggers existed, from its
/// `shortcut` string. Only the three strings the old picker could write are
/// recognised; anything else was never reachable from the UI and becomes Fn.
pub fn from_legacy_shortcut(shortcut: &str) -> Trigger {
    match shortcut {
        "Alt+Space" => Trigger::Key { code: 49, mods: FLAG_OPTION },
        "CmdOrCtrl+Shift+R" => Trigger::Key { code: 15, mods: FLAG_COMMAND | FLAG_SHIFT },
        _ => Trigger::Fn,
    }
}

/// Packs a trigger into one `u64` so the tap callback can read it from an
/// atomic without taking a lock on every keystroke system-wide.
///
/// Layout: kind in the top byte, then a 16-bit code or button, then the
/// low 32 bits of the modifier mask (every combo modifier lives there).
pub fn encode(trigger: &Trigger) -> u64 {
    let (kind, code, mods) = match *trigger {
        Trigger::Fn => (1u64, 0u64, 0u64),
        Trigger::Modifier { code } => (2, code as u64, 0),
        Trigger::Key { code, mods } => (3, code as u64, mods & 0xFFFF_FFFF),
        Trigger::Mouse { button } => (4, button as u64, 0),
    };
    (kind << 56) | (code << 32) | mods
}

pub fn decode(packed: u64) -> Trigger {
    let code = ((packed >> 32) & 0xFFFF) as u16;
    match packed >> 56 {
        2 => Trigger::Modifier { code },
        3 => Trigger::Key { code, mods: packed & 0xFFFF_FFFF },
        4 => Trigger::Mouse { button: code as u8 },
        _ => Trigger::Fn,
    }
}

/// Short slug for trace lines. Never includes a typed character.
pub fn slug(trigger: &Trigger) -> String {
    match *trigger {
        Trigger::Fn => "fn".into(),
        Trigger::Modifier { code } => format!("modifier:{}", code),
        Trigger::Key { code, mods } => format!("key:{}+0x{:X}", code, mods),
        Trigger::Mouse { button } => format!("mouse:{}", button),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags_changed(keycode: u16, flags: u64) -> RawEvent {
        RawEvent { event_type: EV_FLAGS_CHANGED, keycode, flags, ..Default::default() }
    }
    fn key(event_type: u32, keycode: u16, flags: u64) -> RawEvent {
        RawEvent { event_type, keycode, flags, ..Default::default() }
    }
    fn mouse(event_type: u32, button: u8) -> RawEvent {
        RawEvent { event_type, button, ..Default::default() }
    }

    const RIGHT_CMD: u16 = 54;
    const RIGHT_OPT: u16 = 61;
    const SPACE: u16 = 49;
    const F13: u16 = 105;
    const A: u16 = 0;

    // ── held_change ─────────────────────────────────────────────────────

    #[test]
    fn fn_is_pressed_and_released_by_its_own_flags_changed() {
        let t = Trigger::Fn;
        assert_eq!(held_change(&t, &flags_changed(KEY_FN, FLAG_FUNCTION)), Some(true));
        assert_eq!(held_change(&t, &flags_changed(KEY_FN, 0)), Some(false));
        // An F-key bleeding the Function bit is not the Fn key.
        assert_eq!(held_change(&t, &key(EV_KEY_DOWN, 99, FLAG_FUNCTION)), None);
    }

    #[test]
    fn a_sided_modifier_reads_its_own_side_bit() {
        let t = Trigger::Modifier { code: RIGHT_CMD };
        assert_eq!(held_change(&t, &flags_changed(RIGHT_CMD, FLAG_COMMAND | 0x10)), Some(true));
        assert_eq!(held_change(&t, &flags_changed(RIGHT_CMD, 0)), Some(false));
    }

    #[test]
    fn the_other_side_of_the_same_modifier_does_not_count() {
        let t = Trigger::Modifier { code: RIGHT_CMD };
        // Left command down: same device-independent bit, different keycode.
        assert_eq!(held_change(&t, &flags_changed(55, FLAG_COMMAND | 0x08)), None);
        // Right command released while left is still held: the command bit is
        // still set but the right-side bit is not, so this is a release.
        assert_eq!(held_change(&t, &flags_changed(RIGHT_CMD, FLAG_COMMAND | 0x08)), Some(false));
    }

    #[test]
    fn a_key_combo_needs_its_exact_modifiers_to_press() {
        let t = Trigger::Key { code: SPACE, mods: FLAG_OPTION };
        assert_eq!(held_change(&t, &key(EV_KEY_DOWN, SPACE, FLAG_OPTION)), Some(true));
        assert_eq!(held_change(&t, &key(EV_KEY_DOWN, SPACE, 0)), None);
        assert_eq!(held_change(&t, &key(EV_KEY_DOWN, SPACE, FLAG_OPTION | FLAG_SHIFT)), None);
    }

    #[test]
    fn a_key_combo_releases_even_if_the_modifier_went_first() {
        let t = Trigger::Key { code: SPACE, mods: FLAG_OPTION };
        assert_eq!(held_change(&t, &key(EV_KEY_UP, SPACE, 0)), Some(false));
    }

    #[test]
    fn autorepeat_is_not_a_new_press() {
        let t = Trigger::Key { code: F13, mods: 0 };
        let mut ev = key(EV_KEY_DOWN, F13, 0);
        ev.autorepeat = true;
        assert_eq!(held_change(&t, &ev), None);
    }

    #[test]
    fn a_mouse_button_matches_only_its_number() {
        let t = Trigger::Mouse { button: 3 };
        assert_eq!(held_change(&t, &mouse(EV_OTHER_MOUSE_DOWN, 3)), Some(true));
        assert_eq!(held_change(&t, &mouse(EV_OTHER_MOUSE_UP, 3)), Some(false));
        assert_eq!(held_change(&t, &mouse(EV_OTHER_MOUSE_DOWN, 4)), None);
    }

    // ── interrupts / swallows ───────────────────────────────────────────

    #[test]
    fn typing_while_a_modifier_trigger_is_held_interrupts_it() {
        // Right ⌥ + 5 types `{` on a French layout.
        let t = Trigger::Modifier { code: RIGHT_OPT };
        assert!(interrupts(&t, &key(EV_KEY_DOWN, 23, FLAG_OPTION)));
        assert!(!interrupts(&Trigger::Key { code: F13, mods: 0 }, &key(EV_KEY_DOWN, A, 0)));
        assert!(!interrupts(&Trigger::Fn, &key(EV_KEY_DOWN, A, FLAG_FUNCTION)));
    }

    #[test]
    fn modifiers_and_fn_are_never_swallowed() {
        assert!(!swallows(&Trigger::Fn, &flags_changed(KEY_FN, FLAG_FUNCTION), false));
        let t = Trigger::Modifier { code: RIGHT_CMD };
        assert!(!swallows(&t, &flags_changed(RIGHT_CMD, FLAG_COMMAND | 0x10), false));
    }

    #[test]
    fn a_bound_key_is_swallowed_and_other_keys_are_not() {
        let t = Trigger::Key { code: SPACE, mods: FLAG_OPTION };
        assert!(swallows(&t, &key(EV_KEY_DOWN, SPACE, FLAG_OPTION), false));
        assert!(swallows(&t, &key(EV_KEY_UP, SPACE, 0), true));
        // Plain Space still types a space — down and up both go through.
        assert!(!swallows(&t, &key(EV_KEY_DOWN, SPACE, 0), false));
        assert!(!swallows(&t, &key(EV_KEY_UP, SPACE, 0), false));
        assert!(!swallows(&t, &key(EV_KEY_DOWN, A, FLAG_OPTION), false));
    }

    #[test]
    fn only_the_bound_mouse_button_is_swallowed() {
        // Handy #1758: an active tap that swallowed buttons 4/5 broke Back and
        // Forward system-wide for users who had not bound them.
        let t = Trigger::Mouse { button: 3 };
        assert!(swallows(&t, &mouse(EV_OTHER_MOUSE_DRAGGED, 3), false));
        assert!(!swallows(&t, &mouse(EV_OTHER_MOUSE_DOWN, 4), false));
        assert!(!swallows(&Trigger::Fn, &mouse(EV_OTHER_MOUSE_DOWN, 3), false));
    }

    // ── validate ────────────────────────────────────────────────────────

    #[test]
    fn a_bare_letter_is_refused_but_a_letter_with_a_modifier_is_not() {
        assert_eq!(validate(&Trigger::Key { code: A, mods: 0 }), Err(Rejection::TypesACharacter));
        assert_eq!(validate(&Trigger::Key { code: SPACE, mods: 0 }), Err(Rejection::TypesACharacter));
        assert_eq!(validate(&Trigger::Key { code: A, mods: FLAG_CONTROL }), Ok(()));
    }

    #[test]
    fn function_keys_can_be_bound_on_their_own() {
        assert_eq!(validate(&Trigger::Key { code: F13, mods: 0 }), Ok(()));
        assert_eq!(validate(&Trigger::Key { code: 96, mods: 0 }), Ok(())); // F5
    }

    #[test]
    fn caps_lock_is_refused_even_with_modifiers() {
        assert_eq!(validate(&Trigger::Key { code: KEY_CAPS_LOCK, mods: FLAG_SHIFT }), Err(Rejection::CapsLock));
    }

    // ── Capture ─────────────────────────────────────────────────────────

    #[test]
    fn a_modifier_pressed_and_released_alone_is_captured() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&flags_changed(RIGHT_CMD, FLAG_COMMAND | 0x10)), CaptureStep::Pending);
        assert_eq!(
            c.feed(&flags_changed(RIGHT_CMD, 0)),
            CaptureStep::Captured(Trigger::Modifier { code: RIGHT_CMD })
        );
    }

    #[test]
    fn fn_pressed_and_released_alone_is_captured_as_fn() {
        let mut c = Capture::default();
        c.feed(&flags_changed(KEY_FN, FLAG_FUNCTION));
        assert_eq!(c.feed(&flags_changed(KEY_FN, 0)), CaptureStep::Captured(Trigger::Fn));
    }

    #[test]
    fn a_modifier_held_through_a_key_captures_the_combo() {
        let mut c = Capture::default();
        c.feed(&flags_changed(58, FLAG_OPTION | 0x20));
        assert_eq!(
            c.feed(&key(EV_KEY_DOWN, SPACE, FLAG_OPTION)),
            CaptureStep::Captured(Trigger::Key { code: SPACE, mods: FLAG_OPTION })
        );
        // Releasing ⌥ afterwards must not replace the combo with "⌥ alone".
        assert_eq!(c.feed(&flags_changed(58, 0)), CaptureStep::Pending);
    }

    #[test]
    fn escape_cancels_and_a_bare_letter_is_rejected() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&key(EV_KEY_DOWN, KEY_ESCAPE, 0)), CaptureStep::Cancelled);
        assert_eq!(c.feed(&key(EV_KEY_DOWN, A, 0)), CaptureStep::Rejected(Rejection::TypesACharacter));
    }

    #[test]
    fn left_and_right_clicks_are_not_capturable() {
        let mut c = Capture::default();
        assert_eq!(c.feed(&mouse(EV_OTHER_MOUSE_DOWN, 1)), CaptureStep::Pending);
        assert_eq!(c.feed(&mouse(EV_OTHER_MOUSE_DOWN, 3)), CaptureStep::Captured(Trigger::Mouse { button: 3 }));
    }

    // ── storage ─────────────────────────────────────────────────────────

    #[test]
    fn every_trigger_survives_the_atomic_encoding() {
        for t in [
            Trigger::Fn,
            Trigger::Modifier { code: RIGHT_OPT },
            Trigger::Key { code: SPACE, mods: FLAG_OPTION | FLAG_COMMAND },
            Trigger::Mouse { button: 4 },
        ] {
            assert_eq!(decode(encode(&t)), t);
        }
    }

    #[test]
    fn the_json_shape_is_tagged_by_kind() {
        let json = serde_json::to_value(Trigger::Key { code: 49, mods: FLAG_OPTION }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "key", "code": 49, "mods": FLAG_OPTION }));
        let back: Trigger = serde_json::from_value(serde_json::json!({ "kind": "fn" })).unwrap();
        assert_eq!(back, Trigger::Fn);
    }

    #[test]
    fn the_old_picker_choices_migrate_to_the_same_keys() {
        assert_eq!(from_legacy_shortcut("FnKey"), Trigger::Fn);
        assert_eq!(from_legacy_shortcut("Alt+Space"), Trigger::Key { code: SPACE, mods: FLAG_OPTION });
        assert_eq!(
            from_legacy_shortcut("CmdOrCtrl+Shift+R"),
            Trigger::Key { code: 15, mods: FLAG_COMMAND | FLAG_SHIFT }
        );
    }
}
