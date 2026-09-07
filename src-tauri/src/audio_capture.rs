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
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc, LazyLock, Mutex,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, Stream,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use tauri::{command, AppHandle, Emitter, Manager};

use crate::logging::{log_error, log_info, log_warn};

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
                // cpal's `Device::name()` returns `Result<String, DeviceNameError>`;
                // unwrap the success arm explicitly and compare strings rather
                // than relying on Result comparison (which fails to compile
                // because the error arms aren't PartialEq-compatible).
                if device.name().ok().as_deref() == Some(name) {
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

/// Name of the input device that served the most recent recording.
///
/// The transcription pipeline reads this when a capture comes back as
/// digital silence. "Every sample was zero" has at least two very different
/// causes — a Bluetooth headset that connected but never streamed, versus a
/// revoked microphone permission or another process holding the device — and
/// the device name is what separates them. Without it, both land in the trace
/// as an identical `dead_capture` line and the user is back to guessing.
static LAST_CAPTURE_DEVICE: Mutex<Option<String>> = Mutex::new(None);

/// The input device used for the most recent recording, if one has run.
pub fn last_capture_device() -> Option<String> {
    LAST_CAPTURE_DEVICE.lock().ok().and_then(|g| g.clone())
}

/// Name of the current OS-default input device, or `None` if there isn't one.
///
/// Read again at stop so a device that changed mid-recording is visible:
/// AirPods connecting (or going to sleep) while the user is talking moves the
/// default out from under an already-open stream.
fn current_default_input_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}

type WavWriterHandle = Arc<Mutex<Option<WavWriter<std::io::BufWriter<std::fs::File>>>>>;

struct RecordingState {
    stream: Option<SafeStream>,
    writer: WavWriterHandle,
    save_path: PathBuf,
    samples_written: Arc<AtomicU64>,
}

static STATE: LazyLock<Mutex<Option<RecordingState>>> = LazyLock::new(|| Mutex::new(None));

/// Starting a capture is not instant: it checks microphone authorisation,
/// resolves the device, queries its config and creates a WAV writer, and on
/// Bluetooth that can take most of a second. `stop_recording` waits 400 ms for
/// the driver to drain and then takes STATE, so a short enough tap has the two
/// crossing in mid-air.
///
/// Observed 2026-08-30, a 60 ms tap:
///
///     .386 press    → Recording
///     .446 release  → Processing
///     .852 capture.stop_failed "No recording in progress"
///     .930 capture.start                     ← start finishes after the stop
///
/// The recording was lost, which is bad, and the stream was left running with
/// nobody holding a handle to stop it, which is worse: the microphone stayed
/// live for 10 h 57 m, until the next launch. For an app whose pitch is that
/// your voice does not leave your machine, a hot mic nobody asked for is the
/// one bug that must not exist.
///
/// Two halves close it, and both are needed:
///
///   * **This one.** `stop_recording` waits for an in-flight start before
///     concluding it has nothing to stop, so the recording survives rather
///     than merely not leaking. It is the good outcome, and it is a timing
///     guess: it only helps when the start had already begun.
///   * **`crate::capture_arbiter`.** A start that has built a stream asks
///     permission before publishing it, and the `Idle` transition reclaims
///     anything still live. That half has no clock in it and cannot be
///     defeated by a late poll, a slow device, or a second press — see the
///     three holes named in that module's header.
///
/// Whether a start is in flight now lives in the arbiter, so one fact has one
/// home and the two halves cannot disagree about it.

/// How long `stop_recording` will wait for an in-flight start to publish.
///
/// Generous, because the alternative to waiting is the orphaned stream above.
/// It only ever elapses in full when a start genuinely failed, and the
/// existing "No recording in progress" path then handles it as before.
const START_SETTLE_TIMEOUT_MS: u64 = 3_000;
const START_SETTLE_POLL_MS: u64 = 20;

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

/// Whether any non-zero audio has arrived since the current capture began.
static SIGNAL_SEEN: AtomicBool = AtomicBool::new(false);

/// Wall-clock ms at which the current capture began. Zero when idle.
static CAPTURE_STARTED_MS: AtomicU64 = AtomicU64::new(0);

/// How long a capture may deliver nothing but zeros before we say so.
///
/// A Bluetooth device that is still handing itself over from a phone needs
/// about a second, and interrupting that would be worse than useless. But
/// there is no honest reading of two seconds of pure digital silence other
/// than "this microphone is not going to produce anything", and on
/// 2026-08-30 that state lasted **twenty-one seconds** while the user talked:
/// 503520 samples at 24 kHz, every one of them zero, discovered only after
/// they let go of the key.
///
/// Telling them at second two costs one interrupted sentence. Not telling
/// them costs the whole thing, and the trust that the app was listening.
const DEAD_INPUT_GRACE_MS: u64 = 2_000;

