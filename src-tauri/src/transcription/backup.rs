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
pub fn backup_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("Failed to get app data dir")
        .join("audio_backups")
}

/// Copy an audio file to the backup directory before transcription.
///
/// Creates the backup directory if it does not exist. The backup filename
/// matches the source filename. Returns the full path to the backup file.
pub fn backup_audio(app: &AppHandle, audio_path: &str) -> Result<PathBuf, String> {
    let dir = backup_dir(app);
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
    let dir = backup_dir(app);
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
