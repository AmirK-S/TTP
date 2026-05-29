// TTP - Talk To Paste
// Fn key monitoring for macOS using NSTimer + NSEvent.modifierFlags
//
// Uses a 20ms NSTimer on the main run loop to poll [NSEvent modifierFlags].
// Detects the physical Fn/Globe key press/release for push-to-talk.
//
// Key filtering: arrow keys and F-keys also set NSEventModifierFlagFunction,
// so we filter them out:
//   - Arrow keys: set NumericPad flag (0x200000) alongside Function — rejected
//   - F-keys: set Function flag alone, but are short presses (<150ms) — filtered by debounce
//   - Physical Fn key: sets ONLY the Function flag, held for >150ms — accepted
//
// Debounce: Fn must be held for 150ms before recording starts,
// to ignore the system's quick Fn/Globe key tap (emoji picker, etc.)
// and to filter out brief F-key presses.

use crate::shortcuts::handle_shortcut_event_public;
use block::ConcreteBlock;
use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::ShortcutState;

extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

// CoreGraphics event tap — captures keys at the HID level, BEFORE macOS
// dispatches them to system overlays (Mission Control, Launchpad, DND).
// addGlobalMonitorForEventsMatchingMask does NOT receive these consumed
// keys, which is why F3/F4/F6 used to slip past the veto.
#[allow(non_camel_case_types)]
type CFMachPortRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CFRunLoopSourceRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CFRunLoopRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CFAllocatorRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CFStringRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CGEventRef = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type CGEventTapProxy = *mut std::ffi::c_void;

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut std::ffi::c_void,
) -> CGEventRef;

extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut std::ffi::c_void,
    ) -> CFMachPortRef;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CGEventTapEnable(tap: CFMachPortRef, enable: u8);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    static kCFRunLoopCommonModes: CFStringRef;
}

const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const KCG_TAIL_APPEND_EVENT_TAP: u32 = 1;
/// Active tap: callback may modify/delete events (return null to swallow).
/// Requires Accessibility trust.
const KCG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;
/// Passive tap: callback observes only, return value ignored. Needs only
/// Input Monitoring.
const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
/// macOS disabled our tap (slow callback / heavy input). We re-enable it.
const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
/// CGEventType for modifier-key changes (Fn, Shift, Cmd, Option, Ctrl).
/// macOS emits this with the dedicated keycode of the modifier being touched
/// — including keycode 63 for the physical Fn/Globe key. F1..F12, in
/// contrast, fire kCGEventKeyDown / KeyUp with their own keycodes (99 for
/// F3 etc.) and never fire FlagsChanged with keycode 63 — which is exactly
/// what lets us tell apart a real Fn press from an F-key "flag bleed".
const KCG_EVENT_FLAGS_CHANGED: u32 = 12;
const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
/// kVK_Function — virtual keycode of the physical Fn/Globe key. The single
/// source of truth for "is the Fn key actually held?". Independent of any
/// modifier flag inference.
const KVK_FUNCTION: u16 = 0x3F;

/// Whether Fn key is currently held (raw, before debounce)
static FN_KEY_DOWN: AtomicBool = AtomicBool::new(false);

/// Whether we've actually fired the "Pressed" event (after debounce)
static FN_RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Timestamp (ms since epoch) when Fn was first pressed (for debounce)
static FN_PRESS_TIME_MS: AtomicU64 = AtomicU64::new(0);

/// Timestamp (ms since epoch) of last Fn press for double-tap detection
static LAST_FN_PRESS_TIME_MS: AtomicU64 = AtomicU64::new(0);

/// Global app handle
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Whether Fn key monitoring is active
static FN_MONITORING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Timestamp (ms) of the most recent F-key (F1..F12) event (down or up).
/// The timer-based Fn detector rejects the Function modifier for
/// FKEY_VETO_WINDOW_MS after this stamp — covers the brief window where the
/// system overlay keeps the Function flag alive after the key was released.
static LAST_FKEY_PRESS_MS: AtomicU64 = AtomicU64::new(0);

/// True while a system F-key (F1..F12) is currently physically held down.
/// While this is true, the Fn detector rejects the Function modifier
/// unconditionally — no time window can be too short for a held key.
/// Cleared by the KEY_UP event (or by a stale-keepalive check, see below).
static FKEY_CURRENTLY_HELD: AtomicBool = AtomicBool::new(false);