/// How long the current capture has been delivering pure silence, if it has
/// passed the grace period and nothing has arrived at all.
///
/// `None` while idle, inside the grace window, or once any signal has been
/// seen — so a recording that starts slowly and then works never reports.
pub fn dead_input_elapsed_ms() -> Option<u64> {
    if SIGNAL_SEEN.load(Ordering::Relaxed) {
        return None;
    }
    let started = CAPTURE_STARTED_MS.load(Ordering::Relaxed);
    if started == 0 {
        return None;
    }
    let elapsed = now_ms().saturating_sub(started);
    (elapsed >= DEAD_INPUT_GRACE_MS).then_some(elapsed)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn store_rms(value: f32) {
    if value > 0.0 {
        SIGNAL_SEEN.store(true, Ordering::Relaxed);
    }
    // Clamp to a sane range so a freakishly loud sample can't break the
    // pill's transform-based animation.
    let clamped = value.clamp(0.0, 4.0);
    RMS_BUCKET.store(clamped.to_bits(), Ordering::Relaxed);
}

fn reset_rms() {
    RMS_BUCKET.store(0u32, Ordering::Relaxed);
}

/// Arm dead-input detection for a capture that is about to begin.
fn arm_dead_input_watch() {
    SIGNAL_SEEN.store(false, Ordering::Relaxed);
    CAPTURE_STARTED_MS.store(now_ms(), Ordering::Relaxed);
}

/// Disarm it. Called on stop so an idle app never reports a dead input.
fn disarm_dead_input_watch() {
    CAPTURE_STARTED_MS.store(0, Ordering::Relaxed);
    SIGNAL_SEEN.store(false, Ordering::Relaxed);
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

    /// Serialises the two tests that mutate the process-global RMS bucket.
    ///
    /// `store_rms` / `current_rms` / `reset_rms` operate on one `AtomicU32`
    /// shared by the whole process. Two tests writing it concurrently under
    /// the default parallel runner clobber each other: measured failing about
    /// one run in three with just those two tests selected. It was latent
    /// before — the suite happened not to schedule them together — which is
    /// exactly the shape of flake that gets re-run until it passes and then
    /// believed.
    static BUCKET: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        let _guard = BUCKET.lock().unwrap_or_else(|e| e.into_inner());
        store_rms(0.42);
        assert!(approx(current_rms(), 0.42));
        reset_rms();
        assert_eq!(current_rms(), 0.0);
    }

    #[test]
    fn store_rms_clamps_obnoxious_values() {
        let _guard = BUCKET.lock().unwrap_or_else(|e| e.into_inner());
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
/// Begin capturing audio.
///
/// A thin wrapper around [`start_recording_inner`] whose only job is to make
/// failure observable. The inner function has nine early exits — a poisoned
/// lock, two microphone-permission states, device config, WAV writer, stream
/// build — and every one of them means the user pressed the hotkey, spoke,
/// and got nothing. Tracing them individually would leave the next one added
/// untraced by default; tracing the boundary cannot miss any.
#[command]
pub async fn start_recording<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let result = start_recording_inner(app).await;
    if let Err(ref e) = result {
        log_error(&format!("[AudioCapture] start_recording failed: {}", e));
        crate::trace::event("capture.start_failed", serde_json::json!({ "error": e }));
    }
    result
}

async fn start_recording_inner<R: tauri::Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // Released on every exit path below — including the nine `?` ones — by
    // the guard, so a failed start can never wedge a subsequent stop. The
    // ticket carries the generation of the press this start belongs to.
    let ticket = crate::capture_arbiter::ARBITER.begin_start();
    let _starting_guard = StartingGuard;
    // How long this start takes to get from "dispatched" to "stream built".
    // The whole race is a function of that number, and it was never recorded.
    let start_span = crate::trace::Span::start();
    // 1. Self-heal a stale STATE.
    //
    // Spam clicks on the tray button used to leave a "phantom" cpal stream
    // parked in STATE after a race between an in-flight start_recording
    // IPC and an early stop_recording / reset_to_idle from the JS side. The
    // next start would then return "recording_already_in_progress" forever
    // until the user restarted the app. Instead of failing, we discard the
    // stale state (dropping the stream closes the cpal callback, the
    // unfinalised WAV stays on disk and gets swept by the next backup
    // cleanup pass) and proceed with a fresh start.
    {
        let mut state = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
        if let Some(stale) = state.take() {
            let stale_samples = stale.samples_written.load(Ordering::Relaxed);
            log_warn(&format!(
                "[AudioCapture] start_recording: dropping stale STATE for {} (samples written: {})",
                stale.save_path.display(),
                stale_samples
            ));
            // The arbiter believed a capture was live. It was, and we have
            // just closed it — so say so, or the Idle backstop will later
            // find nothing where it expected something and report a
            // disagreement that is really this line.
            crate::capture_arbiter::ARBITER.mark_reclaimed();
            // A stale capture reaching here means a previous cycle ended with
            // the microphone still open. It used to leave a `log_warn` in a
            // file that is filtered to Warn in release and read by nobody.
            crate::trace::event(
                "capture.stale_dropped",
                serde_json::json!({ "samples": stale_samples }),
            );
            // Dropping `stale` closes the stream and releases the writer.
            // We do NOT attempt to finalize the WAV — the file's payload is
            // already discardable since the caller is asking for a fresh
            // session.
        }
    }
    reset_rms();

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

    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
    let preferred = crate::settings::get_settings().audio_device_name;
    let default_name = current_default_input_name();

    log_info(&format!(
        "[AudioCapture] starting: device={:?} rate={} ch={} fmt={:?} → {}",
        device_name,
        config.sample_rate().0,
        config.channels(),
        config.sample_format(),
        save_path.display()
    ));

    if let Ok(mut slot) = LAST_CAPTURE_DEVICE.lock() {
        *slot = Some(device_name.clone());
    }

    crate::trace::event(
        "capture.start",
        serde_json::json!({
            "device": device_name,
            "is_os_default": default_name.as_deref() == Some(device_name.as_str()),
            "preferred": preferred,
            "rate": config.sample_rate().0,
            "channels": config.channels(),
            "format": format!("{:?}", config.sample_format()),
        }),
    );

    // 5. Build the cpal input stream. Cloned handles go into the callback;
    //    err_fn forwards to the frontend (and Sentry breadcrumbs).
    let samples_written = Arc::new(AtomicU64::new(0));
    // Reset the RMS bucket so the pill doesn't briefly mirror the previous
    // session's last sample on session start.
    reset_rms();
    arm_dead_input_watch();
    let stream = build_stream(&device, &supported_config, &writer_handle, &samples_written, &app)?;
    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {}", e))?;

    // 6. Ask permission, then stash everything in shared state for
    //    stop_recording to pick up.
    //
    //    The question and the publish share one critical section, so the
    //    answer cannot be overtaken by the `take()` in `stop_recording_inner`
    //    between deciding and acting.
    let mut state = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
    if let crate::capture_arbiter::Publish::Refuse(reason) =
        crate::capture_arbiter::ARBITER.try_publish(ticket)
    {
        drop(state);
        // The stream is already playing. Dropping it closes the callback —
        // this is the microphone going off, and it must happen before we
        // return.
        drop(stream);
        reset_rms();
        disarm_dead_input_watch();
        if let Ok(mut w) = writer_handle.lock() {
            if let Some(writer) = w.take() {
                let _ = writer.finalize();
            }
        }
        let _ = std::fs::remove_file(&save_path);
        log_warn(&format!(
            "[AudioCapture] refusing to publish a capture nobody is waiting for ({})",
            reason.slug()
        ));
        crate::trace::event(
            "capture.orphan_prevented",
            serde_json::json!({
                "reason": reason.slug(),
                "device": device_name,
                // How long this start took to build a stream. The 2026-08-30
                // incident took 544 ms; a number much larger than that is the
                // next question to ask.
                "build_ms": start_span.ms(),
            }),
        );
        // `Ok`, not `Err`, and the distinction is the point.
        //
        // Returning an error here would route through the frontend's
        // `handleStartRecording` catch, which renders a red pill. But nothing
        // has gone wrong from the user's seat: they tapped, they let go, the
        // app went Idle, and the only thing that happened is that a stream
        // arriving too late was thrown away instead of being left on. A pill
        // would be the app apologising for having done the right thing.
        //
        // The failure is not being swallowed — `capture.orphan_prevented`
        // above is the record, and it names the reason, the device and how
        // long the start took. What is deliberately unchanged is the user's
        // experience of an abandoned press.
        return Ok(());
    }
    *state = Some(RecordingState {
        stream: Some(SafeStream(stream)),
        writer: writer_handle,
        save_path,
        samples_written,
    });
    Ok(())
}

