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
    /// Which start/stop sound set plays. `None` or an unknown/locked id
    /// resolves to the house sounds — see `cosmetics::effective_sound_pack`.
    #[serde(default)]
    pub sound_pack: Option<String>,
    /// Whether the pill draws a face. Independent of the sound choice on
    /// purpose: a face in peripheral vision is the part most likely to wear
    /// out its welcome, and it must be switchable without giving up the rest.
    #[serde(default)]
    pub companion_face_enabled: bool,
    /// What the user named their pill. Purely local, never sent anywhere.
    /// The naming is the point — it is what converts a purchase into a
    /// possession — so an empty value simply means "not named yet".
    #[serde(default)]
    pub companion_name: Option<String>,
    /// Epoch seconds at which `companion_face_enabled` last became true;
    /// `None` while the face is off.
    ///
    /// The denominator for the fourteen-day survival question in
    /// `docs/companion-faces-design.md` §2.2: "how long did the face last
    /// before it was turned off" needs a start, and a process-lifetime timer
    /// would reset on every relaunch. Managed entirely by `set_settings` —
    /// the frontend never sends it, and a payload that omits it (every
    /// payload, since the UI does not know the field) keeps the stored value
    /// rather than clearing it.
    #[serde(default)]
    pub companion_face_enabled_at: Option<i64>,
    /// Which coat the app wears — the Pro aesthetic's palette, type stack,
    /// radius and motion character, all selected together. `None` means the
    /// house coat (`wild`).
    ///
    /// Stored opaquely on purpose. The catalogue lives in
    /// `src/lib/theme-coats.ts` next to `src/styles/coats.css`, which is the
    /// only place that can actually implement a coat, and `effectiveCoat`
    /// there falls back to `wild` for any id it does not recognise — the same
    /// silent, total degradation `cosmetics::effective_sound_pack` gives
    /// sounds. A second copy of the id list here would be a second thing to
    /// forget to update, and it would let Rust reject a coat the frontend can
    /// paint perfectly well.
    ///
    /// This used to live in `localStorage` alone, which meant it did not
    /// survive clearing site data. `localStorage` is still written, but now as
    /// a MIRROR — the anti-flash script in `index.html` runs before any of our
    /// code and cannot await an IPC call, so it needs a synchronous copy. This
    /// field is the source of truth; the mirror is repaired from it on boot.
    #[serde(default)]
    pub coat: Option<String>,
    /// Every key in `settings.json` that this build has no field for.
    ///
    /// Not decoration — see the `payload_merge_tests` module for the failure
    /// this closes. Without it, a settings file written by a newer binary
    /// loses its new settings the first time an older binary saves, and a
    /// caller that misspells a field is told nothing at all.
    ///
    /// `flatten` means these are spliced back in at the top level on write, so
    /// they round-trip byte-for-byte in value rather than being parked under a
    /// wrapper key that a newer build would then have to know about.
    #[serde(flatten)]
    pub unknown: serde_json::Map<String, serde_json::Value>,
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
            sound_pack: None,
            companion_face_enabled: false,
            companion_name: None,
            companion_face_enabled_at: None,
            coat: None,
            unknown: serde_json::Map::new(),
        }
    }
}

// ── Companion survival instrumentation ──────────────────────────────────
//
// `docs/companion-faces-design.md` §2 asks one question — does a face in
// peripheral vision survive fourteen days of ordinary work — and answers it
// from the trace rather than from memory, because the test subject is also
// the person who wants the answer to be yes.
//
// Everything below is slugs, booleans and numbers. In particular
// `companion.named` records the LENGTH of the name and never the name: it is
// user text, it is local and private, and a diagnostic log is not where it
// belongs. That is not a style preference, it is the same rule that keeps
// transcriptions behind `diagnostics_enabled`.

