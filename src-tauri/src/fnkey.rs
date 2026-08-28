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

use crate::fnkey_fsm::{
    fn_decide, fn_stale_check, FnAction, FnFsmState, FN_DEBOUNCE_MS, FN_STALE_RESYNC_TICKS,
};
// DOUBLE_TAP_THRESHOLD_MS / HANDS_FREE_STOP_GRACE_MS are referenced via the
// FSM module's internal logic; we don't need them here. FN_DEBOUNCE_MS is
// still used by the startup diagnostic log so the operator can read the
// active value at a glance.
use crate::shortcuts::handle_shortcut_event_public;
use block::ConcreteBlock;
use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
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
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRelease(cf: *const std::ffi::c_void);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    static kCFRunLoopCommonModes: CFStringRef;
}

const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_TAIL_APPEND_EVENT_TAP: u32 = 1;
const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
/// CGEventType for modifier-key changes (Fn, Shift, Cmd, Option, Ctrl).
/// macOS emits this with the dedicated keycode of the modifier being touched
/// — including keycode 63 for the physical Fn/Globe key. F1..F12, in
/// contrast, fire kCGEventKeyDown / KeyUp with their own keycodes (99 for
/// F3 etc.) and never fire FlagsChanged with keycode 63 — which is exactly
/// what lets us tell apart a real Fn press from an F-key "flag bleed".
const KCG_EVENT_FLAGS_CHANGED: u32 = 12;
/// kCGEventTapDisabledByTimeout — the window server unhooked our tap because
/// a callback took too long to return. Until the tap is re-armed it delivers
/// NOTHING, which means `FN_KEY_PHYSICALLY_DOWN` freezes at whatever it was:
/// stuck false → the Fn key silently stops starting recordings; stuck true →
/// the FSM believes Fn is held forever AND the session keeps the Globe
/// modifier set, so injected characters get routed to the Globe shortcut
/// layer instead of the text field. Both look to the user like "TTP just
/// stopped working", which is why this is handled rather than ignored.
const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
/// kCGEventTapDisabledByUserInput — same consequence, different trigger.
const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
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

/// True while the app is recording in hands-free / toggle mode. Set by
/// `shortcuts.rs` via [`set_hands_free_recording`]. When set, a single quick Fn
/// tap STOPS the recording (instead of being ignored as too-short / treated as
/// a double-tap candidate).
static HANDS_FREE_RECORDING: AtomicBool = AtomicBool::new(false);

/// The live event-tap port, kept so the tap can be re-armed after macOS
/// disables it. Previously the `CFMachPortRef` was a local that went out of
/// scope at the end of `start_fn_key_monitor`, which made recovery
/// impossible: nothing in the process could name the tap any more.
static TAP_PORT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Consecutive timer ticks on which the tap claims Fn is held while
/// `NSEvent.modifierFlags` says it is not. See `FN_STALE_RESYNC_TICKS`.
static FN_STALE_TICKS: AtomicU64 = AtomicU64::new(0);

/// Timer ticks counted since launch, used to pace the tap watchdog.
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Wall-clock time of the last re-arm message we wrote to the log.
static LAST_REARM_LOG_MS: AtomicU64 = AtomicU64::new(0);

/// The run loop source feeding the tap, kept so a dead tap can be fully torn
/// down rather than merely disabled.
static TAP_SOURCE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Consecutive re-arms without the tap being observed healthy in between.
static REARM_STREAK: AtomicU64 = AtomicU64::new(0);

/// Rebuilds attempted before we accept that this process cannot hold a tap.
///
/// Evidence from 2026-08-28: one session rebuilt the tap 444 times over 90
/// minutes, every 12 seconds, and never recovered. It ended only when the app
/// was relaunched. So a tap that will not stay enabled is not a property of
/// the tap object — recreating it inside the same process does not help —
/// it is a property of the process, almost certainly its Input Monitoring
/// grant being evaluated once at launch.
///
/// Which means unbounded escalation is not persistence, it is 444 pointless
/// teardown/create cycles and 2600 lines of noise in the file the user is
/// keeping in order to find real failures. Three attempts, then stop and say
/// so — the only remedy is a restart, and only the user can do that.
const TAP_REBUILD_MAX_ATTEMPTS: u64 = 3;

