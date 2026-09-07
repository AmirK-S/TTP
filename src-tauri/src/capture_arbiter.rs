// TTP - Talk To Paste
// Who owns the microphone, and when.
//
// ── The defect ──────────────────────────────────────────────────────────
//
// 2026-08-30 23:18, from `ttp-trace.log.1`, a 60 ms tap:
//
//     23:18:21.386  hotkey.press                          → Recording
//     23:18:21.446  hotkey.release                        → Processing
//     23:18:21.852  capture.stop_failed "No recording in progress"
//     23:18:21.854  state.transition Processing → Idle
//     23:18:21.930  capture.start   {"device":"AirPods Pro"}
//
// and then nothing until `hotkey.timer_stall {"gap_ms":963122}` the next
// morning. The capture that went live at 23:18:21.930 was never stopped,
// because the only thing that stops a capture is `stop_recording`, and
// `stop_recording` had already run, found nothing, and gone home 78 ms
// earlier. **The microphone stayed open for ten hours and fifty-seven
// minutes.** For an app whose pitch is that your voice does not leave your
// machine, that is the one bug that must not exist.
//
// ── Why the existing fix does not close it ──────────────────────────────
//
// `audio_capture::STARTING` plus the settle-wait in `stop_recording_inner`
// were written against this exact incident, and they close the common
// ordering: a stop that arrives while a start is in flight waits for the
// start to publish, then collects it. That fix is real and it stays.
//
// It is also one-directional, and it has three holes, all with the same
// shape — *the losing branch is never reconciled*:
//
//   1. `STARTING` is set by the first line of `start_recording_inner`, which
//      is an `async fn`. Nothing runs until the future is first polled. The
//      window between the JS `invoke('start_recording')` and that first poll
//      is unbounded and invisible, and a stop that samples the flag inside it
//      sees `false`.
//   2. The wait is bounded at `START_SETTLE_TIMEOUT_MS` (3 s). Its comment
//      asserts the timeout "only ever elapses in full when a start genuinely
//      failed". That is an assumption, not a guarantee — a Bluetooth device
//      handing itself over from a phone can take longer — and when it is
//      wrong the orphan is back.
//   3. `STARTING` is one process-wide `bool` for what can be two concurrent
//      starts (double-tap). The first to finish clears it for both.
//
// In all three the trace prints the same five lines above.
//
// ── The mechanism, rather than the timing ───────────────────────────────
//
// The invariant nobody was enforcing:
//
//     A live capture exists only while the state machine wants one.
//
// The state machine already knows. `AppState::set_state` is the single site
// every transition passes through, it runs synchronously on the hotkey path
// *before* the IPC that starts the capture is even emitted, and `Recording`
// means precisely "the user is holding the key". So the arbiter is driven
// from there, and the capture layer asks it two questions:
//
//   * `try_publish` — a start that has built a stream asks whether it may go
//     live. If the user has since returned to Idle, or a newer press has
//     superseded this one, the answer is no and the start tears its own
//     stream down. This catches the observed defect deterministically: at
//     23:18:21.930 the state had been Idle for 76 ms.
//   * `should_reclaim` — the Idle transition asks whether a capture is live
//     with nobody in flight to collect it. This is the backstop for the
//     reverse order, where the stream publishes in the gap between the failed
//     stop and the Idle transition.
//
// Neither question involves a clock, so neither can be defeated by a slow
// device, a descheduled process, or a runtime that polls a future late.
//
// This module is deliberately free of cpal, Tauri and I/O so the arbitration
// can be tested by replaying the trace above, rather than by holding a key
// for eleven hours.

use std::sync::Mutex;

/// Why a start was refused permission to go live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The state machine is back in Idle: the user pressed, released, and the
    /// dictation already concluded. This is the 2026-08-30 shape.
    UserIdle,
    /// A newer press has happened since this start was dispatched. A newer
    /// start is on its way and will publish; this one is stale and would be
    /// discarded by its self-heal anyway, but not before spending time live.
    Superseded,
}