/// Tells the arbiter the start has returned, however it returned.
///
/// A plain call at the end of the function would be skipped by every `?` in
/// it, and there are nine — leaving `stop_recording` waiting three seconds for
/// a start that already failed.
struct StartingGuard;

impl Drop for StartingGuard {
    fn drop(&mut self) {
        crate::capture_arbiter::ARBITER.end_start();
    }
}

/// Tells the arbiter the stop has returned, and whether it found anything.
///
/// `found` starts `false` — the "No recording in progress" arm — and is set
/// the moment the stop takes ownership of a capture, so every early return in
/// between still reports the truth.
struct StoppingGuard {
    found: std::cell::Cell<bool>,
}

impl StoppingGuard {
    fn begin() -> Self {
        crate::capture_arbiter::ARBITER.begin_stop();
        Self { found: std::cell::Cell::new(false) }
    }

    fn took_capture(&self) {
        self.found.set(true);
    }
}

impl Drop for StoppingGuard {
    fn drop(&mut self) {
        crate::capture_arbiter::ARBITER.end_stop(self.found.get());
    }
}

/// Tear down a capture that is live with nobody coming to collect it.
///
/// Called from `AppState::set_state` on the transition to `Idle` — the moment
/// the app has finished with the user's press. In the healthy case the stop
/// has already taken the capture, `should_reclaim()` is false, and this costs
/// one uncontended mutex. It fires only in the reverse-ordered race, where a
/// start published its stream in the gap between a stop that found nothing and
/// the Idle transition.
///
/// Synchronous and blocking, on the caller's thread, while `AppState` is
/// locked. That is deliberate: the alternative is spawning, and a microphone
/// that is closed "soon" is not closed. Dropping a cpal stream and finalising
/// a WAV is the same work `stop_recording` does, and it only happens on a path
/// that is already broken.
pub fn reclaim_orphaned_capture() {
    use crate::capture_arbiter::ARBITER;
    if !ARBITER.should_reclaim() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        // A poisoned STATE lock means a panic while holding it. Say so — the
        // microphone may well still be live and this is the only place that
        // would have noticed.
        crate::trace::degraded(
            "capture.reclaim",
            serde_json::json!({ "error": "state lock poisoned" }),
        );
        return;
    };
    let Some(orphan) = guard.take() else {
        // The arbiter thought something was live and STATE disagrees. Not
        // fatal, but the two are supposed to move together, so record it
        // rather than resyncing in silence.
        ARBITER.mark_reclaimed();
        drop(guard);
        crate::trace::degraded(
            "capture.reclaim",
            serde_json::json!({ "error": "arbiter and STATE disagree", "live": true }),
        );
        return;
    };
    drop(guard);

    let samples = orphan.samples_written.load(Ordering::SeqCst);
    let path = orphan.save_path.clone();
    // Dropping the stream closes the cpal callback. This is the line that
    // turns the microphone off.
    drop(orphan.stream);
    reset_rms();
    disarm_dead_input_watch();
    let finalised = match orphan.writer.lock() {
        Ok(mut w) => match w.take() {
            Some(writer) => writer.finalize().is_ok(),
            None => true,
        },
        Err(poisoned) => match poisoned.into_inner().take() {
            Some(writer) => writer.finalize().is_ok(),
            None => true,
        },
    };
    // Nobody will ever transcribe this file: the dictation it belonged to
    // finished without it. Leaving it would grow the recordings directory
    // one orphan at a time.
    let _ = std::fs::remove_file(&path);
    ARBITER.mark_reclaimed();

    log_warn(&format!(
        "[AudioCapture] reclaimed an orphaned capture ({} samples) — the microphone was live with no owner",
        samples
    ));
    // The defect was invisible for a day because nothing said the microphone
    // was on. It now announces itself the moment it happens.
    crate::trace::event(
        "capture.orphan_reclaimed",
        serde_json::json!({
            "samples": samples,
            "device": last_capture_device(),
            "wav_finalised": finalised,
        }),
    );
}