/// Rebuilds attempted since the tap was last seen healthy.
static REBUILD_ATTEMPTS: AtomicU64 = AtomicU64::new(0);

/// Set once we have given up on this process's tap, so the watchdog stops
/// touching it and stops logging about it.
static TAP_ABANDONED: AtomicBool = AtomicBool::new(false);

/// Failed re-arms before we stop re-enabling and rebuild the tap outright.
///
/// `CGEventTapEnable` on a tap the window server has given up on is a no-op,
/// so the watchdog can "recover" a dead tap every two seconds forever while
/// the Fn key stays dead. Observed on 2026-08-28: a session re-armed 37 times
/// across 73 seconds, never recovered, and recorded not one key press. The
/// trigger is visible one line earlier — `timer_stall {"gap_ms":2085}` during
/// Tauri's startup, long enough for macOS to time the tap out before the app
/// had finished launching.
///
/// 5 attempts is ~10s at the watchdog interval: long enough that a tap merely
/// wedged by a transient stall gets its chance to come back, short enough
/// that the user is not left without a hotkey.
const TAP_REBUILD_AFTER_FAILED_REARMS: u64 = 5;

/// Minimum gap between two re-arm log lines while the tap keeps flapping.
///
/// A tap that macOS refuses to keep enabled — which is what happens while
/// Input Monitoring is being granted — is re-armed on every watchdog pass.
/// Logging each attempt buried the interesting first occurrence under a
/// dozen identical lines. We log the first, then at most one line per
/// interval, carrying the streak count so the flapping is still visible.
const REARM_LOG_INTERVAL_MS: u64 = 30_000;

/// Wall-clock time of the previous timer tick, for stall detection.
static LAST_TICK_MS: AtomicU64 = AtomicU64::new(0);

/// A 20ms timer that goes quiet for at least this long was not idle — the
/// process was descheduled.
///
/// macOS App Nap suspends `LSUIElement` background agents aggressively, and a
/// napped TTP stops polling the Fn key, stops advancing the recording state
/// machine, and leaves an in-flight dictation parked mid-pipeline. From the
/// user's seat that is indistinguishable from a crash. There is no API that
/// reports "you were napped", so the only way to observe it is to notice that
/// our own clock skipped.
const TIMER_STALL_THRESHOLD_MS: u64 = 1_000;

/// How often the timer verifies the tap is still armed (~2s at 20ms/tick).
/// Belt-and-braces for the case where the disable notification itself is
/// never delivered — the failure mode is total silence, so we cannot rely on
/// being told about it.
const TAP_WATCHDOG_TICKS: u64 = 100;

/// Timestamp (ms) when the current hands-free recording started. A single tap
/// may only stop the recording after [`HANDS_FREE_STOP_GRACE_MS`] has elapsed —
/// this stops the *second* tap of the starting double-tap (and HID jitter right
/// after it) from instantly ending the recording it just began.
static HANDS_FREE_START_MS: AtomicU64 = AtomicU64::new(0);

// Timing constants moved to `fnkey_fsm` so the pure FSM owns them. We keep
// only the macOS-platform-specific constants below.
//
// HANDS_FREE_STOP_GRACE_MS, DOUBLE_TAP_THRESHOLD_MS, FN_DEBOUNCE_MS:
//   see `fnkey_fsm` — re-exported via the `use` block above.

/// NSEventModifierFlagFunction = 1 << 23 = 0x800000
const NS_EVENT_MODIFIER_FLAG_FUNCTION: u64 = 0x800000;

/// NSEventModifierFlagNumericPad = 1 << 21 = 0x200000
/// Arrow keys set this alongside the Function flag.
const NS_EVENT_MODIFIER_FLAG_NUMERIC_PAD: u64 = 0x200000;

