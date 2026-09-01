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

/// Max dictation-trace file size before rotation (2.5MB per file).
///
/// Five times the main log's cap, and with one more historical file: the
/// trace is verbose by design, and its entire value is still holding the
/// dictation the user is asking about — which is usually the one from twenty
/// minutes ago, not the one from ten seconds ago.
///
/// Raised from 2 MB in Polaris. Per-dictation volume went from ~3.7 KB to
/// ~5.1 KB when every stage gained a `dur_ms`, the filters started recording
/// their negative verdicts, and the keychain got timed. At 2 MB that would
/// have cut the retained window by a third; at 2.5 MB it is back where it
/// was. The guaranteed floor is the three ROTATED files — the live one can
/// be nearly empty right after a rotation — so the number that matters is
/// 3 x 2.5 MB = 7.5 MB, about 1470 dictations, three weeks at the observed
/// rate of ~68 a day. See `docs/tracing.md`, "Retention".
const MAX_TRACE_SIZE: u64 = 2_500_000;

/// Historical trace files retained (`ttp-trace.log.1` .. `.3`).
const KEEP_TRACE_ROTATIONS: usize = 3;

/// Rotation policy, exposed so the trace API can report the real retained
/// window to a viewer instead of the viewer hardcoding a guess.
pub fn trace_rotation() -> (u64, usize) {
    (MAX_TRACE_SIZE, KEEP_TRACE_ROTATIONS)
}

/// Every trace file that currently exists, newest first: the live
/// `ttp-trace.log` followed by `.1` .. `.3`.
///
/// The read side of the trace API walks these in order. Returning paths
/// rather than contents keeps the "where do the files live" knowledge in the
/// one module that already owns it.
pub fn trace_files_newest_first() -> Vec<PathBuf> {
    let Some(live) = trace_path() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if live.exists() {
        out.push(live.clone());
    }
    for i in 1..=KEEP_TRACE_ROTATIONS {
        let p = rotated_path(&live, i);
        if p.exists() {
            out.push(p);
        }
    }
    out
}

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

/// Failed appends, counted so the gap they leave can announce itself.
///
/// A write that fails here cannot be reported by logging it — that is the
/// call that just failed, and retrying it recursively would be worse than the
/// silence. So it is counted, and `crate::trace` folds the tally into the
/// next record that does reach the file, the same way it reports queue
/// overflow. See [`take_write_failures`].
static WRITE_FAILURES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Read and reset the failed-append tally.
///
/// Called by the trace writer thread immediately before it hands a record to
/// [`log_trace_line`], so a burst of failed writes is reported on the first
/// line that gets through rather than never.
pub fn take_write_failures() -> u64 {
    WRITE_FAILURES.swap(0, std::sync::atomic::Ordering::Relaxed)
}