/// Stop the active recording, finalise the WAV file, and return its path.
///
/// Returns `Err("Empty recording: ...")` if zero audio samples were captured
/// (the WAV file is still finalised + path returned so callers can inspect /
/// log it). The pipeline already has a second defense via `wav_duration_secs`
/// — both layers stay so a bug in either still surfaces the right error.
/// Stop the active recording and return the finalised WAV path.
///
/// Wrapper for the same reason as [`start_recording`]: an error here means a
/// recording the user believes they made is gone, and "No recording in
/// progress" in particular means the state machine and the input layer had
/// already diverged.
#[command]
pub async fn stop_recording() -> Result<PathBuf, String> {
    let result = stop_recording_inner().await;
    if let Err(ref e) = result {
        log_error(&format!("[AudioCapture] stop_recording failed: {}", e));
        crate::trace::event("capture.stop_failed", serde_json::json!({ "error": e }));
    }
    result
}

async fn stop_recording_inner() -> Result<PathBuf, String> {
    // Registered before the drain, not after it, so a start dispatched during
    // the 400 ms below is already visible to the arbiter as "a stop is coming
    // for you" rather than looking like an orphan.
    let stopping = StoppingGuard::begin();

    // Drain delay BEFORE touching STATE.
    //
    // WASAPI (Windows) and several CoreAudio drivers hold up to ~150 ms of
    // captured input in an internal buffer between the hardware and the
    // cpal callback. If we drop the stream immediately on key release that
    // trailing buffer is discarded — surfaces as the user's last syllable
    // being chopped off (reported by a v3.1.3-1 Windows user).
    //
    // We sleep here, BEFORE we acquire STATE or take ownership of the
    // RecordingState, so the cpal callback continues running against the
    // still-live stream during the drain — every late sample gets written
    // to the shared WavWriterHandle. After 200 ms we tear down. Putting
    // the .await at the top also keeps the future Send (a Stream / Mutex
    // guard / RecordingState held across an .await is not Send-safe and
    // makes the Tauri command macro fail to compile).
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Let any in-flight start finish publishing before we look for it.
    //
    // Without this a short tap races: stop takes STATE while start is still
    // building the stream, finds nothing, reports "No recording in progress",
    // and start then publishes a stream with nobody left to stop it. Waiting
    // costs nothing in the normal case, where no start is in flight by the
    // time the drain above has elapsed.
    //
    // This wait SAVES the recording. It does not close the leak on its own —
    // it cannot see a start that has not begun, and it gives up after
    // START_SETTLE_TIMEOUT_MS. `capture_arbiter::try_publish` is what makes
    // the leak impossible; this is what makes the good outcome likely.
    if crate::capture_arbiter::ARBITER.start_in_flight() {
        let waited_from = std::time::Instant::now();
        let mut timed_out = false;
        while crate::capture_arbiter::ARBITER.start_in_flight() {
            if waited_from.elapsed() >= std::time::Duration::from_millis(START_SETTLE_TIMEOUT_MS) {
                timed_out = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(START_SETTLE_POLL_MS)).await;
        }
        let waited_ms = waited_from.elapsed().as_millis() as u64;
        log_info(&format!(
            "[AudioCapture] stop waited {}ms for an in-flight start (timed_out={})",
            waited_ms, timed_out
        ));
        crate::trace::event(
            "capture.stop_waited_for_start",
            // `timed_out:true` is the hole the settle constant's comment says
            // cannot happen. If it ever appears, the arbiter is the only thing
            // standing between the user and a hot microphone, and the number
            // next to it says how long the start really took.
            serde_json::json!({ "ms": waited_ms, "timed_out": timed_out }),
        );
    }

    let mut state_guard = STATE.lock().map_err(|e| format!("state lock poisoned: {}", e))?;
    let state = state_guard
        .take()
        .ok_or_else(|| "No recording in progress".to_string())?;
    // We own a capture: the arbiter can stop considering one live, and the
    // Idle backstop has nothing to do.
    stopping.took_capture();
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
    disarm_dead_input_watch();

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

    // Re-read the OS default now. If it no longer matches the device we
    // opened, something moved the default while the user was talking — a
    // Bluetooth headset connecting, or going to sleep and handing input back
    // to the built-in mic. That switch is invisible to an already-open cpal
    // stream, which keeps happily delivering buffers from a device that has
    // stopped producing audio.
    let opened = last_capture_device();
    let default_now = current_default_input_name();
    let device_changed = match (&opened, &default_now) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };

    crate::trace::event(
        "capture.stop",
        serde_json::json!({
            "device": opened,
            "default_now": default_now,
            "device_changed": device_changed,
            // Samples the callback actually delivered. Zero means the stream
            // never fired at all; a healthy count here alongside an all-zero
            // WAV means the device was streaming silence, which is the
            // Bluetooth-not-really-connected signature.
            "samples": written,
        }),
    );

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
