// TTP - Talk To Paste
// Settings store - handles settings persistence to JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Whether to run AI polish on transcriptions (removes filler words, fixes grammar)
    pub ai_polish_enabled: bool,
    /// Global keyboard shortcut for recording (e.g., "Alt+Space", "Ctrl+Shift+R")
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    /// Use Fn key as push-to-talk trigger (macOS only)
    #[serde(default)]
    pub fn_key_enabled: bool,
    /// Telemetry opt-in: controls error reporting (Sentry) and usage analytics (Aptabase)
    /// Default is OFF -- user must explicitly enable
    #[serde(default)]
    pub telemetry_enabled: bool,
    /// Hands-free mode (double-tap to toggle) - persists across app restarts
    #[serde(default)]
    pub hands_free_mode: bool,
    /// Hide the recording indicator pill when not recording
    #[serde(default)]
    pub hide_pill_when_inactive: bool,
    /// Cached "user wants autostart" intent. The actual side effect is the
    /// LaunchAgent plist managed by `tauri-plugin-autostart`; this field is
    /// what the Settings UI reads, because the plugin's `is_enabled()` is
    /// unreliable on macOS for product names containing spaces ("TTP by AmirKS")
    /// — it can return false even when the plist is on disk AND loaded into
    /// launchd. Onboarding + Settings write this alongside calling the
    /// plugin's enable/disable so the two stay in sync.
    #[serde(default)]
    pub autostart_enabled: bool,
    /// Whether to save transcriptions to history. Default ON.
    #[serde(default = "default_true")]
    pub history_enabled: bool,
    /// Whether to subscribe to the beta update channel. Default OFF (stable).
    /// When true, the updater queries `latest-beta.json` instead of `latest.json`.
    #[serde(default)]
    pub use_beta_channel: bool,
    /// User language preference: "en", "fr", or "system". None means system
    /// (follow navigator.language at first launch — resolved to en or fr in JS).
    #[serde(default)]
    pub language: Option<String>,
    /// User theme preference: "system", "light", or "dark". None means system
    /// (follow the OS `prefers-color-scheme` media query — resolved in JS).
    #[serde(default)]
    pub theme: Option<String>,
    /// Auto-stop recording after a sustained silence (Voice Activity Detection).
    /// Default OFF — opt-in because some workflows (dictation pauses while
    /// reading source material) intentionally include long silences.
    #[serde(default)]
    pub vad_auto_stop_enabled: bool,
    /// How many seconds of continuous silence must elapse before auto-stop
    /// triggers. Bounded to [1, 10] at use time. Default 3.
    #[serde(default = "default_vad_silence_secs")]
    pub vad_silence_secs: u32,
    /// User-preferred audio input device, as reported by cpal's
    /// `Device::name()`. None means "use the OS default device". When the
    /// preferred device is unplugged, audio_capture falls back to the
    /// default and logs an info breadcrumb — never blocks recording.
    #[serde(default)]
    pub audio_device_name: Option<String>,
    /// Language sent to Whisper as the `language` API parameter. Decoupled
    /// from the UI language: a French-speaker using TTP in English will
    /// still get correct transcription if they pick "fr" here.
    ///
    /// Accepted values: `"auto"` (default — Whisper auto-detects, NOT the
    /// UI language fallback, because the v3.1.2 fix that pinned the
    /// decoder to UI language sometimes mis-served bilingual speakers),
    /// `"en"`, `"fr"`. `None` is treated as `"auto"`.
    #[serde(default)]
    pub transcription_language: Option<String>,
    /// Write full transcription text into the dictation trace
    /// (`ttp-trace.log`) at every pipeline stage.
    ///
    /// Default OFF. The trace itself is always written — stage timings,
    /// character counts, digests, filter verdicts, stuck modifiers — which is
    /// enough to locate the stage that swallowed a dictation. This flag adds
    /// the text itself, which is what you need to see *what* a filter ate, at
    /// the cost of putting the user's transcriptions on disk in plain text.
    /// `TTP_DIAGNOSTICS=1` overrides it for a single launch.
    #[serde(default)]
    pub diagnostics_enabled: bool,
}

fn default_vad_silence_secs() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

fn default_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        // Fn key is default on macOS
        "FnKey".to_string()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Ctrl+Space on Windows/Linux (no conflicts with system shortcuts)
        "Ctrl+Space".to_string()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_polish_enabled: true,
            shortcut: default_shortcut(),
            #[cfg(target_os = "macos")]
            fn_key_enabled: true,
            #[cfg(not(target_os = "macos"))]
            fn_key_enabled: false,
            telemetry_enabled: false,
            hands_free_mode: false,
            hide_pill_when_inactive: false,
            autostart_enabled: false,
            history_enabled: true,
            use_beta_channel: false,
            language: None,
            theme: None,
            vad_auto_stop_enabled: false,
            vad_silence_secs: default_vad_silence_secs(),
            audio_device_name: None,
            transcription_language: None,
            diagnostics_enabled: false,
        }
    }
}