impl Refusal {
    /// Stable slug for the trace. Greppable, and never a sentence.
    pub fn slug(self) -> &'static str {
        match self {
            Refusal::UserIdle => "user_idle",
            Refusal::Superseded => "superseded_by_newer_press",
        }
    }
}

/// The arbiter's answer to "may this capture go live?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publish {
    /// Publish it. The arbiter now considers a capture live.
    Go,
    /// Do not publish. Tear the stream down and say so on the trace.
    Refuse(Refusal),
}

/// Handed to a start when it begins; presented back when it wants to publish.
///
/// Carries the generation of the press it belongs to, so a start can be told
/// apart from the press that superseded it without consulting a clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartTicket {
    gen: u64,
}

#[derive(Debug, Default)]
struct Inner {
    /// True from `state → Recording` until `state → Idle`. `Processing` does
    /// NOT clear it: the stop's 400 ms driver drain runs in Processing and the
    /// stream must stay live for all of it.
    wanted: bool,
    /// Incremented on every `state → Recording`.
    want_gen: u64,
    /// Starts that have begun and not yet returned.
    starts_in_flight: u32,
    /// Stops that have begun and not yet returned. A stop in flight will
    /// collect whatever is live, so the Idle backstop must not race it.
    stops_in_flight: u32,
    /// Whether a capture is currently published in `audio_capture::STATE`.
    live: bool,
}

/// Process-wide arbiter. One microphone, one instance.
#[derive(Debug, Default)]
pub struct Arbiter {
    inner: Mutex<Inner>,
}

impl Arbiter {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                wanted: false,
                want_gen: 0,
                starts_in_flight: 0,
                stops_in_flight: 0,
                live: false,
            }),
        }
    }

    /// A poisoned lock here must not be able to wedge the microphone shut, so
    /// every accessor recovers the inner value rather than panicking. The
    /// state is five integers; there is no invariant a panicking thread could
    /// have left half-applied that is worse than refusing to record at all.
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `state → Recording`. The user is asking for the microphone.
    pub fn user_wants_capture(&self) {
        let mut inner = self.lock();
        inner.wanted = true;
        inner.want_gen = inner.want_gen.wrapping_add(1);
    }

    /// `state → Idle`. The user is not asking for the microphone any more,
    /// whatever any in-flight command still believes.
    pub fn user_done_with_capture(&self) {
        self.lock().wanted = false;
    }

    /// Begin a start. The returned ticket must be presented to
    /// [`try_publish`] and released with [`end_start`].
    pub fn begin_start(&self) -> StartTicket {
        let mut inner = self.lock();
        inner.starts_in_flight = inner.starts_in_flight.saturating_add(1);
        StartTicket { gen: inner.want_gen }
    }

    /// A start has returned, however it returned.
    pub fn end_start(&self) {
        let mut inner = self.lock();
        inner.starts_in_flight = inner.starts_in_flight.saturating_sub(1);
    }

    /// May this start go live?
    ///
    /// Called under `audio_capture::STATE`'s lock, in the same critical
    /// section as the publish, so it cannot be overtaken by the `take()` in
    /// `stop_recording_inner`.
    pub fn try_publish(&self, ticket: StartTicket) -> Publish {
        let mut inner = self.lock();
        if !inner.wanted {
            return Publish::Refuse(Refusal::UserIdle);
        }
        if inner.want_gen != ticket.gen {
            return Publish::Refuse(Refusal::Superseded);
        }
        inner.live = true;
        Publish::Go
    }

    /// Begin a stop. Released with [`end_stop`].
    pub fn begin_stop(&self) {
        let mut inner = self.lock();
        inner.stops_in_flight = inner.stops_in_flight.saturating_add(1);
    }

    /// A stop has returned. `found` is whether it actually had a capture to
    /// tear down — `false` is the "No recording in progress" arm.
    pub fn end_stop(&self, found: bool) {
        let mut inner = self.lock();
        inner.stops_in_flight = inner.stops_in_flight.saturating_sub(1);
        if found {
            inner.live = false;
        }
    }

    /// Whether a start is in flight. `stop_recording_inner` waits on this
    /// before concluding it has nothing to stop.
    pub fn start_in_flight(&self) -> bool {
        self.lock().starts_in_flight > 0
    }

    /// Is there a live capture that nobody is coming for?
    ///
    /// Asked on the `Idle` transition. `true` only when the user is done, a
    /// capture is published, and no start or stop is running that would
    /// otherwise handle it — so in the healthy case, where the stop has just
    /// taken the capture and returned, this is `false` and the backstop costs
    /// one uncontended mutex.
    pub fn should_reclaim(&self) -> bool {
        let inner = self.lock();
        !inner.wanted && inner.live && inner.starts_in_flight == 0 && inner.stops_in_flight == 0
    }

    /// The reclaim happened; nothing is live any more.
    pub fn mark_reclaimed(&self) {
        self.lock().live = false;
    }

    #[cfg(test)]
    fn is_live(&self) -> bool {
        self.lock().live
    }
}

