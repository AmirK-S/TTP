// TTP - Talk To Paste
// In-house cpal-based microphone capture.
//
// Replaces `tauri-plugin-mic-recorder` v2, which had three structural defects
// that caused 11/12 recordings on the user's machine to produce 68-byte WAV
// files (header only, zero audio samples):
//
//   1. Audio callback used `try_lock()` and silently dropped any sample
//      batch that hit lock contention (commands.rs:277).
//   2. `write_sample(...).ok()` swallowed every hound write error
//      (commands.rs:281).
//   3. `err_fn = |e| eprintln!()` — fatal cpal stream errors were logged to
//      stderr and never surfaced to the user (commands.rs:145).
//
// Plus the plugin built every cpal stream WITHOUT first checking macOS
// microphone permission, so on the very common "permission silently revoked
// after an unsigned-app update" path, the user got a successful start, an
// empty WAV at stop, and a generic "Transcription failed" pill downstream.
//
// This module:
//   - Pre-flights AVCaptureDevice authorization status before building the
//     stream (returns the existing `error.microphone_permission_denied` pill
//     instead of a silent failure).
//   - Uses blocking `lock()` in the audio callback — contention is bounded
//     to one short `take()` in stop_recording, so blocking briefly is
//     correct and never loses samples.
//   - Propagates cpal stream errors to the frontend via the existing
//     `audio-stream-error` event (already wired in useRecordingControl.ts).
//   - Counts samples written via an `AtomicU64` so the pipeline can short-
//     circuit on truly-empty recordings without re-reading the WAV.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc, LazyLock, Mutex,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, Stream,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use tauri::{command, AppHandle, Emitter, Manager};

use crate::logging::{log_error, log_info};

/// cpal::Stream isn't Send on macOS (CoreAudio limitation). We only ever
/// touch the stream from the Tauri command thread — building it in start
/// and dropping it in stop — and the underlying CoreAudio callback runs on
/// its own thread regardless. Mirrors the upstream plugin's wrapper.
struct SafeStream(Stream);
unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

/// Public payload type for `list_audio_input_devices`. Field order is
/// preserved for JSON serialisation so the renderer can render a dropdown
/// directly from the response.
#[derive(serde::Serialize)]
pub struct AudioInputDeviceInfo {
    /// Device name as cpal reports it. Used as the key the user persists in
    /// Settings.audio_device_name. Stable enough across reboots for our
    /// purposes (CoreAudio renames are rare and the fallback handles them).
    pub name: String,
    /// Whether this device is the OS-level default input. The UI shows a
    /// "(default)" suffix so a user who picked "default" intentionally
    /// can see which physical device that resolves to.
    pub is_default: bool,
}

/// Enumerate every cpal input device on the system. Skips devices whose
/// `name()` call errors (rare; usually means the device was unplugged
/// between enumeration and the name() lookup).
#[command]
pub fn list_audio_input_devices() -> Result<Vec<AudioInputDeviceInfo>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let mut out = Vec::new();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to list input devices: {}", e))?;
    for device in devices {
        if let Ok(name) = device.name() {
            let is_default = default_name.as_deref() == Some(name.as_str());
            out.push(AudioInputDeviceInfo { name, is_default });
        }
    }
    Ok(out)
}

/// Resolve the cpal input device to record from. When `preferred_name` is
/// Some, walk the device list and match by name. Falls back to the system
/// default if not found (logged as info — common on hot-unplug events).
fn resolve_input_device(
    host: &cpal::Host,
    preferred_name: &Option<String>,
) -> Result<cpal::Device, String> {
    if let Some(name) = preferred_name.as_deref().filter(|s| !s.is_empty()) {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if device.name().as_deref() == Ok(name) {
                    return Ok(device);
                }
            }
        }
        log_info(&format!(
            "[AudioCapture] preferred input device '{}' not found, falling back to default",
            name
        ));
    }
    host.default_input_device()
        .ok_or_else(|| "No default input device available".to_string())
}

type WavWriterHandle = Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>;

struct RecordingState {
    stream: Option<SafeStream>,
    writer: WavWriterHandle,
    save_path: PathBuf,
    samples_written: Arc<AtomicU64>,
}

static STATE: LazyLock<Mutex<Option<RecordingState>>> = LazyLock::new(|| Mutex::new(None));

