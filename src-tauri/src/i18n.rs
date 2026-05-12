// TTP - Talk To Paste
// Rust-side i18n: translates the small set of user-facing strings that the
// Rust backend owns (tray menu, native notifications, accessibility re-grant
// notice). Most translation lives in the React frontend via i18next — the
// only reason we need Rust translations at all is for surfaces the frontend
// can't render: the macOS tray menu/tooltip and OS-level notifications.
//
// The two locale JSONs are embedded at compile time via `include_str!` and
// parsed lazily on first lookup. There's no filesystem dependency at runtime
// and no async overhead — `tr()` is a synchronous, allocation-light call
// suitable for any call site (notification builders, menu builders, etc.).
//
// Resolution order for the current language:
//   1. `settings.language == Some("fr"|"en")` — explicit user choice wins.
//   2. `settings.language == Some("system")` or `None` — sniff the
//      $LANG / $LC_ALL / $LC_MESSAGES env vars and pick `fr` if any starts
//      with "fr", else `en`.
//
// Fallback for missing keys: `fr` → `en` → the key string itself. Returning
// the key (rather than empty) is intentional — a missing key shows up
// visibly in the UI ("tray.startRecording") instead of silently rendering
// blank, which would mask the bug.

use std::sync::OnceLock;

const EN_JSON: &str = include_str!("../../src/i18n/locales/en.json");
const FR_JSON: &str = include_str!("../../src/i18n/locales/fr.json");

/// Lazily-parsed English translation tree. Embedded at compile time and
/// parsed once on first access — subsequent lookups are pointer reads.
static EN_TREE: OnceLock<serde_json::Value> = OnceLock::new();
/// Lazily-parsed French translation tree. Same lifecycle as `EN_TREE`.
static FR_TREE: OnceLock<serde_json::Value> = OnceLock::new();

fn en_tree() -> &'static serde_json::Value {
    EN_TREE.get_or_init(|| {
        serde_json::from_str(EN_JSON).unwrap_or_else(|e| {
            // Should be impossible: the file is embedded at compile time, so
            // if it's malformed we have a build-time bug. Fall back to an
            // empty object so `tr()` returns the key string and the app
            // doesn't crash.
            eprintln!("[i18n] Failed to parse en.json at runtime: {}", e);
            serde_json::Value::Object(Default::default())
        })
    })
}

fn fr_tree() -> &'static serde_json::Value {
    FR_TREE.get_or_init(|| {
        serde_json::from_str(FR_JSON).unwrap_or_else(|e| {
            eprintln!("[i18n] Failed to parse fr.json at runtime: {}", e);
            serde_json::Value::Object(Default::default())
        })
    })
}

/// Resolve the active language ("en" or "fr") from user settings, falling
/// back to environment locale variables when the user has chosen "system"
/// (or hasn't picked anything yet).
///
/// Returns a `&'static str` so callers can use it without allocating.
pub fn current_language() -> &'static str {
    let lang = crate::settings::get_settings().language;
    match lang.as_deref() {
        Some("fr") => "fr",
        Some("en") => "en",
        // "system" or None: sniff env locale. Honoured in priority order
        // matching POSIX: LC_ALL > LC_MESSAGES > LANG.
        _ => system_locale_language(),
    }
}

/// Inspect $LC_ALL / $LC_MESSAGES / $LANG and return "fr" if any starts with
/// "fr" (case-insensitive), otherwise "en". This is best-effort — the
/// frontend has its own navigator.language sniffing for the renderer side.
fn system_locale_language() -> &'static str {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if val.to_lowercase().starts_with("fr") {
                return "fr";
            }
            // First non-empty value wins — don't keep walking if the user
            // has e.g. LANG=en_US set, we shouldn't downgrade to LANG.
            if !val.is_empty() {
                return "en";
            }
        }
    }
    "en"
}

/// Walk a dot-separated key through a `serde_json::Value` tree (e.g.
/// `"tray.startRecording"` → tree["tray"]["startRecording"]). Returns the
/// string value at the leaf, or `None` if any segment is missing or the
/// leaf isn't a string.
fn lookup<'a>(tree: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    let mut node = tree;
    for segment in key.split('.') {
        node = node.get(segment)?;
    }
    node.as_str()
}

/// Look up a translation key in the current language. Falls back to English
/// when the key is missing in French (the orchestrator keeps both files in
/// sync, but defensive fallback prevents UI breakage during in-flight key
/// additions). If the key is missing from both files, returns the key
/// itself so the bug surfaces visibly instead of rendering as an empty
/// string.
pub fn tr(key: &str) -> String {
    let lang = current_language();
    let primary = if lang == "fr" { fr_tree() } else { en_tree() };

    if let Some(s) = lookup(primary, key) {
        return s.to_string();
    }
    // Fallback to English when the primary lookup misses (and we weren't
    // already querying English).
    if lang != "en" {
        if let Some(s) = lookup(en_tree(), key) {
            return s.to_string();
        }
    }
    key.to_string()
}

/// Same as `tr` but interpolates `{{var}}` placeholders. Each `(name,
/// value)` pair in `args` replaces every occurrence of `{{name}}` with
/// `value`. Mirrors i18next's interpolation syntax so the same keys can be
/// used on both sides of the IPC boundary.
///
/// ```ignore
/// // "Recording too long ({{mb}} MB). Max ~14 min." with mb="42"
/// let msg = tr_with("error.audio_too_large", &[("mb", "42")]);
/// ```
pub fn tr_with(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = tr(key);
    for (name, value) in args {
        let placeholder = format!("{{{{{}}}}}", name);
        if s.contains(&placeholder) {
            s = s.replace(&placeholder, value);
        }
    }
    s
}
