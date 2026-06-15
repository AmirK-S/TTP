// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::get_groq_api_key_internal;
use crate::dictionary::detection::start_correction_window;
use crate::dictionary::apply_dictionary;
use crate::history::add_history_entry;
use crate::paste::{check_accessibility, simulate_paste, simulate_typing, ClipboardGuard};
#[cfg(target_os = "macos")]
use crate::paste::{probe_accessibility, reset_accessibility_tcc};
// Pill stays visible - no hide needed
use crate::settings::get_settings;
use crate::state::{AppState, RecordingState};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

/// Global rate limiter for `process_audio`: 20 transcriptions per 60s (sliding window via GCRA).
/// Sized to never trip a power user transcribing rapidly while still capping a runaway
/// frontend loop in the seconds it would take to notice.
static PROCESS_AUDIO_LIMITER: OnceLock<DefaultDirectRateLimiter> = OnceLock::new();

/// Maximum audio file size in bytes (25MB Groq API limit)
const MAX_AUDIO_SIZE: u64 = 25_000_000;

/// Threshold above which we fall back to clipboard+Cmd+V instead of direct
/// keystroke injection. Below this, we type the transcription directly via
/// enigo (CGEventKeyboardSetUnicodeString on macOS) — the clipboard is never
/// touched, eliminating the NSPasteboard read/restore race that bites slow
/// Electron targets (Slack, Notion, Mail). 99 % of voice transcriptions are
/// well under this length; over it, character-by-character typing would feel
/// laggy so we accept the clipboard race for the rare long-text case and
/// mitigate it with a longer post-paste wait (1500 ms).
const DIRECT_TYPING_MAX_CHARS: usize = 2000;

/// How long to wait after Cmd+V before restoring the user's pre-record
/// clipboard, when we have to use the clipboard path (text > DIRECT_TYPING_MAX_CHARS).
/// Slow Electron apps (Slack, Notion, Mail, Discord) can take 200–600 ms to
/// actually read NSPasteboard after receiving Cmd+V — restoring sooner makes
/// the target read the OLD clipboard. 1500 ms is a conservative ceiling that
/// covers the worst observed Electron pauses without making the UX painful
/// (the user only hits this branch for >2000-char transcriptions, which are
/// rare and themselves take seconds to produce).
const CLIPBOARD_PASTE_RESTORE_DELAY_MS: u64 = 1500;

/// Common Whisper hallucinations on silent/empty audio.
///
/// Each entry MUST be lowercase, NFKD-normalized (accents stripped), with
/// straight quotes and no trailing punctuation. The matcher applies the
/// same normalization to the candidate before comparing, so adding a new
/// entry = just write it in canonical form here.
///
/// IMPORTANT: do NOT add single common particles like "thank you", "you",
/// "so", "okay", "bye" without thinking — users genuinely dictate these.
/// Only add phrases that have ZERO non-hallucination interpretation
/// (multi-word, video-credit-flavored, foreign-language thanks-bombs).
///
/// Sources: HuggingFace `sachaarbonel/whisper-hallucinations` dataset
/// (7890 phrases), arXiv 2501.11378, openai/whisper discussions
/// #679 / #928 / #1455 / #1873 / #2280, whisper.cpp #2660. Refreshed
/// 2026-05-26.
const HALLUCINATIONS: &[&str] = &[
    // --- English video-end / channel-promo bombs ---
    "thanks for watching",
    "thank you for watching",
    "thanks for watching please subscribe",
    "thanks for watching dont forget to subscribe",
    "thank you for watching please subscribe",
    "thanks for watching ill see you next time",
    "thanks for watching see you next time",
    "thanks for listening",
    "please subscribe",
    "please subscribe to my channel",
    "subscribe to my channel",
    "like and subscribe",
    "dont forget to subscribe",
    "comment and subscribe",
    "hello everyone welcome to my channel",
    "welcome back to my channel",
    "see you next time",
    "ill see you next time",
    "the end",
    "music",
    "applause",
    "laughter",
    "clapping",
    // Pure-disfluency-only outputs. The match requires whole-string equality,
    // so a real utterance with "uh"/"um"/"hmm" inside (e.g. "uh let me think
    // about it") never triggers these. We deliberately DON'T include "okay",
    // "yeah", "so", "you", "oh", "the" — those are common short replies the
    // user genuinely dictates into Slack / iMessage.
    "uh",
    "um",
    "hmm",

    // --- French (THE bug being fixed: accents) ---
    "merci davoir regarde cette video",
    "merci davoir regarde",
    "merci davoir regarde la video",
    "merci davoir regarde cette video nhesitez pas a vous abonner",
    "merci davoir regarde la video nhesitez pas a vous abonner",
    "je vous remercie",
    "abonnez vous",
    "abonnez vous a ma chaine",
    "noubliez pas de vous abonner",
    "noubliez pas de liker et de vous abonner",
    "likez et abonnez vous",
    "bonjour a tous bienvenue sur ma chaine",
    // NOTE: "a la prochaine" / "a bientot" / "bonne journee" / "bonne
    // soiree" are NOT in this list — they're legitimate French email
    // closings the user often dictates as a standalone short message.
    // We accept the occasional Whisper hallu of these phrases as the
    // lesser evil vs filtering real user speech.

    // --- Spanish ---
    "gracias por ver",
    "gracias por ver el video",
    "gracias por ver este video",
    "suscribete al canal",
    "hasta la proxima",

    // --- German (broadcaster credits are very common DE hallu) ---
    "danke furs zuschauen",
    "vielen dank furs zuschauen",
    "bis zum nachsten mal",
    "abonniert den kanal",

    // --- Italian ---
    "grazie per la visione",
    "grazie per aver guardato il video",
    "iscrivetevi al canale",

    // --- Portuguese ---
    "obrigado por assistir",
    "inscreva se no canal",

    // --- Japanese ---
    "ご視聴ありがとうございました",
    "見てくれてありがとう",
    "チャンネル登録お願いします",

    // --- Chinese ---
    "谢谢观看",
    "谢谢观看 下集再见",
    "请订阅我的频道",

    // --- Russian ---
    "спасибо за просмотр",
    "подписывайтесь на мой канал",

    // --- Arabic ---
    "شكرا على المشاهدة",
    "اشترك في القناة",

    // --- Korean ---
    "시청해주셔서 감사합니다",
    "구독 부탁드립니다",

    // --- Greek ---
    "ευχαριστω που παρακολουθησατε",

    // --- Punctuation-only / empty ---
    ".",
    "..",
    "...",
    "…",
];

/// Substrings: if the (normalized) candidate CONTAINS any of these, it's
/// considered a hallucination even if other text surrounds it. Used for
/// signatures/credits/URLs that Whisper bleeds into otherwise-valid text.
const HALLUCINATION_SUBSTRINGS: &[&str] = &[
    // Amara / fan-sub signatures (all languages, normalized to ASCII)
    "amara.org",
    "amara org",
    "sous titres realises par",
    "sous titrage st 501",
    "sous titrage societe radio canada",
    "soustitreur.com",
    "subtitles by the amara",
    "untertitel der amara",
    "subtitulos realizados por la comunidad de amara",
    "sottotitoli creati dalla comunita amara",
    "legendas pela comunidade amara",
    "ondertiteld door de amara",
    "由 amara",
    // Generic "Subtitles/Captions by X" credits Whisper bleeds onto silence.
    // Reported by a v3.1.1 user who got "Sous-titré par <random studio>"
    // on otherwise-valid recordings. We match the credit STEM, not the
    // studio name, so any future studio is also filtered.
    "sous titre par",
    "sous titres par",
    "soustitrage",
    "sous titrage par",
    "captions by",
    "subtitles by",
    "untertitel von",
    "untertitelung",
    "ondertiteling",
    "subtitulos por",
    "sottotitoli a cura di",
    "legendas por",
    // German broadcaster credits (very common Whisper hallu on DE silence)
    "untertitel im auftrag des zdf",
    "copyright wdr",
    // Other service signatures
    "transcription by castingwords",
    "transcribed by https://otter.ai",
    "transcribed by otter.ai",
    "sottotitoli e revisione a cura di qtss",
    "субтитры сделал",
    // Spiritual / training-data sites
    "mooji.org",
    "satsang with mooji",
    "alimmenta.com",
    "pissedconsumer.com",
];