/// The one arbiter.
pub static ARBITER: Arbiter = Arbiter::new();

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays `ttp-trace.log.1` 23:18:21.386 → .930 exactly, in order.
    ///
    /// Without the arbiter the last step publishes a live cpal stream that
    /// nothing holds a handle to, and the microphone stays open — which it did,
    /// for 10 h 57 m. The assertion is that the publish is refused.
    #[test]
    fn a_start_that_lands_after_the_stop_gave_up_is_refused() {
        let a = Arbiter::new();

        // 23:18:21.386  hotkey.press → Recording
        a.user_wants_capture();
        // start_recording is dispatched and begins resolving the AirPods.
        let ticket = a.begin_start();

        // 23:18:21.446  hotkey.release → Processing. `wanted` deliberately
        // survives Processing: the stop's 400 ms drain still needs the stream.
        // 23:18:21.852  stop concludes with "No recording in progress".
        a.begin_stop();
        a.end_stop(false);

        // 23:18:21.854  Processing → Idle (the frontend's reset_to_idle).
        a.user_done_with_capture();

        // 23:18:21.930  the start finally has a stream and asks to publish.
        assert_eq!(
            a.try_publish(ticket),
            Publish::Refuse(Refusal::UserIdle),
            "a capture must not go live 76ms after the user returned to Idle"
        );
        a.end_start();
        assert!(!a.is_live(), "nothing may be left live");
    }

    /// The ordinary push-to-talk cycle must be completely unaffected. If this
    /// fails the fix has cost the user their dictation, which is worse than
    /// the bug.
    #[test]
    fn the_ordinary_cycle_publishes_and_stops() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let ticket = a.begin_start();
        assert_eq!(a.try_publish(ticket), Publish::Go);
        a.end_start();
        assert!(a.is_live());

        a.begin_stop();
        assert!(!a.should_reclaim(), "a stop in flight collects it, not the backstop");
        a.end_stop(true);
        a.user_done_with_capture();

        assert!(!a.should_reclaim(), "the stop took it; there is nothing to reclaim");
        assert!(!a.is_live());
    }

    /// Hole 1: the start's future is polled late, so `STARTING` was false when
    /// the stop sampled it. The stop then concludes before the start has even
    /// begun, which no amount of waiting on the start flag can catch.
    #[test]
    fn a_start_dispatched_but_not_yet_polled_is_still_refused() {
        let a = Arbiter::new();

        a.user_wants_capture();
        // JS has invoked start_recording. Nothing Rust-side has run yet:
        // no ticket exists, so `start_in_flight()` is false.
        a.begin_stop();
        assert!(!a.start_in_flight(), "this is precisely the hole");
        a.end_stop(false);
        a.user_done_with_capture();

        // Only now does the runtime poll the start's future.
        let ticket = a.begin_start();
        assert_eq!(a.try_publish(ticket), Publish::Refuse(Refusal::UserIdle));
        a.end_start();
    }

    /// Hole 2: the start outlives `START_SETTLE_TIMEOUT_MS`, so the stop stops
    /// waiting and reports "No recording in progress" while the start is still
    /// building. The comment on that constant asserts this cannot happen; the
    /// arbiter does not need it to be true.
    #[test]
    fn a_start_slower_than_the_settle_timeout_is_refused() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let ticket = a.begin_start();

        a.begin_stop();
        assert!(a.start_in_flight(), "the stop waits...");
        a.end_stop(false); // ...gives up after 3s and reports nothing to stop
        a.user_done_with_capture();

        assert_eq!(a.try_publish(ticket), Publish::Refuse(Refusal::UserIdle));
        a.end_start();
    }

    /// Hole 3: two concurrent starts share one flag. The older one must not
    /// publish over the newer one's press.
    #[test]
    fn a_start_superseded_by_a_newer_press_is_refused() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let first = a.begin_start();

        // A second press arrives before the first start finished resolving.
        a.user_wants_capture();
        let second = a.begin_start();

        assert_eq!(a.try_publish(first), Publish::Refuse(Refusal::Superseded));
        a.end_start();
        assert_eq!(a.try_publish(second), Publish::Go, "the live press still records");
        a.end_start();
    }

    /// The backstop: the stream published in the gap between the failed stop
    /// and the Idle transition, so `try_publish` legitimately said yes. The
    /// Idle transition must then reclaim it.
    #[test]
    fn a_capture_live_at_idle_with_nobody_in_flight_is_reclaimed() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let ticket = a.begin_start();
        a.begin_stop();
        a.end_stop(false); // stop gave up
        // The start publishes while the state machine is still in Processing.
        assert_eq!(a.try_publish(ticket), Publish::Go);
        a.end_start();

        // Now Idle arrives.
        a.user_done_with_capture();
        assert!(a.should_reclaim(), "a live capture nobody owns must be reclaimed");
        a.mark_reclaimed();
        assert!(!a.should_reclaim());
        assert!(!a.is_live());
    }

    /// A start that fails (permission denied, no device) never publishes, so
    /// nothing is live and the backstop must stay quiet.
    #[test]
    fn a_failed_start_leaves_nothing_to_reclaim() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let _ticket = a.begin_start();
        a.end_start(); // returned Err before reaching try_publish
        a.user_done_with_capture();

        assert!(!a.should_reclaim());
    }

    /// Idle without any capture at all — the app sitting there — must not
    /// trip the backstop.
    #[test]
    fn idle_with_no_capture_never_reclaims() {
        let a = Arbiter::new();
        a.user_done_with_capture();
        assert!(!a.should_reclaim());
    }

    /// `Processing` is not `Idle`. The 400 ms drain in `stop_recording_inner`
    /// runs there and the stream must survive it — a start that publishes
    /// during the drain is the normal case, not an orphan.
    #[test]
    fn processing_does_not_revoke_the_users_request() {
        let a = Arbiter::new();
        a.user_wants_capture();
        let ticket = a.begin_start();
        a.begin_stop(); // release → Processing, drain running
        assert_eq!(
            a.try_publish(ticket),
            Publish::Go,
            "the stop is still coming for this capture"
        );
        a.end_start();
        a.end_stop(true);
    }

    /// A refused publish must leave the arbiter exactly where it found it, so
    /// the next press works. A fix that wedges the microphone shut would be a
    /// worse bug than the one it replaced.
    #[test]
    fn a_refusal_does_not_wedge_the_next_recording() {
        let a = Arbiter::new();

        a.user_wants_capture();
        let stale = a.begin_start();
        a.begin_stop();
        a.end_stop(false);
        a.user_done_with_capture();
        assert!(matches!(a.try_publish(stale), Publish::Refuse(_)));
        a.end_start();

        // Next press.
        a.user_wants_capture();
        let fresh = a.begin_start();
        assert_eq!(a.try_publish(fresh), Publish::Go);
        a.end_start();
    }

    #[test]
    fn refusal_slugs_are_stable_and_greppable() {
        assert_eq!(Refusal::UserIdle.slug(), "user_idle");
        assert_eq!(Refusal::Superseded.slug(), "superseded_by_newer_press");
    }
}