/// True when the physical Fn/Globe key is currently held, as reported by
/// `kCGEventFlagsChanged` events with keycode `KVK_FUNCTION` (63). This is
/// the single source of truth for Fn detection — F1..F12 never emit
/// FlagsChanged with keycode 63, so the old modifier-flag inference race
/// (timer reads `NSEvent.modifierFlags` showing Function set because an
/// F-key bled the bit, the F-key veto callback hadn't run yet) is gone.
static FN_KEY_PHYSICALLY_DOWN: AtomicBool = AtomicBool::new(false);

/// True when the event tap was created in active (consuming) mode, so the
/// callback can return null to swallow the Fn FlagsChanged event and stop
/// macOS opening the Globe/emoji picker. False when we fell back to a passive
/// listen-only tap (Accessibility not granted) — Fn detection still works, but
/// the emoji picker is not suppressed (unchanged from before).
static FN_TAP_CONSUMES: AtomicBool = AtomicBool::new(false);

/// The CGEventTap mach port (pointer as usize), so the callback can re-enable
/// the tap if macOS disables it on timeout / heavy user input.
static FN_TAP_PORT: AtomicUsize = AtomicUsize::new(0);

/// True while the app is recording in hands-free / toggle mode. Set by
/// `shortcuts.rs` via [`set_hands_free_recording`]. When set, a single quick Fn
/// tap STOPS the recording (instead of being ignored as too-short / treated as
/// a double-tap candidate).
static HANDS_FREE_RECORDING: AtomicBool = AtomicBool::new(false);

/// Timestamp (ms) when the current hands-free recording started. A single tap
/// may only stop the recording after [`HANDS_FREE_STOP_GRACE_MS`] has elapsed —
/// this stops the *second* tap of the starting double-tap (and HID jitter right
/// after it) from instantly ending the recording it just began.
static HANDS_FREE_START_MS: AtomicU64 = AtomicU64::new(0);

/// Grace period after a hands-free recording starts before a single Fn tap is
/// allowed to stop it. Must comfortably exceed DOUBLE_TAP_THRESHOLD_MS so the
/// starting gesture can't self-cancel.
const HANDS_FREE_STOP_GRACE_MS: u64 = 400;

/// Double-tap detection threshold in milliseconds
const DOUBLE_TAP_THRESHOLD_MS: u64 = 300;

/// NSEventModifierFlagFunction = 1 << 23 = 0x800000
const NS_EVENT_MODIFIER_FLAG_FUNCTION: u64 = 0x800000;

/// NSEventModifierFlagNumericPad = 1 << 21 = 0x200000
/// Arrow keys set this alongside the Function flag.
const NS_EVENT_MODIFIER_FLAG_NUMERIC_PAD: u64 = 0x200000;

/// Mask for all "real" modifier keys (Shift, Ctrl, Option, Command)
/// If any of these are set alongside Function, it's likely a key combo, not bare Fn.
const NS_MODIFIER_KEY_MASK: u64 = 0x1E0000; // Shift|Ctrl|Option|Command

/// Debounce: Fn must be held for this long before recording starts (ms)
const FN_DEBOUNCE_MS: u64 = 150;

/// How long after the LAST F-key event (down OR up) to ignore Function-flag
/// events. macOS holds the Function modifier flag for the duration of system
/// overlays (Mission Control, Launchpad, brightness HUD, etc.) — sometimes
/// well past 250 ms — and at 250 ms the veto would expire while the flag was
/// still set, mis-firing recording. We extend to 1500 ms and also re-stamp on
/// key-up so a user tapping the F-key once gets the full window after release,
/// not after press.
const FKEY_VETO_WINDOW_MS: u64 = 1500;

/// Carbon kVK_F1..kVK_F12 keycodes — any of these arriving as keyDown means
/// the user pressed a system F-key, not the physical Fn/Globe key.
fn is_fkey_keycode(code: u16) -> bool {
    matches!(
        code,
        0x7A // F1
        | 0x78 // F2
        | 0x63 // F3
        | 0x76 // F4
        | 0x60 // F5
        | 0x61 // F6
        | 0x62 // F7
        | 0x64 // F8
        | 0x65 // F9
        | 0x6D // F10
        | 0x67 // F11
        | 0x6F // F12
    )
}

macro_rules! fnlog {
    ($($arg:tt)*) => {
        { let _ = writeln!(std::io::stderr(), $($arg)*); }
    };
}

const OBJC_YES: i8 = 1;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn has_input_monitoring() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}

pub fn request_input_monitoring() -> bool {
    unsafe { CGRequestListenEventAccess() }
}