use crate::logging::log_error;
use super::cleanup::cleanup;
use super::polish::{guard_polish, GuardVerdict};
use super::{convert::convert_to_mono_16khz, polish_text, transcribe_audio};

/// Progress event sent to frontend during transcription pipeline.
///
/// `message` carries a translation KEY (e.g. `"error.no_speech"`,
/// `"progress.transcribing"`) — not a finished English string. The frontend
/// resolves it via i18next using its own `t()` function, so the language of
/// the pill text matches whatever the user selected in settings.
///
/// Empty `message` means "no text" — the frontend hides the message label
/// in that case (used for stages like `"pasting"` and `"complete"` where
/// the stage icon is enough).
///
/// `params` carries interpolation values for `{{var}}` placeholders inside
/// the translation (e.g. `{"mb": "42"}` for `error.audio_too_large`).
/// Serialised as a JSON object on the wire; `None` means no interpolation
/// needed.
#[derive(Clone, serde::Serialize)]
pub struct TranscriptionProgress {
    pub stage: String, // "transcribing", "polishing", "pasting", "complete", "error"
    /// Translation key for the frontend to resolve via i18next (e.g. "error.no_speech").
    /// Empty string means "no message" (the frontend hides the text).
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Emit a progress event to the frontend and tag the current pipeline stage in Sentry scope.
///
/// `message` MUST be a translation key from `en.json` / `fr.json` (or the
/// empty string when no message is appropriate). Do not pass raw English
/// here — the frontend will surface it as a missing-key tag.
///
/// Sentry tagging policy: terminal stages (`complete`, `error`) REMOVE the
/// pipeline_stage tag from the global scope. Intermediate stages set it.
/// Without this, the tag would persist process-globally after the pipeline
/// finishes and misattribute every later Sentry event (tray click, updater
/// check, fnkey monitor) to the previous transcription's last stage.
fn emit_progress(app: &AppHandle, stage: &str, message: &str, params: Option<serde_json::Value>) {
    sentry::configure_scope(|scope| {
        if stage == "complete" || stage == "error" {
            scope.remove_tag("pipeline_stage");
        } else {
            scope.set_tag("pipeline_stage", stage);
        }
    });

    let progress = TranscriptionProgress {
        stage: stage.to_string(),
        message: message.to_string(),
        params,
    };
    app.emit("transcription-progress", &progress).ok();
}

/// Show a system notification. `message` is the already-translated body
/// (this function does not look up keys — callers pass `crate::i18n::tr(...)`).
fn notify(app: &AppHandle, message: &str) {
    // Try Tauri notification first
    let result = app.notification()
        .builder()
        .title(crate::i18n::tr("notification.appName"))
        .body(message)
        .show();

    if result.is_err() {
        // Fallback to osascript
        #[cfg(target_os = "macos")]
        {
            let app_name = crate::i18n::tr("notification.appName");
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{}\" with title \"{}\"",
                    message.replace("\"", "\\\""),
                    app_name.replace("\"", "\\\"")
                ))
                .spawn();
        }
    }
}

/// Set the app state (updates frontend via event)
fn set_state(app: &AppHandle, state: RecordingState) {
    if let Some(app_state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = app_state.try_lock() {
            guard.set_state(state, app);
        }
    }
}