/// Most recent RMS level computed inside the audio callback, stored as the
/// bit-pattern of an `f32` in [0.0, 1.0].
///
/// Why this exists: before v3.1 the pill waveform was driven by a SECOND cpal
/// input stream owned by `audio_monitor`, opened in parallel with the WAV-
/// writing stream. CoreAudio (and several WASAPI drivers) silently downgrade
/// or fail the second `default_input_device()` open when the same device is
/// already in use by the same process — the symptom was an intermittent
/// "recording silently captured nothing" failure that the audit flagged as
/// high.
///
/// The fix: compute RMS inside the existing WAV-writing callback (one float
/// loop per buffer, negligible vs the WAV write itself), publish via this
/// atomic, and have `audio_monitor` poll the bucket on a 30fps tick instead
/// of owning its own cpal stream.
static RMS_BUCKET: AtomicU32 = AtomicU32::new(0u32);

/// Read the current RMS level (in [0.0, 1.0]) for the pill visualisation.
/// Returns 0.0 when no capture is in progress (the bucket is reset on every
/// start_recording, so a stale level can't leak across sessions).
pub fn current_rms() -> f32 {
    f32::from_bits(RMS_BUCKET.load(Ordering::Relaxed))
}

fn store_rms(value: f32) {
    // Clamp to a sane range so a freakishly loud sample can't break the
    // pill's transform-based animation.
    let clamped = value.clamp(0.0, 4.0);
    RMS_BUCKET.store(clamped.to_bits(), Ordering::Relaxed);
}

fn reset_rms() {
    RMS_BUCKET.store(0u32, Ordering::Relaxed);
}

fn rms_from_i8(data: &[i8]) -> f32 {
    if data.is_empty() { return 0.0; }
    let sum: f32 = data.iter().map(|&s| {
        let f = s as f32 / 128.0;
        f * f
    }).sum();
    (sum / data.len() as f32).sqrt()
}

fn rms_from_i16(data: &[i16]) -> f32 {
    if data.is_empty() { return 0.0; }
    let sum: f32 = data.iter().map(|&s| {
        let f = s as f32 / 32_768.0;
        f * f
    }).sum();
    (sum / data.len() as f32).sqrt()
}

fn rms_from_i32(data: &[i32]) -> f32 {
    if data.is_empty() { return 0.0; }
    let sum: f32 = data.iter().map(|&s| {
        let f = s as f32 / 2_147_483_648.0;
        f * f
    }).sum();
    (sum / data.len() as f32).sqrt()
}

fn rms_from_f32(data: &[f32]) -> f32 {
    if data.is_empty() { return 0.0; }
    let sum: f32 = data.iter().map(|s| s * s).sum();
    (sum / data.len() as f32).sqrt()
}

#[cfg(test)]
mod rms_tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn empty_buffers_return_zero() {
        assert_eq!(rms_from_f32(&[]), 0.0);
        assert_eq!(rms_from_i16(&[]), 0.0);
        assert_eq!(rms_from_i32(&[]), 0.0);
        assert_eq!(rms_from_i8(&[]), 0.0);
    }

    #[test]
    fn silence_returns_zero() {
        let zeros = vec![0.0f32; 1024];
        assert_eq!(rms_from_f32(&zeros), 0.0);
    }

    #[test]
    fn dc_offset_returns_magnitude() {
        // RMS of a constant signal is just its absolute value.
        let const_half = vec![0.5f32; 100];
        assert!(approx(rms_from_f32(&const_half), 0.5));
    }

    #[test]
    fn f32_full_scale_alternating_sample_returns_one() {
        // Square wave at full-scale: every sample is ±1. RMS = 1.0.
        let mut wave = Vec::with_capacity(100);
        for i in 0..100 { wave.push(if i % 2 == 0 { 1.0 } else { -1.0 }); }
        assert!(approx(rms_from_f32(&wave), 1.0));
    }

    #[test]
    fn i16_full_scale_alternating_sample_returns_near_one() {
        let mut wave: Vec<i16> = Vec::with_capacity(100);
        for i in 0..100 {
            wave.push(if i % 2 == 0 { 32_767 } else { -32_768 });
        }
        // Normalization divides by 32 768 — full scale lands at ~1.0.
        assert!(rms_from_i16(&wave) > 0.999);
        assert!(rms_from_i16(&wave) <= 1.001);
    }

    #[test]
    fn i8_normalization_uses_128() {
        // Constant +64 should normalize to 64/128 = 0.5.
        let buf = vec![64i8; 32];
        assert!(approx(rms_from_i8(&buf), 0.5));
    }

    #[test]
    fn store_and_read_rms_round_trip() {
        store_rms(0.42);
        assert!(approx(current_rms(), 0.42));
        reset_rms();
        assert_eq!(current_rms(), 0.0);
    }

    #[test]
    fn store_rms_clamps_obnoxious_values() {
        store_rms(99.0);
        // Clamp ceiling is 4.0 — anything higher gets pinned.
        assert!(current_rms() <= 4.0);
        store_rms(-1.0);
        // Negative inputs clamp to 0.0 (the floor).
        assert_eq!(current_rms(), 0.0);
        reset_rms();
    }
}