/// Mask for all "real" modifier keys (Shift, Ctrl, Option, Command)
/// If any of these are set alongside Function, it's likely a key combo, not bare Fn.
const NS_MODIFIER_KEY_MASK: u64 = 0x1E0000; // Shift|Ctrl|Option|Command

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

/// True once this process has given up on keeping an event tap alive.
///
/// Surfaced to the tray because the user-visible consequence is identical to
/// Input Monitoring being missing — the Fn key does nothing — and the tray
/// already knows how to show that. The remedy differs (restart rather than
/// grant a permission) but the signal that something is wrong should not wait
/// for the user to go reading a log.
pub fn tap_abandoned() -> bool {
    TAP_ABANDONED.load(Ordering::Relaxed)
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

/// Create the HID event tap, wire it into the current run loop, and enable it.
///
/// Must run on the thread whose run loop will service the callback — the main
/// thread, both at startup and from the watchdog inside the NSTimer.
///
/// We tap at `kCGHIDEventTap` rather than using NSEvent's global monitor
/// because macOS consumes F3/F4/F6 for Mission Control / Launchpad / DND
/// before they reach global monitors, and those keys hold the Function flag
/// for the duration of the system overlay — which used to trigger false
/// recordings. Subscribed events:
///   * FlagsChanged (12) for the physical Fn key — the primary signal;
///     keycode 63 fires only when Fn itself is pressed or released.
///   * KeyDown (10) + KeyUp (11) for the defensive F-key path, which only
///     matters on non-Apple keyboards that never emit FlagsChanged for Fn.
///
/// Returns whether a live tap is now installed.
unsafe fn install_tap(reason: &str) -> bool {
    let mask: u64 = (1u64 << KCG_EVENT_FLAGS_CHANGED)
        | (1u64 << KCG_EVENT_KEY_DOWN)
        | (1u64 << KCG_EVENT_KEY_UP);
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
        crate::logging::log_error(
            "[FnKey] CGEventTapCreate returned null — the Fn key will not work. \
             Input Monitoring permission is probably missing.",
        );
        crate::trace::event(
            "hotkey.tap_create_failed",
            serde_json::json!({ "reason": reason }),
        );
        return false;
    }

    let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
    CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
    CGEventTapEnable(tap, 1);

    // Publish both handles BEFORE anything can disable the tap: they are what
    // make re-arming and, failing that, rebuilding possible at all.
    TAP_PORT.store(tap as *mut std::ffi::c_void, Ordering::Relaxed);
    TAP_SOURCE.store(source as *mut std::ffi::c_void, Ordering::Relaxed);
    REARM_STREAK.store(0, Ordering::Relaxed);

    fnlog!("[FnKey] CGEventTap armed at HID level ({})", reason);
    crate::trace::event(
        "hotkey.tap_armed",
        serde_json::json!({ "reason": reason }),
    );
    true
}

/// Remove and release the current tap and its run loop source.
///
/// Must run on the same thread that installed them.
unsafe fn teardown_tap() {
    let source = TAP_SOURCE.swap(std::ptr::null_mut(), Ordering::Relaxed) as CFRunLoopSourceRef;
    if !source.is_null() {
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        CFRelease(source as *const std::ffi::c_void);
    }
    let tap = TAP_PORT.swap(std::ptr::null_mut(), Ordering::Relaxed) as CFMachPortRef;
    if !tap.is_null() {
        CGEventTapEnable(tap, 0);
        CFRelease(tap as *const std::ffi::c_void);
    }
}