/// Normalize a string for hallucination matching.
///
/// 1. Lowercase
/// 2. Unicode NFKD decompose + strip combining marks (é → e, à → a, ñ → n)
/// 3. Normalize typographic punctuation (' → ', « » → ", — → -, NBSP → space)
/// 4. Strip leading/trailing punctuation (.,;:!?…"'`-_)
/// 5. Collapse runs of whitespace into single space
///
/// CJK / Arabic / Cyrillic characters pass through unchanged — NFKD on
/// those scripts is a no-op for matching purposes.
fn normalize_for_hallucination_match(s: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;

    let lower = s.to_lowercase();

    // NFKD + strip combining marks (Unicode category Mn)
    let stripped: String = lower
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect();

    // Typographic punctuation normalization
    let punct_normalized: String = stripped
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '`' | '´' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '«' | '»' => '"',
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{00A0}' | '\u{2009}' | '\u{200A}' | '\u{202F}' => ' ',
            _ => c,
        })
        .collect();

    // Ellipsis → "..."
    let with_ellipsis = punct_normalized.replace('\u{2026}', "...");

    // Strip apostrophes (so "d'avoir" → "davoir") and replace internal
    // sentence punctuation (, ; :) with spaces so a long Whisper hallu
    // like "merci d'avoir regardé cette vidéo, n'hésitez pas..." still
    // matches the canonical list entry without the comma.
    let internal_cleaned: String = with_ellipsis
        .chars()
        .map(|c| match c {
            '\'' => None,
            ',' | ';' | ':' => Some(' '),
            other => Some(other),
        })
        .flatten()
        .collect();

    // Trim outer punctuation/whitespace
    let trimmed = internal_cleaned.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '-' | '_')
    });

    // Collapse internal whitespace
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Detect a 3-gram repetition loop (Whisper large-v3-turbo classic failure
/// mode: "thanks for watching thanks for watching thanks for watching").
///
/// Whisper's pathology produces the same 3-gram CONSECUTIVELY, with positions
/// typically the trigram length apart (3 words). Natural speech may also
/// repeat short filler 3-grams ("et donc je", "you know what") across a long
/// recording, but those occurrences are spread over many words. We trigger
/// only when at least `MIN_CHAIN` occurrences of the same 3-gram form a chain
/// where each consecutive pair is within `TIGHT_GAP` words — preserving real
/// long-monologue transcriptions while still catching Whisper loops.
fn has_repetition_loop(normalized: &str) -> bool {
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.len() < 9 {
        return false;
    }

    const TIGHT_GAP: usize = 8;
    const MIN_CHAIN: usize = 3;

    use std::collections::HashMap;
    let mut positions: HashMap<(&str, &str, &str), Vec<usize>> = HashMap::new();
    for (i, w) in words.windows(3).enumerate() {
        positions.entry((w[0], w[1], w[2])).or_default().push(i);
    }

    for occurrences in positions.values() {
        if occurrences.len() < MIN_CHAIN {
            continue;
        }
        let mut chain = 1usize;
        for pair in occurrences.windows(2) {
            if pair[1] - pair[0] <= TIGHT_GAP {
                chain += 1;
                if chain >= MIN_CHAIN {
                    return true;
                }
            } else {
                chain = 1;
            }
        }
    }
    false
}

/// Filter out common Whisper hallucinations on silent / non-speech audio.
///
/// Strategy:
///   1. Normalize candidate (lowercase + NFKD accent strip + typographic
///      punctuation + apostrophes + whitespace)
///   2. Exact match against `HALLUCINATIONS` (canonical-form whole string)
///   3. Substring match against `HALLUCINATION_SUBSTRINGS` (signatures
///      that bleed into otherwise-valid text)
///   4. Detect 3-gram repetition loops
fn is_hallucination(text: &str) -> bool {
    let normalized = normalize_for_hallucination_match(text);

    if normalized.is_empty() {
        return true;
    }

    // PRIVACY: we never log the literal `text` or `normalized` payload — both
    // contain the user's verbatim speech. The hallucination filter is the
    // most aggressive false-positive source in the pipeline, and packaged
    // macOS stderr lands in Console.app where unrelated humans can read it.
    // We keep the diagnostic signal (which arm fired, how long) without the
    // content.
    let char_count = text.chars().count();
    if HALLUCINATIONS.iter().any(|h| *h == normalized) {
        crate::logging::log_info(&format!(
            "[Pipeline] filtered exact hallucination chars={}",
            char_count
        ));
        return true;
    }

    if HALLUCINATION_SUBSTRINGS
        .iter()
        .any(|sub| normalized.contains(*sub))
    {
        crate::logging::log_info(&format!(
            "[Pipeline] filtered substring hallucination chars={}",
            char_count
        ));
        return true;
    }

    if has_repetition_loop(&normalized) {
        crate::logging::log_info(&format!(
            "[Pipeline] filtered 3-gram repetition loop chars={}",
            char_count
        ));
        return true;
    }

    false
}

/// Map a transcription API error message into an analytics category +
/// the HTTP status code when one is recoverable from the string.
///
/// Pure function: no I/O, no globals. The pipeline uses the category for
/// telemetry attribution (so we can tell at a glance whether the recurring
/// failure mode is rate-limiting, auth, oversized payloads, or network) and
/// the status code for the same Sentry split. Extracted so the classification
/// rules can be tested without spinning up the full pipeline.
pub(crate) fn classify_transcription_error(e: &str) -> (&'static str, Option<u16>) {
    if e.contains(" 429 ") || e.contains(": 429") {
        ("rate_limited", Some(429))
    } else if e.contains(" 401 ") || e.contains(": 401") {
        ("invalid_api_key", Some(401))
    } else if e.contains(" 403 ") || e.contains(": 403") {
        ("invalid_api_key", Some(403))
    } else if e.contains("413") {
        ("too_long", Some(413))
    } else if e.to_lowercase().contains("entity too large") || e.to_lowercase().contains("too long") {
        ("too_long", None)
    } else if e.to_lowercase().contains("timeout")
        || e.to_lowercase().contains("connect")
        || e.to_lowercase().contains("network")
        || e.to_lowercase().contains("dns")
    {
        ("network", None)
    } else {
        ("api_error", None)
    }
}

/// Categorise a polish API error string for analytics. Three buckets:
///
///   * `rate_limited`: a 429 status was somewhere in the error string.
///   * `invalid_api_key`: a 401 or 403.
///   * `polish_failed`: anything else, including network and parsing errors.
///
/// We do NOT surface a user-facing toast for the first two — they're
/// terminal for the polish step but the cleaned text still pastes, so we
/// keep the UX silent and rely on telemetry to fix the underlying issue.
/// Only the catch-all `polish_failed` triggers the "polish_unavailable"
/// pill so the user knows their text shipped without polish.
pub(crate) fn classify_polish_error(e: &str) -> &'static str {
    if e.contains(" 429 ") || e.contains(": 429") {
        "rate_limited"
    } else if e.contains(" 401 ") || e.contains(": 401") || e.contains(" 403 ") || e.contains(": 403") {
        "invalid_api_key"
    } else {
        "polish_failed"
    }
}

#[cfg(test)]
mod pipeline_classifier_tests {
    use super::{classify_polish_error, classify_transcription_error};

    #[test]
    fn polish_rate_limited() {
        assert_eq!(classify_polish_error("HTTP 429 Too Many Requests"), "rate_limited");
        assert_eq!(classify_polish_error("status: 429"), "rate_limited");
    }

    #[test]
    fn polish_invalid_api_key() {
        assert_eq!(classify_polish_error("HTTP 401 Unauthorized"), "invalid_api_key");
        // Space-separated 403 form — what reqwest emits via Display.
        assert_eq!(classify_polish_error("Groq returned 403 Forbidden"), "invalid_api_key");
        // ': 403' colon form.
        assert_eq!(classify_polish_error("status: 403"), "invalid_api_key");
    }

    #[test]
    fn polish_fallback_catches_everything_else() {
        assert_eq!(classify_polish_error("network timeout"), "polish_failed");
        assert_eq!(classify_polish_error("malformed JSON in response"), "polish_failed");
        assert_eq!(classify_polish_error(""), "polish_failed");
    }

    #[test]
    fn polish_does_not_misclassify_random_numbers() {
        // "429" without separator should NOT match — keeps noise out of the
        // rate-limited bucket.
        assert_eq!(classify_polish_error("hash=abc429def"), "polish_failed");
    }
}

#[cfg(test)]
mod pipeline_transcription_classifier_tests {
    use super::classify_transcription_error;

    #[test]
    fn rate_limit_via_space_separators() {
        assert_eq!(classify_transcription_error("HTTP 429 Too Many Requests"), ("rate_limited", Some(429)));
    }

    #[test]
    fn rate_limit_via_colon_separator() {
        assert_eq!(classify_transcription_error("Status: 429"), ("rate_limited", Some(429)));
    }

    #[test]
    fn invalid_api_key_401_and_403() {
        assert_eq!(classify_transcription_error("HTTP 401 Unauthorized"), ("invalid_api_key", Some(401)));
        assert_eq!(classify_transcription_error("HTTP 403 Forbidden"), ("invalid_api_key", Some(403)));
    }

    #[test]
    fn too_long_via_413() {
        assert_eq!(classify_transcription_error("status 413 Payload Too Large"), ("too_long", Some(413)));
    }

    #[test]
    fn too_long_via_words_without_code() {
        assert_eq!(classify_transcription_error("Request Entity Too Large"), ("too_long", None));
        assert_eq!(classify_transcription_error("audio file too long for this endpoint"), ("too_long", None));
    }

    #[test]
    fn network_via_keywords() {
        assert_eq!(classify_transcription_error("request timeout"), ("network", None));
        assert_eq!(classify_transcription_error("connect refused"), ("network", None));
        assert_eq!(classify_transcription_error("DNS resolution failed"), ("network", None));
    }

    #[test]
    fn fallback_api_error() {
        assert_eq!(classify_transcription_error("something unexpected"), ("api_error", None));
        assert_eq!(classify_transcription_error(""), ("api_error", None));
    }

    #[test]
    fn does_not_misclassify_random_429_substring() {
        // Strings containing "429" without separators around it are noisier
        // — the existing matcher tolerates these as "api_error" rather than
        // false-positive rate_limited.
        assert_eq!(classify_transcription_error("hash=abc429def"), ("api_error", None));
    }
}

#[cfg(test)]
mod hallucination_tests {
    use super::*;

    #[test]
    fn normalizes_french_accents() {
        // The bug we're fixing: "regardé" (with é) should match the entry
        // "merci davoir regarde cette video" (without accents).
        assert!(is_hallucination("Merci d'avoir regardé cette vidéo."));
        assert!(is_hallucination("Merci d'avoir regardé cette vidéo"));
        assert!(is_hallucination("merci d'avoir regardé"));
    }

