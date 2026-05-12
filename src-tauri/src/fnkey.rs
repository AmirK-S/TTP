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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    static kCFRunLoopCommonModes: CFStringRef;
}

const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_TAIL_APPEND_EVENT_TAP: u32 = 1;
const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

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

/// State of the Function flag at the previous timer tick. Combined with
/// FN_FLAG_EPISODE_REJECTED, this lets us tell apart a NEW press (flag just
/// went false→true) from a continuous hold (flag was already true).
/// Critical because macOS keeps the Function flag set for the entire duration
/// a system overlay is up (Mission Control, Launchpad, brightness HUD…),
/// which can be 3+ seconds — longer than any sane fixed veto window. Once we
/// reject the initial transition, we lock the rejection in until the flag
/// clears, regardless of how long the overlay stays on screen.
static LAST_FN_FLAG_OBSERVED: AtomicBool = AtomicBool::new(false);

/// True if the current "Function flag is set" episode was rejected at its
/// onset (because an F-key was held or recently pressed). Stays true for the
/// whole episode, even after FKEY_CURRENTLY_HELD becomes false and the
/// fixed-time veto expires. Cleared only when the Function flag itself goes
/// back to unset, signalling the user has actually let go of everything.
static FN_FLAG_EPISODE_REJECTED: AtomicBool = AtomicBool::new(false);

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

/// Check if the current modifier flags indicate the physical Fn key is held
/// (as opposed to arrow keys or F-keys which also set the Function flag).
///
/// State-machine approach: we don't just look at the flag in isolation — we
/// look at the *transition*. The Function flag stays continuously set for the
/// whole duration of a system overlay (Mission Control etc.), often well
/// beyond any sane veto window. Once we decide the current "flag set" episode
/// belongs to an F-key, we lock that decision in for the whole episode and
/// only re-evaluate when the flag goes back to unset.
fn is_physical_fn_key(flags: u64) -> bool {
    let fn_set = (flags & NS_EVENT_MODIFIER_FLAG_FUNCTION) != 0;

    if !fn_set {
        // Flag is off — reset episode state so the next set transition is
        // evaluated cleanly. Don't touch FKEY_CURRENTLY_HELD or LAST_FKEY_*,
        // those are owned by the CGEventTap callback.
        LAST_FN_FLAG_OBSERVED.store(false, Ordering::Relaxed);
        FN_FLAG_EPISODE_REJECTED.store(false, Ordering::Relaxed);
        return false;
    }

    // Arrow keys set NumericPad (0x200000) alongside Function — reject
    if (flags & NS_EVENT_MODIFIER_FLAG_NUMERIC_PAD) != 0 {
        return false;
    }

    let was_set = LAST_FN_FLAG_OBSERVED.swap(true, Ordering::Relaxed);

    if was_set {
        // Continuous "flag is set" episode — keep the decision we made at
        // the onset. If we rejected it then (F-key was active), keep
        // rejecting; otherwise accept.
        return !FN_FLAG_EPISODE_REJECTED.load(Ordering::Relaxed);
    }

    // New transition: flag just went unset → set. Evaluate ONCE who the
    // Function bit belongs to, then freeze that decision for the rest of
    // the episode.
    let fkey_held = FKEY_CURRENTLY_HELD.load(Ordering::Relaxed);
    let last_fkey = LAST_FKEY_PRESS_MS.load(Ordering::Relaxed);
    let fkey_recent = last_fkey > 0
        && now_ms().saturating_sub(last_fkey) < FKEY_VETO_WINDOW_MS;

    if fkey_held || fkey_recent {
        FN_FLAG_EPISODE_REJECTED.store(true, Ordering::Relaxed);
        return false;
    }

    FN_FLAG_EPISODE_REJECTED.store(false, Ordering::Relaxed);

    // If other modifier keys (Shift/Ctrl/Option/Command) are held with Fn,
    // still accept — user might hold Fn+Shift intentionally, and the Fn
    // key is still physically held.
    true
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
                    // Released before debounce — ignore (system emoji tap)
                    // But track this as a potential double-tap candidate
                    let press_time = FN_PRESS_TIME_MS.load(Ordering::Relaxed);
                    let elapsed = now_ms() - press_time;
                    if elapsed >= FN_DEBOUNCE_MS && elapsed < 500 {
                        // Valid press (not too short, not too long) — track for double-tap
                        LAST_FN_PRESS_TIME_MS.store(press_time, Ordering::Relaxed);
                    }
                    fnlog!("[FnKey] Fn key UP ({}ms, flags=0x{:X}, ignored — too short)", elapsed, flags);
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
        // Listen for both down AND up — the callback re-stamps LAST_FKEY_PRESS_MS
        // on either, so the veto stays armed for the full release window.
        let mask: u64 = (1u64 << KCG_EVENT_KEY_DOWN) | (1u64 << KCG_EVENT_KEY_UP);
        let tap = CGEventTapCreate(
            KCG_HID_EVENT_TAP,
            KCG_TAIL_APPEND_EVENT_TAP,
            KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
            mask,
            fkey_tap_callback,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            fnlog!("[FnKey] CGEventTapCreate returned null — F3/F4/F6 veto disabled (Input Monitoring permission missing?)");
        } else {
            let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, 1);
            fnlog!("[FnKey] CGEventTap armed at HID level ({}ms window, catches system-consumed F-keys)", FKEY_VETO_WINDOW_MS);
        }
    }
}

unsafe extern "C" fn fkey_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> CGEventRef {
    if FN_MONITORING_ACTIVE.load(Ordering::Relaxed) {
        let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u16;
        if is_fkey_keycode(keycode) {
            // Re-stamp on every event (down, up, auto-repeat) so the release
            // window starts from the most recent activity.
            LAST_FKEY_PRESS_MS.store(now_ms(), Ordering::Relaxed);

            if event_type == KCG_EVENT_KEY_DOWN {
                FKEY_CURRENTLY_HELD.store(true, Ordering::Relaxed);
            } else if event_type == KCG_EVENT_KEY_UP {
                FKEY_CURRENTLY_HELD.store(false, Ordering::Relaxed);
            }
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
    }
    fnlog!("[FnKey] Fn key monitoring {}", if enabled { "enabled" } else { "disabled" });
}