/// Fractional days since `since`, or 0.0 when the clock has nothing to say.
fn days_since(since: Option<i64>) -> f64 {
    let Some(start) = since else { return 0.0 };
    let now = chrono::Utc::now().timestamp();
    if now <= start {
        return 0.0;
    }
    ((now - start) as f64 / 86_400.0 * 100.0).round() / 100.0
}

/// Dictations completed since `since`, from the daily buckets the usage store
/// already keeps. Day-granular, which is the resolution the question needs —
/// no new counter, and nothing extra on the dictation path.
fn dictations_since(since: Option<i64>) -> u32 {
    let Some(start) = since else { return 0 };
    let Some(start_day) = chrono::DateTime::from_timestamp(start, 0) else {
        return 0;
    };
    let start_key = start_day.format("%Y-%m-%d").to_string();
    crate::usage::load_usage()
        .daily_stats
        .iter()
        .filter(|(date, _)| **date >= start_key)
        .map(|(_, stats)| stats.transcriptions)
        .sum()
}

/// Emit the companion events implied by the difference between two settings.
///
/// Called from `set_settings` with the value that was stored and the value
/// about to replace it. Emits nothing when nothing companion-related moved,
/// which is almost every save.
fn companion_events(
    prev: &Settings,
    next: &Settings,
    days_on: f64,
    dictations_on: u32,
) -> Vec<(&'static str, serde_json::Value)> {
    let mut out = Vec::new();

    if prev.companion_face_enabled != next.companion_face_enabled {
        // The primary measurement. On enable both spans are zero; on disable
        // they are the finding — how long the face lasted, and across how
        // many dictations.
        out.push((
            "companion.face",
            serde_json::json!({
                "enabled": next.companion_face_enabled,
                "days_on": days_on,
                "dictations_on": dictations_on,
            }),
        ));
    }

    if prev.companion_name != next.companion_name {
        let len = next
            .companion_name
            .as_deref()
            .map(|n| n.chars().count())
            .unwrap_or(0);
        out.push((
            "companion.named",
            serde_json::json!({ "len": len, "cleared": len == 0 }),
        ));
    }

    if prev.hide_pill_when_inactive != next.hide_pill_when_inactive {
        // The sharper tell, per §2.3: hiding the idle pill while the face is
        // still enabled is "get this off my screen" without saying why.
        // `face_on` is the whole point of the event — the correlation is the
        // signal, and the event is nearly worthless without it.
        out.push((
            "companion.pill_hidden",
            serde_json::json!({
                "hidden": next.hide_pill_when_inactive,
                "face_on": next.companion_face_enabled,
            }),
        ));
    }

    out
}

/// Emit whatever `companion_events` decided. The split exists so the decision
/// can be tested without a filesystem, a usage record or a writer thread —
/// and the spans are computed here, once, because `dictations_since` reads
/// (and HMAC-verifies) the usage file and must not run when nothing changed.
fn trace_companion_diff(prev: &Settings, next: &Settings) {
    let face_changed = prev.companion_face_enabled != next.companion_face_enabled;
    let name_changed = prev.companion_name != next.companion_name;
    let pill_changed = prev.hide_pill_when_inactive != next.hide_pill_when_inactive;
    if !(face_changed || name_changed || pill_changed) {
        return;
    }
    let (days_on, dictations_on) = if face_changed {
        (
            days_since(prev.companion_face_enabled_at),
            dictations_since(prev.companion_face_enabled_at),
        )
    } else {
        (0.0, 0)
    };
    for (name, fields) in companion_events(prev, next, days_on, dictations_on) {
        crate::trace::event(name, fields);
    }
}

/// Where `companion_face_enabled_at` should land after a save.
///
/// Carry the stored value forward while the face stays on, stamp on the
/// transition to on, clear on the transition to off. Pure so the rule can be
/// tested; the alternative is a test that has to write settings files and
/// wait for a clock.
fn resolve_face_timestamp(
    enabled: bool,
    incoming: Option<i64>,
    stored: Option<i64>,
    now: i64,
) -> Option<i64> {
    if !enabled {
        return None;
    }
    incoming.or(stored).or(Some(now))
}

