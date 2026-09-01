// TTP - Talk To Paste
// "What's New" version tracking. Shows the changelog after an app update.
//
// ── Why this file is short ──────────────────────────────────────────────────
// It used to hold the changelog itself: 47 English entries and 11 French ones
// as `&'static str` inside two `match` arms, ~500 lines of user-facing prose
// compiled into the Rust binary and handed to `WhatsNew.tsx` as a string.
//
// That broke the standing rule — anything displayed goes through
// `src/i18n/locales/`, and `npm run i18n:check` enforces it — and it broke it
// for EVERY entry, not just the newest one. The consequences were the ones the
// rule exists to prevent: a French entry had to be hand-paired with each
// English one by a `changelog_for_fr` dispatcher that could silently fall
// through to English, the `i18n:check` gate could not see the strings at all,
// and a translator had to be able to build Rust to fix a typo.
//
// The prose now lives in `whatsNew.notes.<version>` in `en.json` and
// `fr.json`, keyed by version with dots replaced by dashes (i18next reads `.`
// as a path separator, so `3.1.7` would address three nested objects). The
// locale-selection and fallback logic that `changelog_for_lang` reimplemented
// is now just i18next's `fallbackLng`.
//
// What is left here is the only part that was ever Rust's business: which
// version is running, and whether the user has already been shown its note.
//
// ── The guard ───────────────────────────────────────────────────────────────
// v3.1.7 shipped with no note because three version numbers were bumped and
// `changelog_for` was not told, and nothing connected the two. The test module
// below is that connection, and it survived the move: it `include_str!`s both
// locale files (test builds only — the shipped binary still contains no
// changelog prose) and fails the build on a version bump that forgets a note,
// in either language.

use std::fs;
use std::path::PathBuf;
use tauri::command;

/// Get the config directory (same as permissions.rs)
fn get_config_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ttp");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }

    config_dir
}

/// Path to the file that stores the last version the user has seen
fn last_seen_version_path() -> PathBuf {
    get_config_dir().join("last_seen_version")
}

/// Current app version (from Cargo.toml / tauri.conf.json)
fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Whether `current` still needs announcing, given what was last dismissed.
///
/// Pure so the one behaviour anybody depends on — "do not show it twice" —
/// can be tested without a config directory. `check_whats_new` is otherwise
/// two filesystem calls and would need a redirectable `dirs::config_dir()` to
/// cover at all.
fn unseen_version(last_seen: Option<&str>, current: &str) -> Option<String> {
    match last_seen {
        Some(seen) if seen.trim() == current => None,
        _ => Some(current.to_string()),
    }
}

/// Check whether a "What's New" popup should be shown.
///
/// Returns `Some(version)` when the app has been updated since the user last
/// dismissed the note, `None` when they have already seen this one.
///
/// It returns the version and NOT the text. The text is a translation, the
/// frontend already holds every translation, and it knows which language the
/// user is reading in without a round trip — the language lookup this command
/// used to do (`get_settings().language`, then an `if lang == "fr"`) was Rust
/// re-deciding something i18next had already decided better.
#[command]
pub fn check_whats_new() -> Option<String> {
    let last_seen = fs::read_to_string(last_seen_version_path()).ok();
    unseen_version(last_seen.as_deref(), current_version())
}

