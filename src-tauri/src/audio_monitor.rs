// TTP - Talk To Paste
// Real-time audio level monitoring for the pill waveform visualisation.
//
// History: until v3.1 this module owned a SECOND cpal input stream opened
// in parallel with the WAV-writing stream in `audio_capture`. On CoreAudio
// (and several WASAPI drivers) the second `default_input_device()` open
// against the same device can fail or degrade silently — symptom was the
// intermittent "recording captured nothing" failure mode flagged by the
// audit as high.
//
// v3.1: there is no longer a second stream. `audio_capture` computes RMS
// inside its existing write callback and publishes via
// `audio_capture::current_rms()`. This module just polls that bucket at
// ~30fps and emits `audio-level` events.
//
// Side benefit: this thread no longer needs cpal at all — it's a pure
// "read atomic, emit event, sleep" loop. Easy to reason about, no stream
// lifecycle bugs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::ShortcutState;

/// Hard upper bound on a single recording session, in seconds. Past this
/// point the watchdog forces a stop so a user who walked away mid-session
/// (forgot to release the hotkey, double-tap entered hands-free and never
/// re-engaged) doesn't end up with a 30-minute file that fails Groq's
/// 25 MB upload limit downstream.
///
/// Also keeps a runaway hands-free session from hammering CPU forever.
pub const MAX_RECORDING_SECS: u64 = 5 * 60;

/// Whether the monitor loop is currently active.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Start emitting `audio-level` events at ~30fps from the shared RMS bucket
/// that `audio_capture` updates inside its WAV-writing callback.
///
/// Safe to call multiple times. Subsequent calls are no-ops while the loop
/// is already running.
pub fn start(app: AppHandle) {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    // Use Tauri's blocking runtime so the spawn binds to the active reactor.
    // Bare `std::thread::spawn` historically panicked here with "no reactor
    // running" on macOS (TTP-B) when the emit fired from a thread detached
    // from the Tauri runtime.
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = run(&app) {
            crate::logging::log_warn(&format!("[AudioMonitor] Failed: {}", e));
        }
        ACTIVE.store(false, Ordering::SeqCst);
    });
}

/// Stop the audio level monitor. The loop checks ACTIVE between ticks and
/// exits within ~33ms.
pub fn stop() {
    ACTIVE.store(false, Ordering::SeqCst);
}

/// Main monitor loop: read the shared RMS bucket, emit normalised levels
/// to the pill window, and enforce the hard recording-duration ceiling.
fn run(app: &AppHandle) -> Result<(), String> {
    let started_at = Instant::now();
    while ACTIVE.load(Ordering::SeqCst) {
        // Hard duration cap. We check from the audio_monitor (always live
        // during Recording) so the cap applies regardless of whether VAD
        // is opt-in or not. If the user crossed the threshold, force a
        // stop via the same code path as a manual release — the pipeline
        // then transcribes whatever we have and returns to Idle.
        if started_at.elapsed().as_secs() >= MAX_RECORDING_SECS {
            crate::logging::log_warn(&format!(
                "[AudioMonitor] hard duration cap reached ({}s), forcing stop",
                MAX_RECORDING_SECS
            ));
            crate::shortcuts::handle_shortcut_event_public(app, ShortcutState::Released);
            ACTIVE.store(false, Ordering::SeqCst);
            break;
        }

        let rms = crate::audio_capture::current_rms();
        // Amplify RMS (typical speech RMS sits around 0.0-0.2) and cap at 1.0
        // so the pill's transform-scale stays inside its bounded envelope.
        let normalized = (rms * 18.0).min(1.0);
        app.emit("audio-level", normalized).ok();
        std::thread::sleep(std::time::Duration::from_millis(33));
    }
    Ok(())
}
