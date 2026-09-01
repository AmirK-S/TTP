// TTP - Talk To Paste
// Voice Activity Detection (VAD) auto-stop.
//
// Reads the shared RMS bucket maintained by `audio_capture` and triggers
// a recording stop once a configurable run of silence has elapsed. Pure
// energy threshold — we don't pull in a webrtc-vad model because:
//   - Whisper itself handles silence well in the transcription step;
//     this VAD exists purely to free the user from having to remember
//     to release the hotkey after they finish dictating.
//   - A pure energy gate is testable on every host without bringing in
//     a heavy audio model + license + size hit (~2MB).
//
// Tuning:
//   - SILENCE_RMS_THRESHOLD is the RMS value below which we consider the
//     mic "silent". 0.005 is a few dB above mic-self-noise on a typical
//     MacBook built-in mic. Loud rooms (open offices) will still register
//     above this — VAD is a "you stopped speaking" detector, not a "the
//     room is quiet" detector.
//   - START_GRACE_MS: ignore silence in the first N ms after recording
//     starts. Without this, a user who taps the hotkey and takes a beat
//     before speaking would be auto-stopped before they got a word out.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::ShortcutState;

use crate::audio_capture;
use crate::shortcuts::handle_shortcut_event_public;

/// RMS below this is "silent". Stored as a float constant; the bucket is
/// f32 in [0.0, 1.0] (clamped at 4.0 by audio_capture::store_rms).
const SILENCE_RMS_THRESHOLD: f32 = 0.005;

/// Don't fire VAD inside this window after recording start. Users frequently
/// pause before speaking — we don't want to clip them.
const START_GRACE_MS: u64 = 1_200;

/// VAD loop tick rate. 100 ms keeps CPU near zero while staying responsive
/// to a 3-second silence threshold.
const TICK_MS: u64 = 100;

/// Lower / upper bounds on the user-configurable silence-secs setting.
pub const MIN_SILENCE_SECS: u32 = 1;
pub const MAX_SILENCE_SECS: u32 = 10;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static STOP_AT_NS: AtomicU64 = AtomicU64::new(0);
/// Set the instant VAD decides to cut the recording, cleared on arm.
///
/// Purely so `vad.disarmed` can say whether the watchdog stopped because it
/// fired or because the user released the key. Firing calls into
/// `handle_shortcut_event_public`, which re-enters `stop()` through
/// `set_state` before the loop clears `ACTIVE` — so without this the trace
/// would report every auto-stop as a manual one.
static FIRED: AtomicBool = AtomicBool::new(false);

/// Pure decision: given an RMS value and the consecutive silent-ticks
/// counter, return the new counter and whether VAD should fire.
///
/// Used only for testing — the real loop maintains the counter in local
/// scope. Extracted so the rules (threshold + tick-count math) live in
/// one place and can be exercised without spinning a thread.
pub fn vad_step(
    current_rms: f32,
    consecutive_silent_ticks: u32,
    required_ticks: u32,
) -> (u32, bool) {
    let new_count = if current_rms < SILENCE_RMS_THRESHOLD {
        consecutive_silent_ticks.saturating_add(1)
    } else {
        0
    };
    let fire = new_count >= required_ticks;
    (new_count, fire)
}

/// Convert the user-configurable silence-secs setting into the number of
/// VAD ticks that constitutes a fire. Bounds-checks the input.
pub fn required_ticks(silence_secs: u32) -> u32 {
    let clamped = silence_secs.clamp(MIN_SILENCE_SECS, MAX_SILENCE_SECS);
    ((clamped as u64 * 1_000) / TICK_MS) as u32
}

