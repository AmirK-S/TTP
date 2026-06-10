// TTP - Talk To Paste
// Persistent error logging to file
//
// Logs API errors, crashes, and failures to a rotating log file in the app
// data directory. Users can share this for debugging.
//
// Level filter:
//   The active threshold is read from $TTP_LOG_LEVEL once at first use (off /
//   error / warn / info / debug). Defaults: Warn in release, Info in debug.
//   Drops everything below the threshold before touching disk — keeps the log
//   file small for non-debugging users while leaving a knob power users can
//   crank up when reporting an issue.
//
// Rotation:
//   Two historical files (ttp.log.1, ttp.log.2). Total cap ~1.5 MB. The audit
//   flagged the previous single-rotation policy as too aggressive — when a
//   user reports a bug days after it happened, the relevant signal has often
//   already rolled off.

use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Max log file size before rotation (500KB per file).
const MAX_LOG_SIZE: u64 = 500_000;

/// Number of historical rotated files to retain. Total disk usage caps at
/// roughly `(KEEP_ROTATIONS + 1) * MAX_LOG_SIZE`.
const KEEP_ROTATIONS: usize = 2;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Level {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl Level {
    fn from_env_or_default() -> Self {
        if let Ok(raw) = std::env::var("TTP_LOG_LEVEL") {
            match raw.trim().to_lowercase().as_str() {
                "off" | "0" => return Level::Off,
                "error" | "err" | "1" => return Level::Error,
                "warn" | "warning" | "2" => return Level::Warn,
                "info" | "3" => return Level::Info,
                "debug" | "trace" | "4" => return Level::Debug,
                _ => {} // unknown -> use compile-time default
            }
        }
        // Defaults: be verbose in dev so contributors see what's happening,
        // be terse in release so the persistent log file isn't full of routine
        // chatter the user never reads.
        if cfg!(debug_assertions) {
            Level::Info
        } else {
            Level::Warn
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Level::Off => "OFF",
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
        }
    }
}

static LEVEL: OnceLock<Level> = OnceLock::new();

fn active_level() -> Level {
    *LEVEL.get_or_init(Level::from_env_or_default)
}

/// Get the live log file path in the app data directory.
fn log_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.ttp.desktop").join("ttp.log"))
}

/// Rotated file at index N (`ttp.log.1`, `ttp.log.2`, ...).
fn log_path_at(index: usize) -> Option<PathBuf> {
    dirs::data_dir().map(|d| {
        d.join("com.ttp.desktop")
            .join(format!("ttp.log.{}", index))
    })
}

/// Rotate `path` if it's grown past `MAX_LOG_SIZE`. Cascades through
/// `ttp.log.N -> ttp.log.N+1` so the oldest is overwritten last.
fn rotate_if_needed(path: &PathBuf) {
    let Ok(meta) = fs::metadata(path) else { return };
    if meta.len() <= MAX_LOG_SIZE {
        return;
    }
    // Cascade from oldest to newest: .2 <- .1, then .1 <- live.
    for i in (1..KEEP_ROTATIONS).rev() {
        let (Some(src), Some(dst)) = (log_path_at(i), log_path_at(i + 1)) else {
            continue;
        };
        if src.exists() {
            let _ = fs::rename(&src, &dst);
        }
    }
    if let Some(first) = log_path_at(1) {
        let _ = fs::rename(path, &first);
    }
}

/// Write a log entry to the persistent log file.
fn log_to_file_at(level: Level, message: &str) {
    if level > active_level() {
        return;
    }
    let Some(path) = log_path() else { return };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    rotate_if_needed(&path);

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] [{}] {}\n", timestamp, level.tag(), message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(entry.as_bytes());
    }
}

/// Back-compat helper for callers that pass a level string directly.
/// Prefer `log_error` / `log_warn` / `log_info`.
pub fn log_to_file(level: &str, message: &str) {
    let lvl = match level.to_uppercase().as_str() {
        "ERROR" => Level::Error,
        "WARN" => Level::Warn,
        "INFO" => Level::Info,
        "DEBUG" => Level::Debug,
        _ => Level::Info,
    };
    log_to_file_at(lvl, message);
}

/// Log an error. Always echoes to stderr in addition to the file because
/// errors are the only level a user is asked to inspect via Console.app
/// for live triage.
pub fn log_error(message: &str) {
    log_to_file_at(Level::Error, message);
    if active_level() >= Level::Error {
        eprintln!("[ERROR] {}", message);
    }
}

/// Log a warning. File only — keeps Console.app quiet in normal use so a
/// burst of `try_lock` contention warnings doesn't drown out the genuine
/// errors a user might be there to investigate.
pub fn log_warn(message: &str) {
    log_to_file_at(Level::Warn, message);
}

/// Log an info event. Dropped from the persistent log in release builds
/// unless the user opts in via `TTP_LOG_LEVEL=info`.
pub fn log_info(message: &str) {
    log_to_file_at(Level::Info, message);
}

/// Debug-level breadcrumb. Almost always filtered out in release; useful
/// for state-machine traces during contributor debugging.
#[allow(dead_code)]
pub fn log_debug(message: &str) {
    log_to_file_at(Level::Debug, message);
}

/// Open the log directory in the OS file manager. Users hit this from
/// Settings -> Advanced when reporting an issue: it gives them direct access
/// to ttp.log and the rotated history so they can attach them to a bug
/// report without hunting through `~/Library/Application Support`.
#[tauri::command]
pub fn reveal_log_folder() -> Result<(), String> {
    let path = log_path().ok_or_else(|| "Could not determine log folder".to_string())?;
    let dir = path
        .parent()
        .ok_or_else(|| "Log path has no parent directory".to_string())?
        .to_path_buf();

    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open log folder: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open log folder: {}", e))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open log folder: {}", e))?;
        Ok(())
    }
}