/// One line per session, next to `app.launched`.
///
/// Rotation is why this exists: a `companion.face {"enabled":false}` line can
/// age out of the retained window, and without a state line at every session
/// boundary the log would then show no evidence the face had ever been on.
pub fn trace_companion_state() {
    let s = get_settings();
    crate::trace::event(
        "companion.state",
        serde_json::json!({
            "face": s.companion_face_enabled,
            "named": s.companion_name.as_deref().map(|n| !n.is_empty()).unwrap_or(false),
            "pack": crate::cosmetics::effective_sound_pack(s.sound_pack.as_deref()).id,
            "pill_hidden": s.hide_pill_when_inactive,
            "days_on": days_since(s.companion_face_enabled_at),
            "dictations_on": dictations_since(s.companion_face_enabled_at),
        }),
    );
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
        // The fsync is not decoration: it is the single step that makes the
        // temp-write-then-rename dance crash-safe, and `architecture.md`
        // advertises it — "temp file → fsync → atomic rename … so a crash
        // mid-write can never corrupt the live settings file".
        //
        // Discarding its error voided exactly that. If the fsync fails, the
        // rename below installs a file whose bytes are not on disk,
        // `set_settings` returns `Ok(())`, the in-memory cache is refreshed
        // with the new values, and the app is confidently serving settings
        // that will not survive a power cut. The user was told nothing and
        // the trace recorded nothing — one line below a `write_all` whose
        // error is caught, cleaned up after, and described.
        f.sync_all().map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            // Recorded as well as returned. The caller surfaces the string;
            // this makes the event greppable next to the dictation it
            // happened during, which is how anyone would find it.
            crate::trace::degraded(
                "settings.fsync",
                serde_json::json!({ "error": e.to_string(), "installed": false }),
            );
            format!("Failed to fsync temp settings file: {}", e)
        })?;
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

