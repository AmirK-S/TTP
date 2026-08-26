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
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Max log file size before rotation (500KB per file).
const MAX_LOG_SIZE: u64 = 500_000;

/// Number of historical rotated files to retain. Total disk usage caps at
/// roughly `(KEEP_ROTATIONS + 1) * MAX_LOG_SIZE`.
const KEEP_ROTATIONS: usize = 2;

/// Max dictation-trace file size before rotation (2MB per file).
///
/// Four times the main log's cap, and with one more historical file: the
/// trace is verbose by design, and its entire value is still holding the
/// dictation the user is asking about — which is usually the one from twenty
/// minutes ago, not the one from ten seconds ago.
const MAX_TRACE_SIZE: u64 = 2_000_000;

/// Historical trace files retained (`ttp-trace.log.1` .. `.3`).
const KEEP_TRACE_ROTATIONS: usize = 3;

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

/// Directory holding every file this module writes.
fn log_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.ttp.desktop"))
}

/// Get the live log file path in the app data directory.
fn log_path() -> Option<PathBuf> {
    log_dir().map(|d| d.join("ttp.log"))
}

/// Get the live dictation-trace file path. Deliberately a separate file from
/// `ttp.log`: the trace must not be filtered by the main log's level, and a
/// burst of warnings must not rotate away the dictation history the user is
/// about to be asked for.
fn trace_path() -> Option<PathBuf> {
    log_dir().map(|d| d.join("ttp-trace.log"))
}

/// Rotated sibling of `path` at index N (`ttp.log` -> `ttp.log.1`).
fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ttp.log");
    path.with_file_name(format!("{}.{}", name, index))
}

/// Rotate `path` if it's grown past `max_size`. Cascades through
/// `<name>.N -> <name>.N+1` so the oldest is overwritten last, retaining
/// `keep` historical files.
fn rotate_if_needed(path: &Path, max_size: u64, keep: usize) {
    let Ok(meta) = fs::metadata(path) else { return };
    if meta.len() <= max_size {
        return;
    }
    // Cascade from oldest to newest: .N <- .N-1, ... then .1 <- live.
    for i in (1..keep).rev() {
        let src = rotated_path(path, i);
        if src.exists() {
            let _ = fs::rename(&src, rotated_path(path, i + 1));
        }
    }
    let _ = fs::rename(path, rotated_path(path, 1));
}

/// Append one line to `path`, rotating first if needed. Every failure is
/// swallowed: logging must never be able to break the thing it is observing.
fn append_line(path: &Path, max_size: u64, keep: usize, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    rotate_if_needed(path, max_size, keep);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
        let _ = file.write_all(b"\n");
    }
}

/// Write a log entry to the persistent log file.
fn log_to_file_at(level: Level, message: &str) {
    if level > active_level() {
        return;
    }
    let Some(path) = log_path() else { return };

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] [{}] {}", timestamp, level.tag(), message);
    append_line(&path, MAX_LOG_SIZE, KEEP_ROTATIONS, &entry);
}

/// Append one pre-formatted line to the dictation trace.
///
/// Bypasses `active_level()` on purpose. The trace exists precisely because
/// the release-default Warn threshold hides every stage that can silently
/// swallow a transcription; gating it behind that same threshold would
/// reproduce the blind spot it was written to remove. Volume is bounded by
/// the file's own rotation, and text payloads are redacted unless the user
/// opts in — see `crate::trace`.
pub fn log_trace_line(line: &str) {
    let Some(path) = trace_path() else { return };
    append_line(&path, MAX_TRACE_SIZE, KEEP_TRACE_ROTATIONS, line);
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
