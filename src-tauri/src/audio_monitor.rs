// TTP - Talk To Paste
// Real-time audio level monitoring for pill wave visualization
//
// Opens a separate cpal input stream to compute RMS volume levels
// and emits them as Tauri events (~30fps) for the pill window bars.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Whether the monitor is currently active
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Start monitoring microphone input levels.
/// Spawns a background thread that emits `audio-level` events at ~30fps.
/// Safe to call multiple times — subsequent calls are no-ops while active.
pub fn start(app: AppHandle) {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    // Use Tauri's blocking runtime so cpal callbacks live inside the
    // tokio reactor (raw std::thread::spawn caused "no reactor running"
    // panics in TTP-B; same latent risk here).
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = run(&app) {
            eprintln!("[AudioMonitor] Failed: {}", e);
        }
        ACTIVE.store(false, Ordering::SeqCst);
    });
}

/// Stop the audio level monitor.
pub fn stop() {
    ACTIVE.store(false, Ordering::SeqCst);
}

/// Main monitor loop: opens cpal input stream, reads RMS, emits events.
fn run(app: &AppHandle) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No input device available")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("No input config: {}", e))?;

    // Shared RMS level (f32 stored as u32 bits for atomic access)
    let level = Arc::new(AtomicU32::new(0f32.to_bits()));
    let level_writer = level.clone();

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let lw = level_writer;
            let err_app = app.clone();
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[f32], _| {
                        let rms = rms_f32(data);
                        lw.store(rms.to_bits(), Ordering::Relaxed);
                    },
                    move |e| handle_stream_error(&err_app, e),
                    None,
                )
                .map_err(|e| format!("Failed to build F32 stream: {}", e))?
        }
        cpal::SampleFormat::I16 => {
            let lw = level_writer;
            let err_app = app.clone();
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[i16], _| {
                        let rms = rms_i16(data);
                        lw.store(rms.to_bits(), Ordering::Relaxed);
                    },
                    move |e| handle_stream_error(&err_app, e),
                    None,
                )
                .map_err(|e| format!("Failed to build I16 stream: {}", e))?
        }
        format => return Err(format!("Unsupported sample format: {:?}", format)),
    };

    stream.play().map_err(|e| format!("Failed to play: {}", e))?;

    // Emit audio level events at ~30fps
    while ACTIVE.load(Ordering::SeqCst) {
        let rms = f32::from_bits(level.load(Ordering::Relaxed));
        // Amplify RMS (raw mic RMS is typically 0.0-0.2 for speech)
        let normalized = (rms * 18.0).min(1.0);
        app.emit("audio-level", normalized).ok();
        std::thread::sleep(std::time::Duration::from_millis(33));
    }

    // Stream is dropped here, stopping capture
    Ok(())
}

/// Handle a fatal cpal input-stream error (typically: another process — e.g. macOS
/// Dictation on F5 — took exclusive access of the mic, so our stream was kicked).
///
/// We can't recover the stream from inside the callback, so we:
///   1. Flip ACTIVE off so the 30fps emit loop in `run()` exits and drops the stream
///      cleanly instead of pumping zeros forever.
///   2. Emit `audio-stream-error` so the frontend can reset the recording state
///      machine to Idle and toast the user (instead of leaving them with a stuck
///      pill and an empty audio file).
///   3. Drop a Sentry breadcrumb so we get visibility on Sentry when consent is on.
fn handle_stream_error(app: &AppHandle, e: cpal::StreamError) {
    let msg = e.to_string();
    eprintln!("[AudioMonitor] Stream error: {}", msg);

    // Stop the emit loop — it would otherwise keep firing audio-level=0 forever
    // and leave the pill visible with the bars stuck flat.
    ACTIVE.store(false, Ordering::SeqCst);

    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("audio_stream".into()),
        level: sentry::Level::Warning,
        message: Some(format!("cpal input stream error: {}", msg)),
        ..Default::default()
    });

    app.emit("audio-stream-error", msg).ok();
}

fn rms_f32(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt()
}

fn rms_i16(data: &[i16]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum: f32 = data
        .iter()
        .map(|&s| {
            let f = s as f32 / 32768.0;
            f * f
        })
        .sum();
    (sum / data.len() as f32).sqrt()
}