    #[test]
    fn handles_typographic_apostrophes() {
        // U+2019 right single quotation mark (curly apostrophe)
        assert!(is_hallucination("Merci d\u{2019}avoir regardé cette vidéo."));
    }

    #[test]
    fn normalizes_trailing_punctuation() {
        assert!(is_hallucination("Thanks for watching!"));
        assert!(is_hallucination("Thanks for watching..."));
        assert!(is_hallucination("Thanks for watching!!!"));
    }

    #[test]
    fn matches_subtitle_signature_substring() {
        assert!(is_hallucination(
            "blah blah subtitles by the Amara.org community"
        ));
        assert!(is_hallucination("Sous-titres réalisés par la communauté d'Amara.org"));
    }

    #[test]
    fn detects_3gram_repetition_loop() {
        assert!(is_hallucination(
            "thanks for watching thanks for watching thanks for watching"
        ));
    }

    #[test]
    fn detects_tight_repetition_with_minor_noise_between() {
        // Whisper sometimes injects 1-2 words between repetitions of the loop
        // phrase. Still pathological — must trigger.
        assert!(is_hallucination(
            "thanks for watching everyone thanks for watching today thanks for watching"
        ));
    }

    #[test]
    fn does_not_filter_natural_long_speech_with_spread_repetition() {
        // The core regression: a real 2-3 minute monologue that naturally
        // repeats a short filler 3-gram across the recording. The old
        // count-only loop detector killed these; the proximity-aware version
        // must let them through.
        let monologue = "et donc je voulais te parler du projet polaris parce qu il y a \
            plusieurs choses qui me chiffonnent depuis ce matin notamment la latence sur \
            les longs audios qui est devenue franchement insupportable surtout quand on \
            essaie de dicter une note un peu longue. et donc je pense qu il faut qu on \
            regarde ensemble cette semaine ou la prochaine pour trouver une solution \
            durable parce que sinon les utilisateurs vont decrocher et on perdra le peu \
            de credit qu on a accumule avec la sortie de polaris. et donc je te propose \
            qu on se cale un creneau lundi ou mardi matin pour faire le tour des options \
            possibles et choisir un cap clair avant la fin de la semaine.";
        assert!(!is_hallucination(monologue));
    }

    #[test]
    fn does_not_filter_legit_2gram_triple_repetition() {
        // "I think that I think that I think that" — 2-gram triple repetition
        // is not what we target. The 3-gram path is only triggered when the
        // SAME 3-gram is tight-clustered. Sanity check that short natural
        // speech with a casual stutter does not get flagged.
        assert!(!is_hallucination(
            "I think that maybe we should try that approach again next week"
        ));
    }

    #[test]
    fn does_not_filter_real_user_speech() {
        // Users genuinely say these — must NOT filter.
        assert!(!is_hallucination("Thank you Marie for the help."));
        assert!(!is_hallucination("Goodbye Paul, see you tomorrow."));
        assert!(!is_hallucination("I subscribe to that newsletter."));
        assert!(!is_hallucination(
            "I'm sorry, I can't make it to the meeting today."
        ));
    }

    #[test]
    fn filters_empty_and_punctuation_only() {
        assert!(is_hallucination(""));
        assert!(is_hallucination("."));
        assert!(is_hallucination("..."));
        assert!(is_hallucination("…"));
        assert!(is_hallucination("   "));
    }

    #[test]
    fn filters_pure_disfluencies_only() {
        // Pure disfluencies — filter
        assert!(is_hallucination("Uh"));
        assert!(is_hallucination("Um."));
        assert!(is_hallucination("Hmm"));
    }

    #[test]
    fn does_not_filter_real_short_replies() {
        // Common Slack / iMessage quick replies the user genuinely dictates.
        // These MUST pass through (no over-protection).
        assert!(!is_hallucination("Okay"));
        assert!(!is_hallucination("Okay."));
        assert!(!is_hallucination("Yeah"));
        assert!(!is_hallucination("Yes"));
        assert!(!is_hallucination("No"));
        assert!(!is_hallucination("Sure"));
        assert!(!is_hallucination("So"));
        assert!(!is_hallucination("You"));
        assert!(!is_hallucination("The"));
        assert!(!is_hallucination("Oh"));
    }

    #[test]
    fn does_not_filter_french_email_closings() {
        // Real email/message closings — must pass through.
        assert!(!is_hallucination("Bonne journée"));
        assert!(!is_hallucination("Bonne journée !"));
        assert!(!is_hallucination("Bonne soirée"));
        assert!(!is_hallucination("À bientôt"));
        assert!(!is_hallucination("À la prochaine"));
        assert!(!is_hallucination("Au revoir"));
        assert!(!is_hallucination("Salut"));
        assert!(!is_hallucination("Voilà"));
        assert!(!is_hallucination("Merci"));
    }

    #[test]
    fn filters_spanish_video_credit() {
        assert!(is_hallucination("Gracias por ver el video."));
    }

    #[test]
    fn filters_german_broadcaster_signature() {
        assert!(is_hallucination(
            "Untertitel im Auftrag des ZDF, 2017"
        ));
    }

    #[test]
    fn filters_long_french_promo() {
        assert!(is_hallucination(
            "Merci d'avoir regardé cette vidéo, n'hésitez pas à vous abonner !"
        ));
    }
}

// strip_llm_wrapper() removed in Polaris v3.0.0: the new anti-injection
// polish prompt requests structured JSON {intent, polished} and serde does
// the unwrapping. No more prefix-matching hacks — if the LLM emits anything
// other than valid JSON, the polish.rs fallback treats it as raw polished
// text and the post-LLM guards catch real anomalies (length explosion,
// low overlap, refusal markers).