/// Put a tally back after the line carrying it failed to land, so the count
/// stays true rather than being lost along with the line that reported it.
pub fn restore_write_failures(n: u64) {
    if n > 0 {
        WRITE_FAILURES.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Frame one record as the exact bytes of a single append.
///
/// The record and its terminating newline are ONE buffer and therefore one
/// `write_all`, because two writes are not one append.
///
/// Twenty records in the August 2026 corpus share a physical line with the
/// record that followed them, and exactly twenty blank lines accompany them —
/// `grep -c '^$'` returns the same number as the merged-line count, in both
/// the live file and the rotated one. That pairing is the signature, and it
/// only has one cause:
///
///     A-payload  B-payload  A-newline  B-newline
///     └──────── one physical line ───┘ └ blank ┘
///
/// Moving the trace onto a single writer thread (commit e9449e0) serialises
/// the trace's own callers *within this process*. It does not serialise a
/// second process appending to the same path, which is the ordinary state of
/// this machine while TTP is being developed: an installed build and a
/// `tauri dev` build share one log directory and one `ttp-trace.log`. Nor
/// does it serialise `ttp.log`, whose writers are still every thread in the
/// app. Framing the record atomically closes all of those at once, because
/// `write(2)` on a file opened `O_APPEND` positions and writes under the
/// inode lock — no other writer's bytes can land inside it.
fn frame_record(line: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    buf
}

/// Append one line to `path`, rotating first if needed. Failures are counted
/// rather than propagated: logging must never be able to break the thing it
/// is observing, but it must not be able to go quiet without saying so
/// either.
fn append_line(path: &Path, max_size: u64, keep: usize, line: &str) -> bool {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    rotate_if_needed(path, max_size, keep);

    let wrote = match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => file.write_all(&frame_record(line)).is_ok(),
        Err(_) => false,
    };
    if !wrote {
        WRITE_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    wrote
}

/// Write a log entry to the persistent log file.
fn log_to_file_at(level: Level, message: &str) {
    if level > active_level() {
        return;
    }
    let Some(path) = log_path() else { return };

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] [{}] {}", timestamp, level.tag(), message);
    let _ = append_line(&path, MAX_LOG_SIZE, KEEP_ROTATIONS, &entry);
}

/// Append one pre-formatted line to the dictation trace.
///
/// Bypasses `active_level()` on purpose. The trace exists precisely because
/// the release-default Warn threshold hides every stage that can silently
/// swallow a transcription; gating it behind that same threshold would
/// reproduce the blind spot it was written to remove. Volume is bounded by
/// the file's own rotation, and text payloads are redacted unless the user
/// opts in — see `crate::trace`.
/// Returns whether the record reached the file, so the one caller — the
/// trace writer thread — can tell a lost line from a written one and keep the
/// failure tally honest instead of losing it along with the line that was
/// carrying it.
pub fn log_trace_line(line: &str) -> bool {
    let Some(path) = trace_path() else { return false };
    append_line(&path, MAX_TRACE_SIZE, KEEP_TRACE_ROTATIONS, line)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch path in the OS temp dir, unique per test and per run.
    fn scratch(name: &str) -> PathBuf {
        let unique = format!(
            "ttp-log-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        std::env::temp_dir().join(unique)
    }

    /// Twenty records in the August corpus share a physical line with the
    /// record that followed them, and the same twenty are followed by a blank
    /// line. That pairing is the signature of an append written as *two*
    /// syscalls — payload, then newline — with a second writer landing its own
    /// payload in between:
    ///
    ///     A-payload  B-payload  A-newline  B-newline
    ///     └──────── one physical line ───┘ └ blank ┘
    ///
    /// Moving the trace to a single writer thread serialises the callers
    /// *inside this process*. It cannot serialise a second process appending
    /// to the same file, which is the ordinary state of this machine during
    /// development (an installed build and a `tauri dev` build share one log
    /// directory). So the framing itself has to be atomic: one `write_all` of
    /// `line + "\n"`, which under `O_APPEND` the kernel completes without
    /// another writer's bytes landing inside it.
    ///
    /// This test fails on the two-write version. It is a race, so it is
    /// written to lose that race reliably: many threads, many lines, and
    /// payloads long enough that the window between the two writes is wide.
    #[test]
    fn concurrent_appends_never_share_a_line() {
        let path = scratch("interleave");
        let threads = 8;
        let per_thread = 150;
        let mut handles = Vec::new();
        for t in 0..threads {
            let p = path.clone();
            handles.push(std::thread::spawn(move || {
                for i in 0..per_thread {
                    // Long enough that the payload write is not a single
                    // trivially-fast memcpy into a warm buffer.
                    let line = format!("[{:02}-{:04}] {}", t, i, "x".repeat(600));
                    append_line(&p, u64::MAX, 0, &line);
                }
            }));
        }
        for h in handles {
            h.join().expect("writer thread panicked");
        }

        let content = std::fs::read_to_string(&path).expect("log file");
        let _ = std::fs::remove_file(&path);

        // A well-formed record is `[tt-iiii] ` followed by exactly 600 x's:
        // one '[', one ']', and 610 characters. A merged line is longer and
        // has two brackets; a torn line is shorter. Both fail this.
        let lines: Vec<&str> = content.lines().collect();
        let malformed: Vec<&&str> = lines
            .iter()
            .filter(|l| l.len() != 610 || l.matches('[').count() != 1)
            .collect();
        assert!(
            malformed.is_empty(),
            "{} of {} lines were torn or merged; first: {:?}",
            malformed.len(),
            lines.len(),
            malformed.first().map(|l| &l[..l.len().min(120)])
        );
        assert_eq!(
            lines.len(),
            threads * per_thread,
            "expected one physical line per record"
        );
        // The other half of the corpus signature: a merged line is always
        // accompanied by a blank one, because the newline that should have
        // ended it lands after the intruder's payload.
        assert_eq!(content.matches("\n\n").count(), 0, "a blank line means a torn record");
    }

    /// The rotation cascade renames the live file out from under the writer.
    /// A record must never be split across that boundary either.
    #[test]
    fn a_record_and_its_newline_are_one_write() {
        let path = scratch("framing");
        append_line(&path, u64::MAX, 0, "hello");
        let content = std::fs::read_to_string(&path).expect("log file");
        let _ = std::fs::remove_file(&path);
        assert_eq!(content, "hello\n");
    }
}