/// Start capturing microphone input to a fresh WAV file in the app data dir.
///
/// Pre-flights microphone permission and rejects with a stable error string
/// (containing "permission") so the frontend's existing handler maps it to
/// the `error.microphone_permission_denied` pill.
#[command]
pub async fn start_recording<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // 1. Reject if a recording is already in progress.
    {
        let state = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
        if state.is_some() {
            // Error codes — the JS-side `translateRustMessage` maps these
            // to the user's locale via the `error.*` namespace. Raw English
            // strings here would skip the parity check + ship as a missing-key
            // tag on FR pills.
            return Err("error.recording_already_in_progress".to_string());
        }
    }

    // 2. Pre-flight mic permission. On macOS an unsigned-app update can
    //    silently flip the user from Authorized → NotDetermined; cpal will
    //    still build a stream but the callback never fires. We catch that
    //    here so the user gets a clear pill instead of an empty WAV.
    #[cfg(target_os = "macos")]
    {
        use crate::permissions::{check_microphone_permission_impl, PermissionStatus};
        match check_microphone_permission_impl() {
            PermissionStatus::Granted => { /* good */ }
            PermissionStatus::Denied => {
                log_error("Mic permission denied - cannot start recording");
                return Err("error.microphone_permission_denied".to_string());
            }
            PermissionStatus::Undetermined => {
                log_error("Mic permission undetermined - start blocked until user grants");
                return Err("error.microphone_permission_undetermined".to_string());
            }
        }
    }

    // 3. Resolve device + config. If the user picked an explicit input
    //    device in Settings, look it up by name and fall back to the system
    //    default if it isn't present (e.g. AirPods got disconnected since
    //    the user set the preference). Without the fallback, a missing
    //    device would surface as a generic "no device available" error
    //    that the user wouldn't know how to fix.
    let host = cpal::default_host();
    let device = resolve_input_device(&host, &crate::settings::get_settings().audio_device_name)?;
    let supported_config = device
        .default_input_config()
        .map_err(|e| format!("No default input config: {}", e))?;
    let config = supported_config.clone();

    // 4. Build the output WAV path and open the writer.
    let save_path = build_save_path(&app)?;
    let spec = wav_spec_from_config(&supported_config);
    let writer_handle: WavWriterHandle = Arc::new(Mutex::new(Some(
        WavWriter::create(&save_path, spec)
            .map_err(|e| format!("Failed to create WAV writer: {}", e))?,
    )));

    log_info(&format!(
        "[AudioCapture] starting: device={:?} rate={} ch={} fmt={:?} → {}",
        device.name().unwrap_or_else(|_| "<unknown>".into()),
        config.sample_rate().0,
        config.channels(),
        config.sample_format(),
        save_path.display()
    ));

    // 5. Build the cpal input stream. Cloned handles go into the callback;
    //    err_fn forwards to the frontend (and Sentry breadcrumbs).
    let samples_written = Arc::new(AtomicU64::new(0));
    // Reset the RMS bucket so the pill doesn't briefly mirror the previous
    // session's last sample on session start.
    reset_rms();
    let stream = build_stream(&device, &supported_config, &writer_handle, &samples_written, &app)?;
    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {}", e))?;

    // 6. Stash everything in shared state for stop_recording to pick up.
    let mut state = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
    *state = Some(RecordingState {
        stream: Some(SafeStream(stream)),
        writer: writer_handle,
        save_path,
        samples_written,
    });
    Ok(())
}

/// Stop the active recording, finalise the WAV file, and return its path.
///
/// Returns `Err("Empty recording: ...")` if zero audio samples were captured
/// (the WAV file is still finalised + path returned so callers can inspect /
/// log it). The pipeline already has a second defense via `wav_duration_secs`
/// — both layers stay so a bug in either still surfaces the right error.
#[command]
pub async fn stop_recording() -> Result<PathBuf, String> {
    let mut state_guard = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
    let state = state_guard
        .take()
        .ok_or_else(|| "No recording in progress".to_string())?;
    // Release the outer STATE lock before any blocking work so concurrent
    // status reads from other commands don't pile up behind us.
    drop(state_guard);

    // Dropping the stream stops the cpal callback. Any in-flight callback
    // will finish writing its current buffer (it holds the writer lock
    // briefly) before this drop completes; that's the correct behaviour —
    // we want every sample the device gave us.
    drop(state.stream);
    // The pill subscribes to RMS via current_rms(); reset it now so it
    // doesn't briefly render the last captured frame after the stream ends.
    reset_rms();

    // Finalise the WAV — flushes BufWriter and patches the RIFF/data chunk
    // sizes. Errors here mean the file on disk is unusable, so we surface
    // them rather than swallowing.
    if let Some(writer) = state
        .writer
        .lock()
        .map_err(|e| format!("writer lock poisoned: {}", e))?
        .take()
    {
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalise WAV: {}", e))?;
    }

    let written = state.samples_written.load(Ordering::SeqCst);
    log_info(&format!(
        "[AudioCapture] stopped: {} samples → {}",
        written,
        state.save_path.display()
    ));

    if written == 0 {
        log_error(
            "[AudioCapture] zero samples captured — cpal stream produced no data. \
             Likely cause: mic permission silently revoked, or mic held exclusive \
             by another process.",
        );
        // We still return the path so the pipeline's downstream
        // wav_duration_secs check produces the same recording_empty pill —
        // single source of truth for the user-facing message.
    }

    Ok(state.save_path)
}