/// Main pipeline function: process a completed recording
///
/// Orchestrates the flow:
/// 1. Transcribe audio via Groq Whisper
/// 2. Polish text via Groq LLM (llama-3.3-70b-versatile)
/// 3. Paste into active app (or clipboard fallback)
///
/// Emits progress events throughout for frontend updates.
pub async fn process_recording(app: &AppHandle, audio_path: String) -> Result<String, String> {
    let pipeline_start = std::time::Instant::now();

    // Set state to Processing
    set_state(app, RecordingState::Processing);

    // Check if audio file exists
    let audio_file = Path::new(&audio_path);
    let file_size = match std::fs::metadata(audio_file) {
        Ok(meta) => meta.len(),
        Err(e) => {
            emit_progress(app, "error", "error.audio_file_not_found", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "api_error", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
            set_state(app, RecordingState::Idle);
            return Err(format!("Audio file error: {}", e));
        }
    };

    // AUDI-04: Validate WAV header before any processing.
    // The validation error detail is preserved in the developer log; the
    // user-facing message is a single localized "audio corrupt" key.
    if let Err(msg) = super::backup::validate_wav(&audio_path) {
        let _ = std::fs::remove_file(&audio_path);
        log_error(&format!("WAV validation failed: {}", msg));
        emit_progress(app, "error", "error.audio_corrupt", None);
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
            "error_category": "corrupt_audio",
            "duration_seconds": pipeline_start.elapsed().as_secs_f64()
        })));
        set_state(app, RecordingState::Idle);
        return Err(msg);
    }

    // AUDI-06: Detect empty WAV files (valid header, 0 audio samples). This
    // is the tauri-plugin-mic-recorder failure mode where cpal opens the
    // stream but the audio callback never delivers samples — typically a
    // silently-revoked mic permission after an unsigned-app update, or the
    // mic being held exclusive by another process. Without this check we'd
    // upload a 68-byte WAV to Groq, get a 400 "Audio file is too short",
    // and surface a useless generic error.
    match super::backup::wav_duration_secs(&audio_path) {
        Ok(secs) if secs < 0.1 => {
            log_error(&format!(
                "Empty recording detected: {:.3}s of audio in {} bytes. \
                 Likely cause: revoked mic permission (unsigned-app update) \
                 or mic held exclusive by another app.",
                secs, file_size
            ));
            let _ = std::fs::remove_file(&audio_path);
            emit_progress(app, "error", "error.recording_empty", None);
            notify(app, &crate::i18n::tr("notification.recordingEmpty"));
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
                "error_category": "recording_empty",
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
                "wav_bytes": file_size,
                "wav_audio_secs": secs,
            })));
            set_state(app, RecordingState::Idle);
            return Err(format!("Empty recording: {:.3}s of audio", secs));
        }
        Ok(_) => { /* non-empty, continue */ }
        Err(e) => {
            // Defensive: if we can't even read the duration, treat as corrupt
            // (validate_wav already passed, so this should be rare).
            log_error(&format!("Could not read WAV duration: {}", e));
        }
    }

    // Silence pre-check: skip Whisper entirely when the recording is
    // effectively silent. Whisper's most embarrassing failure mode is
    // hallucinating "thank you", "Sous-titré par <studio>", or a random
    // foreign-language sentence on silent input — the downstream
    // hallucination filter catches many of these by string match but
    // can't safely block bare "thank you" (a real user reply). Cutting
    // the API call entirely is both more correct and cheaper.
    //
    // Threshold ~0.005 is a few dB above the self-noise floor of a stock
    // MacBook / WASAPI mic, well below the RMS of any voice sample we've
    // observed. Confirmed empirically on 3-second silent clips and 3-second
    // whispered speech (lowest valid speech sat at ~0.008).
    const SILENCE_RMS_FLOOR: f32 = 0.005;
    match super::backup::wav_average_rms(&audio_path) {
        Ok(rms) if rms < SILENCE_RMS_FLOOR => {
            crate::logging::log_info(&format!(
                "Silent recording detected (avg RMS {:.4} < {:.4}), skipping Whisper",
                rms, SILENCE_RMS_FLOOR
            ));
            let _ = std::fs::remove_file(&audio_path);
            emit_progress(app, "error", "error.no_speech", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
                "error_category": "silent_audio",
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
                "avg_rms": rms,
            })));
            set_state(app, RecordingState::Idle);
            return Err("Silent recording — skipped Whisper".to_string());
        }
        Ok(_) => { /* has signal, continue */ }
        Err(e) => {
            // Defensive: any RMS read error is non-fatal — fall through to
            // Whisper. validate_wav already vouched for the file structure.
            crate::logging::log_warn(&format!("Could not compute WAV RMS: {}", e));
        }
    }

    // Convert stereo 48kHz WAV → mono 16kHz WAV (reduces size ~6x)
    let converted_path = match convert_to_mono_16khz(&audio_path) {
        Ok(path) => path,
        Err(e) => {
            crate::logging::log_warn(&format!("[Pipeline] Conversion failed: {} - sending original", e));
            // Tell the user the next stage may reject it (~25 MB cap on
            // un-compressed audio is hit much earlier than on compressed).
            emit_progress(app, "transcribing", "error.compression_failed", None);
            audio_path.clone()
        }
    };
    let use_converted = converted_path != audio_path;

    let final_upload_path = converted_path.clone();

    // AUDI-05: If conversion failed and original exceeds 25MB, reject early
    if !use_converted && file_size > MAX_AUDIO_SIZE {
        let original_mb = file_size as f64 / 1_000_000.0;
        let _ = std::fs::remove_file(&audio_path);
        let mb_str = format!("{:.0}", original_mb);
        emit_progress(
            app,
            "error",
            "error.audio_too_large",
            Some(serde_json::json!({ "mb": mb_str })),
        );
        notify(app, &crate::i18n::tr("notification.recordingTooLong"));
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
            "error_category": "too_long",
            "duration_seconds": pipeline_start.elapsed().as_secs_f64()
        })));
        set_state(app, RecordingState::Idle);
        log_error(&format!("Conversion failed and original too large: {:.1}MB exceeds {}MB limit", original_mb, MAX_AUDIO_SIZE / 1_000_000));
        return Err(format!("Audio too large: {:.1}MB exceeds API limit", original_mb));
    }

    // Check size AFTER conversion (raw WAV can be large but converts down)
    let final_size = std::fs::metadata(&final_upload_path)
        .map(|m| m.len())
        .unwrap_or(file_size);
    let final_mb = final_size as f64 / 1_000_000.0;

    if final_size > MAX_AUDIO_SIZE {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        let mb_str = format!("{:.0}", final_mb);
        emit_progress(
            app,
            "error",
            "error.audio_too_large",
            Some(serde_json::json!({ "mb": mb_str })),
        );
        notify(app, &crate::i18n::tr("notification.recordingTooLong"));
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "too_long", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        log_error(&format!("Audio too large after conversion: {:.1}MB exceeds {}MB limit", final_mb, MAX_AUDIO_SIZE / 1_000_000));
        return Err(format!("Audio too large: {:.1}MB exceeds API limit", final_mb));
    }

    // Use the converted mono 16kHz WAV for transcription
    let transcription_path = &final_upload_path;

    // Load settings
    let settings = get_settings();

    // Read input mode from app state for analytics. We use the effective
    // value (session override or persisted) so a single Fn-double-tap
    // recording in a push-to-talk-default install is correctly attributed
    // to "toggle".
    let input_mode = if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(guard) = state.try_lock() {
            if guard.effective_hands_free() { "toggle" } else { "push_to_talk" }
        } else {
            "unknown"
        }
    } else {
        "unknown"
    };

    // Get Groq API key
    let groq_key = get_groq_api_key_internal(app)?.filter(|k| !k.is_empty());

    let api_key = match groq_key {
        Some(key) => key,
        None => {
            let _ = std::fs::remove_file(&audio_path);
            if use_converted { let _ = std::fs::remove_file(&converted_path); }
            emit_progress(app, "error", "error.no_api_key", None);
            if let Some(window) = app.get_webview_window("setup") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            set_state(app, RecordingState::Idle);
            return Err("No Groq API key configured".to_string());
        }
    };

    // AUDI-01: Backup original audio before API call
    let backup_path = match super::backup::backup_audio(app, &audio_path) {
        Ok(path) => Some(path),
        Err(e) => {
            // Local log keeps the full error (incl. paths) for the user's
            // own debugging; analytics gets only the error category to
            // avoid leaking filesystem paths off-device.
            crate::logging::log_warn(&format!("Audio backup failed: {}", e));
            let category = if e.contains("Permission") { "permission_denied" }
                else if e.contains("space") || e.contains("No space") { "disk_full" }
                else if e.contains("dir") { "create_dir_failed" }
                else { "other" };
            crate::telemetry::analytics::track(app, "backup_failed", Some(serde_json::json!({
                "category": category
            })));
            None // Continue without backup -- don't block transcription
        }
    };

    // HARD RULE: do NOT pass any prompt to Whisper.
    //
    // Every prompt iteration has leaked into the output on trailing silence:
    //   * v3.1.2-1: "Glossary: <names>" → Whisper wrote sentences ABOUT
    //     glossaries on silence.
    //   * v3.1.3-1: dropped "Glossary:" but kept the inline name list →
    //     names appended at the END of legitimate transcriptions.
    //   * v3.1.4-1: minimised to "French and English bilingual speaker." →
    //     even that leaked: user reported "...tu vois ce que je veux dire ?
    //     English bilingual speaker. Claud" appended to real speech.
    //
    // Conclusion (v3.1.6): no prompt is the only safe prompt. The
    // `language` API parameter handles bilingual decoder pinning, and the
    // post-transcription `apply_dictionary` pass handles proper-noun
    // substitution via word-boundary matching (it never adds words that
    // weren't already present in the transcription, so it has zero
    // hallucination surface).
    let whisper_prompt: Option<String> = None;

    // Stage 1: Transcribe audio via Groq Whisper
    emit_progress(app, "transcribing", "progress.transcribing", None);

    // Pin Whisper's decoder to the user-selected transcription language.
    //
    // The setting is a SEPARATE field from `settings.language` (the UI
    // locale). v3.1.2 implicitly tied them: a user with English UI but
    // who dictates in French got their French audio mis-served as English.
    // v3.1.5 exposes an explicit dropdown ("Auto" / "English" / "French").
    //
    // "Auto" / missing → return None so Whisper auto-detects. We still get
    // protection against zh/ru/ko hallucinations on silence because the
    // upstream silence pre-check (RMS gate) skips Whisper entirely on
    // truly silent audio. For audible bilingual speech, auto-detect is
    // actually better than a wrong hard pin.
    let whisper_lang: Option<&'static str> = match crate::settings::get_settings()
        .transcription_language
        .as_deref()
    {
        Some("en") => Some("en"),
        Some("fr") => Some("fr"),
        // "auto", None, or any unrecognised value → let Whisper decide.
        _ => None,
    };

    let raw_text = match transcribe_audio(&api_key, transcription_path, whisper_prompt.as_deref(), whisper_lang).await {
        Ok(text) => text,
        Err(e) => {
            // AUDI-02: Do NOT delete the original audio on API failure.
            // The backup (and original) remain on disk so the user can inspect or retry.
            log_error(&format!("Transcription failed: {}", e));
            crate::logging::log_info(&format!(
                "Audio preserved in backup after API failure: {}",
                backup_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "no backup".into())
            ));

            // Classify error for analytics — capture HTTP status alongside the category
            // so Sentry can split the catch-all "api_error" bucket by code.
            let (error_category, status_code) = classify_transcription_error(&e);

            // User-facing translation key + optional interpolation params,
            // based on error category. The frontend resolves the key into
            // localized pill text via i18next.
            let (user_key, user_params) = match error_category {
                "rate_limited" => ("error.transcription_rate_limited", None),
                "invalid_api_key" => ("error.transcription_invalid_key", None),
                "too_long" => ("error.transcription_too_long", None),
                _ => (
                    "error.transcription_generic",
                    Some(serde_json::json!({ "error": e.clone() })),
                ),
            };

            emit_progress(app, "error", user_key, user_params);
            notify(app, &crate::i18n::tr("notification.transcriptionFailed"));
            let mut payload = serde_json::json!({
                "error_category": error_category,
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
            });
            if let Some(code) = status_code {
                payload["status_code"] = code.into();
            }
            crate::telemetry::analytics::track(app, "transcription_failed", Some(payload));
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

    // Groq whisper-large-v3 occasionally returns 200 OK with an empty body on
    // long single-speaker monologues — a known silent-failure mode. One
    // resubmit almost always recovers the transcription. Cheaper than the
    // user-facing "no sound" error.
    let raw_text = if raw_text.trim().is_empty() {
        crate::logging::log_warn(
            "Empty transcription from Groq, retrying once before surfacing no_speech",
        );
        transcribe_audio(&api_key, transcription_path, whisper_prompt.as_deref(), whisper_lang)
            .await
            .unwrap_or_default()
    } else {
        raw_text
    };

    // Check for empty transcription (no speech detected)
    if raw_text.trim().is_empty() {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "error.no_speech", None);
        notify(app, &crate::i18n::tr("notification.noSpeech"));
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected".to_string());
    }

    // Filter out dictionary-induced hallucinations: if the entire transcription
    // is just 1-2 words that all appear in the dictionary, Whisper likely
    // hallucinated a glossary word on silence rather than transcribing real
    // speech. Only applied to SHORT recordings (~< 10s of audio): on a long
    // recording, "2 dict words" is more likely a real (if degenerate) result
    // than a silence hallucination, and dropping the user's minute-long
    // capture is worse than pasting the wrong two words.
    let approx_duration_secs = final_size as f64 / 32_000.0; // 16kHz mono 16-bit
    {
        let dict_entries = crate::dictionary::store::get_dictionary();
        if !dict_entries.is_empty() && approx_duration_secs < 10.0 {
            let words: Vec<&str> = raw_text.trim().split_whitespace().collect();
            if words.len() <= 2 {
                let dict_words: Vec<String> = dict_entries.iter()
                    .map(|e| e.correction.to_lowercase())
                    .collect();
                let all_in_dict = words.iter().all(|w| {
                    let w_lower = w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                    dict_words.iter().any(|d| d == &w_lower)
                });
                if all_in_dict {
                    let _ = std::fs::remove_file(&audio_path);
                    if use_converted { let _ = std::fs::remove_file(&converted_path); }
                    if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
                    emit_progress(app, "error", "error.no_speech", None);
                    crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
                    set_state(app, RecordingState::Idle);
                    return Err("No speech detected (glossary ghost)".to_string());
                }
            }
        }
    }

    // Prompt-introducer hallucination: short recordings starting with
    // "Glossary, ..." / "Glossaire, ..." / "Dictionary, ..." are almost
    // always Whisper bleeding the prompt's introducer word into the
    // output on near-silent audio. We dropped the literal "Glossary:" word
    // from the prompt itself (above) so this should be rare; the filter
    // is belt-and-suspenders for older recordings or future re-introductions.
    //
    // We only fire on SHORT recordings (≤ 8 words AND ≤ 6 s of audio) so a
    // legitimate dictation like "I need to update the team glossary
    // tomorrow with the new acronyms" is preserved.
    {
        let lower = raw_text.trim().to_lowercase();
        let leading = lower
            .split(|c: char| !c.is_alphabetic())
            .next()
            .unwrap_or("");
        let intro_words = ["glossary", "glossaire", "vocabulary", "vocabulaire", "lexique"];
        let word_count = raw_text.trim().split_whitespace().count();
        if approx_duration_secs < 6.0
            && word_count <= 8
            && intro_words.iter().any(|&w| leading == w)
        {
            crate::logging::log_info(&format!(
                "[Pipeline] Filtered prompt-introducer hallucination chars={}, leading={:?}",
                raw_text.chars().count(),
                leading
            ));
            let _ = std::fs::remove_file(&audio_path);
            if use_converted { let _ = std::fs::remove_file(&converted_path); }
            if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
            emit_progress(app, "error", "error.no_speech", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
                "error_category": "no_speech",
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
                "sub_category": "prompt_introducer_leak",
            })));
            set_state(app, RecordingState::Idle);
            return Err("No speech detected (prompt-introducer leak)".to_string());
        }
    }

    // Filter out common Whisper hallucinations on silent audio
    if is_hallucination(&raw_text) {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "error.no_speech", None);
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected (filtered)".to_string());
    }

    // Phase-1 deterministic cleanup BEFORE the LLM. Handles punctuation
    // commands, tech term normalization, acronyms, repetitions, standalone
    // fillers. Idempotent. Roughly 80% of cleanups happen here without
    // touching the LLM — faster, cheaper, safer.
    let cleaned_text = cleanup(&raw_text);

    // Stage 2: Polish text (if enabled AND user has quota)
    let polish_quota_ok = if !settings.ai_polish_enabled {
        false
    } else if crate::licensing::is_pro_or_trial_disk() {
        true
    } else {
        let used = crate::usage::polish_count_this_month(&crate::usage::load_usage());
        if used < crate::licensing::FREE_POLISH_PER_MONTH {
            true
        } else {
            let used_str = used.to_string();
            let limit_str = crate::licensing::FREE_POLISH_PER_MONTH.to_string();
            // Once-per-month gate (see Windows toast spam fix v2.1.7).
            if crate::usage::should_notify_polish_cap_once_this_month() {
                notify(
                    app,
                    &crate::i18n::tr_with(
                        "notification.polishLimitReached",
                        &[("used", &used_str), ("limit", &limit_str)],
                    ),
                );
            }
            crate::telemetry::analytics::track(
                app,
                "free_cap_hit",
                Some(serde_json::json!({"feature": "ai_polish"})),
            );
            false
        }
    };

    let final_text = if polish_quota_ok {
        emit_progress(app, "polishing", "progress.polishing", None);

        let polish_start = std::time::Instant::now();
        match polish_text(&api_key, &cleaned_text).await {
            Ok(result) => {
                crate::usage::record_polish_success();
                let polish_ms = polish_start.elapsed().as_millis() as u64;

                // Post-LLM guards: length ratio, content-word Jaccard,
                // refusal markers. On rejection → fall back to phase-1
                // cleaned text + log to Sentry. Silent fallback (no UI
                // popup mid-dictation, just slightly less-polished text).
                match guard_polish(&cleaned_text, &result.polished) {
                    GuardVerdict::Accept => {
                        let mut data = std::collections::BTreeMap::new();
                        data.insert("intent".to_string(), format!("{:?}", result.intent).into());
                        data.insert("latency_ms".to_string(), polish_ms.into());
                        sentry::add_breadcrumb(sentry::Breadcrumb {
                            ty: "polish".into(),
                            category: Some("polish.success".into()),
                            level: sentry::Level::Info,
                            data,
                            ..Default::default()
                        });
                        // Intent classification is captured in the Sentry
                        // breadcrumb above for future per-app routing (v3.1+).
                        result.polished
                    }
                    GuardVerdict::Reject(reason) => {
                        log_error(&format!(
                            "Polish guard rejected: {} (intent={:?}, polish_ms={})",
                            reason, result.intent, polish_ms
                        ));
                        let mut data = std::collections::BTreeMap::new();
                        data.insert("reason".to_string(), reason.into());
                        data.insert("intent".to_string(), format!("{:?}", result.intent).into());
                        data.insert("latency_ms".to_string(), polish_ms.into());
                        sentry::add_breadcrumb(sentry::Breadcrumb {
                            ty: "polish".into(),
                            category: Some("polish.guard_reject".into()),
                            level: sentry::Level::Warning,
                            data,
                            ..Default::default()
                        });
                        crate::telemetry::analytics::track(
                            app,
                            "polish_guard_rejected",
                            Some(serde_json::json!({ "reason": reason })),
                        );
                        cleaned_text.clone()
                    }
                }
            }
            Err(e) => {
                crate::logging::log_warn(&format!("[Pipeline] Polish failed, using cleaned text: {}", e));
                let category = classify_polish_error(&e);
                if category == "polish_failed" {
                    emit_progress(app, "pasting", "error.polish_unavailable", None);
                }
                crate::telemetry::analytics::track(app, "polish_failed", Some(serde_json::json!({
                    "category": category
                })));
                cleaned_text.clone()
            }
        }
    } else {
        cleaned_text.clone()
    };

    // Apply dictionary corrections as hard post-processing.
    // Even with the new polish pipeline, the personal dictionary is a hard
    // guarantee — applied after polish (and after cleanup) so user-specific
    // term spellings always win.
    let final_text = apply_dictionary(&final_text);

    // Stage 3: Paste into active app
    emit_progress(app, "pasting", "", None);

    // Create clipboard guard to save original content.
    // We always write the transcription to the clipboard first so that if AX
    // is denied — or anything in the paste path fails — the user can still
    // recover the text with a manual Cmd+V. The transcription stays in the
    // clipboard until either (a) we successfully paste/type and restore, or
    // (b) we error out (in which case we deliberately leave it).
    let clipboard_guard = ClipboardGuard::new(app);

    if let Err(e) = clipboard_guard.write_text(&final_text) {
        emit_progress(app, "error", "error.clipboard_write_failed", None);
        notify(app, &crate::i18n::tr("notification.clipboardFailed"));
        set_state(app, RecordingState::Idle);
        return Err(e);
    }

    // Check accessibility permission and try to paste.
    //
    // On macOS we use `probe_accessibility()` — a real AXUIElement API call —
    // instead of just `check_accessibility()` (which only reads the TCC grant
    // flag). After an in-place app-bundle replacement (auto-updater), macOS
    // commonly leaves the TCC entry marked "trusted" while the actual AX API
    // is silently rejected. The cheap check then lies: pipeline thinks paste
    // will work, simulate_typing/paste calls CGEventPost which returns void
    // (no error), and the user sees clipboard + history populated but no
    // keystroke ever delivered. The probe catches this stale state.
    //
    // If we detect the stale state, also call `reset_accessibility_tcc()` so
    // the entry gets wiped — the user's next interaction will re-prompt
    // cleanly via the System Settings deep link below instead of looking at
    // a "TTP is enabled but not really" entry.
    #[cfg(target_os = "macos")]
    let has_accessibility = {
        let trusted_flag = check_accessibility();
        let actually_works = probe_accessibility();
        if trusted_flag && !actually_works {
            crate::logging::log_warn("[Pipeline] Accessibility TCC entry is stale - resetting so user can re-grant.");
            if let Err(e) = reset_accessibility_tcc() {
                crate::logging::log_error(&format!("[Pipeline] reset_accessibility_tcc failed: {}", e));
            }
        }
        actually_works
    };
    #[cfg(not(target_os = "macos"))]
    let has_accessibility = check_accessibility();

    // Pick the paste strategy based on length.
    //
    // For short text (the overwhelming majority of voice transcriptions) we
    // type the characters directly via enigo. The clipboard is written for
    // manual-Cmd+V fallback only — the actual insertion never touches the
    // pasteboard, so there is no race: as soon as enigo.text() returns, every
    // keystroke has been delivered to the focused app's HID queue and we can
    // restore the user's pre-record clipboard immediately.
    //
    // For long text we keep the clipboard+Cmd+V path because typing thousands
    // of characters one by one would be unacceptably slow. We trade the race
    // for speed and mitigate by waiting CLIPBOARD_PASTE_RESTORE_DELAY_MS
    // (1500 ms) before restoring — long enough for slow Electron apps to
    // actually read the pasteboard after Cmd+V.
    let use_direct_typing = final_text.chars().count() <= DIRECT_TYPING_MAX_CHARS;

    // Use spawn_blocking to run sync paste code safely in async context.
    // We deliberately use tauri::async_runtime::spawn_blocking instead of the
    // raw tokio variant because it binds the worker to the same runtime Tauri
    // is using — bare tokio::task::spawn_blocking has been seen to panic with
    // "there is no reactor running" on macOS when the call site is somehow
    // detached from the active runtime (see sounds.rs / audio_monitor.rs
    // for the same fix pattern).
    let paste_success = if has_accessibility {
        let paste_result = if use_direct_typing {
            let text_for_typing = final_text.clone();
            tauri::async_runtime::spawn_blocking(move || {
                std::panic::catch_unwind(|| simulate_typing(&text_for_typing))
            })
            .await
        } else {
            tauri::async_runtime::spawn_blocking(|| {
                std::panic::catch_unwind(|| simulate_paste())
            })
            .await
        };

        match paste_result {
            Ok(Ok(Ok(()))) => {
                if use_direct_typing {
                    // Direct typing already delivered every character to the
                    // focused app — no async pasteboard read in flight, so we
                    // can restore the user's clipboard immediately.
                } else {
                    // Clipboard+Cmd+V: give the target app a generous window
                    // to actually read NSPasteboard before we overwrite it
                    // with the original contents.
                    sleep(Duration::from_millis(CLIPBOARD_PASTE_RESTORE_DELAY_MS)).await;
                }

                // Restore original clipboard content
                if let Err(e) = clipboard_guard.restore() {
                    crate::logging::log_warn(&format!("[Pipeline] Failed to restore clipboard: {}", e));
                }

                // Start correction detection window (10 seconds to detect user corrections).
                // Skip when the final text is empty — the detection task would otherwise
                // poll Accessibility API for 15s for no reason (phantom F5 race, empty API).
                if final_text.trim().is_empty() {
                    crate::logging::log_info("[Pipeline] start_correction_window skipped - empty final_text");
                } else {
                    start_correction_window(app, final_text.clone());
                }

                true
            }
            Ok(Ok(Err(e))) => {
                crate::logging::log_error(&format!("[Pipeline] Paste simulation failed: {}", e));
                false
            }
            Ok(Err(_)) => {
                crate::logging::log_error("[Pipeline] Paste simulation panicked");
                false
            }
            Err(e) => {
                crate::logging::log_error(&format!("[Pipeline] Paste task failed: {}", e));
                false
            }
        }
    } else {
        crate::logging::log_info("[Pipeline] No accessibility permission - using clipboard fallback");

        // Open System Settings to Accessibility pane to help user grant permission
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                .spawn();
        }

        false
    };

    // Save to history (before completing) — only when user has history enabled
    if settings.history_enabled {
        // Store both final and raw text so user can see the unpolished transcription
        let raw_for_history = if settings.ai_polish_enabled {
            Some(raw_text.as_str())
        } else {
            None // No raw text if polish was disabled (they're the same)
        };

        if let Err(e) = add_history_entry(&final_text, raw_for_history) {
            crate::logging::log_warn(&format!("[Pipeline] Failed to save to history: {}", e));
        }
    }

    // Complete with appropriate message
    if paste_success {
        emit_progress(app, "complete", "", None);
    } else {
        // Clipboard fallback - show error in pill + system notification
        if !has_accessibility {
            emit_progress(app, "error", "error.enable_accessibility", None);
            notify(app, &crate::i18n::tr("notification.addToAccessibility"));
        } else {
            emit_progress(app, "error", "error.paste_failed", None);
            notify(app, &crate::i18n::tr("notification.textCopied"));
        }
    }

    // Record the successful transcription in the local daily-stats bucket
    // so the in-app Analytics section can show "this week / this month".
    // Local-only — no network. Capped at u32 to keep the on-disk payload
    // bounded; even a power user shouldn't dent that ceiling per day.
    let word_count = final_text.split_whitespace().count();
    let char_count = final_text.chars().count();
    crate::usage::record_transcription(
        word_count.try_into().unwrap_or(u32::MAX),
        char_count.try_into().unwrap_or(u32::MAX),
    );

    // Clean up audio files after processing
    let _ = std::fs::remove_file(&audio_path);
    if use_converted { let _ = std::fs::remove_file(&converted_path); }

    // AUDI-02: Delete backup only after successful transcription
    if let Some(ref bp) = backup_path {
        super::backup::remove_backup(bp);
    }

    set_state(app, RecordingState::Idle);
    Ok(final_text)
}

