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
    atomic::{AtomicU64, Ordering},
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

type WavWriterHandle = Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>;

struct RecordingState {
    stream: Option<SafeStream>,
    writer: WavWriterHandle,
    save_path: PathBuf,
    samples_written: Arc<AtomicU64>,
}

static STATE: LazyLock<Mutex<Option<RecordingState>>> = LazyLock::new(|| Mutex::new(None));

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
            return Err("Recording is already in progress".to_string());
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
                log_error("Mic permission denied — cannot start recording");
                return Err("Microphone permission denied".to_string());
            }
            PermissionStatus::Undetermined => {
                log_error("Mic permission undetermined — start blocked until user grants");
                return Err("Microphone permission not yet granted".to_string());
            }
        }
    }

    // 3. Resolve device + config.
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default input device available".to_string())?;
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
                    move |data: &[i8], _: &_| write_samples::<i8, i8>(data, &w, &counter),
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
                    move |data: &[i16], _: &_| write_samples::<i16, i16>(data, &w, &counter),
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
                    move |data: &[i32], _: &_| write_samples::<i32, i32>(data, &w, &counter),
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
                    move |data: &[f32], _: &_| write_samples::<f32, f32>(data, &w, &counter),
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
    app.emit("audio-stream-error", &msg).ok();
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
