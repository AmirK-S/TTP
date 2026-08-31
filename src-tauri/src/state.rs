// TTP - Talk To Paste
// Application state management.
//
// Separates the PERSISTED hands-free preference (`hands_free_mode`, mirrored
// from settings.json and surfaced in Settings UI) from the TRANSIENT
// per-session override (`session_hands_free`, set when the user enters
// hands-free for a single recording via Fn double-tap or the tray "Start
// recording" item). Previously both shared a single field, which meant:
//
//   - A double-tap that lit `hands_free_mode = true` had to be manually
//     reset to `settings_hands_free` on stop, in every code path that
//     observed the stop. Miss one path (a crash, an early return, a future
//     refactor) and the user's persistent preference silently flipped on
//     them for the rest of the session.
//   - Reading "is this session hands-free?" required knowing whether
//     `hands_free_mode` was currently in "persisted" or "override" mode,
//     which is exactly the kind of disambiguation a type system is for.
//
// The new contract: `hands_free_mode` always reflects the persisted setting.
// `session_hands_free` is `Some(true)` only for the duration of a single
// hands-free recording, and is cleared automatically on every transition
// back to Idle. Callers read `effective_hands_free()` instead of the raw
// field.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

pub struct AppState {
    pub recording_state: RecordingState,
    /// Persistent user preference, mirrored from `settings.hands_free_mode`.
    /// Mutate ONLY when settings change (see `tray::sync_from_settings` and
    /// `lib.rs::set_fn_key_enabled`). Per-recording overrides go through
    /// `session_hands_free` below.
    pub hands_free_mode: bool,
    /// Transient hands-free override active for the current recording only.
    /// Set to `Some(true)` when the user enters hands-free via Fn double-tap
    /// or the tray "Start" menu item. Cleared back to `None` automatically
    /// on every transition to Idle so it can never leak across sessions.
    pub session_hands_free: Option<bool>,
    pub last_shortcut_time: Option<Instant>,
    /// Wall-clock time at which the current recording started. `None` outside
    /// the Recording state. Used by the Fn hands-free grace window
    /// (`fnkey.rs::HANDS_FREE_STOP_GRACE_MS`) and by the pipeline's failure
    /// telemetry to attribute "how long was the session before it died".
    pub recording_started_at: Option<Instant>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_state: RecordingState::Idle,
            hands_free_mode: false,
            session_hands_free: None,
            last_shortcut_time: None,
            recording_started_at: None,
        }
    }
}