/// Tauri command to process a completed recording
///
/// Called by frontend after mic-recorder stops and returns the file path.
/// Runs the full transcription pipeline asynchronously.
#[tauri::command]
pub async fn process_audio(app: AppHandle, audio_path: String) -> Result<String, String> {
    // CRITICAL: every early-return Err MUST first reset state to Idle.
    // process_audio is called when the recording state machine is already
    // in Processing (set by shortcuts::stop_recording before stop_recording
    // IPC returns the path to JS). If we Err out without resetting state,
    // the app stays in Processing forever and ALL subsequent shortcut
    // presses are no-ops (handle_shortcut_pressed only acts on Idle or
    // Recording states). v3.1.0 user reported this exact stuck state after
    // a Ctrl+Space double-tap on Windows: very short recording triggered an
    // early-return path that bypassed the set_state in process_recording.
    //
    // The helper guarantees that every Err on the path between the start
    // of this function and the call into process_recording flips state
    // back to Idle and emits the user-facing error pill.
    let err_idle = |app: &AppHandle, key: &str| -> String {
        emit_progress(app, "error", key, None);
        set_state(app, RecordingState::Idle);
        key.to_string()
    };

    let limiter = PROCESS_AUDIO_LIMITER.get_or_init(|| {
        RateLimiter::direct(Quota::per_minute(NonZeroU32::new(20).unwrap()))
    });
    if limiter.check().is_err() {
        // Return the translation key rather than a finished string — the
        // frontend resolves it via i18next so the toast/pill matches the
        // user's selected language.
        return Err(err_idle(&app, "error.rate_limit_exceeded"));
    }

    // SECURITY: confine `audio_path` to the app's recordings dir. Without this,
    // a compromised renderer (XSS via an i18n string, malicious webview content,
    // a future dev oversight) could pass any local path — `~/.ssh/id_ed25519`,
    // `/etc/passwd`, a Keychain export — and we'd happily upload its bytes to
    // Groq Whisper and surface the result back. Canonicalize both sides so
    // symlinks, `..`, and `/var`↔`/private/var` (macOS) collapse before the
    // prefix check.
    //
    // On Windows, `std::fs::canonicalize` returns paths with the `\\?\`
    // extended-length prefix, so we canonicalize BOTH sides — comparing a
    // bare path against a `\\?\C:\...` would otherwise wrongly reject every
    // recording.
    let recordings_dir = match crate::recording::get_recording_dir(&app) {
        Ok(d) => d,
        Err(_) => return Err(err_idle(&app, "error.audio_file_not_found")),
    };
    let recordings_canon = std::fs::canonicalize(&recordings_dir).unwrap_or(recordings_dir);
    let audio_canon = match std::fs::canonicalize(&audio_path) {
        Ok(c) => c,
        Err(_) => return Err(err_idle(&app, "error.audio_file_not_found")),
    };
    if !audio_canon.starts_with(&recordings_canon) {
        crate::logging::log_error(&format!(
            "process_audio rejected out-of-tree path (canonical parent: {})",
            audio_canon.parent().map(|p| p.display().to_string()).unwrap_or_default()
        ));
        return Err(err_idle(&app, "error.audio_file_not_found"));
    }
    // Pass the canonical path forward — downstream fs::remove_file calls now
    // operate on a verified path, not the renderer-supplied string.
    let audio_path_str = match audio_canon.to_str() {
        Some(s) => s.to_string(),
        None => return Err(err_idle(&app, "error.audio_file_not_found")),
    };
    process_recording(&app, audio_path_str).await
}