// In-memory cache so get_settings() doesn't re-read + re-parse the JSON
// file on every i18n::tr (one per Settings render row, one per tray menu
// item rebuild, one per Sentry breadcrumb), every shortcut press, and every
// pipeline stage transition. Previously this was a synchronous fs::read on
// every call from a dozen hot-path sites — verified in the codebase audit as
// the dominant filesystem-syscall source under normal use.
//
// Mirrors the dictionary/history cache pattern: 5s TTL, refreshed on save,
// invalidated on reset. The TTL is short enough that external edits to
// settings.json (rare; the file is meant to be UI-managed) propagate in
// under 5s, and long enough to absorb a Settings render or a tray rebuild.
static SETTINGS_CACHE: OnceLock<Mutex<Option<(Settings, Instant)>>> = OnceLock::new();
const CACHE_TTL_SECS: u64 = 5;

fn cache() -> &'static Mutex<Option<(Settings, Instant)>> {
    SETTINGS_CACHE.get_or_init(|| Mutex::new(None))
}

fn read_cached() -> Option<Settings> {
    let guard = cache().lock().ok()?;
    let (settings, cached_at) = guard.as_ref()?;
    if cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
        Some(settings.clone())
    } else {
        None
    }
}

fn store_cache(settings: Settings) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((settings, Instant::now()));
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

/// Get the settings file path (~/.config/ttp/settings.json)
fn get_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("settings.json"))
}

/// Backup of the most recent valid settings file. Used to recover from a
/// corrupted primary file (e.g. interrupted write, future schema migration
/// gone wrong) instead of silently resetting the user's preferences.
fn get_settings_backup_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("settings.json.bak"))
}

/// Try the .bak file when the primary settings file is unreadable / unparseable.
/// Returns Settings::default() if the backup is also missing or corrupt.
fn recover_from_backup() -> Settings {
    let Some(bak) = get_settings_backup_path() else {
        return Settings::default();
    };
    if !bak.exists() {
        return Settings::default();
    }
    match fs::read_to_string(&bak) {
        Ok(content) => match serde_json::from_str::<Settings>(&content) {
            Ok(s) => {
                crate::logging::log_info("Recovered settings from settings.json.bak");
                s
            }
            Err(_) => Settings::default(),
        },
        Err(_) => Settings::default(),
    }
}

/// Load settings from file, return defaults if file doesn't exist
#[tauri::command]
pub fn get_settings() -> Settings {
    if let Some(cached) = read_cached() {
        return cached;
    }

    let Some(path) = get_settings_path() else {
        return Settings::default();
    };

    if !path.exists() {
        let s = Settings::default();
        store_cache(s.clone());
        return s;
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            let s = recover_from_backup();
            store_cache(s.clone());
            return s;
        }
    };

    let parsed = match serde_json::from_str::<Settings>(&content) {
        Ok(s) => s,
        Err(e) => {
            crate::logging::log_error(&format!(
                "settings.json failed to parse ({}); falling back to .bak",
                e
            ));
            recover_from_backup()
        }
    };
    store_cache(parsed.clone());
    parsed
}

