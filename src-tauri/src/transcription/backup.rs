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

/// Delete backup files older than 24 hours.
///
/// Called once during app startup in `setup()`. Logs the count of cleaned
/// files but never fails or panics -- if the backup directory does not
/// exist, returns silently.
pub fn cleanup_stale_backups(app: &AppHandle) {
    let Ok(dir) = backup_dir(app) else {
        crate::logging::log_warn("backup cleanup skipped: app data dir unavailable");
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let now = SystemTime::now();
    let mut cleaned = 0u32;

    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };

        if age > BACKUP_MAX_AGE {
            if std::fs::remove_file(entry.path()).is_ok() {
                cleaned += 1;
            }
        }
    }

    if cleaned > 0 {
        crate::logging::log_info(&format!(
            "Cleaned {} stale audio backup(s)",
            cleaned
        ));
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
pub fn wav_average_rms(path: &str) -> Result<f32, String> {
    let mut reader = WavReader::open(path)
        .map_err(|e| format!("Cannot read WAV: {}", e))?;
    let spec = reader.spec();
    let mut sum: f64 = 0.0;
    let mut count: u64 = 0;
    match spec.sample_format {
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => {
                for s in reader.samples::<i16>() {
                    let v = s.unwrap_or(0) as f64 / 32_768.0;
                    sum += v * v;
                    count += 1;
                }
            }
            32 => {
                for s in reader.samples::<i32>() {
                    let v = s.unwrap_or(0) as f64 / 2_147_483_648.0;
                    sum += v * v;
                    count += 1;
                }
            }
            8 => {
                for s in reader.samples::<i8>() {
                    let v = s.unwrap_or(0) as f64 / 128.0;
                    sum += v * v;
                    count += 1;
                }
            }
            bits => return Err(format!("Unsupported int width: {}", bits)),
        },
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                let v = s.unwrap_or(0.0) as f64;
                sum += v * v;
                count += 1;
            }
        }
    }
    if count == 0 {
        return Ok(0.0);
    }
    Ok((sum / count as f64).sqrt() as f32)
}