impl AppState {
    pub fn set_state(&mut self, state: RecordingState, app: &AppHandle) {
        let old_state = self.recording_state.clone();
        self.recording_state = state.clone();

        // The state machine is the thing that gets stuck. `handle_shortcut_pressed`
        // only acts from Idle or Recording, so a session that never leaves
        // Processing makes every subsequent hotkey press a silent no-op — the
        // app looks dead while behaving exactly as written. Recording every
        // transition means the trace shows which state we are parked in and
        // when we arrived, instead of leaving it to be inferred from the
        // absence of other lines.
        crate::trace::event(
            "state.transition",
            serde_json::json!({
                "from": format!("{:?}", old_state),
                "to": format!("{:?}", state),
            }),
        );

        // Maintain the auxiliary fields tied to the recording lifecycle. Doing
        // this here (in the single transition site) means consumers don't have
        // to remember to reset them and can never see a stale value.
        match &state {
            RecordingState::Recording => {
                // The user is asking for the microphone. This is the earliest
                // and most authoritative statement of that: it runs
                // synchronously on the hotkey thread, before the event that
                // makes the frontend invoke `start_recording` has even been
                // emitted. `crate::capture_arbiter` arbitrates against it.
                //
                // Guarded on a real entry into Recording. A Recording →
                // Recording transition is not a second press and must not
                // supersede a start that is already in flight for the first.
                if old_state != RecordingState::Recording {
                    crate::capture_arbiter::ARBITER.user_wants_capture();
                }
                if self.recording_started_at.is_none() {
                    self.recording_started_at = Some(Instant::now());
                }
                // Hold an activity assertion for the whole
                // Recording → Processing → Idle window. A napped process
                // stops rather than slows: the Fn timer stops firing, the
                // pipeline stops advancing, and macOS disables our event tap
                // for timeout — which is the "TTP just stopped working"
                // failure. See `crate::activity`.
                crate::activity::begin_dictation();
                // Tell the pill which input modality is active so it can
                // show a hands-free affordance (lock icon) and the user
                // sees at a glance that they need to TAP again to stop,
                // not RELEASE. The mode is computed at session start —
                // it cannot change mid-session.
                let mode = if self.effective_hands_free() { "toggle" } else { "push_to_talk" };
                let _ = app.emit("recording-mode-changed", mode);
            }
            RecordingState::Idle => {
                // The app is done with the user's press, whatever any
                // in-flight capture command still believes. Anything that has
                // not published yet is refused; anything that has is reclaimed
                // below.
                //
                // Guarded on a real move INTO Idle. The pipeline sets Idle a
                // second time when it finishes, long after the first — that
                // trailing `Idle → Idle` says nothing about what the user
                // wants now, and acting on it would revoke a recording they
                // may have started in the meantime.
                if old_state != RecordingState::Idle {
                    crate::capture_arbiter::ARBITER.user_done_with_capture();
                    // The backstop for the reverse-ordered race: a capture
                    // that went live in the gap between a stop that found
                    // nothing and this transition. In the healthy case the
                    // stop already took it and this is one uncontended mutex.
                    //
                    // Safe under the AppState lock for the same reason
                    // `hide_pill` is: it touches STATE in `audio_capture`,
                    // the trace channel and the filesystem, and re-enters
                    // nothing here.
                    crate::audio_capture::reclaim_orphaned_capture();
                }
                self.session_hands_free = None;
                self.recording_started_at = None;
                // The user is no longer waiting on us; let the OS nap the
                // process again. Released here rather than in the pipeline
                // because every abort path also funnels through Idle.
                crate::activity::end_dictation();
                let _ = app.emit("recording-mode-changed", serde_json::Value::Null);
            }
            RecordingState::Processing => {
                // Keep the timestamp through Processing so failure attribution
                // can still compute total session duration in the pipeline.
            }
        }

        // Start/stop audio level monitoring for pill wave visualization,
        // and the VAD watchdog. Both consult `audio_capture::current_rms()`
        // so they're cheap to run concurrently.
        match &state {
            RecordingState::Recording => {
                crate::audio_monitor::start(app.clone());
                crate::vad::start(app.clone());
            }
            _ => {
                crate::audio_monitor::stop();
                crate::vad::stop();
            }
        }

        // Handle pill visibility when transitioning to Idle.
        //
        // CAREFUL: we are holding `&mut self` (the AppState mutex is locked
        // by the caller). `should_show_pill_for_state` and `hide_pill` MUST
        // NOT re-enter AppState — they currently only read settings / call
        // window APIs. If anyone refactors `hide_pill` to take the AppState
        // lock or do async work that re-enters here, this becomes a deadlock.
        // Verified safe 2026-05-07, re-verified 2026-06-10.
        if old_state != RecordingState::Idle && state == RecordingState::Idle {
            if !crate::tray::should_show_pill_for_state(&state) {
                crate::tray::hide_pill(app);
            }
        }

        app.emit("recording-state-changed", &state).ok();
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.recording_state, RecordingState::Recording)
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.recording_state, RecordingState::Idle)
    }

    pub fn is_processing(&self) -> bool {
        matches!(self.recording_state, RecordingState::Processing)
    }

    /// Whether the current (or upcoming) recording session is hands-free.
    /// Session overrides always win over the persisted preference; absent a
    /// transient override the persisted value applies.
    pub fn effective_hands_free(&self) -> bool {
        self.session_hands_free.unwrap_or(self.hands_free_mode)
    }

    /// Mark the next-or-current recording as hands-free for this session
    /// only. Cleared automatically on the next Idle transition.
    pub fn enter_hands_free_session(&mut self) {
        self.session_hands_free = Some(true);
    }

    /// Push a refreshed value from settings.json into the persisted field.
    /// Use this at the boundary where settings actually change; never mutate
    /// `hands_free_mode` to express a transient state.
    pub fn set_persistent_hands_free(&mut self, value: bool) {
        self.hands_free_mode = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_idle_with_no_session_override() {
        let s = AppState::default();
        assert!(s.is_idle());
        assert!(!s.is_recording());
        assert!(!s.is_processing());
        assert_eq!(s.session_hands_free, None);
        assert!(!s.effective_hands_free());
    }

    #[test]
    fn effective_hands_free_falls_back_to_persisted_when_no_session_override() {
        let mut s = AppState::default();
        s.set_persistent_hands_free(true);
        assert!(s.effective_hands_free());

        s.set_persistent_hands_free(false);
        assert!(!s.effective_hands_free());
    }

    #[test]
    fn session_override_wins_over_persisted_value() {
        let mut s = AppState::default();
        s.set_persistent_hands_free(false);
        s.enter_hands_free_session();
        assert!(s.effective_hands_free());
        // Persisted field is untouched.
        assert!(!s.hands_free_mode);
    }

    #[test]
    fn enter_hands_free_session_sets_only_the_override_field() {
        let mut s = AppState::default();
        s.enter_hands_free_session();
        assert_eq!(s.session_hands_free, Some(true));
        assert!(!s.hands_free_mode);
    }

    #[test]
    fn is_processing_distinguishes_from_is_recording_and_is_idle() {
        let mut s = AppState::default();
        s.recording_state = RecordingState::Processing;
        assert!(s.is_processing());
        assert!(!s.is_recording());
        assert!(!s.is_idle());
    }

    #[test]
    fn set_persistent_does_not_clobber_session_override() {
        let mut s = AppState::default();
        s.enter_hands_free_session();
        s.set_persistent_hands_free(false);
        // Session override is still active even after the persisted field
        // was updated — this is the documented contract.
        assert_eq!(s.session_hands_free, Some(true));
        assert!(s.effective_hands_free());
    }
}