/// Replace a tap the system has stopped honouring.
///
/// The distinction from [`re_arm_tap`] is the whole point: `CGEventTapEnable`
/// on a tap the window server has written off does nothing at all, so the
/// watchdog can report a successful recovery every two seconds while the Fn
/// key stays dead. Only a fresh tap gets the events flowing again.
unsafe fn rebuild_tap(streak: u64) {
    let attempt = REBUILD_ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;

    if attempt > TAP_REBUILD_MAX_ATTEMPTS {
        // Rebuilding demonstrably is not working. Stop: the process cannot
        // hold a tap and will not until it is restarted. Said once, then
        // never again for this session.
        if !TAP_ABANDONED.swap(true, Ordering::Relaxed) {
            crate::logging::log_error(&format!(
                "[FnKey] Event tap could not be kept alive after {} rebuilds. The Fn \
                 key will not work until TTP is restarted. This is a process-level \
                 condition — most likely Input Monitoring was granted after launch.",
                TAP_REBUILD_MAX_ATTEMPTS
            ));
            crate::trace::event(
                "hotkey.tap_abandoned",
                serde_json::json!({ "rebuilds": TAP_REBUILD_MAX_ATTEMPTS }),
            );
        }
        return;
    }

    crate::logging::log_warn(&format!(
        "[FnKey] Event tap did not survive {} re-arms — rebuilding it (attempt {} of {}). \
         The Fn key was not delivering events until now.",
        streak, attempt, TAP_REBUILD_MAX_ATTEMPTS
    ));
    crate::trace::event(
        "hotkey.tap_rebuilt",
        serde_json::json!({ "after_failed_rearms": streak, "attempt": attempt }),
    );
    teardown_tap();
    install_tap("rebuild");
}