/// Dismiss the "What's New" popup by recording the current version.
#[command]
pub fn dismiss_whats_new() -> Result<(), String> {
    let version = current_version();
    fs::write(last_seen_version_path(), version)
        .map_err(|e| format!("Failed to save last_seen_version: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test builds only. The release binary carries no changelog prose — that
    // is the whole point of the move — but the guard still has to be able to
    // see the notes, and `include_str!` also makes cargo rebuild these tests
    // whenever a locale file changes, so the guard cannot go stale.
    const EN: &str = include_str!("../../src/i18n/locales/en.json");
    const FR: &str = include_str!("../../src/i18n/locales/fr.json");

    /// The i18next key segment a version's note lives under.
    ///
    /// Dots are path separators in i18next, so `3.1.7` would address
    /// `notes → 3 → 1 → 7`. `#[cfg(test)]` because nothing in the shipped
    /// binary ever builds this key — `WhatsNew.tsx` derives it from the
    /// version this module hands back, and `noteKey` there is the production
    /// implementation. Both sides pin the same literal in a test, so a change
    /// to either turns one of them red.
    fn note_key(version: &str) -> String {
        version.replace('.', "-")
    }

    fn note(locale: &str, version: &str) -> Option<String> {
        let doc: serde_json::Value =
            serde_json::from_str(locale).expect("locale file must be valid JSON");
        doc.get("whatsNew")?
            .get("notes")?
            .get(note_key(version))?
            .as_str()
            .map(str::to_string)
    }

    /// The mechanism, not the instance.
    ///
    /// 3.1.7 shipped with no What's New note because the version was bumped in
    /// `Cargo.toml`, `tauri.conf.json` and `package.json` and the changelog was
    /// not told. Nothing anywhere connected the two, so the omission was
    /// silent: the app simply showed nothing, which is indistinguishable from
    /// "the user has already seen it".
    ///
    /// This test is that connection, and moving the prose to the locale files
    /// did not weaken it — it only changed where it looks. Every future bump
    /// fails here until the note exists, which is the only moment anyone is
    /// thinking about the release anyway.
    #[test]
    fn the_shipping_version_has_a_changelog() {
        assert!(
            note(EN, current_version()).is_some(),
            "v{} ships with no What's New entry — add `whatsNew.notes.{}` to \
             src/i18n/locales/en.json",
            current_version(),
            note_key(current_version())
        );
    }

    /// The app is bilingual, and What's New is one of the few surfaces a
    /// French user reaches without going through Settings → Language. An
    /// EN-only note is a visible seam on exactly the screen that introduces
    /// the release.
    ///
    /// `check-i18n-parity.mjs` enforces this too, from the other side of the
    /// boundary. Keeping it here as well means `cargo test` alone still fails
    /// on a half-translated release, which is the gate a Rust-side version
    /// bump is most likely to be run against.
    #[test]
    fn the_shipping_version_has_a_french_changelog() {
        assert!(
            note(FR, current_version()).is_some(),
            "v{} has an English What's New entry and no French one",
            current_version()
        );
    }

    /// The failure the old `changelog_for_lang` dispatcher could produce: a
    /// French user quietly served the English text. i18next's `fallbackLng`
    /// does that job now, and it can only do it wrongly if the two entries are
    /// the same string.
    #[test]
    fn a_french_user_gets_the_french_note() {
        let fr = note(FR, current_version()).expect("fr note");
        let en = note(EN, current_version()).expect("en note");
        assert_ne!(fr, en, "the French note is the English one verbatim");
    }

    /// A version nobody has ever shipped has no note, and says so rather than
    /// resolving to the previous release's.
    #[test]
    fn an_unknown_version_has_no_note() {
        assert!(note(EN, "0.0.0-not-a-release").is_none());
        assert!(note(FR, "0.0.0-not-a-release").is_none());
    }

    /// Every note that exists at all exists in both languages. The guard above
    /// only covers the shipping version; this covers the archive, so a note
    /// added for a future release cannot land EN-only and pass.
    #[test]
    fn every_note_exists_in_both_locales() {
        let keys = |src: &str| -> Vec<String> {
            let doc: serde_json::Value = serde_json::from_str(src).unwrap();
            let mut k: Vec<String> = doc["whatsNew"]["notes"]
                .as_object()
                .expect("notes is an object")
                .keys()
                .cloned()
                .collect();
            k.sort();
            k
        };
        assert_eq!(keys(EN), keys(FR));
    }

    /// The key transform. `WhatsNew.tsx` performs the same substitution and
    /// has its own test; if either side changes it, one of the two fails.
    #[test]
    fn the_note_key_is_a_single_i18next_path_segment() {
        assert_eq!(note_key("3.1.7"), "3-1-7");
        assert!(!note_key(current_version()).contains('.'));
    }

    /// The behaviour every user notices: the note is shown once.
    #[test]
    fn a_version_the_user_has_already_seen_is_not_announced_again() {
        assert_eq!(unseen_version(Some("3.1.7"), "3.1.7"), None);
        // The file is written without a trailing newline, but an editor, a
        // sync tool or a hand-edit can add one. Trimming it here is what stops
        // the note reappearing on every launch forever.
        assert_eq!(unseen_version(Some("3.1.7\n"), "3.1.7"), None);
    }

    #[test]
    fn a_newer_version_is_announced() {
        assert_eq!(
            unseen_version(Some("3.1.6"), "3.1.7"),
            Some("3.1.7".to_string())
        );
    }

    #[test]
    fn a_first_launch_with_no_record_is_announced() {
        // No `last_seen_version` file: a fresh install, or a user who has
        // never updated. They have not seen it, so they are shown it.
        assert_eq!(unseen_version(None, "3.1.7"), Some("3.1.7".to_string()));
    }
}