/// Start the VAD watchdog for the current recording. No-op if VAD is
/// disabled in settings or if already running. Reads the shared RMS bucket
/// maintained by `audio_capture::store_rms`.
pub fn start(app: AppHandle) {
    let settings = crate::settings::get_settings();
    if !settings.vad_auto_stop_enabled {
        // Deliberately silent about the common case: VAD is off by default and
        // a line per recording saying so is noise, not signal. `settings.snapshot`
        // on every dictation already records that the feature was off.
        return;
    }
    if ACTIVE.swap(true, Ordering::SeqCst) {
        // A second start against a live watchdog. Benign today, and exactly
        // the shape of the start/stop race that once left the microphone open.
        crate::trace::event("vad.armed", serde_json::json!({ "already_running": true }));
        return;
    }
    let required = required_ticks(settings.vad_silence_secs);
    FIRED.store(false, Ordering::SeqCst);
    crate::trace::event(
        "vad.armed",
        serde_json::json!({
            "silence_secs": settings.vad_silence_secs,
            "required_ticks": required,
            "grace_ms": START_GRACE_MS,
        }),
    );

    // Stamp the "no fire before this absolute time" deadline so the loop
    // doesn't have to re-read the clock to know when it started.
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let start_grace_deadline = now_ns + START_GRACE_MS * 1_000_000;
    STOP_AT_NS.store(start_grace_deadline, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        let mut silent_ticks: u32 = 0;
        while ACTIVE.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(TICK_MS));
            if !ACTIVE.load(Ordering::SeqCst) {
                break;
            }

            // Respect the start-grace window: pretend the first ~1.2s are
            // never silent regardless of mic state.
            let now_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            if now_ns < STOP_AT_NS.load(Ordering::Relaxed) {
                continue;
            }

            let rms = audio_capture::current_rms();
            let (new_count, fire) = vad_step(rms, silent_ticks, required);
            silent_ticks = new_count;

            if fire {
                crate::logging::log_info(&format!(
                    "[VAD] auto-stop after {}s of silence",
                    settings.vad_silence_secs
                ));
                // docs/tracing.md listed this as a blind spot in as many
                // words: "the decision to cut a recording short is not
                // recorded". A user whose sentence was truncated mid-thought
                // had no way to tell VAD from a dropped hotkey release, and
                // the two have opposite fixes.
                crate::trace::event(
                    "vad.fired",
                    serde_json::json!({
                        "silence_secs": settings.vad_silence_secs,
                        "silent_ticks": new_count,
                        "rms": rms,
                        "threshold": SILENCE_RMS_THRESHOLD,
                    }),
                );
                FIRED.store(true, Ordering::SeqCst);
                // Drive the same code path as a hotkey release. shortcuts
                // already handles "stop while recording", "stop while
                // hands-free", and the post-stop transcription kickoff —
                // we don't need a VAD-specific variant.
                handle_shortcut_event_public(&app, ShortcutState::Released);
                ACTIVE.store(false, Ordering::SeqCst);
                break;
            }
        }
    });
}

/// Stop the VAD loop. Called from `state.rs::set_state` whenever the
/// recording state leaves Recording.
pub fn stop() {
    // Only report a stop that actually stopped something. `set_state` calls
    // this on every transition that is not Recording, most of which had no
    // watchdog running.
    if ACTIVE.swap(false, Ordering::SeqCst) {
        crate::trace::event(
            "vad.disarmed",
            serde_json::json!({ "fired": FIRED.load(Ordering::SeqCst) }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_rms_increments_counter() {
        let (count, fire) = vad_step(0.001, 0, 30);
        assert_eq!(count, 1);
        assert!(!fire);
    }

    #[test]
    fn loud_rms_resets_counter() {
        let (count, fire) = vad_step(0.2, 25, 30);
        assert_eq!(count, 0);
        assert!(!fire);
    }

    #[test]
    fn rms_exactly_at_threshold_resets_counter() {
        // Strictly less than threshold counts as silent; equal does not.
        let (count, _) = vad_step(SILENCE_RMS_THRESHOLD, 5, 30);
        assert_eq!(count, 0);
    }

    #[test]
    fn fires_once_required_ticks_reached() {
        let (count, fire) = vad_step(0.0, 29, 30);
        assert_eq!(count, 30);
        assert!(fire);
    }

    #[test]
    fn saturating_add_does_not_overflow_u32() {
        let (count, _) = vad_step(0.0, u32::MAX, 30);
        assert_eq!(count, u32::MAX);
    }

    #[test]
    fn required_ticks_with_default_3s_yields_30() {
        assert_eq!(required_ticks(3), 30);
    }

    #[test]
    fn required_ticks_clamps_below_min() {
        assert_eq!(required_ticks(0), required_ticks(MIN_SILENCE_SECS));
    }

    #[test]
    fn required_ticks_clamps_above_max() {
        assert_eq!(required_ticks(99), required_ticks(MAX_SILENCE_SECS));
    }

    #[test]
    fn required_ticks_scales_linearly() {
        // 5 seconds * 1000 ms / 100 ms = 50 ticks.
        assert_eq!(required_ticks(5), 50);
        // 1 second.
        assert_eq!(required_ticks(1), 10);
    }
}
