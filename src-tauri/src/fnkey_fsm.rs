// TTP - Talk To Paste
// Pure-function FSM for the Fn / Globe key push-to-talk timer.
//
// This module exists because `fnkey.rs` historically buried all the timing
// rules (debounce, double-tap, hands-free grace, F-key veto) inside a
// closure that called atomics, objc selectors, and CGEventTap — none of
// which can be exercised from a unit test. The F3 / F4 / F6 "Mission
// Control bleed" bug from v1.6.x and the v2.0.9 double-tap regression both
// shipped because we had no way to assert the decision tree in isolation.
//
// `fn_decide` is a side-effect-free pure function: feed it the snapshot
// of the FSM state + an event + a wall-clock timestamp, it returns the
// action to take and the new state to commit. The wrapper in `fnkey.rs`
// is then a thin glue layer that:
//
//   1. Reads the atomics into an `FnFsmState`.
//   2. Calls `fn_decide`.
//   3. Writes the new state back to the atomics.
//   4. Dispatches the action by calling `handle_shortcut_event_public` /
//      `handle_fn_double_tap` / `handle_fn_stop`.
//
// Every change to debounce / double-tap / grace timing is then either a
// test failure (caught at build time) or a deliberate update to the
// expectation. No more "we shipped a value but didn't realise it broke
// double-tap detection on user X's keyboard".

/// Debounce: Fn must be held for this long before a real recording starts (ms).
///
/// Lower bound is set by the duration of the system "Globe / Fn tap" gesture
/// macOS uses to open the emoji picker — at 100ms we still mis-triggered on
/// fast users. 150ms is the smallest value that consistently rejects the
/// emoji picker tap on a 2024 MacBook Pro Apple silicon keyboard while
/// staying responsive (the user perceives "instant" up to ~200ms).
pub const FN_DEBOUNCE_MS: u64 = 150;

/// Window during which two Fn taps count as a double-tap (ms).
///
/// Tuned against the natural tempo of a deliberate double-tap — 80-180ms
/// per tap, ~60-100ms between them. 300ms is generous on the upper end so
/// users with motor-control jitter still get hands-free mode, without
/// hitting the false-positive ceiling that 500ms briefly caused in v2.0.8.
pub const DOUBLE_TAP_THRESHOLD_MS: u64 = 300;

/// Grace period after a hands-free recording starts before a single Fn tap
/// is allowed to stop it (ms).
///
/// MUST comfortably exceed DOUBLE_TAP_THRESHOLD_MS so the SECOND tap of the
/// starting double-tap cannot self-cancel the session it just began. We
/// give it ~100ms of slack over the double-tap window for hardware jitter.
pub const HANDS_FREE_STOP_GRACE_MS: u64 = 400;

/// Minimum elapsed-since-press to count an Fn release as a double-tap
/// candidate (ms). Below this we treat it as HID jitter and ignore.
///
/// Set to the timer polling rate (20ms) — the timer can't reliably tell
/// "key tapped for 5ms" from "noise" anyway.
pub const DOUBLE_TAP_MIN_ELAPSED_MS: u64 = 20;

/// Maximum elapsed-since-press for a release to be a double-tap candidate
/// (ms). Releases longer than this hit the recording / debounce branches
/// instead and are already handled.
pub const DOUBLE_TAP_MAX_ELAPSED_MS: u64 = 500;

/// Snapshot of every piece of state the Fn timer needs to make a decision.
/// The wrapper in `fnkey.rs` builds this from atomics on every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FnFsmState {
    /// Was the Fn key physically held according to the previous tick.
    pub fn_was_held: bool,
    /// Have we already fired the "Pressed" event for the current press
    /// (i.e. the debounce window has elapsed).
    pub recording_active: bool,
    /// Wall-clock time (ms since epoch) at which the current press started.
    /// Zero when no press is in progress.
    pub press_time_ms: u64,
    /// Wall-clock time of the most recent registered Fn release. Used for
    /// double-tap detection on the NEXT press. Zero means "no recent tap".
    pub last_press_time_ms: u64,
    /// Whether the shortcut layer has entered hands-free mode for the
    /// current session.
    pub hands_free_recording: bool,
    /// Wall-clock time at which the hands-free session began. Zero when
    /// no hands-free session is in progress.
    pub hands_free_start_ms: u64,
}