/// The field names this build's `Settings` struct actually has.
///
/// Derived by serialising a default rather than written out by hand: a literal
/// list would go stale the moment somebody added a field, and the machinery
/// that exists to report unknown fields would then report every new field as
/// unknown. `unknown` is `flatten`ed and empty in a default, so it contributes
/// no name of its own.
fn known_field_names() -> std::collections::BTreeSet<String> {
    match serde_json::to_value(Settings::default()) {
        Ok(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        _ => std::collections::BTreeSet::new(),
    }
}

/// Merge an incoming settings payload over the stored settings.
///
/// Returns the merged value together with the names of the keys the caller
/// sent that this build does not recognise. Those keys are NOT discarded —
/// they ride in `Settings::unknown` and are written back out — but they are
/// named so the caller's mistake, or the newer build's new setting, is
/// visible instead of inferred.
///
/// Merge semantics, stated because they are the whole point:
///   - a key the payload omits keeps its stored value,
///   - a key the payload sets to `null` clears the field, so "unset this" is
///     still expressible and is distinct from "I did not mention it",
///   - a key of the wrong type is an error, not a shrug.
fn merge_payload(
    stored: &Settings,
    incoming: &serde_json::Value,
) -> Result<(Settings, Vec<String>), String> {
    let incoming = incoming
        .as_object()
        .ok_or_else(|| "settings payload must be a JSON object".to_string())?;

    let mut merged = match serde_json::to_value(stored) {
        Ok(serde_json::Value::Object(map)) => map,
        Ok(_) => return Err("stored settings did not serialise to an object".to_string()),
        Err(e) => return Err(format!("Failed to read stored settings: {}", e)),
    };

    let known = known_field_names();
    let mut unknown = Vec::new();
    for (key, value) in incoming {
        if !known.contains(key.as_str()) {
            unknown.push(key.clone());
        }
        merged.insert(key.clone(), value.clone());
    }

    let settings: Settings = serde_json::from_value(serde_json::Value::Object(merged))
        .map_err(|e| format!("Failed to apply settings payload: {}", e))?;
    Ok((settings, unknown))
}

/// Say out loud that a field arrived that this build does not know.
///
/// Preserving it silently would be the same anti-pattern in a nicer coat: the
/// data would survive, and a typo'd field name — `hide_pill_when_idle` for
/// `hide_pill_when_inactive` — would still look like a save that worked. So it
/// is both: preserved AND reported, with the field named, because a count
/// tells you something is wrong and nothing about what.
fn report_unknown_fields(unknown: &[String]) {
    for field in unknown {
        crate::trace::event(
            "settings.unknown_field",
            serde_json::json!({ "field": field, "preserved": true }),
        );
        crate::logging::log_warn(&format!(
            "set_settings received a field this build does not know: `{}`. \
             It has been preserved in settings.json, not applied. Either it \
             comes from a newer build or it is a misspelt field name.",
            field
        ));
    }
}

/// Persist a settings payload.
///
/// Takes raw JSON rather than a `Settings` on purpose. Deserialising the
/// command argument straight into the struct — which is what this did until
/// the P1 debt payoff — applied two silent edits to the caller's intent: an
/// unrecognised key vanished, and a key the caller simply did not mention was
/// reset to its default. The second is what kept the coat out of this store
/// entirely (`src/lib/theme-coats.ts` names the reason), and the first is why
/// `companion_face_enabled_at` needed hand-written carry-forward code that
/// every future field would have needed too.
#[tauri::command]
pub fn set_settings(settings: serde_json::Value, app: AppHandle) -> Result<(), String> {
    let path = get_settings_path().ok_or("Could not determine config directory")?;

    // Read the outgoing value before it is overwritten. `get_settings()` is
    // the cache, and the cache is refreshed on every write, so it holds
    // exactly what this call is about to replace.
    let previous = get_settings();

    let (mut settings, unknown) = merge_payload(&previous, &settings)?;
    report_unknown_fields(&unknown);

    // Own the timestamp here rather than in the UI. The frontend does not
    // know this field exists and every payload it sends omits it, so the
    // rules are: carry the stored value forward while the face stays on,
    // stamp on the transition to on, clear on the transition to off.
    //
    // The merge above already carries an omitted field forward, so
    // `incoming` and `stored` are now the same value in the common case. The
    // rule is kept because the transitions — stamp on enable, clear on
    // disable — are not merge behaviour and still have to be applied.
    settings.companion_face_enabled_at = resolve_face_timestamp(
        settings.companion_face_enabled,
        settings.companion_face_enabled_at,
        previous.companion_face_enabled_at,
        chrono::Utc::now().timestamp(),
    );

    write_settings_atomic(&path, &settings)?;

    trace_companion_diff(&previous, &settings);

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
mod companion_instrumentation_tests {
    use super::*;

    fn base() -> Settings {
        Settings::default()
    }

    fn names(events: &[(&'static str, serde_json::Value)]) -> Vec<&'static str> {
        events.iter().map(|(n, _)| *n).collect()
    }

    #[test]
    fn an_unrelated_save_emits_nothing() {
        // set_settings runs on every toggle in the Settings window. If this
        // ever fires on an unrelated change, the fourteen-day window fills
        // with noise and the signal it was built for is unreadable.
        let prev = base();
        let mut next = base();
        next.ai_polish_enabled = !prev.ai_polish_enabled;
        next.vad_silence_secs = 9;
        assert!(companion_events(&prev, &next, 0.0, 0).is_empty());
    }

    #[test]
    fn enabling_the_face_reports_zero_spans() {
        let prev = base();
        let mut next = base();
        next.companion_face_enabled = true;
        let ev = companion_events(&prev, &next, 0.0, 0);
        assert_eq!(names(&ev), vec!["companion.face"]);
        assert_eq!(ev[0].1["enabled"], true);
        assert_eq!(ev[0].1["days_on"], 0.0);
        assert_eq!(ev[0].1["dictations_on"], 0);
    }

    #[test]
    fn disabling_the_face_carries_the_span_it_survived() {
        // The finding, per docs/companion-faces-design.md §2.3(1): the answer
        // is not "it was turned off", it is "it was turned off after N days
        // and M dictations".
        let mut prev = base();
        prev.companion_face_enabled = true;
        prev.companion_face_enabled_at = Some(1_756_000_000);
        let next = base();
        let ev = companion_events(&prev, &next, 4.25, 118);
        assert_eq!(ev[0].1["enabled"], false);
        assert_eq!(ev[0].1["days_on"], 4.25);
        assert_eq!(ev[0].1["dictations_on"], 118);
    }

    #[test]
    fn hiding_the_pill_records_whether_the_face_was_still_on() {
        // §2.3(2) rates this the more reliable tell, and it is only a tell in
        // conjunction with `face_on` — "get this off my screen" while the
        // face is still enabled. Without the correlation the event says
        // nothing about the face at all.
        let mut prev = base();
        prev.companion_face_enabled = true;
        let mut next = prev.clone();
        next.hide_pill_when_inactive = true;
        let ev = companion_events(&prev, &next, 0.0, 0);
        assert_eq!(names(&ev), vec!["companion.pill_hidden"]);
        assert_eq!(ev[0].1["hidden"], true);
        assert_eq!(ev[0].1["face_on"], true);
    }

    #[test]
    fn hiding_the_pill_with_no_face_is_recorded_but_not_a_tell() {
        let prev = base();
        let mut next = base();
        next.hide_pill_when_inactive = true;
        let ev = companion_events(&prev, &next, 0.0, 0);
        assert_eq!(ev[0].1["face_on"], false);
    }

    #[test]
    fn naming_records_the_length_and_never_the_name() {
        // Non-negotiable: the name is user text. A diagnostic log that
        // contains it is a privacy defect, not a richer diagnostic.
        let prev = base();
        let mut next = base();
        next.companion_name = Some("Hervé".to_string());
        let ev = companion_events(&prev, &next, 0.0, 0);
        assert_eq!(names(&ev), vec!["companion.named"]);
        assert_eq!(ev[0].1["len"], 5); // chars, not bytes — "Hervé" is 6 bytes
        assert_eq!(ev[0].1["cleared"], false);
        let rendered = ev[0].1.to_string();
        assert!(
            !rendered.contains("Herv"),
            "the companion's name reached the trace payload: {}",
            rendered
        );
    }

    #[test]
    fn clearing_the_name_is_distinguishable_from_never_naming() {
        let mut prev = base();
        prev.companion_name = Some("Hervé".to_string());
        let mut next = base();
        next.companion_name = Some(String::new());
        let ev = companion_events(&prev, &next, 0.0, 0);
        assert_eq!(ev[0].1["len"], 0);
        assert_eq!(ev[0].1["cleared"], true);
    }

    #[test]
    fn several_companion_changes_in_one_save_all_report() {
        let prev = base();
        let mut next = base();
        next.companion_face_enabled = true;
        next.companion_name = Some("Bip".to_string());
        next.hide_pill_when_inactive = true;
        assert_eq!(
            names(&companion_events(&prev, &next, 0.0, 0)),
            vec!["companion.face", "companion.named", "companion.pill_hidden"]
        );
    }

    #[test]
    fn the_enable_timestamp_is_stamped_once_and_carried_forward() {
        const NOW: i64 = 1_756_600_000;
        // Turning it on with nothing stored: stamp now.
        assert_eq!(resolve_face_timestamp(true, None, None, NOW), Some(NOW));
        // Any later save while it stays on: the frontend omits the field, so
        // the stored value has to survive or `days_on` resets on every toggle
        // of an unrelated setting.
        assert_eq!(
            resolve_face_timestamp(true, None, Some(1_756_000_000), NOW),
            Some(1_756_000_000)
        );
        // Turning it off clears it, so a later re-enable measures the second
        // run and not the first.
        assert_eq!(resolve_face_timestamp(false, Some(1_756_000_000), Some(1_756_000_000), NOW), None);
    }

    #[test]
    fn days_since_is_zero_for_absent_or_future_timestamps() {
        assert_eq!(days_since(None), 0.0);
        // A clock that moved backwards must not produce a negative span the
        // analysis would then average into a nonsense number.
        assert_eq!(days_since(Some(chrono::Utc::now().timestamp() + 10_000)), 0.0);
    }

    #[test]
    fn days_since_reports_fractional_days() {
        let twelve_hours_ago = chrono::Utc::now().timestamp() - 43_200;
        let d = days_since(Some(twelve_hours_ago));
        assert!((d - 0.5).abs() < 0.01, "expected ~0.5 days, got {}", d);
    }

    #[test]
    fn companion_face_timestamp_survives_a_serde_round_trip() {
        // The field is `#[serde(default)]` and the frontend never sends it;
        // if it failed to persist, `days_on` would be zero on every disable
        // and the fourteen-day question would be unanswerable.
        let mut s = Settings::default();
        s.companion_face_enabled = true;
        s.companion_face_enabled_at = Some(1_756_000_000);
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.companion_face_enabled_at, Some(1_756_000_000));

        // And a payload from the current frontend, which does not know the
        // field exists, deserialises rather than failing — otherwise adding
        // the field would break every save from the Settings window.
        let mut obj: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&serde_json::to_string(&Settings::default()).unwrap()).unwrap();
        obj.remove("companion_face_enabled_at");
        assert!(!obj.contains_key("companion_face_enabled_at"));
        let from_ui: Settings =
            serde_json::from_value(serde_json::Value::Object(obj)).expect("frontend payload loads");
        assert_eq!(from_ui.companion_face_enabled_at, None);
    }
}

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

// ─────────────────────────────────────────────────────────────────────────────
// The silent drop.
//
// `set_settings` used to take a `Settings` directly, which meant serde applied
// two silent transformations to whatever the frontend sent:
//
//   1. A key the struct has never heard of was DISCARDED. That is the reason
//      `src/lib/theme-coats.ts` refused to persist the coat here at all and
//      routed around the settings store into `localStorage` instead — the
//      comment at the top of that file says so in as many words.
//   2. A key the struct DOES know but the payload omitted was reset to its
//      `#[serde(default)]`. The frontend's `Settings` type does not carry
//      `companion_face_enabled_at`, and it does not carry `coat`, so every
//      unrelated toggle in the Settings window would have wiped both. The
//      timestamp survived only because `set_settings` carried it forward by
//      hand, one field at a time — a fix that has to be remembered again for
//      every field added after it.
//
// Both are the polite-degradation shape `docs/engineering-standards.md` was
// written about: the store quietly did something other than what it was asked,
// returned `Ok(())`, and left no record.
//
// The replacement merges the incoming object over the stored one:
//   - an omitted key keeps its stored value (no per-field carry-forward),
//   - an explicit `null` still clears a nullable field, so "unset this" is
//     expressible and is distinct from "I did not mention it",
//   - an unknown key is preserved through `#[serde(flatten)]` AND named in a
//     trace event, so a newer build's setting written by a newer binary is not
//     destroyed by an older one, and nobody has to guess where it went.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod payload_merge_tests {
    use super::*;
    use serde_json::json;

    fn merged(stored: &Settings, incoming: serde_json::Value) -> (Settings, Vec<String>) {
        merge_payload(stored, &incoming).expect("payload should merge")
    }

    /// What the Settings window actually sends: every field its TypeScript
    /// `Settings` interface knows about, and nothing else.
    fn frontend_payload() -> serde_json::Value {
        json!({
            "ai_polish_enabled": true,
            "shortcut": "FnKey",
            "fn_key_enabled": true,
            "telemetry_enabled": false,
            "hands_free_mode": false,
            "hide_pill_when_inactive": false,
            "autostart_enabled": false,
            "history_enabled": true,
            "use_beta_channel": false,
            "vad_auto_stop_enabled": false,
            "vad_silence_secs": 3,
            "audio_device_name": null,
            "transcription_language": null,
            "diagnostics_enabled": false,
            "sound_pack": "bowl",
            "companion_face_enabled": true,
            "companion_name": "Bip",
            "language": "fr",
            "theme": "dark"
        })
    }

    #[test]
    fn an_omitted_field_keeps_its_stored_value() {
        // THE COAT BUG. The frontend's payload has no `coat` key, because its
        // TypeScript interface has no such field. Under the old
        // `settings: Settings` signature this deserialised to `None` and the
        // user's coat was gone the next time they toggled anything at all.
        let mut stored = Settings::default();
        stored.coat = Some("merle".to_string());
        stored.companion_face_enabled_at = Some(1_756_000_000);

        let (out, unknown) = merged(&stored, frontend_payload());

        assert_eq!(out.coat.as_deref(), Some("merle"));
        assert_eq!(out.companion_face_enabled_at, Some(1_756_000_000));
        assert!(unknown.is_empty(), "the frontend payload is all known fields");
        // and the fields it did send are applied
        assert_eq!(out.sound_pack.as_deref(), Some("bowl"));
        assert_eq!(out.language.as_deref(), Some("fr"));
    }

    #[test]
    fn an_explicit_null_still_clears_a_field() {
        // "Leave it alone" and "unset it" must stay distinguishable, or the
        // merge trades one silent failure for another: a user who picks the
        // system default audio device again could never get back to `None`.
        let mut stored = Settings::default();
        stored.audio_device_name = Some("Yeti".to_string());
        stored.coat = Some("roan".to_string());

        let (out, _) = merged(&stored, json!({ "audio_device_name": null }));
        assert_eq!(out.audio_device_name, None);
        assert_eq!(out.coat.as_deref(), Some("roan"), "untouched by an unrelated clear");
    }

    #[test]
    fn a_field_the_struct_does_not_know_survives_the_round_trip() {
        // The scenario is ordinary here: an installed 3.1.7 build and a dev
        // build share one settings.json (see docs/overhaul-status.md on the
        // lost-newline defect — "an installed build alongside a dev build is
        // the normal state"). The older binary must not eat the newer one's
        // settings.
        let stored = Settings::default();
        let (out, unknown) = merged(&stored, json!({ "coat": "piebald", "gait": "ambling" }));

        assert_eq!(unknown, vec!["gait".to_string()]);
        let written = serde_json::to_value(&out).expect("serialises");
        assert_eq!(
            written.get("gait").and_then(|v| v.as_str()),
            Some("ambling"),
            "an unrecognised field was dropped on the way back out"
        );
        // It is spliced in at the top level, not parked under a wrapper key.
        assert!(written.get("unknown").is_none());
    }

    #[test]
    fn an_unknown_field_already_on_disk_is_carried_by_a_save_that_never_mentions_it() {
        let mut stored = Settings::default();
        stored.unknown.insert("gait".to_string(), json!("ambling"));

        let (out, unknown) = merged(&stored, frontend_payload());
        assert!(unknown.is_empty(), "it came from disk, not from this caller");
        assert_eq!(out.unknown.get("gait"), Some(&json!("ambling")));
    }

    #[test]
    fn every_unknown_field_is_named_so_the_trace_can_say_which() {
        // A count would tell you something went wrong and nothing about what.
        let stored = Settings::default();
        let (_, unknown) = merged(
            &stored,
            json!({ "gait": 1, "withers": 2, "dentition": 3 }),
        );
        let mut sorted = unknown.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["dentition", "gait", "withers"]);
    }

    #[test]
    fn a_payload_that_is_not_an_object_is_refused_rather_than_ignored() {
        let stored = Settings::default();
        let err = merge_payload(&stored, &json!(["not", "an", "object"]))
            .expect_err("an array is not a settings payload");
        assert!(err.contains("object"), "the error names the problem: {}", err);
    }

    #[test]
    fn a_payload_with_a_wrongly_typed_field_fails_loudly() {
        // Previously this deserialised the whole command argument and Tauri
        // rejected the call; the merge must not turn it into a shrug.
        let stored = Settings::default();
        assert!(merge_payload(&stored, &json!({ "vad_silence_secs": "three" })).is_err());
    }

    #[test]
    fn the_known_field_list_is_derived_from_the_struct_and_not_hand_written() {
        // If this were a literal list it would go stale the first time
        // somebody added a field, and every new field would then be reported
        // as unknown by the very machinery meant to catch unknown fields.
        let names = known_field_names();
        assert!(names.contains("coat"));
        assert!(names.contains("companion_face_enabled_at"));
        assert!(names.contains("ai_polish_enabled"));
        assert!(!names.contains("unknown"), "the catch-all map is not itself a field");
    }
}

#[cfg(test)]
mod coat_persistence_tests {
    use super::*;
    use std::fs;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "ttp_coat_test_{}_{}_{}",
            label,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&p).expect("create tempdir");
        p.join("settings.json")
    }

    #[test]
    fn the_coat_survives_a_write_and_a_read() {
        // The whole point of debt item 1: a coat in localStorage does not
        // survive clearing site data. A coat in settings.json does.
        let path = temp_path("roundtrip");
        let mut s = Settings::default();
        s.coat = Some("tortie".to_string());
        write_settings_atomic(&path, &s).expect("write");

        let back: Settings =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).expect("parse");
        assert_eq!(back.coat.as_deref(), Some("tortie"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_settings_file_written_before_coats_existed_still_loads() {
        // Every existing user's settings.json. If this failed, the upgrade
        // would take everyone through `recover_from_backup` to defaults.
        let json = r#"{
            "ai_polish_enabled": true,
            "shortcut": "FnKey",
            "fn_key_enabled": true
        }"#;
        let s: Settings = serde_json::from_str(json).expect("an old file must still parse");
        assert_eq!(s.coat, None);
        assert!(s.unknown.is_empty());
    }

    #[test]
    fn an_unknown_field_on_disk_is_not_erased_by_writing_the_file_back() {
        let path = temp_path("preserve");
        let json = r#"{
            "ai_polish_enabled": true,
            "shortcut": "FnKey",
            "fn_key_enabled": true,
            "gait": "ambling"
        }"#;
        let s: Settings = serde_json::from_str(json).expect("parse");
        assert_eq!(s.unknown.get("gait"), Some(&serde_json::json!("ambling")));

        write_settings_atomic(&path, &s).expect("write");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"gait\""), "written file lost the field: {}", raw);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_coat_is_stored_opaquely_and_validated_where_the_catalogue_lives() {
        // Rust does not keep a second copy of the coat catalogue. The ids live
        // in `src/lib/theme-coats.ts` (COATS) next to the CSS that implements
        // them, and `effectiveCoat` falls back to `wild` for anything it does
        // not recognise, exactly as `cosmetics::effective_sound_pack` does for
        // packs. Storing an id Rust cannot vet is therefore fine and must not
        // become an error — a hand-edited file should wear the house coat, not
        // fail to load.
        let s: Settings = serde_json::from_str(r#"{
            "ai_polish_enabled": true,
            "shortcut": "FnKey",
            "fn_key_enabled": true,
            "coat": "gingham"
        }"#).expect("an unknown coat id is data, not a parse error");
        assert_eq!(s.coat.as_deref(), Some("gingham"));
    }
}