/// Re-arm the HID event tap after macOS disabled it.
///
/// This is the recovery path for the single worst failure mode in the input
/// layer: a disabled tap is completely silent, so without this the Fn key
/// stops working until the app is restarted — and the user has no way to
/// tell that is what happened. Logged at WARN (release builds keep Warn) and
/// mirrored into the dictation trace so the event lines up chronologically
/// with the dictations that failed around it.
///
/// Safe to call from the tap callback and from the timer; `CGEventTapEnable`
/// on an already-enabled tap is a no-op.
fn re_arm_tap(reason: &str) {
    let port = TAP_PORT.load(Ordering::Relaxed) as CFMachPortRef;
    if port.is_null() {
        return;
    }
    unsafe { CGEventTapEnable(port, 1) };

    let streak = REARM_STREAK.fetch_add(1, Ordering::Relaxed) + 1;
    let now = now_ms();
    let last_logged = LAST_REARM_LOG_MS.load(Ordering::Relaxed);
    let should_log = streak == 1 || now.saturating_sub(last_logged) >= REARM_LOG_INTERVAL_MS;

    if should_log {
        LAST_REARM_LOG_MS.store(now, Ordering::Relaxed);
        fnlog!("[FnKey] event tap was disabled ({}) — re-armed (streak {})", reason, streak);
        crate::logging::log_warn(&format!(
            "[FnKey] Event tap was disabled ({}) and has been re-armed \
             ({} consecutive re-arms). The Fn key would have stopped \
             responding until restart.",
            reason, streak
        ));
    }

    // The trace keeps every occurrence — it is the timeline you consult to
    // line a broken dictation up against the tap dying. Only the human-facing
    // log is rate-limited.
    crate::trace::event(
        "hotkey.tap_rearmed",
        serde_json::json!({ "reason": reason, "streak": streak }),
    );
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

            // The timer was historically a sprawl of branches with the
            // timing rules buried inline. Now the whole decision is one call
            // to `fnkey_fsm::fn_decide`. Steps:
            //   1. Snapshot the atomics into an FnFsmState.
            //   2. Read whether Fn is physically held right now (the source
            //      of truth maintained by `fkey_tap_callback`).
            //   3. Run the FSM.
            //   4. Apply the new state to the atomics.
            //   5. Dispatch the action.
            //
            // `flags` is read only for the diagnostic log — the FSM no
            // longer consults NSEvent.modifierFlags directly.
            // Stall detection. This timer is scheduled every 20ms, so a gap
            // of a second or more means nothing ran — see
            // TIMER_STALL_THRESHOLD_MS. Recorded before anything else so the
            // trace shows the stall even if the tick that noticed it goes on
            // to do nothing interesting.
            let tick_now = now_ms();
            let prev_tick = LAST_TICK_MS.swap(tick_now, Ordering::Relaxed);
            if prev_tick != 0 {
                let gap_ms = tick_now.saturating_sub(prev_tick);
                if gap_ms >= TIMER_STALL_THRESHOLD_MS {
                    crate::trace::event(
                        "hotkey.timer_stall",
                        serde_json::json!({ "gap_ms": gap_ms }),
                    );
                }
            }

            let tick = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

            // Watchdog. A tap that macOS disabled without us seeing the
            // notification is indistinguishable from "the user isn't
            // pressing anything", so the only way to detect it is to ask.
            if tick % TAP_WATCHDOG_TICKS == 0 {
                let port = TAP_PORT.load(Ordering::Relaxed) as CFMachPortRef;
                if TAP_ABANDONED.load(Ordering::Relaxed) {
                    // Given up for this session. Touching the tap again would
                    // only burn cycles and flood the trace.
                } else if port.is_null() {
                    // No tap at all — an earlier create failed. Keep trying:
                    // Input Monitoring may have been granted since.
                    install_tap("watchdog_no_tap");
                } else if CGEventTapIsEnabled(port) {
                    // Healthy: end any flapping streak so the next genuine
                    // failure logs immediately instead of being rate-limited,
                    // and forgive earlier rebuilds — they evidently worked.
                    REARM_STREAK.store(0, Ordering::Relaxed);
                    REBUILD_ATTEMPTS.store(0, Ordering::Relaxed);
                } else if REARM_STREAK.load(Ordering::Relaxed) >= TAP_REBUILD_AFTER_FAILED_REARMS {
                    // Re-enabling has demonstrably stopped working. Stop
                    // asking and build a new tap.
                    rebuild_tap(REARM_STREAK.load(Ordering::Relaxed));
                } else {
                    re_arm_tap("watchdog");
                }
            }

            let flags: u64 = msg_send![class!(NSEvent), modifierFlags];
            let mut fn_held = is_physical_fn_key(flags);

            // Stale-flag resync.
            //
            // `FN_KEY_PHYSICALLY_DOWN` is maintained purely by the tap, so a
            // key-up that the tap never saw (it was disabled, or another
            // process swallowed the event) latches it at `true` forever. The
            // FSM then never fires StopRecording, and every keystroke TTP
            // injects inherits the Globe modifier from the session state and
            // is eaten by the shortcut layer.
            //
            // `NSEvent.modifierFlags` is an independent view of the same
            // hardware, so a sustained disagreement means our copy is stale.
            // We only ever use it to force the flag DOWN, never up: F-keys
            // bleed the Function bit ON (the whole reason the tap exists),
            // so trusting it in that direction would resurrect the F3/F4/F6
            // false-trigger bug. Forcing down has no such failure mode.
            let resync = fn_stale_check(
                fn_held,
                (flags & NS_EVENT_MODIFIER_FLAG_FUNCTION) != 0,
                FN_STALE_TICKS.load(Ordering::Relaxed),
            );
            FN_STALE_TICKS.store(resync.new_ticks, Ordering::Relaxed);
            if resync.clear_flag {
                FN_KEY_PHYSICALLY_DOWN.store(false, Ordering::Relaxed);
                fn_held = false;
                let stale_ms = FN_STALE_RESYNC_TICKS * 20;
                fnlog!("[FnKey] stale Fn-down flag cleared (NSEvent disagreed for {}ms)", stale_ms);
                crate::logging::log_warn(
                    "[FnKey] Cleared a stuck Fn-down flag: the tap reported the Globe key \
                     held while NSEvent reported it up. Injected keystrokes would have been \
                     routed to the Globe shortcut layer.",
                );
                crate::trace::event(
                    "hotkey.stale_fn_cleared",
                    serde_json::json!({ "stale_ms": stale_ms }),
                );
            }

            let state = FnFsmState {
                fn_was_held: FN_KEY_DOWN.load(Ordering::Relaxed),
                recording_active: FN_RECORDING_ACTIVE.load(Ordering::Relaxed),
                press_time_ms: FN_PRESS_TIME_MS.load(Ordering::Relaxed),
                last_press_time_ms: LAST_FN_PRESS_TIME_MS.load(Ordering::Relaxed),
                hands_free_recording: HANDS_FREE_RECORDING.load(Ordering::Relaxed),
                hands_free_start_ms: HANDS_FREE_START_MS.load(Ordering::Relaxed),
            };

            let now = now_ms();
            let decision = fn_decide(state, fn_held, now);

            // Commit every field the FSM touches. We write unconditionally
            // (even when nothing changed) so a future field added to
            // FnFsmState can't accidentally desync the atomics — the FSM
            // is the single source of truth for the state shape.
            FN_KEY_DOWN.store(decision.new_state.fn_was_held, Ordering::Relaxed);
            FN_RECORDING_ACTIVE.store(decision.new_state.recording_active, Ordering::Relaxed);
            FN_PRESS_TIME_MS.store(decision.new_state.press_time_ms, Ordering::Relaxed);
            LAST_FN_PRESS_TIME_MS.store(decision.new_state.last_press_time_ms, Ordering::Relaxed);

            // Note: we do NOT write back `hands_free_recording` /
            // `hands_free_start_ms` — those are set by `set_hands_free_recording`,
            // which is called from `shortcuts.rs` on session boundaries. The
            // FSM treats them as read-only inputs.

            match decision.action {
                FnAction::None => {}
                FnAction::FireDoubleTap => {
                    fnlog!(
                        "[FnKey] Fn key DOUBLE-TAP detected ({}ms since last press)",
                        now.saturating_sub(state.last_press_time_ms)
                    );
                    crate::trace::event(
                        "hotkey.double_tap",
                        serde_json::json!({ "gap_ms": now.saturating_sub(state.last_press_time_ms) }),
                    );
                    if let Some(app) = APP_HANDLE.get() {
                        crate::shortcuts::handle_fn_double_tap(app);
                    }
                }
                FnAction::StartRecording => {
                    fnlog!(
                        "[FnKey] Fn key HELD ({}ms, flags=0x{:X}) — starting recording",
                        now.saturating_sub(state.press_time_ms),
                        flags
                    );
                    crate::trace::event(
                        "hotkey.press",
                        serde_json::json!({
                            "held_ms": now.saturating_sub(state.press_time_ms),
                            "flags": format!("0x{:X}", flags),
                        }),
                    );
                    if let Some(app) = APP_HANDLE.get() {
                        handle_shortcut_event_public(app, ShortcutState::Pressed);
                    }
                }
                FnAction::StopRecording => {
                    fnlog!("[FnKey] Fn key UP (flags=0x{:X}) — stopping recording", flags);
                    crate::trace::event(
                        "hotkey.release",
                        serde_json::json!({ "flags": format!("0x{:X}", flags) }),
                    );
                    if let Some(app) = APP_HANDLE.get() {
                        handle_shortcut_event_public(app, ShortcutState::Released);
                    }
                }
                FnAction::StopHandsFree => {
                    fnlog!(
                        "[FnKey] Fn key UP ({}ms) — single tap stops hands-free recording",
                        now.saturating_sub(state.press_time_ms)
                    );
                    crate::trace::event(
                        "hotkey.hands_free_stop",
                        serde_json::json!({ "tap_ms": now.saturating_sub(state.press_time_ms) }),
                    );
                    if let Some(app) = APP_HANDLE.get() {
                        crate::shortcuts::handle_fn_stop(app);
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
        install_tap("startup");
    }
}

unsafe extern "C" fn fkey_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> CGEventRef {
    if !FN_MONITORING_ACTIVE.load(Ordering::Relaxed) {
        return event;
    }

    // macOS delivers these two instead of a key event when it has unhooked
    // the tap. They must be handled first: the tap is dead from this moment
    // until it is re-armed, and every Fn press in between is lost.
    if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT
        || event_type == KCG_EVENT_TAP_DISABLED_BY_USER_INPUT
    {
        let reason = if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT {
            "timeout"
        } else {
            "user_input"
        };
        re_arm_tap(reason);
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