/// Return whether the physical Fn/Globe key is currently held.
///
/// The signal comes from `kCGEventFlagsChanged` events with keycode 63
/// (`KVK_FUNCTION`), processed in `fkey_tap_callback`. F1..F12 generate
/// `kCGEventKeyDown` with their own keycodes — they never generate a
/// FlagsChanged event with keycode 63, so this state is immune to the bleed
/// of the Function modifier bit that F-keys would otherwise produce.
///
/// The `_flags` parameter is preserved for ABI compatibility with the timer
/// caller but is no longer used — `NSEvent.modifierFlags` was the ambiguous
/// source we just replaced.
fn is_physical_fn_key(_flags: u64) -> bool {
    FN_KEY_PHYSICALLY_DOWN.load(Ordering::Relaxed)
}

/// Start Fn key monitoring using NSTimer on the main run loop.
/// Must be called from the main thread (during app setup).
pub fn start_fn_key_monitor(app: &AppHandle) {
    let _ = APP_HANDLE.set(app.clone());
    FN_MONITORING_ACTIVE.store(true, Ordering::Relaxed);

    if !has_input_monitoring() {
        fnlog!("[FnKey] Input Monitoring permission not granted — requesting...");
        let granted = request_input_monitoring();
        if !granted {
            fnlog!("[FnKey] Input Monitoring denied — Fn key won't work");
        }
    }

    unsafe {
        let timer_block = ConcreteBlock::new(move |_timer: id| {
            if !FN_MONITORING_ACTIVE.load(Ordering::Relaxed) {
                return;
            }

            let flags: u64 = msg_send![class!(NSEvent), modifierFlags];
            let fn_held = is_physical_fn_key(flags);
            let was_held = FN_KEY_DOWN.load(Ordering::Relaxed);
            let recording_active = FN_RECORDING_ACTIVE.load(Ordering::Relaxed);

            if fn_held && !was_held {
                // Fn just pressed — note the time, but don't start recording yet
                let now = now_ms();
                FN_KEY_DOWN.store(true, Ordering::Relaxed);
                FN_PRESS_TIME_MS.store(now, Ordering::Relaxed);
                
                // Check for double-tap (within 300ms of last press)
                let last_press = LAST_FN_PRESS_TIME_MS.load(Ordering::Relaxed);
                let is_double_tap = last_press > 0 && (now - last_press) < DOUBLE_TAP_THRESHOLD_MS;
                
                if is_double_tap {
                    fnlog!("[FnKey] Fn key DOUBLE-TAP detected ({}ms since last press)", now - last_press);
                    // Reset the last press time to prevent triple-tap detection
                    LAST_FN_PRESS_TIME_MS.store(0, Ordering::Relaxed);
                    // Handle double-tap - toggle mode
                    if let Some(app) = APP_HANDLE.get() {
                        crate::shortcuts::handle_fn_double_tap(app);
                    }
                } else {
                    fnlog!("[FnKey] Fn key DOWN (flags=0x{:X}, debouncing {}ms...)", flags, FN_DEBOUNCE_MS);
                }
            } else if fn_held && was_held && !recording_active {
                // Fn still held — check if debounce period has passed
                let press_time = FN_PRESS_TIME_MS.load(Ordering::Relaxed);
                let elapsed = now_ms() - press_time;
                if elapsed >= FN_DEBOUNCE_MS {
                    // Debounce passed — start recording
                    FN_RECORDING_ACTIVE.store(true, Ordering::Relaxed);
                    fnlog!("[FnKey] Fn key HELD ({}ms, flags=0x{:X}) — starting recording", elapsed, flags);
                    if let Some(app) = APP_HANDLE.get() {
                        handle_shortcut_event_public(app, ShortcutState::Pressed);
                    }
                }
            } else if !fn_held && was_held {
                // Fn released (or another key now set NumericPad flag)
                FN_KEY_DOWN.store(false, Ordering::Relaxed);

                if recording_active {
                    // Was recording — stop it
                    FN_RECORDING_ACTIVE.store(false, Ordering::Relaxed);
                    fnlog!("[FnKey] Fn key UP (flags=0x{:X}) — stopping recording", flags);
                    if let Some(app) = APP_HANDLE.get() {
                        handle_shortcut_event_public(app, ShortcutState::Released);
                    }
                } else {
                    // Released before debounce — too short to start recording
                    // (system emoji tap, or first half of a double-tap).
                    //
                    // We still register this as a double-tap candidate. The
                    // previous lower bound of FN_DEBOUNCE_MS (150 ms) silently
                    // killed double-tap detection: a natural double-tap is
                    // ~50–100 ms per tap, so the first tap was always discarded
                    // and the second tap never saw a `LAST_FN_PRESS_TIME_MS`
                    // to compare against — `is_double_tap` could not become
                    // true on macOS even when the user did exactly what was
                    // supposed to trigger hands-free mode.
                    //
                    // 20 ms is enough to filter hardware/HID jitter (the timer
                    // itself polls at 20 ms) while accepting any deliberate
                    // tap. The 500 ms upper bound is moot in practice — at
                    // anything ≥150 ms the recording branch above fires first
                    // and we never reach this else — but kept defensively.
                    let press_time = FN_PRESS_TIME_MS.load(Ordering::Relaxed);
                    let elapsed = now_ms() - press_time;

                    // If a hands-free recording is in progress, a single quick
                    // tap STOPS it — what the user expects once hands-free is
                    // engaged. (Holding Fn >150ms already stops via the debounce
                    // branch above; this adds the quick-tap path.) The grace
                    // window prevents the second tap of the starting double-tap
                    // from instantly ending the recording it just began.
                    let started = HANDS_FREE_START_MS.load(Ordering::Relaxed);
                    let stop_allowed = HANDS_FREE_RECORDING.load(Ordering::Relaxed)
                        && started > 0
                        && now_ms().saturating_sub(started) > HANDS_FREE_STOP_GRACE_MS;
                    if stop_allowed {
                        fnlog!("[FnKey] Fn key UP ({}ms) — single tap stops hands-free recording", elapsed);
                        LAST_FN_PRESS_TIME_MS.store(0, Ordering::Relaxed);
                        if let Some(app) = APP_HANDLE.get() {
                            crate::shortcuts::handle_fn_stop(app);
                        }
                    } else {
                        if elapsed >= 20 && elapsed < 500 {
                            LAST_FN_PRESS_TIME_MS.store(press_time, Ordering::Relaxed);
                        }
                        fnlog!("[FnKey] Fn key UP ({}ms, flags=0x{:X}, ignored — too short)", elapsed, flags);
                    }
                }
            }
        });
        let timer_block = timer_block.copy();

        let _timer: id = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: 0.02f64
            repeats: OBJC_YES
            block: &*timer_block
        ];

        std::mem::forget(timer_block);
        fnlog!("[FnKey] Fn key monitor started (20ms poll, {}ms debounce, arrow key filter)", FN_DEBOUNCE_MS);

        // HID-level event tap: records the timestamp of any F-key press so
        // the timer above can veto the Function flag that the OS attaches to
        // them. We use CGEventTap at kCGHIDEventTap (not NSEvent's global
        // monitor) because macOS consumes F3/F4/F6 for Mission Control /
        // Launchpad / DND BEFORE they reach NSEvent global monitors. Without
        // this, those keys hold the Function flag for the duration of the
        // system overlay and falsely trigger Fn recording.
        // Subscribe to:
        //   - FlagsChanged (12) for the physical Fn key — primary signal,
        //     keycode 63 fires only when Fn itself is pressed/released
        //   - KeyDown (10) + KeyUp (11) for the defensive F-key belt-and-
        //     suspenders path (helpful only on non-Apple keyboards that
        //     don't emit FlagsChanged for Fn)
        let mask: u64 = (1u64 << KCG_EVENT_FLAGS_CHANGED)
            | (1u64 << KCG_EVENT_KEY_DOWN)
            | (1u64 << KCG_EVENT_KEY_UP);

        // To suppress the macOS Globe/Fn emoji picker we must CONSUME the Fn
        // FlagsChanged event (return null from the callback). Only an active,
        // head-inserted tap can delete events, and that requires Accessibility
        // trust. If Accessibility isn't granted we fall back to the passive
        // listen-only tap used before: Fn detection still works (Input
        // Monitoring is enough) but the emoji picker is not suppressed.
        //
        // The app's setup runs its stale-Accessibility reset+reprompt BEFORE
        // start_fn_key_monitor, so this read is reliable. The tap mode is fixed
        // at creation: a user who grants Accessibility later gets emoji
        // suppression on the next launch.
        let can_consume = crate::paste::check_accessibility();
        FN_TAP_CONSUMES.store(can_consume, Ordering::Relaxed);
        let (placement, option) = if can_consume {
            (KCG_HEAD_INSERT_EVENT_TAP, KCG_EVENT_TAP_OPTION_DEFAULT)
        } else {
            (KCG_TAIL_APPEND_EVENT_TAP, KCG_EVENT_TAP_OPTION_LISTEN_ONLY)
        };
        let tap = CGEventTapCreate(
            KCG_HID_EVENT_TAP,
            placement,
            option,
            mask,
            fkey_tap_callback,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            fnlog!("[FnKey] CGEventTapCreate returned null — F3/F4/F6 veto disabled (Input Monitoring permission missing?)");
        } else {
            FN_TAP_PORT.store(tap as usize, Ordering::Relaxed);
            let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, 1);
            fnlog!(
                "[FnKey] CGEventTap armed at HID level (consume_fn={}, FlagsChanged keycode 63 + F-key safety net)",
                can_consume
            );
        }
    }
}

