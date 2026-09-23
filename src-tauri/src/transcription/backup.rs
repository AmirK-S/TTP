// TTP - Talk To Paste
// Audio backup and validation utilities for transcription reliability
//
// Provides backup-before-transcribe, stale backup cleanup, and WAV
// header validation. These functions are wired into the pipeline by
// pipeline.rs and into app startup by lib.rs.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use hound::WavReader;
use tauri::{AppHandle, Manager};

/// Maximum age for backup files before they are cleaned up (24 hours).
const BACKUP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Get the backup directory path: `app_data_dir/audio_backups/`
///
/// Follows the same pattern as `recording.rs::get_recording_dir()`.
pub fn backup_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("audio_backups"))
        .map_err(|e| format!("Failed to get app data dir: {}", e))
}

/// Copy an audio file to the backup directory before transcription.
///
/// Creates the backup directory if it does not exist. The backup filename
/// matches the source filename. Returns the full path to the backup file.
pub fn backup_audio(app: &AppHandle, audio_path: &str) -> Result<PathBuf, String> {
    let dir = backup_dir(app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create backup dir: {}", e))?;

    let source = Path::new(audio_path);
    let filename = source
        .file_name()
        .ok_or_else(|| "Invalid audio path: no filename".to_string())?;
    let backup_path = dir.join(filename);

    std::fs::copy(audio_path, &backup_path)
        .map_err(|e| format!("Failed to backup audio: {}", e))?;

    sentry::add_breadcrumb(sentry::Breadcrumb {
        message: Some("Audio backed up".into()),
        ..Default::default()
    });

    Ok(backup_path)
}

/// Delete a backup file after successful transcription.
///
/// On Windows, uses a retry loop with backoff (100ms/200ms/300ms) to handle
/// file locking by Windows Defender or the audio subsystem. On macOS/Linux,
/// performs a single-shot deletion. Logs a warning on failure rather than
/// silently discarding errors.
pub fn remove_backup(backup_path: &Path) {
    #[cfg(target_os = "windows")]
    {
        for attempt in 0..3u32 {
            match std::fs::remove_file(backup_path) {
                Ok(()) => return,
                Err(e) if attempt < 2 => {
                    std::thread::sleep(Duration::from_millis(100 * (attempt as u64 + 1)));
                    crate::logging::log_warn(&format!(
                        "Retry {}/{} removing backup {}: {}",
                        attempt + 1,
                        3,
                        backup_path.display(),
                        e
                    ));
                }
                Err(e) => {
                    crate::logging::log_warn(&format!(
                        "Failed to remove backup {}: {}",
                        backup_path.display(),
                        e
                    ));
                    return;
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Err(e) = std::fs::remove_file(backup_path) {
            crate::logging::log_warn(&format!(
                "Failed to remove backup {}: {}",
                backup_path.display(),
                e
            ));
        }
    }
}

/// What one sweep of a directory removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    pub count: u32,
    pub bytes: u64,
    /// Age of the oldest file removed, in whole hours. Zero when none was.
    pub oldest_age_h: u64,
}

/// Delete every `.wav` in `dir` last modified more than `max_age` before `now`.
///
/// `.wav` only, so the sweep cannot take a file it does not own; the
/// converted `.16k.wav` files match too. Age is modification time. A capture
/// still being written is not a candidate in practice: its writer touches the
/// file on every flush, and a recording is capped at minutes, not a day.
///
/// `now` is a parameter so the age arithmetic can be tested without waiting a
/// day or forging timestamps.
fn sweep_older_than(dir: &Path, max_age: Duration, now: SystemTime) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return report;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("wav") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age > max_age && std::fs::remove_file(&path).is_ok() {
            report.count += 1;
            report.bytes += metadata.len();
            report.oldest_age_h = report.oldest_age_h.max(age.as_secs() / 3600);
        }
    }
    report
}

/// Delete audio nobody will read again: every WAV older than 24 hours in
/// `audio_backups/` and in `recordings/`.
///
/// This used to be `cleanup_stale_backups`, and it had two blind spots that
/// together let one file reach 7.2 GB and sit on disk for eleven days:
///
///   * **It never looked in `recordings/`.** Every pipeline exit that ends
///     before `files.cleaned` — a tap under 0.3 s, a Whisper error, a stale
///     capture, a process that died mid-recording — leaves its working WAV
///     there, and `audio_capture`'s stale path said in a comment that "the
///     backup cleanup pass" would collect it. It would not.
///   * **It only ran at launch.** TTP is a tray app that runs for weeks, and
///     backups are now deliberately kept on every text-dropping path so a
///     wrong verdict stays recoverable. A sweep that waits for a relaunch
///     lets those accumulate without bound.
///
/// `trigger` is `"launch"` or `"hourly"`. A launch sweep always writes its
/// `files.swept` lines, including `count:0`, so a trace can show the sweep ran;
/// an hourly one writes only when it removed something, because a line an
/// hour that says nothing happened would be most of the trace's growth.
pub fn sweep_stale_audio(app: &AppHandle, trigger: &'static str) {
    let dirs = [
        ("audio_backups", backup_dir(app)),
        ("recordings", crate::recording::get_recording_dir(app)),
    ];
    for (name, dir) in dirs {
        let Ok(dir) = dir else {
            crate::logging::log_warn(&format!(
                "audio sweep skipped {}: app data dir unavailable",
                name
            ));
            continue;
        };
        let report = sweep_older_than(&dir, BACKUP_MAX_AGE, SystemTime::now());
        if report.count > 0 {
            crate::logging::log_info(&format!(
                "Swept {} stale audio file(s) from {} ({} bytes)",
                report.count, name, report.bytes
            ));
        }
        if report.count > 0 || trigger == "launch" {
            crate::trace::event(
                "files.swept",
                serde_json::json!({
                    "dir": name,
                    "trigger": trigger,
                    "count": report.count,
                    "bytes": report.bytes,
                    "oldest_age_h": report.oldest_age_h,
                }),
            );
        }
    }
}

/// Validate that a file has a valid WAV header.
///
/// Uses `hound::WavReader::open()` to parse the RIFF/WAVE header and fmt
/// chunk. Returns `Ok(())` if the header is valid with reasonable channel
/// count and sample rate. Returns `Err` with a user-facing error message
/// if the file is corrupt, unreadable, or unsupported.
pub fn validate_wav(path: &str) -> Result<(), String> {
    match WavReader::open(path) {
        Ok(reader) => {
            let spec = reader.spec();
            if spec.channels == 0 {
                return Err("Corrupt audio: no channels".to_string());
            }
            if spec.sample_rate == 0 {
                return Err("Corrupt audio: invalid sample rate".to_string());
            }
            Ok(())
        }
        Err(hound::Error::FormatError(msg)) => {
            Err(format!("Corrupt audio file: {}", msg))
        }
        Err(hound::Error::IoError(e)) => {
            Err(format!("Cannot read audio file: {}", e))
        }
        Err(hound::Error::Unsupported) => {
            Err("Unsupported audio format".to_string())
        }
        Err(e) => {
            Err(format!("Invalid audio file: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-for-byte copy of an actual 68-byte WAV the plugin produced on
    /// the user's machine when mic permission was silently revoked after
    /// the v3.0.3 update. RIFF + fmt (WAVE_FORMAT_EXTENSIBLE, 48 kHz mono
    /// IEEE float 32-bit) + data chunk with size 0. validate_wav() passes
    /// this — only wav_duration_secs() catches it.
    const EMPTY_PLUGIN_WAV: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x3c, 0x00, 0x00, 0x00, // RIFF size=60
        0x57, 0x41, 0x56, 0x45,                         // WAVE
        0x66, 0x6d, 0x74, 0x20, 0x28, 0x00, 0x00, 0x00, // fmt  size=40
        0xfe, 0xff, 0x01, 0x00,                         // EXTENSIBLE, 1ch
        0x80, 0xbb, 0x00, 0x00,                         // 48000 Hz
        0x00, 0xee, 0x02, 0x00,                         // 192000 B/s
        0x04, 0x00, 0x20, 0x00,                         // block=4 bps=32
        0x16, 0x00, 0x20, 0x00,                         // cbSize=22 valid=32
        0x01, 0x00, 0x00, 0x00,                         // channel mask FL
        0x03, 0x00, 0x00, 0x00,                         // IEEE_FLOAT GUID...
        0x00, 0x00, 0x10, 0x00,
        0x80, 0x00, 0x00, 0xaa,
        0x00, 0x38, 0x9b, 0x71,
        0x64, 0x61, 0x74, 0x61, 0x00, 0x00, 0x00, 0x00, // data size=0
    ];

    /// Write a 16-bit mono WAV of `samples` and return its path.
    fn write_wav(name: &str, samples: &[i16]) -> String {
        let path = std::env::temp_dir().join(name);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for s in samples {
            writer.write_sample(*s).unwrap();
        }
        writer.finalize().unwrap();
        path.to_str().unwrap().to_string()
    }

    /// A fresh, empty directory under the system temp dir, unique to `name`.
    fn sweep_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ttp_sweep_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_sweep_takes_day_old_wavs_and_nothing_else() {
        // The shape `recordings/` was left in: a working WAV and its converted
        // twin, which also ends in `.wav`. The text file stands for anything
        // the sweep does not own.
        let dir = sweep_dir("old");
        std::fs::write(dir.join("recording_20260830_231821.wav"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("recording_20260830_231821.16k.wav"), [0u8; 4]).unwrap();
        std::fs::write(dir.join("notes.txt"), b"not audio").unwrap();

        let fresh = sweep_older_than(&dir, BACKUP_MAX_AGE, SystemTime::now());
        assert_eq!(fresh, SweepReport::default(), "nothing is a day old yet");
        assert!(dir.join("recording_20260830_231821.wav").exists());

        let a_day_later = SystemTime::now() + BACKUP_MAX_AGE + Duration::from_secs(3600);
        let report = sweep_older_than(&dir, BACKUP_MAX_AGE, a_day_later);
        assert_eq!(report.count, 2);
        assert_eq!(report.bytes, 14);
        assert!(report.oldest_age_h >= 24, "oldest_age_h was {}", report.oldest_age_h);
        assert!(!dir.join("recording_20260830_231821.wav").exists());
        assert!(!dir.join("recording_20260830_231821.16k.wav").exists());
        assert!(dir.join("notes.txt").exists(), "the sweep only takes .wav files");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sweep_leaves_a_directory_of_wav_name_alone() {
        let dir = sweep_dir("nested");
        std::fs::create_dir_all(dir.join("keep.wav")).unwrap();
        let a_day_later = SystemTime::now() + BACKUP_MAX_AGE + Duration::from_secs(3600);
        assert_eq!(sweep_older_than(&dir, BACKUP_MAX_AGE, a_day_later).count, 0);
        assert!(dir.join("keep.wav").is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_is_an_empty_sweep() {
        let missing = std::env::temp_dir().join("ttp_sweep_does_not_exist_9f2c");
        let report = sweep_older_than(&missing, BACKUP_MAX_AGE, SystemTime::now());
        assert_eq!(report, SweepReport::default());
    }

    #[test]
    fn all_zero_samples_are_a_dead_capture() {
        // The signature observed in the wild: 7.8 seconds of audio, 748 KB on
        // disk, and every single sample zero. A live microphone cannot do
        // this — it means the callback delivered nothing.
        let path = write_wav("ttp_test_dead.wav", &[0i16; 16_000]);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(stats.is_dead_capture());
        assert_eq!(stats.rms, 0.0);
        assert_eq!(stats.peak, 0.0);
        assert_eq!(stats.nonzero_ratio, 0.0);
        assert_eq!(stats.samples, 16_000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_quiet_room_is_not_a_dead_capture() {
        // Noise floor: tiny but non-zero, alternating so the mean is ~0 and
        // only the RMS picks it up. This must reach the silence gate and its
        // "no speech" message, NOT the dead-capture path.
        let samples: Vec<i16> = (0..16_000).map(|i| if i % 2 == 0 { 3 } else { -3 }).collect();
        let path = write_wav("ttp_test_quiet.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(!stats.is_dead_capture(), "noise floor must not read as dead");
        assert!(stats.rms > 0.0 && stats.rms < 0.005, "rms was {}", stats.rms);
        assert_eq!(stats.nonzero_ratio, 1.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn leading_silence_does_not_drag_speech_below_the_floor() {
        // The AirPods case, 2026-08-30: the stream opens, the device sends
        // nothing for about a second, then real speech arrives. Judged over
        // the whole file the RMS is diluted below the floor and the whole
        // dictation is dropped as "no speech" — while containing speech.
        // Nine parts dead air to one part speech, and a quiet speaker —
        // amplitude chosen so the speech alone clears the floor (~0.008 RMS)
        // while the diluted whole-file figure lands under it (~0.0025). That
        // ratio is not contrived: the observed AirPods gap was ~0.94s against
        // dictations that often run a few seconds.
        let mut samples = vec![0i16; 144_000];
        samples.extend((0..16_000).map(|i| ((i as f32 * 0.05).sin() * 370.0) as i16));
        let path = write_wav("ttp_test_leadin.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();

        // At least the dead air, and not much more: a sine legitimately
        // starts at zero, so the first sample or two of real speech can be
        // silent as well. Asserting an exact count would be asserting a
        // property of the test signal rather than of the code.
        assert!(
            (144_000..144_010).contains(&stats.leading_silence),
            "leading_silence was {}",
            stats.leading_silence
        );
        assert!(!stats.is_dead_capture(), "there is real audio in here");
        assert!(
            stats.rms < 0.005,
            "the whole-file RMS should be dragged under the floor: {}",
            stats.rms
        );
        assert!(
            stats.rms_after_silence > 0.005,
            "the audio that arrived is clearly speech: {}",
            stats.rms_after_silence
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Deterministic room noise around ±`amp`, so tests do not need a RNG.
    fn room_noise(len: usize, amp: f32) -> Vec<i16> {
        (0..len)
            .map(|i| ((((i * 7919) % 1000) as f32 / 500.0 - 1.0) * amp) as i16)
            .collect()
    }

    #[test]
    fn long_pauses_do_not_hide_speech() {
        // The 2026-09-23 loss, rebuilt: 46 s at 16 kHz, one second of clear
        // speech every eight seconds, room noise everywhere else. The
        // whole-file RMS lands under the 0.005 floor — the old gate dropped
        // this — while the speech windows plainly add up to seconds.
        let mut samples = Vec::new();
        for _ in 0..6 {
            samples.extend((0..16_000).map(|i| ((i as f32 * 0.07).sin() * 600.0) as i16));
            samples.extend(room_noise(112_000, 40.0));
        }
        let path = write_wav("ttp_test_pauses.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(stats.rms_after_silence < 0.005, "whole-file rms {}", stats.rms_after_silence);
        assert!(stats.speech_ms >= 5_000, "speech_ms was {}", stats.speech_ms);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn room_noise_alone_is_not_speech() {
        let path = write_wav("ttp_test_room.wav", &room_noise(16_000 * 8, 40.0));
        let stats = wav_signal_stats(&path).unwrap();
        assert_eq!(stats.speech_ms, 0, "noise floor {}", stats.noise_floor);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_loud_room_raises_the_speech_threshold() {
        // A fan at ~0.01 RMS would clear the absolute floor on its own; the
        // relative floor keeps it from reading as eight seconds of speech.
        let path = write_wav("ttp_test_fan.wav", &room_noise(16_000 * 8, 560.0));
        let stats = wav_signal_stats(&path).unwrap();
        assert!(stats.speech_window_floor > SPEECH_WINDOW_MIN_RMS);
        assert_eq!(stats.speech_ms, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_wholly_silent_file_reports_zero_for_both_measures() {
        // The dead-capture branch runs first and must still see a zero, so
        // rms_after_silence falls back rather than dividing by nothing.
        let path = write_wav("ttp_test_alldead.wav", &[0i16; 8_000]);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(stats.is_dead_capture());
        assert_eq!(stats.rms_after_silence, 0.0);
        assert_eq!(stats.leading_silence, 8_000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn clean_audio_reports_no_leading_silence() {
        let samples: Vec<i16> = (0..8_000)
            .map(|i| (((i as f32 * 0.05).sin() * 6_000.0) as i16).max(1))
            .collect();
        let path = write_wav("ttp_test_clean.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();
        assert_eq!(stats.leading_silence, 0);
        assert!((stats.rms - stats.rms_after_silence).abs() < 1e-6);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_single_nonzero_sample_defeats_dead_capture() {
        // The predicate is deliberately strict: ANY signal at all means the
        // device was alive, and we must not tell the user their microphone
        // is broken on the strength of a near-silent recording.
        let mut samples = [0i16; 16_000];
        samples[9_000] = 1;
        let path = write_wav("ttp_test_onesample.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(!stats.is_dead_capture());
        assert!(stats.nonzero_ratio > 0.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn speech_level_audio_clears_the_silence_floor() {
        let samples: Vec<i16> = (0..16_000)
            .map(|i| ((i as f32 * 0.05).sin() * 8_000.0) as i16)
            .collect();
        let path = write_wav("ttp_test_speech.wav", &samples);
        let stats = wav_signal_stats(&path).unwrap();
        assert!(!stats.is_dead_capture());
        assert!(stats.rms > 0.005, "rms was {}", stats.rms);
        assert!(stats.peak > 0.2, "peak was {}", stats.peak);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_wav_with_no_samples_is_not_reported_as_dead_capture() {
        // Zero samples is the AUDI-06 empty-recording case, caught earlier by
        // wav_duration_secs. is_dead_capture requires samples > 0 so the two
        // paths cannot both claim the same recording.
        let path = write_wav("ttp_test_nosamples.wav", &[]);
        let stats = wav_signal_stats(&path).unwrap();
        assert_eq!(stats.samples, 0);
        assert!(!stats.is_dead_capture());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validates_empty_plugin_wav_header_passes() {
        // The OLD validate_wav passes this — header is technically valid.
        // This documents the gap that wav_duration_secs closes.
        let tmp = std::env::temp_dir().join("ttp_test_empty.wav");
        std::fs::write(&tmp, EMPTY_PLUGIN_WAV).unwrap();
        assert!(validate_wav(tmp.to_str().unwrap()).is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn detects_empty_plugin_wav_via_duration() {
        let tmp = std::env::temp_dir().join("ttp_test_empty_dur.wav");
        std::fs::write(&tmp, EMPTY_PLUGIN_WAV).unwrap();
        let secs = wav_duration_secs(tmp.to_str().unwrap()).unwrap();
        assert_eq!(secs, 0.0, "data chunk size is 0 → 0 samples → 0 seconds");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn non_empty_wav_returns_positive_duration() {
        // Build a 1-second 16 kHz mono i16 WAV via hound and confirm we
        // measure ~1.0 s. Guards against regressions where we'd reject
        // legitimate recordings.
        let tmp = std::env::temp_dir().join("ttp_test_1sec.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut w = hound::WavWriter::create(&tmp, spec).unwrap();
            for _ in 0..16_000 {
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let secs = wav_duration_secs(tmp.to_str().unwrap()).unwrap();
        assert!((secs - 1.0).abs() < 0.001, "expected ~1.0 s, got {}", secs);
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Verify that a WAV file actually contains audio samples (not just a valid
/// header with an empty data chunk).
///
/// `tauri-plugin-mic-recorder` v2 has a `try_lock()` in its audio callback
/// (`commands.rs:277`) that silently drops samples on contention, and its
/// error callback only writes to stderr (`commands.rs:145`) — both can leave
/// us with a syntactically-valid WAV that has 0 audio samples. The most
/// common real-world trigger on macOS is mic permission silently revoked
/// after an unsigned app update: cpal opens the stream, the callback never
/// fires, stop_recording finalises a 68-byte file (header only).
///
/// Sending such a file to Groq returns "Audio file is too short" (HTTP 400),
/// which the pipeline currently maps to a generic "Transcription failed".
/// This pre-check lets the pipeline surface a clear, actionable error before
/// the API call.
///
/// Returns `Ok(duration_secs)` for non-empty recordings, `Err` for empty.
pub fn wav_duration_secs(path: &str) -> Result<f64, String> {
    let reader = WavReader::open(path)
        .map_err(|e| format!("Cannot read WAV: {}", e))?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return Err("Invalid sample rate".to_string());
    }
    let samples = reader.duration() as f64;
    Ok(samples / spec.sample_rate as f64)
}

/// Whisper hallucinates on silence: it returns "thank you", "Sous-titré par
/// XYZ", broadcaster credits, and (the most striking failure mode) random
/// foreign-language sentences when fed audio with no speech. The
/// hallucination filter downstream catches many of these by exact / substring
/// match, but it fundamentally can't catch a 4-word "thank you" that the
/// user might actually have dictated.
///
/// The robust fix is to never SEND silent audio to Whisper. This helper
/// computes the average RMS of the WAV in [0.0, 1.0]; the pipeline skips
/// the API call entirely when the value falls below a hand-tuned silence
/// floor (~0.005, a few dB above MacBook mic self-noise).
///
/// Reads the full PCM payload, so it's only suitable for the post-recording
/// gate where we already have the file open. NOT for the realtime callback.
/// Signal characteristics of a recording, gathered in a single pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalStats {
    /// Root-mean-square amplitude across every sample, 0.0..=1.0.
    pub rms: f32,
    /// Largest absolute sample amplitude, 0.0..=1.0.
    pub peak: f32,
    /// Fraction of samples that are not exactly zero, 0.0..=1.0.
    pub nonzero_ratio: f32,
    /// Total samples examined.
    pub samples: u64,
    /// Leading samples that were exactly zero before any signal arrived.
    ///
    /// Bluetooth input devices open their stream and then take up to a second
    /// to actually start sending audio. Observed on AirPods Pro 2026-08-30:
    /// the stream ran at 24 kHz (HFP), delivered 22560 samples, and every one
    /// of them was zero. Whatever the user said in that window does not exist.
    pub leading_silence: u64,
    /// RMS measured over the audio that actually arrived — everything from
    /// the first non-zero sample onward.
    ///
    /// The silence gate must use this rather than `rms`. A dictation that is
    /// one second of Bluetooth dead air followed by real speech has its
    /// overall RMS dragged below the floor by the dead air, and gets dropped
    /// as "no speech" while containing speech. Measuring the audio that
    /// exists, rather than the audio plus the silence in front of it, is the
    /// difference between losing the recording and losing the first word.
    pub rms_after_silence: f32,
    /// How much of the recording sounds like speech, in milliseconds: the
    /// number of 50 ms windows louder than `speech_window_floor`, times 50.
    ///
    /// A whole-file RMS cannot tell "nobody spoke" from "somebody spoke,
    /// then thought for a long time". On 2026-09-23 a 46.7 s dictation with
    /// a 0.31 peak — a clear voice — averaged 0.00489 over the whole file
    /// because of its pauses, and was thrown away as silence. Speech is
    /// short loud bursts; counting the bursts is what survives the pauses.
    pub speech_ms: u32,
    /// The recording's own background level: the 20th-percentile RMS of its
    /// non-silent 50 ms windows.
    pub noise_floor: f32,
    /// The level a 50 ms window must exceed to count as speech:
    /// `max(SPEECH_WINDOW_MIN_RMS, SPEECH_OVER_NOISE × noise_floor)`.
    pub speech_window_floor: f32,
}

/// Length of one analysis window. 50 ms is shorter than a syllable, so a
/// single spoken word already spans several windows.
const SPEECH_WINDOW_MS: u32 = 50;
/// Absolute floor for a speech window. A quiet MacBook room sits around
/// 0.001–0.002 per window; quiet speech on the same mic clears 0.01.
const SPEECH_WINDOW_MIN_RMS: f32 = 0.008;
/// Relative floor: a window must also stand 4× above the recording's own
/// background, so a fan or a noisy café does not read as speech.
const SPEECH_OVER_NOISE: f32 = 4.0;

impl SignalStats {
    /// True when the capture device handed us digital silence — every sample
    /// exactly zero.
    ///
    /// This is emphatically NOT the same as "the user did not speak". A real
    /// microphone in a quiet room still produces a noise floor; RMS lands
    /// around 0.0005–0.003 and individual samples are never all zero. An
    /// all-zero buffer means the audio callback delivered nothing: mic
    /// permission silently revoked (classic after an unsigned-app update),
    /// the device held exclusively by another process, or a stream that
    /// opened but never ran.
    ///
    /// Worth separating because the two cases need opposite messages. "No
    /// speech detected" told a user whose microphone was dead that they had
    /// not spoken — while they had just dictated for eight seconds.
    pub fn is_dead_capture(&self) -> bool {
        self.samples > 0 && self.nonzero_ratio == 0.0
    }
}

/// Compute [`SignalStats`] for a WAV file in one pass.
pub fn wav_signal_stats(path: &str) -> Result<SignalStats, String> {
    let mut reader = WavReader::open(path)
        .map_err(|e| format!("Cannot read WAV: {}", e))?;
    let spec = reader.spec();
    // Samples per analysis window, counting every channel of a frame.
    let window_len = ((spec.sample_rate as u64 * SPEECH_WINDOW_MS as u64 / 1000)
        * spec.channels.max(1) as u64)
        .max(1);
    let mut windows: Vec<f32> = Vec::new();
    let mut window_sum: f64 = 0.0;
    let mut window_count: u64 = 0;
    let mut sum: f64 = 0.0;
    let mut peak: f64 = 0.0;
    let mut nonzero: u64 = 0;
    let mut count: u64 = 0;
    // Second accumulator, started at the first non-zero sample.
    let mut sum_after: f64 = 0.0;
    let mut count_after: u64 = 0;
    let mut leading_silence: u64 = 0;
    let mut seen_signal = false;

    // One accumulator for every sample width, so the branch on format stays
    // a thin decode step rather than four copies of the statistics.
    let mut accumulate = |v: f64| {
        sum += v * v;
        let magnitude = v.abs();
        if magnitude > peak {
            peak = magnitude;
        }
        if v != 0.0 {
            nonzero += 1;
            seen_signal = true;
        }
        if seen_signal {
            sum_after += v * v;
            count_after += 1;
        } else {
            leading_silence += 1;
        }
        count += 1;
        window_sum += v * v;
        window_count += 1;
        if window_count == window_len {
            windows.push((window_sum / window_count as f64).sqrt() as f32);
            window_sum = 0.0;
            window_count = 0;
        }
    };

    match spec.sample_format {
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => {
                for s in reader.samples::<i16>() {
                    accumulate(s.unwrap_or(0) as f64 / 32_768.0);
                }
            }
            32 => {
                for s in reader.samples::<i32>() {
                    accumulate(s.unwrap_or(0) as f64 / 2_147_483_648.0);
                }
            }
            8 => {
                for s in reader.samples::<i8>() {
                    accumulate(s.unwrap_or(0) as f64 / 128.0);
                }
            }
            bits => return Err(format!("Unsupported int width: {}", bits)),
        },
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                accumulate(s.unwrap_or(0.0) as f64);
            }
        }
    }

    if count == 0 {
        return Ok(SignalStats {
            rms: 0.0,
            peak: 0.0,
            nonzero_ratio: 0.0,
            samples: 0,
            leading_silence: 0,
            rms_after_silence: 0.0,
            speech_ms: 0,
            noise_floor: 0.0,
            speech_window_floor: SPEECH_WINDOW_MIN_RMS,
        });
    }

    // A trailing partial window counts once it holds half a window: enough
    // to be a real measurement, and a word said just before release is kept.
    if window_count * 2 >= window_len {
        windows.push((window_sum / window_count as f64).sqrt() as f32);
    }
    let (speech_ms, noise_floor, speech_window_floor) = speech_in_windows(&windows);

    let rms = (sum / count as f64).sqrt() as f32;
    Ok(SignalStats {
        rms,
        peak: peak as f32,
        nonzero_ratio: nonzero as f32 / count as f32,
        samples: count,
        leading_silence,
        // Falls back to the overall figure when nothing but silence arrived,
        // so the dead-capture branch still sees a zero.
        rms_after_silence: if count_after > 0 {
            (sum_after / count_after as f64).sqrt() as f32
        } else {
            rms
        },
        speech_ms,
        noise_floor,
        speech_window_floor,
    })
}

/// Speech duration, noise floor and speech threshold for a list of 50 ms
/// window RMS values. Windows that are exactly zero (a Bluetooth lead-in)
/// are left out of the noise estimate: they are absence, not background.
fn speech_in_windows(windows: &[f32]) -> (u32, f32, f32) {
    let mut heard: Vec<f32> = windows.iter().copied().filter(|v| *v > 0.0).collect();
    if heard.is_empty() {
        return (0, 0.0, SPEECH_WINDOW_MIN_RMS);
    }
    heard.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let noise_floor = heard[heard.len() / 5];
    let floor = SPEECH_WINDOW_MIN_RMS.max(SPEECH_OVER_NOISE * noise_floor);
    let loud = windows.iter().filter(|v| **v > floor).count() as u32;
    (loud * SPEECH_WINDOW_MS, noise_floor, floor)
}