fn build_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    writer: &WavWriterHandle,
    samples_written: &Arc<AtomicU64>,
    app: &AppHandle<impl tauri::Runtime>,
) -> Result<Stream, String> {
    let config: cpal::StreamConfig = supported.config();
    let err_app = app.clone();
    let err_fn = move |e: cpal::StreamError| handle_stream_error(&err_app, e);

    let stream = match supported.sample_format() {
        cpal::SampleFormat::I8 => {
            let w = writer.clone();
            let counter = samples_written.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i8], _: &_| {
                        store_rms(rms_from_i8(data));
                        write_samples::<i8, i8>(data, &w, &counter);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream(i8): {}", e))?
        }
        cpal::SampleFormat::I16 => {
            let w = writer.clone();
            let counter = samples_written.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _: &_| {
                        store_rms(rms_from_i16(data));
                        write_samples::<i16, i16>(data, &w, &counter);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream(i16): {}", e))?
        }
        cpal::SampleFormat::I32 => {
            let w = writer.clone();
            let counter = samples_written.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i32], _: &_| {
                        store_rms(rms_from_i32(data));
                        write_samples::<i32, i32>(data, &w, &counter);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream(i32): {}", e))?
        }
        cpal::SampleFormat::F32 => {
            let w = writer.clone();
            let counter = samples_written.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _: &_| {
                        store_rms(rms_from_f32(data));
                        write_samples::<f32, f32>(data, &w, &counter);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream(f32): {}", e))?
        }
        fmt => return Err(format!("Unsupported sample format: {:?}", fmt)),
    };
    Ok(stream)
}

/// Write a callback's worth of samples to the WAV file.
///
/// Critical difference vs upstream plugin: uses blocking `lock()`. The only
/// other writer-mutex holder is `stop_recording`, which calls `take()` once
/// and releases — contention is bounded to a single buffer's worth of
/// time, well under any audio deadline. Dropping samples silently (the
/// plugin's bug) is far worse than a single 10 ms hiccup on stop.
fn write_samples<T, U>(input: &[T], writer: &WavWriterHandle, counter: &Arc<AtomicU64>)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    let mut guard = match writer.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            // Lock poisoned means a previous panic in stop_recording or
            // start_recording while holding it. Recover the inner value
            // and keep writing — losing samples here is the exact bug we
            // were sent to fix.
            poisoned.into_inner()
        }
    };
    if let Some(writer) = guard.as_mut() {
        let mut written = 0u64;
        for &sample in input.iter() {
            let s: U = U::from_sample(sample);
            if writer.write_sample(s).is_ok() {
                written += 1;
            }
        }
        counter.fetch_add(written, Ordering::Relaxed);
    }
}

/// Forward a fatal cpal stream error to the frontend via the existing
/// `audio-stream-error` event (already handled in useRecordingControl.ts).
fn handle_stream_error<R: tauri::Runtime>(app: &AppHandle<R>, e: cpal::StreamError) {
    let msg = e.to_string();
    log_error(&format!("[AudioCapture] cpal stream error: {}", msg));
    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("audio_capture".into()),
        level: sentry::Level::Warning,
        message: Some(format!("cpal capture stream error: {}", msg)),
        ..Default::default()
    });
    // Source-tagged payload (mirrors audio_monitor::handle_stream_error). The
    // capture stream is the one whose failure means data loss — the frontend
    // routes this branch to "abort recording + toast" instead of the silent
    // monitor degradation path.
    let payload = serde_json::json!({ "source": "capture", "message": msg });
    app.emit("audio-stream-error", payload).ok();
}

fn build_save_path<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {}", e))?
        .join("recordings");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create recordings dir: {}", e))?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    Ok(dir.join(format!("recording_{}.wav", timestamp)))
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> WavSpec {
    let sample_format = if config.sample_format().is_float() {
        SampleFormat::Float
    } else {
        SampleFormat::Int
    };
    WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: (config.sample_format().sample_size() * 8) as u16,
        sample_format,
    }
}