unsafe extern "C" fn fkey_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> CGEventRef {
    // macOS disables an active tap if the callback is slow or under heavy
    // input. Re-enable it so Fn keeps working. Handle before the monitoring
    // gate — the tap must recover even while paused.
    if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT
        || event_type == KCG_EVENT_TAP_DISABLED_BY_USER_INPUT
    {
        let port = FN_TAP_PORT.load(Ordering::Relaxed) as CFMachPortRef;
        if !port.is_null() {
            CGEventTapEnable(port, 1);
        }
        return event;
    }

    if !FN_MONITORING_ACTIVE.load(Ordering::Relaxed) {
        return event;
    }

    let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u16;

    // Primary signal: FlagsChanged events with the dedicated Fn keycode.
    // This fires only when the *physical* Fn/Globe key changes state — not
    // when an F-key sets the Function bit as a side effect.
    if event_type == KCG_EVENT_FLAGS_CHANGED && keycode == KVK_FUNCTION {
        let flags = CGEventGetFlags(event);
        let fn_down = (flags & NS_EVENT_MODIFIER_FLAG_FUNCTION) != 0;
        FN_KEY_PHYSICALLY_DOWN.store(fn_down, Ordering::Relaxed);
        // Swallow the standalone Fn event so macOS doesn't pop the Globe/emoji
        // picker. We've already recorded the key state above, so our own
        // push-to-talk detection is unaffected. fn+key combos still work: the
        // Function flag rides on each combo key's own event, not on this
        // FlagsChanged notification. Only possible when the tap is active
        // (Accessibility granted); in listen-only mode the return is ignored.
        if FN_TAP_CONSUMES.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        return event;
    }

    // Defensive belt-and-suspenders: if the user's keyboard somehow doesn't
    // emit FlagsChanged for the Fn key (rare on non-Apple keyboards), keep
    // tracking F-key activity so the legacy F-key veto path can still act
    // as a safety net. On Apple keyboards this branch is dead code in
    // practice.
    if (event_type == KCG_EVENT_KEY_DOWN || event_type == KCG_EVENT_KEY_UP)
        && is_fkey_keycode(keycode)
    {
        LAST_FKEY_PRESS_MS.store(now_ms(), Ordering::Relaxed);
        if event_type == KCG_EVENT_KEY_DOWN {
            FKEY_CURRENTLY_HELD.store(true, Ordering::Relaxed);
        } else {
            FKEY_CURRENTLY_HELD.store(false, Ordering::Relaxed);
        }
    }

    event
}

pub fn set_fn_key_enabled(enabled: bool) {
    FN_MONITORING_ACTIVE.store(enabled, Ordering::Relaxed);
    if !enabled {
        // Reset state when disabling to avoid stuck state
        FN_KEY_DOWN.store(false, Ordering::Relaxed);
        FN_RECORDING_ACTIVE.store(false, Ordering::Relaxed);
        FN_PRESS_TIME_MS.store(0, Ordering::Relaxed);
        HANDS_FREE_RECORDING.store(false, Ordering::Relaxed);
        HANDS_FREE_START_MS.store(0, Ordering::Relaxed);
    }
    fnlog!("[FnKey] Fn key monitoring {}", if enabled { "enabled" } else { "disabled" });
}

/// Called by `shortcuts.rs` when a hands-free / toggle recording starts (`true`)
/// or ends (`false`). While active, a single quick Fn tap stops the recording.
/// Idempotent; safe to call from any thread.
pub fn set_hands_free_recording(active: bool) {
    HANDS_FREE_RECORDING.store(active, Ordering::Relaxed);
    if active {
        HANDS_FREE_START_MS.store(now_ms(), Ordering::Relaxed);
    }
}