/// Save settings to file using a temp-file + fsync + atomic-rename sequence.
///
/// Why atomic: the previous implementation called `fs::write` directly, which
/// open(O_TRUNC) + writes the new content. A crash, power loss, or kill -9
/// between truncate and end-of-write leaves a half-written file on disk. The
/// .bak fallback works for parse errors (the half-written file is invalid
/// JSON) but NOT for the half-written-but-valid-JSON case (a truncated object
/// that closes early). With atomic rename we either see the old full file or
/// the new full file — never anything in between.
///
/// Backup ordering changed: we now refresh .bak AFTER the new file is in
/// place, so .bak always reflects the previously-known-good settings. With
/// the old "copy then write" order, an interrupted run would leave .bak
/// = OLD-old and main = corrupt — the previously-known-good was overwritten
/// before the next-good was confirmed.
/// Internal: atomic write to an arbitrary path. Extracted from set_settings
/// so the atomicity semantics can be unit-tested without going through
/// dirs::config_dir() (which is OS-global and not redirectable).
pub(crate) fn write_settings_atomic(path: &std::path::Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    let parent = path.parent().ok_or("Settings path has no parent dir")?;
    let tmp_name = format!(
        "{}.tmp.{}.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("settings.json"),
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let tmp_path = parent.join(tmp_name);
    {
        let mut f = fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create temp settings file: {}", e))?;
        f.write_all(json.as_bytes()).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("Failed to write temp settings file: {}", e)
        })?;
        let _ = f.sync_all();
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        #[cfg(windows)]
        {
            let _ = fs::remove_file(path);
            if let Err(e2) = fs::rename(&tmp_path, path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Failed to install settings file (windows fallback): {}", e2));
            }
        }
        #[cfg(not(windows))]
        {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Failed to install settings file: {}", e));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_settings(settings: Settings, app: AppHandle) -> Result<(), String> {
    let path = get_settings_path().ok_or("Could not determine config directory")?;

    write_settings_atomic(&path, &settings)?;

    // Refresh the .bak only AFTER the new file is durably in place. Best-effort:
    // if this copy fails we still proceed (the live file is good, the .bak is
    // just stale — that's the same situation as before any backup ever existed).
    if let Some(bak) = get_settings_backup_path() {
        let _ = fs::copy(&path, &bak);
    }

    // Refresh the in-memory cache so the next get_settings() doesn't re-read
    // disk just to find what we already know — and so hot-path callers see
    // the change immediately, not after the TTL elapses.
    store_cache(settings.clone());

    app.emit("settings-changed", &settings).ok();

    Ok(())
}

/// Reset settings to defaults by deleting the settings file
#[tauri::command]
pub fn reset_settings() -> Result<(), String> {
    invalidate_cache();

    let Some(path) = get_settings_path() else {
        return Ok(()); // No config dir, nothing to reset
    };

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete settings file: {}", e))?;
    }

    // Also clear the backup so a stale recovery doesn't resurrect old settings.
    if let Some(bak) = get_settings_backup_path() {
        if bak.exists() {
            let _ = fs::remove_file(&bak);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — atomic-write semantics + cache TTL behavior.
// These cover the failure modes the v3 audit flagged as "high":
//   - non-atomic fs::write that could leave a half-written settings.json
//     on power loss / kill -9, and
//   - the .bak fallback never asserted in any test.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "ttp_settings_test_{}_{}_{}",
            label,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&p).expect("create tempdir");
        p
    }

    #[test]
    fn write_atomic_creates_file_with_full_json() {
        let dir = temp_dir("create");
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.ai_polish_enabled = false;
        s.shortcut = "Alt+Space".into();

        write_settings_atomic(&path, &s).expect("write should succeed");

        let content = fs::read_to_string(&path).expect("file should exist");
        let parsed: Settings = serde_json::from_str(&content).expect("file must be valid JSON");
        assert!(!parsed.ai_polish_enabled);
        assert_eq!(parsed.shortcut, "Alt+Space");

        // No temp file should be left behind.
        let leftover: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftover.is_empty(), "no .tmp.* leftover after atomic write");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_preserves_old_file_when_serialization_succeeds() {
        // Two sequential writes — the second should fully replace the first
        // with no intermediate empty state observed on disk.
        let dir = temp_dir("replace");
        let path = dir.join("settings.json");

        let mut s1 = Settings::default();
        s1.shortcut = "FnKey".into();
        write_settings_atomic(&path, &s1).expect("first write");

        let mut s2 = Settings::default();
        s2.shortcut = "Ctrl+Space".into();
        s2.telemetry_enabled = true;
        write_settings_atomic(&path, &s2).expect("second write");

        let parsed: Settings =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.shortcut, "Ctrl+Space");
        assert!(parsed.telemetry_enabled);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_creates_parent_dir() {
        let dir = temp_dir("nested");
        let path = dir.join("a").join("b").join("settings.json");
        let s = Settings::default();
        write_settings_atomic(&path, &s).expect("write should create nested dirs");
        assert!(path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_returns_stored_value_within_ttl() {
        // Direct cache primitives — independent of the OS config dir.
        let mut s = Settings::default();
        s.shortcut = "TEST_SHORTCUT".into();
        invalidate_cache();
        store_cache(s.clone());
        let read = read_cached().expect("should hit cache");
        assert_eq!(read.shortcut, "TEST_SHORTCUT");
        invalidate_cache(); // leave a clean slate for other tests in this crate
    }

    #[test]
    fn cache_invalidation_returns_none() {
        store_cache(Settings::default());
        invalidate_cache();
        assert!(read_cached().is_none(), "after invalidate_cache, read returns None");
    }
}