impl Default for FnFsmState {
    fn default() -> Self {
        Self {
            fn_was_held: false,
            recording_active: false,
            press_time_ms: 0,
            last_press_time_ms: 0,
            hands_free_recording: false,
            hands_free_start_ms: 0,
        }
    }
}

/// What the wrapper should do after applying the new state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnAction {
    /// No-op. The state may still have changed (counters bumped, flags
    /// flipped) but no external dispatch is needed.
    None,
    /// Fire `handle_shortcut_event_public(Pressed)` — debounce has elapsed
    /// and we should start recording.
    StartRecording,
    /// Fire `handle_shortcut_event_public(Released)` — Fn was released
    /// while we were recording.
    StopRecording,
    /// Fire `handle_fn_double_tap` — the user just did a deliberate
    /// double-tap.
    FireDoubleTap,
    /// Fire `handle_fn_stop` — a single Fn tap while hands-free is active,
    /// past the start grace.
    StopHandsFree,
}

/// Outcome of one FSM tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FnDecision {
    pub action: FnAction,
    pub new_state: FnFsmState,
}

/// Pure decision function. Inputs:
///
///   * `state`: snapshot of the FSM at the start of this tick.
///   * `fn_held_now`: result of consulting `FN_KEY_PHYSICALLY_DOWN` (the
///     atomic the CGEventTap callback maintains). The pure function does
///     NOT consult macOS APIs — that's the wrapper's job.
///   * `now_ms`: wall-clock millisecond timestamp for this tick.
///
/// Output: the action to dispatch and the new state to write back.
pub fn fn_decide(state: FnFsmState, fn_held_now: bool, now_ms: u64) -> FnDecision {
    // Case 1: Fn just pressed (transition !held -> held).
    if fn_held_now && !state.fn_was_held {
        let last_press = state.last_press_time_ms;
        let is_double_tap = last_press > 0 && now_ms.saturating_sub(last_press) < DOUBLE_TAP_THRESHOLD_MS;

        let mut new_state = state;
        new_state.fn_was_held = true;
        new_state.press_time_ms = now_ms;

        if is_double_tap {
            // Clear the candidate so a triple-tap doesn't fire again.
            new_state.last_press_time_ms = 0;
            return FnDecision {
                action: FnAction::FireDoubleTap,
                new_state,
            };
        }
        // Single new press — wait for the debounce branch below to fire.
        return FnDecision {
            action: FnAction::None,
            new_state,
        };
    }

    // Case 2: Fn still held, recording hasn't started yet — check debounce.
    if fn_held_now && state.fn_was_held && !state.recording_active {
        let elapsed = now_ms.saturating_sub(state.press_time_ms);
        if elapsed >= FN_DEBOUNCE_MS {
            let mut new_state = state;
            new_state.recording_active = true;
            return FnDecision {
                action: FnAction::StartRecording,
                new_state,
            };
        }
        // Still debouncing.
        return FnDecision {
            action: FnAction::None,
            new_state: state,
        };
    }

    // Case 3: Fn released (transition held -> !held).
    if !fn_held_now && state.fn_was_held {
        let mut new_state = state;
        new_state.fn_was_held = false;

        if state.recording_active {
            // Stop the in-flight push-to-talk recording.
            new_state.recording_active = false;
            return FnDecision {
                action: FnAction::StopRecording,
                new_state,
            };
        }

        // Released BEFORE the debounce — too short to be a recording.
        // Could be: a) hardware jitter, b) the first half of a double-tap,
        // c) a single tap intended to STOP an active hands-free session.
        let elapsed = now_ms.saturating_sub(state.press_time_ms);
        let session_age = now_ms.saturating_sub(state.hands_free_start_ms);
        let stop_allowed = state.hands_free_recording
            && state.hands_free_start_ms > 0
            && session_age > HANDS_FREE_STOP_GRACE_MS;

        if stop_allowed {
            // Single tap during a hands-free session, past the start grace.
            // Stop it. Clear the candidate so the same tap can't also count
            // as the first half of a future double-tap.
            new_state.last_press_time_ms = 0;
            return FnDecision {
                action: FnAction::StopHandsFree,
                new_state,
            };
        }

        // Too short for a recording AND not stopping hands-free. Register
        // as a double-tap candidate if the elapsed time is in the natural
        // tap window.
        if (DOUBLE_TAP_MIN_ELAPSED_MS..DOUBLE_TAP_MAX_ELAPSED_MS).contains(&elapsed) {
            new_state.last_press_time_ms = state.press_time_ms;
        }
        return FnDecision {
            action: FnAction::None,
            new_state,
        };
    }

    // Case 4: Nothing changed — held throughout while recording, or idle
    // throughout. No-op.
    FnDecision {
        action: FnAction::None,
        new_state: state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(fn_was_held: bool, recording_active: bool) -> FnFsmState {
        FnFsmState {
            fn_was_held,
            recording_active,
            ..Default::default()
        }
    }

    // ── Press, debounce, release: push-to-talk happy path ────────────────

    #[test]
    fn idle_state_returns_none_when_no_fn_input() {
        let s = FnFsmState::default();
        let r = fn_decide(s, false, 1_000);
        assert_eq!(r.action, FnAction::None);
        assert_eq!(r.new_state, s);
    }

    #[test]
    fn fn_press_records_press_time_no_action() {
        let s = FnFsmState::default();
        let r = fn_decide(s, true, 1_000);
        assert_eq!(r.action, FnAction::None);
        assert!(r.new_state.fn_was_held);
        assert_eq!(r.new_state.press_time_ms, 1_000);
    }

    #[test]
    fn debounce_expires_starts_recording() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 1_000;

        // Tick before the debounce window.
        let r = fn_decide(s, true, 1_000 + FN_DEBOUNCE_MS - 1);
        assert_eq!(r.action, FnAction::None);
        assert!(!r.new_state.recording_active);

        // Tick AT the debounce window.
        let r = fn_decide(s, true, 1_000 + FN_DEBOUNCE_MS);
        assert_eq!(r.action, FnAction::StartRecording);
        assert!(r.new_state.recording_active);
    }

    #[test]
    fn release_after_debounce_stops_recording() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.recording_active = true;
        s.press_time_ms = 1_000;

        let r = fn_decide(s, false, 1_000 + 500);
        assert_eq!(r.action, FnAction::StopRecording);
        assert!(!r.new_state.fn_was_held);
        assert!(!r.new_state.recording_active);
    }

    // ── Double-tap detection ─────────────────────────────────────────────

    #[test]
    fn release_in_natural_tap_window_registers_double_tap_candidate() {
        // First tap: pressed at t=1000, released at t=1080 (80ms tap).
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 1_000;
        let r = fn_decide(s, false, 1_080);
        assert_eq!(r.action, FnAction::None);
        assert_eq!(r.new_state.last_press_time_ms, 1_000);
    }

    #[test]
    fn release_below_minimum_elapsed_does_not_register_candidate() {
        // 10ms is below DOUBLE_TAP_MIN_ELAPSED_MS = 20.
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 1_000;
        let r = fn_decide(s, false, 1_010);
        assert_eq!(r.action, FnAction::None);
        assert_eq!(r.new_state.last_press_time_ms, 0);
    }

    #[test]
    fn press_within_double_tap_window_fires_double_tap() {
        // First tap candidate registered at t=1000 (from a previous release).
        // Second press arrives at t=1200 — 200ms gap, inside the 300ms window.
        let mut s = FnFsmState::default();
        s.last_press_time_ms = 1_000;
        let r = fn_decide(s, true, 1_200);
        assert_eq!(r.action, FnAction::FireDoubleTap);
        assert!(r.new_state.fn_was_held);
        // Candidate is cleared so a triple-tap can't re-fire.
        assert_eq!(r.new_state.last_press_time_ms, 0);
    }

    #[test]
    fn press_past_double_tap_window_does_not_fire_double_tap() {
        let mut s = FnFsmState::default();
        s.last_press_time_ms = 1_000;
        // 350ms > 300ms threshold.
        let r = fn_decide(s, true, 1_350);
        assert_eq!(r.action, FnAction::None);
        assert!(r.new_state.fn_was_held);
    }

    #[test]
    fn full_double_tap_sequence_end_to_end() {
        let mut s = FnFsmState::default();
        // Tick 1: press at t=1000.
        let r = fn_decide(s, true, 1_000);
        s = r.new_state;
        // Tick 2: release at t=1100 (100ms tap — natural).
        let r = fn_decide(s, false, 1_100);
        s = r.new_state;
        assert_eq!(s.last_press_time_ms, 1_000);
        // Tick 3: second press at t=1200 (100ms gap).
        let r = fn_decide(s, true, 1_200);
        assert_eq!(r.action, FnAction::FireDoubleTap);
    }

    // ── Hands-free single-tap stop ──────────────────────────────────────

    #[test]
    fn single_tap_during_hands_free_after_grace_stops_session() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 5_000;
        s.hands_free_recording = true;
        s.hands_free_start_ms = 4_000; // started 1s ago — past 400ms grace.
        // Release at t=5050 (50ms tap — within natural tap window).
        let r = fn_decide(s, false, 5_050);
        assert_eq!(r.action, FnAction::StopHandsFree);
        // Candidate cleared so the same tap can't also count toward a
        // future double-tap.
        assert_eq!(r.new_state.last_press_time_ms, 0);
    }

    #[test]
    fn single_tap_during_hands_free_inside_grace_is_ignored() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 1_100;
        s.hands_free_recording = true;
        s.hands_free_start_ms = 1_000; // 200ms ago — inside 400ms grace.
        let r = fn_decide(s, false, 1_150);
        assert_eq!(r.action, FnAction::None);
        // This still registers as a double-tap candidate (50ms is in the
        // natural tap window) — the double-tap path is independent of
        // hands-free state.
        assert_eq!(r.new_state.last_press_time_ms, 1_100);
    }

    #[test]
    fn single_tap_without_hands_free_does_not_fire_stop() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 5_000;
        s.hands_free_recording = false; // not in hands-free
        s.hands_free_start_ms = 0;
        let r = fn_decide(s, false, 5_050);
        assert_eq!(r.action, FnAction::None);
    }

    // ── Edge cases the v1.x/v2.x bugs taught us about ───────────────────

    #[test]
    fn press_with_zero_last_press_does_not_double_tap() {
        // last_press_time_ms == 0 is the sentinel for "no candidate"; even
        // at now_ms < DOUBLE_TAP_THRESHOLD_MS it must NOT fire double-tap.
        let s = FnFsmState::default();
        let r = fn_decide(s, true, 100);
        assert_eq!(r.action, FnAction::None);
    }

    #[test]
    fn held_through_idle_tick_is_no_op() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.recording_active = true;
        s.press_time_ms = 1_000;
        // Tick while still held and still recording — should produce no
        // action and no state change.
        let r = fn_decide(s, true, 2_000);
        assert_eq!(r.action, FnAction::None);
        assert_eq!(r.new_state, s);
    }

    #[test]
    fn clock_going_backwards_does_not_panic() {
        // SystemTime can in principle move backwards. saturating_sub keeps
        // us safe — the tap is just considered very recent.
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = 10_000;
        // now < press_time — saturating_sub returns 0 — should be treated
        // as "elapsed = 0", below MIN_ELAPSED, so no candidate.
        let r = fn_decide(s, false, 9_000);
        assert_eq!(r.action, FnAction::None);
        assert_eq!(r.new_state.last_press_time_ms, 0);
    }

    #[test]
    fn debounce_threshold_uses_saturating_arithmetic() {
        let mut s = FnFsmState::default();
        s.fn_was_held = true;
        s.press_time_ms = u64::MAX - 10;
        // now < press_time would underflow without saturating_sub.
        let r = fn_decide(s, true, u64::MAX);
        // elapsed = 10ms < FN_DEBOUNCE_MS = 150ms, so no start.
        assert_eq!(r.action, FnAction::None);
    }
}
