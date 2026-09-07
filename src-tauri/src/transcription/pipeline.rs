// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::get_groq_api_key_internal;
use crate::dictionary::detection::start_correction_window;
use crate::dictionary::apply_dictionary;
use crate::history::add_history_entry;
use crate::paste::{
    check_accessibility, describe_verification, finish_outcome, read_verdict, simulate_paste,
    simulate_typing, ClipboardGuard, FocusSnapshot, PasteVerdictSlot,
};
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

/// Consecutive polish failures, reset by the first success.
static POLISH_CONSECUTIVE_FAILURES: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

/// Whether this app session has already told the user polish is down.
static POLISH_OUTAGE_NOTIFIED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Consecutive failures before we surface a polish outage to the user.
///
/// A single failure is noise — a dropped connection, a transient 5xx — and
/// interrupting a dictation over it would be worse than staying quiet. A run
/// of them is a broken configuration, and the user needs to hear about it:
/// the existing signal was an `emit_progress` on the `pasting` stage, which
/// paints the pill for a fraction of a second before `complete` overwrites
/// it. That is how eight straight days of 404s went unnoticed while the
/// setting kept reporting the feature as on.
const POLISH_OUTAGE_THRESHOLD: u32 = 3;

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

/// How long the paste verification will keep re-reading the focused element
/// before concluding the keystrokes never landed.
///
/// Reading once immediately after `CGEventPost` is meaningless: the events sit
/// in the HID queue and the target application consumes them on its own run
/// loop, typically tens of milliseconds later — longer for Electron. A single
/// early read reports "nothing landed" for a perfectly successful paste, which
/// would make the signal worse than absent: it would send people hunting a
/// failure that never happened. 600 ms comfortably covers the slowest targets
/// we have measured.
const PASTE_VERIFY_TIMEOUT_MS: u64 = 600;

/// Gap between verification reads. Each read is an Accessibility round-trip,
/// so this trades resolution against the cost of hammering the target app.
///
/// Measured over the 540-dictation corpus: of the 329 verifications that could
/// read the target, `first_change_ms` had a median of 30 ms and a 90th
/// percentile of 60 ms, so this resolution is well inside what it needs to
/// resolve.
const PASTE_VERIFY_POLL_MS: u64 = 25;

/// Gap between verification reads on the *retry* path — when the baseline was
/// readable and the read-back was not.
///
/// Coarser than `PASTE_VERIFY_POLL_MS` on purpose. This path exists to catch a
/// target whose focus is momentarily unresolvable right after injection, not to
/// watch a field fill up, and each attempt is an Accessibility round-trip
/// against an app that has just told us it is not answering. Twenty-four such
/// probes in a 600 ms window would be hammering; six is a retry.
const PASTE_VERIFY_RETRY_MS: u64 = 100;

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
/// Write each entry the way Whisper emits it. The matcher canonicalizes
/// BOTH sides — see `canonical_exact` — so an entry does not have to be a
/// fixed point of `normalize_for_hallucination_match`, and must not be
/// hand-canonicalized into something unreadable.
///
/// This comment used to say the opposite: "each entry MUST be lowercase,
/// NFKD-normalized". Ten of the 105 entries did not obey it, six of those
/// could never match anything, and nobody could see it — because the
/// canonical form of a Korean or Japanese entry renders identically to the
/// form you would type. A contract you cannot proofread is not a contract.
/// `every_exact_entry_actually_fires` is, and it fails the build.
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

/// The two lists above, canonicalized once, in the same representation the
/// matcher compares against.
///
/// Measured 2026-09-06: ten of the 105 entries are not fixed points of
/// `normalize_for_hallucination_match`, so an entry written correctly by a
/// human could not equal (or be contained in) a candidate that had been
/// through the normalizer. Six of them could never fire at all — the two
/// Japanese entries (NFKD splits the dakuten off ご/が/ざ and the combining
/// mark is then stripped), the two Korean entries (NFKD splits precomposed
/// Hangul syllables into conjoining jamo, which are NOT combining marks and
/// so survive, leaving entry and candidate in different representations), the
/// Russian `подписывайтесь на мой канал` (й is и + U+0306), and
/// `transcribed by https://otter.ai` (the normalizer rewrites `:` to a
/// space). The other four — `.` `..` `...` `…` — canonicalize to the empty
/// string and are answered by the `empty` arm before the exact list is
/// consulted, so they are redundant rather than dead.
///
/// Canonicalizing here rather than in the literals is deliberate. The lists
/// are read and extended by humans, and no one can review NFKD Hangul jamo or
/// a bare combining breve: the canonical form of the Korean entry renders
/// identically to the precomposed one and differs only in code points. A
/// source list you cannot proofread is a worse defect than the one being
/// fixed. So the entries stay written the way Whisper emits them, the matcher
/// does the normalizing, and `every_exact_entry_actually_fires` /
/// `every_substring_entry_actually_fires_inside_surrounding_text` fail the
/// build if a future entry cannot be reached.
///
/// WHAT SWITCHING THE SIX ON ACTUALLY DELETES, measured 2026-09-06 over the
/// harvest corpus (600 dictations, 564 with text on file). Replayed
/// old-matcher against new-matcher, every text, verdict for verdict.
/// (The `0016`/`0017` below appear twice with different suffixes: the number
/// is a per-session sequence, so only the full id identifies a dictation.)
///
///   * TWO dictations change verdict, both `clean` → `exact`, and both are
///     the Japanese entry: `0024-6328` (2.40 s of audio, 15:58:20) and
///     `0028-ac50` (3.21 s, 15:59:00). Neither is speech. Two different
///     recordings, forty seconds apart, produced the SAME 14 characters —
///     `sha8:0b60f62b` on both — which is the signature of a degenerate
///     Whisper output rather than a transcription. Their neighbours settle
///     it: that run was the sound-pack session, and the maintainer was
///     recording the app's own notification beeps. `0016-3200`,
///     `0017-e9a8`, `0018-72b8` came back "Beeping"; `0019-fb60` and
///     `0023-0a18` "Beep."; `0020-d540`…`0022-db70` "Oh"; `0026-4ed8` and
///     `0027-2b10` "Bye."; `0029-6ff8` "Thank you." Both Japanese takes sit
///     inside that block. The revived rule deletes non-speech, which is what
///     it was written for.
///   * The other five revived rules delete NOTHING: 0 hits each across all
///     564 texts. The corpus is French and English, as expected.
///   * Three non-Latin dictations are NOT touched, and that is the near-miss
///     worth naming: `0016-a2b8` ("감사합니다."), `0017-6ab8`
///     ("ありがとうございます") and `0037-fa80` ("오케이.") all stay `Clean`.
///     The revived Korean entry is `시청해주셔서 감사합니다` and the exact arm
///     compares whole strings, so it cannot swallow the bare thanks — pinned
///     by `revived_entries_do_not_swallow_the_bare_thanks`.
///
/// So the expected answer was zero and the measured answer is two, and the
/// two are the rule working. It is still the measurement that mattered: the
/// same replay is what would have caught it going the other way.
fn canonical_exact() -> &'static [String] {
    static CANONICAL: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CANONICAL.get_or_init(|| {
        HALLUCINATIONS
            .iter()
            .map(|h| normalize_for_hallucination_match(h))
            .collect()
    })
}

/// As `canonical_exact`, minus anything that canonicalizes to nothing.
///
/// The filter is `normalized.contains(entry)`, and `contains("")` is true for
/// every string alive. One punctuation-only entry added to
/// `HALLUCINATION_SUBSTRINGS` would therefore delete every dictation the user
/// ever makes, silently, with the audio backup. Today nothing is dropped here
/// and `no_substring_entry_normalizes_to_nothing` asserts it stays that way;
/// the filter is the belt to that test's braces, because the cost of being
/// wrong is asymmetric enough to want both.
fn canonical_substrings() -> &'static [String] {
    static CANONICAL: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CANONICAL.get_or_init(|| {
        HALLUCINATION_SUBSTRINGS
            .iter()
            .map(|h| normalize_for_hallucination_match(h))
            .filter(|c| !c.is_empty())
            .collect()
    })
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
///
/// Returns the offending 3-grams rather than a bare `true`, because the caller
/// does not act on a loop until `gram_is_corroborated` says the loop is a
/// phrase we already know Whisper hallucinates. A detector that only answers
/// "yes" cannot be second-guessed; one that names its evidence can.
///
/// ALL chaining 3-grams, not the first one found. A phrase looped verbatim
/// chains on every rotation of itself — "sous-titrage société radio-canada"
/// produces three — and only the rotation that starts where the phrase starts
/// is a contiguous run inside the list entry. Returning one arbitrary member
/// of that set made corroboration depend on `HashMap` iteration order, i.e.
/// on nothing.
fn repetition_loop_grams(normalized: &str) -> Vec<[&str; 3]> {
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.len() < 9 {
        return Vec::new();
    }

    // Lexical diversity gate, checked before anything else.
    //
    // A degenerate Whisper loop emits the same phrase over and over —
    // "Sous-titrage Société Radio-Canada" twenty times — so its vocabulary
    // collapses: unique words end up a small fraction of total words. Human
    // speech does not do that even when it is deliberately repetitive.
    //
    // Without this gate the trigram rule below cannot tell rhetoric from a
    // loop, and on 2026-08-27 it deleted sixteen seconds of real dictation:
    // "je sais pas si tu l'as bien étudié, tu l'as bien regardé, tu l'as
    // bien compris" — three anaphoric repeats of "tu l'as bien", five words
    // apart, which is exactly the shape the chain detector looks for. 299
    // characters of genuine speech were dropped and the user was told no
    // speech had been detected.
    //
    // 0.5 is deliberately far from both populations: the dictation above
    // scores ~0.8, and a real loop scores ~0.1–0.2. Anything in between is
    // ambiguous enough that keeping the user's words is the better error.
    const MIN_UNIQUE_WORD_RATIO: f32 = 0.5;
    let unique_words: std::collections::HashSet<&&str> = words.iter().collect();
    if unique_words.len() as f32 / words.len() as f32 > MIN_UNIQUE_WORD_RATIO {
        return Vec::new();
    }

    tight_repeated_trigrams(&words)
}

/// The chain scan on its own: no length floor, no diversity gate, just
/// "does some 3-gram repeat `MIN_CHAIN` times with every consecutive pair
/// within `TIGHT_GAP` words". Returns every 3-gram that does.
///
/// Split out of `repetition_loop_grams` so the corroboration tests can run it
/// with `MIN_UNIQUE_WORD_RATIO` out of the picture. That is the point of the
/// split and not an implementation detail: the gate is a 0.5 threshold fitted
/// to a single dictation, and the safety of this filter is not supposed to
/// rest on it any more.
///
/// Sorted before returning: the scan walks a `HashMap`, and a filter whose
/// output order changes run to run is a filter whose tests are a coin toss.
fn tight_repeated_trigrams<'a>(words: &[&'a str]) -> Vec<[&'a str; 3]> {
    // Measured 2026-09-02 against 442 real dictations from the harvest window
    // and the 50 multi-word entries of `HALLUCINATIONS` looped verbatim.
    // (The corroboration measurement dated 2026-09-06, in
    // `gram_is_corroborated`, says 47 rather than 50. The list has moved
    // since: as of 2026-09-06 it holds 53 entries of two words or more and 47
    // of three or more, and only the latter is a set a 3-gram rule can see.
    // Neither count is wrong, they are four days and one question apart.)
    //
    // TIGHT_GAP bounds the PERIOD of a loop, not the distance between unrelated
    // words: a 3-gram inside a repeated phrase recurs exactly one phrase-length
    // later, so TIGHT_GAP must be at least as long as the phrase Whisper is
    // stuck on. The longest entry on file is ten words ("merci d'avoir regardé
    // cette vidéo n'hésitez pas à vous abonner"). At the previous value of 8,
    // looping the 50 known phrases caught 43/50 with two words of noise
    // injected; the seven misses were the seven longest, which is to say the
    // full-sentence YouTube outros — the most recognisable loops we have.
    // At 12 the detector catches 50/50, and 12 clears the longest observed
    // phrase by two words.
    //
    // Raising it costs nothing on the speech side, and that is the measurement
    // that matters: with the diversity gate above in front, ZERO of the 442
    // real dictations trigger the chain detector at ANY gap from 4 to 16. The
    // gap was never what protected them. Even with the gate disabled the count
    // is identical at 8 and at 12 (4 of 442). There is no separation to lose
    // here, only loop coverage to gain.
    const TIGHT_GAP: usize = 12;

    // MIN_CHAIN is the constraint that actually does the protecting, and its
    // margin is one occurrence.
    //
    // Of the 442 dictations, exactly one had vocabulary poor enough to reach
    // this code at all: trace 0047-9238, a 383-word monologue, unique-word
    // ratio 0.467. Its tightest 3-gram chain ("je ne sais pas", four words
    // apart) is TWO occurrences long. MIN_CHAIN = 3 spares it; MIN_CHAIN = 2
    // deletes it, at every TIGHT_GAP from 3 to 16. That is the whole safety
    // margin — one repeat — and it is why this must not be lowered.
    //
    // It must not be raised either: at MIN_CHAIN = 4 a phrase looped only
    // three times yields a chain of exactly 3 and is missed, which is 0/50 of
    // the known phrases at three repeats. Three tight repeats is both the
    // smallest unambiguous loop and the largest thing a person was observed
    // saying. The two populations meet here with nothing between them, unlike
    // MIN_UNIQUE_WORD_RATIO above, so this value is a boundary and not a
    // comfortable midpoint. Re-measure it if the corpus grows.
    const MIN_CHAIN: usize = 3;

    use std::collections::HashMap;
    let mut positions: HashMap<(&str, &str, &str), Vec<usize>> = HashMap::new();
    for (i, w) in words.windows(3).enumerate() {
        positions.entry((w[0], w[1], w[2])).or_default().push(i);
    }

    let mut found: Vec<[&'a str; 3]> = Vec::new();
    for (gram, occurrences) in positions.iter() {
        if occurrences.len() < MIN_CHAIN {
            continue;
        }
        let mut chain = 1usize;
        for pair in occurrences.windows(2) {
            if pair[1] - pair[0] <= TIGHT_GAP {
                chain += 1;
                if chain >= MIN_CHAIN {
                    found.push([gram.0, gram.1, gram.2]);
                    break;
                }
            } else {
                chain = 1;
            }
        }
    }
    found.sort_unstable();
    found
}

/// Does this 3-gram appear inside one of the phrases we already know Whisper
/// hallucinates?
///
/// This is the corroboration gate, and it is what makes the chain detector
/// safe to keep. Measured 2026-09-06 over the harvest corpus (586 dictations,
/// 550 with text on file) and the 47 multi-word `HALLUCINATIONS` entries
/// looped 3, 4 and 6 times:
///
///   * The chain detector is the ONLY rule that catches a looped known
///     phrase. Exact match catches 0 of 47 (a repeated string is not equal to
///     the entry); the substring list catches 0 of 47 (it holds signature
///     stems like `amara.org`, not the outros). The chain detector catches
///     47 of 47, and all 141 loop/repeat-count combinations are corroborated
///     here — the gate costs the filter nothing it was actually catching.
///   * On real speech the chain detector has ZERO true positives. Two of the
///     586 dictations aborted as `hallucination`, and only ONE of them was
///     this rule: `0004-ad78`, 299 characters of genuine French destroyed.
///     The other (`0125-c648`, "Uh...", 5 characters) was the exact-match
///     list — one word, so the nine-word floor here was never crossed.
///   * With `MIN_UNIQUE_WORD_RATIO` DISABLED the chain rule fires on 6 of the
///     550 texts. This gate spares all six: not one of their 3-grams occurs
///     in any known phrase. Only 1 of the 20,641 3-grams in the whole corpus
///     is corroborated at all ("thank you for"), and it never formed a chain.
///
/// RE-MEASURED 2026-09-06, after the ten non-canonical entries were made
/// matchable. Changing the entries changes the corroboration set, so the
/// numbers above had to be re-taken rather than assumed:
///
///   * The set is in fact IDENTICAL. This function already canonicalized the
///     entries it compared against — it was the only place that did — so
///     reading `canonical_exact`/`canonical_substrings` instead of
///     re-normalizing the literals per call is a speed change and nothing
///     else. The four entries that canonicalize to "" tokenize to nothing
///     and lose to the `needle.len() >= 3` floor exactly as before.
///   * Empirically, over the now-564-text corpus: 141/141 loop/repeat-count
///     combinations still chained and 141/141 still corroborated (exact
///     0/141, substring 0/141) — `every_looped_known_phrase_is_still_caught
///     _and_corroborated` holds it. With the diversity gate disabled the
///     chain rule fires on 7 texts, not 6: the corpus gained 14 texts since
///     2026-09-02 and one of them (`0057-2e90`) loops. 0 of the 7 are
///     corroborated, so all seven are still spared, `0004-ad78` among them.
///     1 of 21,766 3-grams is corroborated at all, still "thank you for",
///     still in one text (`0152-1d88`) and still never in a chain.
///
/// So safety stops depending on a 0.5 threshold fitted to one dictation and
/// on `MIN_CHAIN`'s one-occurrence margin, and starts depending on whether
/// the loop is a phrase we have on file.
///
/// Matching is on alphanumeric tokens, not on the whitespace words, because
/// Whisper writes the credits hyphenated ("sous-titrage société
/// radio-canada") while the list stores them spaced. Comparing words would
/// drop exactly the broadcaster credits this rule exists to catch. Requiring
/// at least three tokens is what stops a punctuation-only 3-gram (`% ? et`)
/// from collapsing to a two-letter needle that matches half the list.
fn gram_is_corroborated(gram: &[&str; 3]) -> bool {
    fn tokens(s: &str) -> Vec<&str> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect()
    }

    let mut needle: Vec<&str> = Vec::with_capacity(3);
    for word in gram {
        needle.extend(tokens(word));
    }
    if needle.len() < 3 {
        return false;
    }

    // The canonical lists, not the raw literals. Ten of the entries are not
    // fixed points of `normalize_for_hallucination_match` — the Japanese
    // dakuten, the Korean jamo and the Russian "й" all decompose under NFKD —
    // and the needle comes from text that has already been through it. Raw
    // comparison would silently fail to corroborate a looped Russian or
    // Japanese outro.
    //
    // This used to normalize the literals on every call, which was 120
    // normalizations per 3-gram and, worse, a second implementation of the
    // canonical form. While only this function canonicalized, it was the
    // exact and substring arms that were left broken. One cache, three
    // readers.
    canonical_exact()
        .iter()
        .chain(canonical_substrings().iter())
        .any(|canonical| {
            let hay = tokens(canonical);
            hay.len() >= needle.len()
                && hay.windows(needle.len()).any(|w| w == needle.as_slice())
        })
}

/// What the hallucination filter decided, and on what grounds.
///
/// The third variant is the whole reason this is not a `bool`. A repetition
/// loop that no known phrase corroborates is a NEAR MISS, not a match: the
/// text is kept and pasted, and the near miss is recorded so the next reader
/// can see the detector considered it. The asymmetry is why. Filtering costs
/// the user the dictation *and* the audio backup (`remove_backup`) and tells
/// them no speech was detected — irreversible. Not filtering costs them a
/// line of visible junk and one keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HallucinationVerdict {
    /// Nothing matched. Paste it.
    Clean,
    /// Drop the text. `rule` is the arm that fired, as a slug for the trace:
    /// `"empty"`, `"exact"`, `"substring"`, `"repetition_loop"`.
    Filtered { rule: &'static str },
    /// A tight 3-gram loop that appears in none of the known phrases.
    /// Non-destructive: keep `raw_text`, fall through to `cleanup`, paste.
    UncorroboratedLoop,
}

/// Filter out common Whisper hallucinations on silent / non-speech audio.
///
/// Strategy:
///   1. Normalize candidate (lowercase + NFKD accent strip + typographic
///      punctuation + apostrophes + whitespace)
///   2. Exact match against `HALLUCINATIONS` (canonical-form whole string)
///   3. Substring match against `HALLUCINATION_SUBSTRINGS` (signatures
///      that bleed into otherwise-valid text)
///   4. Detect 3-gram repetition loops — and delete only when the looping
///      3-gram is corroborated by one of the phrases in 2 or 3
fn classify_hallucination(text: &str) -> HallucinationVerdict {
    let normalized = normalize_for_hallucination_match(text);

    if normalized.is_empty() {
        return HallucinationVerdict::Filtered { rule: "empty" };
    }

    // PRIVACY: we never log the literal `text` or `normalized` payload — both
    // contain the user's verbatim speech. The hallucination filter is the
    // most aggressive false-positive source in the pipeline, and packaged
    // macOS stderr lands in Console.app where unrelated humans can read it.
    // We keep the diagnostic signal (which arm fired, how long) without the
    // content.
    let char_count = text.chars().count();
    if canonical_exact().iter().any(|h| *h == normalized) {
        crate::logging::log_info(&format!(
            "[Pipeline] filtered exact hallucination chars={}",
            char_count
        ));
        return HallucinationVerdict::Filtered { rule: "exact" };
    }

    if canonical_substrings()
        .iter()
        .any(|sub| normalized.contains(sub.as_str()))
    {
        crate::logging::log_info(&format!(
            "[Pipeline] filtered substring hallucination chars={}",
            char_count
        ));
        return HallucinationVerdict::Filtered { rule: "substring" };
    }

    let loop_grams = repetition_loop_grams(&normalized);
    if !loop_grams.is_empty() {
        // PRIVACY: a gram is three of the user's own words when it is
        // uncorroborated — which is the only case where it is interesting.
        // It is never logged, and never put on the trace.
        if loop_grams.iter().any(gram_is_corroborated) {
            crate::logging::log_info(&format!(
                "[Pipeline] filtered 3-gram repetition loop chars={}",
                char_count
            ));
            return HallucinationVerdict::Filtered {
                rule: "repetition_loop",
            };
        }
        crate::logging::log_info(&format!(
            "[Pipeline] uncorroborated repetition loop kept chars={}",
            char_count
        ));
        return HallucinationVerdict::UncorroboratedLoop;
    }

    HallucinationVerdict::Clean
}

/// Thin `bool` view of [`classify_hallucination`]: true only when the text is
/// actually dropped. An `UncorroboratedLoop` is deliberately `false` here.
///
/// Test-only. The pipeline reads the verdict itself, because `matched:false`
/// and "a loop we chose not to act on" are different lines in the trace.
#[cfg(test)]
fn is_hallucination(text: &str) -> bool {
    matches!(
        classify_hallucination(text),
        HallucinationVerdict::Filtered { .. }
    )
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

    /// `MIN_UNIQUE_WORD_RATIO` is a function-local constant in
    /// `repetition_loop_grams`. Mirrored here so the false-positive test can
    /// state where 0004-ad78 sits relative to it; keep the two in step.
    const MIN_UNIQUE_WORD_RATIO_FOR_TESTS: f32 = 0.5;

    // ── Repetition loop vs. human rhetoric ──────────────────────────────

    #[test]
    fn anaphora_is_not_a_repetition_loop() {
        // Real dictation, dropped in full on 2026-08-27. Three anaphoric
        // repeats of "tu l'as bien", five words apart — exactly the shape the
        // trigram chain detector looks for, and unmistakably a person talking.
        let text = "Ok très bien par contre son workflow je sais pas si tu l'as bien \
                    étudié, tu l'as bien regardé, tu l'as bien compris Parce qu'il y a \
                    des flèches dans tous les sens et c'est peut-être pas forcément \
                    simple pour toi pour la vision Ou peut-être mieux vaut pas commenter \
                    On pourra revoir ça ensemble ouais";
        assert!(
            !is_hallucination(text),
            "sixteen seconds of real speech must survive the loop detector"
        );
    }

    #[test]
    fn long_dictation_anaphora_survives() {
        // Trace 0308-9590, 2026-09-07 21:31: 121 seconds of speech, 2087
        // characters, dropped in full by the shipped 3.1.6 binary. The second
        // such loss, and four times longer than 0004-ad78.
        //
        // The trigger is "que ce soit" listing four alternatives — a figure of
        // speech, not a Whisper loop, and the chain detector cannot tell them
        // apart. Nothing in the known-hallucination list corroborates it.
        //
        // His words are not in this file. The shape is; the original is cited
        // by trace id above.
        let text = "J'ai beaucoup de choses à faire. Que ce soit les entraînements, \
                    que ce soit les étirements, que ce soit la prospection, que ce soit \
                    mon propre travail ou d'autres responsabilités qui sont \
                    transitoires. J'ai beaucoup de choses à faire, mais il n'y a que \
                    certaines actions qui sont vraiment clés.";
        assert!(
            !is_hallucination(text),
            "two minutes of real speech must survive the loop detector"
        );
    }

    #[test]
    fn repeated_list_intro_survives() {
        // Another shape people genuinely dictate: parallel clauses.
        let text = "il faut que je pense à acheter du pain, il faut que je pense à \
                    appeler le médecin, il faut que je pense à réserver le train";
        assert!(!is_hallucination(text));
    }

    #[test]
    fn degenerate_loop_is_still_caught() {
        // What the detector is actually for: the same phrase emitted over and
        // over on silence. Vocabulary collapses, so the diversity gate lets it
        // through to the chain detector.
        let text = "sous-titrage société radio-canada sous-titrage société radio-canada \
                    sous-titrage société radio-canada sous-titrage société radio-canada";
        assert!(is_hallucination(text));
    }

    #[test]
    fn uncorroborated_single_word_loop_is_now_kept() {
        // This used to assert `is_hallucination`, and the change is deliberate.
        //
        // Eleven "merci" in a row is a loop, and the chain detector still sees
        // it — but the 3-gram ("merci","merci","merci") occurs in no entry of
        // `HALLUCINATIONS` or `HALLUCINATION_SUBSTRINGS`, so it is not a
        // phrase we know Whisper hallucinates. It is exactly the shape the
        // corroboration gate refuses to act on, and the trade is the one the
        // gate exists to make: this pastes eleven visible words the user
        // deletes with one keystroke, where a wrong deletion takes the
        // dictation, the audio backup and any chance of recovering either.
        //
        // The loop is still DETECTED; only the deletion is withheld. The
        // trace says so: `matched:false, repetition_loop:true,
        // corroborated:false`.
        let text = "merci merci merci merci merci merci merci merci merci merci merci";
        let normalized = normalize_for_hallucination_match(text);
        let grams = repetition_loop_grams(&normalized);
        assert_eq!(grams, vec![["merci", "merci", "merci"]], "still a loop");
        assert!(
            !grams.iter().any(gram_is_corroborated),
            "no entry contains it"
        );
        assert_eq!(
            classify_hallucination(text),
            HallucinationVerdict::UncorroboratedLoop
        );
        assert!(!is_hallucination(text));
    }

    #[test]
    fn diversity_gate_sits_between_the_two_populations() {
        // The gate is only useful if the two populations are far from it.
        let human = "ok très bien par contre son workflow je sais pas si tu l'as bien \
                     étudié tu l'as bien regardé tu l'as bien compris parce qu'il y a des \
                     flèches dans tous les sens";
        let loopy = "thanks for watching thanks for watching thanks for watching thanks \
                     for watching thanks for watching";
        let ratio = |t: &str| {
            let w: Vec<&str> = t.split_whitespace().collect();
            let u: std::collections::HashSet<&&str> = w.iter().collect();
            u.len() as f32 / w.len() as f32
        };
        assert!(ratio(human) > 0.7, "human ratio was {}", ratio(human));
        assert!(ratio(loopy) < 0.3, "loop ratio was {}", ratio(loopy));
    }

    // ── The corroboration gate, measured 2026-09-06 ────────────────────
    //
    // Over 586 dictations (550 with text on file) the chain detector fired
    // exactly once, and it was wrong: 299 characters of real French deleted
    // (`0004-ad78`). The corpus's only other `hallucination` abort was the
    // exact-match list on a one-word take.
    // True positives on real speech: zero. Removing it is still not an
    // option — it is the only rule that catches a looped YouTube outro — so
    // it now has to name its evidence and have it corroborated.

    #[test]
    fn every_looped_known_phrase_is_still_caught_and_corroborated() {
        // The reason the chain detector cannot simply be deleted. Loop each
        // multi-word entry of `HALLUCINATIONS` verbatim and ask each rule
        // what it sees.
        let multi_word: Vec<&&str> = HALLUCINATIONS
            .iter()
            .filter(|h| normalize_for_hallucination_match(h).split_whitespace().count() >= 3)
            .collect();
        assert_eq!(
            multi_word.len(),
            47,
            "the corroboration measurement was taken over 47 multi-word \
             entries; if the list changed, re-take it"
        );

        let (mut exact, mut substring, mut chained, mut corroborated) = (0, 0, 0, 0);
        for phrase in &multi_word {
            for reps in [3usize, 4, 6] {
                let looped = vec![**phrase; reps].join(" ");
                let normalized = normalize_for_hallucination_match(&looped);

                if canonical_exact().iter().any(|h| *h == normalized) {
                    exact += 1;
                }
                if canonical_substrings()
                    .iter()
                    .any(|sub| normalized.contains(sub.as_str()))
                {
                    substring += 1;
                }
                let grams = repetition_loop_grams(&normalized);
                if !grams.is_empty() {
                    chained += 1;
                }
                if grams.iter().any(gram_is_corroborated) {
                    corroborated += 1;
                }

                assert!(
                    is_hallucination(&looped),
                    "a known phrase looped {} times must still be dropped: {:?}",
                    reps,
                    phrase
                );
            }
        }

        // Exact match fails on a repeated string; the substring list holds
        // signature stems (`amara.org`), not the outros. 0 of 141.
        assert_eq!(exact, 0, "exact match is not what catches looped phrases");
        assert_eq!(substring, 0, "the substring list is not either");
        // The chain detector is. 141 of 141 — and every one of them is
        // corroborated, so the gate costs it nothing it was catching.
        assert_eq!(chained, 141, "the chain detector is the only rule that sees these");
        assert_eq!(corroborated, 141, "and the corroboration gate keeps all of them");
    }

    #[test]
    fn the_false_positive_is_spared_with_the_diversity_gate_disabled() {
        // Trace 0004-ad78, 2026-08-27 22:02:55 — 299 characters of genuine
        // French dictation deleted in full, the audio backup deleted with it,
        // and the user told no speech had been detected.
        //
        // Its words stay out of the repository. Its arithmetic does not:
        //   56 words, 45 unique (ratio 0.804), and one 3-gram — "tu l'as
        //   bien", normalized to "tu las bien" — at word positions 11, 15 and
        //   19. Three occurrences, four words apart: a chain of exactly
        //   MIN_CHAIN at well inside TIGHT_GAP. That is a loop by every
        //   structural test the detector applies.
        //
        // Today MIN_UNIQUE_WORD_RATIO is the only thing in front of it, and
        // 0.5 is a threshold fitted to this one dictation. So take the gate
        // away — call the chain scan directly — and check what is left.
        let mut words: Vec<String> = (0..56).map(|i| format!("w{}", i)).collect();
        for start in [11usize, 15, 19] {
            words[start] = "tu".to_string();
            words[start + 1] = "las".to_string();
            words[start + 2] = "bien".to_string();
        }
        // Five more repeats of ordinary filler, far apart, to land on the
        // trace's real 45/56. They are two occurrences at a gap of 30, so
        // they add no chain of their own.
        for i in 0..5 {
            words[30 + i] = format!("w{}", i);
        }
        let refs: Vec<&str> = words.iter().map(|w| w.as_str()).collect();
        let unique: std::collections::HashSet<&&str> = refs.iter().collect();
        assert_eq!(refs.len(), 56, "0004-ad78 was 56 words");
        assert_eq!(unique.len(), 45, "45 of them unique");
        assert!(
            unique.len() as f32 / refs.len() as f32 > MIN_UNIQUE_WORD_RATIO_FOR_TESTS,
            "and its ratio is on the safe side of the gate — which is exactly \
             why the gate must not be the thing we rely on"
        );

        // MIN_UNIQUE_WORD_RATIO neutered: the chain rule on its own.
        let grams = tight_repeated_trigrams(&refs);
        assert_eq!(
            grams,
            vec![["tu", "las", "bien"]],
            "with the diversity gate out of the picture the chain rule DOES \
             fire on this dictation"
        );

        // And this is what now stands between it and deletion. The looping
        // 3-gram appears in no entry of either list — measured over the whole
        // corpus, only 1 of 20,641 3-grams in 550 real dictations is
        // corroborated at all, and it never formed a chain.
        assert!(
            !grams.iter().any(gram_is_corroborated),
            "nothing on file contains this 3-gram, so the loop is not evidence \
             of a hallucination"
        );

        // Both gates together, through the real entry point.
        let joined = refs.join(" ");
        assert!(repetition_loop_grams(&joined).is_empty());
        assert!(!is_hallucination(&joined));
    }

    #[test]
    fn an_uncorroborated_loop_is_kept_not_deleted() {
        // The behaviour change stated as a verdict: a loop the lists do not
        // recognise is a NEAR MISS. `raw_text` survives, the pipeline falls
        // through to `cleanup`, and the paste happens. The trace records
        // `matched:false` with `repetition_loop:true, corroborated:false` so
        // the near miss is still legible to the next reader.
        let text = "zorbat plimful quenda zorbat plimful quenda zorbat plimful \
                    quenda zorbat plimful quenda";
        let normalized = normalize_for_hallucination_match(text);
        assert!(
            !repetition_loop_grams(&normalized).is_empty(),
            "fixture must actually be a loop"
        );
        assert_eq!(
            classify_hallucination(text),
            HallucinationVerdict::UncorroboratedLoop
        );
        assert!(!is_hallucination(text), "and it is NOT deleted");
    }

    #[test]
    fn corroboration_matches_hyphenated_credits() {
        // Whisper writes the broadcaster credits hyphenated; the list stores
        // them spaced. Comparing whitespace words would fail to corroborate
        // exactly the loops this rule exists to catch, so corroboration
        // compares alphanumeric tokens.
        assert!(gram_is_corroborated(&[
            "sous-titrage",
            "societe",
            "radio-canada"
        ]));
        assert!(gram_is_corroborated(&["thanks", "for", "watching"]));
        // And a punctuation-heavy 3-gram must not collapse into a two-letter
        // needle that matches half the list.
        assert!(!gram_is_corroborated(&["%", "?", "et"]));
        assert!(!gram_is_corroborated(&["tu", "las", "bien"]));
    }

    // ── Every entry must be able to fire, measured 2026-09-06 ──────────
    //
    // These are the guard, and they are worth more than the ten entries they
    // caught today. A list that is compared against normalized text but is
    // not itself normalized rots silently: the entry is right there in the
    // source, reviewed and readable, and matches nothing. Ten of the 120
    // entries were in that state, and six of them could never fire at all.
    //
    // Nothing here asserts a canonical *spelling*. The invariant is weaker
    // and more useful: feed the entry to the filter and the filter must drop
    // it. That holds however the entry is written, in whatever script, and it
    // fails the moment someone adds one the matcher cannot reach.

    #[test]
    fn every_exact_entry_actually_fires() {
        for entry in HALLUCINATIONS {
            assert!(
                is_hallucination(entry),
                "HALLUCINATIONS entry cannot match anything: {:?}",
                entry
            );
        }
    }

    #[test]
    fn every_substring_entry_actually_fires_inside_surrounding_text() {
        // The substring list exists for signatures Whisper bleeds INTO valid
        // text, so the fixture has to have text around it.
        for sub in HALLUCINATION_SUBSTRINGS {
            let bled = format!("bonjour voici le message {} et voila", sub);
            assert!(
                is_hallucination(&bled),
                "HALLUCINATION_SUBSTRINGS entry cannot match anything: {:?}",
                sub
            );
        }
    }

    #[test]
    fn the_six_scripts_that_could_never_fire() {
        // The six dead rules, named. Each is written here exactly as Whisper
        // emits it — precomposed, which is what any editor and any input
        // method produce — and each was compared against text that had been
        // NFKD-decomposed by the matcher, so equality was impossible.
        //
        // Japanese: the dakuten/handakuten are combining marks after NFKD, so
        // ご→こ, が→か, ざ→さ. Korean: NFKD decomposes precomposed Hangul
        // syllables into conjoining jamo (U+C2DC → U+1109 U+1175), which are
        // not combining marks and therefore survive the strip — the entry and
        // the candidate end up in different representations. Russian: й is
        // и + U+0306 combining breve. And `:` is rewritten to a space, so no
        // entry containing a URL scheme could survive its own normalizer.
        assert!(is_hallucination("ご視聴ありがとうございました"));
        assert!(is_hallucination("見てくれてありがとう"));
        assert!(is_hallucination("подписывайтесь на мой канал"));
        assert!(is_hallucination("시청해주셔서 감사합니다"));
        assert!(is_hallucination("구독 부탁드립니다"));
        assert!(is_hallucination(
            "voila le compte rendu Transcribed by https://otter.ai"
        ));

        // And the half of the fix the assertions above cannot see. Those
        // compare an entry with itself, so they fail only if the matcher
        // stops canonicalizing at all — true of the bug being fixed, but not
        // a proof that BOTH sides are canonicalized. These are the same three
        // phrases written decomposed (NFD), which is a form Whisper can
        // legitimately return and which no editor distinguishes from the
        // precomposed literals in the list: identical on screen, different
        // code points. They match only because the candidate is normalized
        // too.
        assert!(is_hallucination("\u{3053}\u{3099}\u{8996}\u{8074}\u{3042}\u{308A}\u{304B}\u{3099}\u{3068}\u{3046}\u{3053}\u{3099}\u{3055}\u{3099}\u{3044}\u{307E}\u{3057}\u{305F}"));
        assert!(is_hallucination("\u{1109}\u{1175}\u{110E}\u{1165}\u{11BC}\u{1112}\u{1162}\u{110C}\u{116E}\u{1109}\u{1167}\u{1109}\u{1165} \u{1100}\u{1161}\u{11B7}\u{1109}\u{1161}\u{1112}\u{1161}\u{11B8}\u{1102}\u{1175}\u{1103}\u{1161}"));
        assert!(is_hallucination("\u{043F}\u{043E}\u{0434}\u{043F}\u{0438}\u{0441}\u{044B}\u{0432}\u{0430}\u{0438}\u{0306}\u{0442}\u{0435}\u{0441}\u{044C} \u{043D}\u{0430} \u{043C}\u{043E}\u{0438}\u{0306} \u{043A}\u{0430}\u{043D}\u{0430}\u{043B}"));
    }

    #[test]
    fn revived_entries_do_not_swallow_the_bare_thanks() {
        // The measured near miss. Turning the CJK entries on was checked
        // against 564 real dictations and moved exactly two verdicts, both
        // non-speech (see `canonical_exact`). Three other non-Latin
        // dictations were in range and had to stay untouched — `0016-a2b8`,
        // `0017-6ab8`, `0037-fa80` — because the bare thanks is something a
        // person says and the outro is not.
        //
        // The exact arm compares WHOLE strings, which is what keeps them
        // apart: `시청해주셔서 감사합니다` is on the list, `감사합니다` is not.
        // A future maintainer moving any of these into
        // `HALLUCINATION_SUBSTRINGS` would delete all three, and this is the
        // test that says so.
        assert!(!is_hallucination("감사합니다."));
        assert!(!is_hallucination("오케이."));
        assert!(!is_hallucination("ありがとうございます"));
        assert!(!is_hallucination("спасибо"));
    }

    #[test]
    fn the_punctuation_entries_are_caught_by_the_empty_arm_not_the_exact_one() {
        // The other four non-fixed-points. These are NOT dead: `.` normalizes
        // to the empty string, and so does every all-punctuation candidate,
        // so `classify_hallucination` returns at its `empty` arm before the
        // exact list is ever consulted. They are redundant rather than
        // broken, and this test says which — so nobody "fixes" them by
        // canonicalising them into four empty strings.
        let mut via_empty: Vec<&str> = Vec::new();
        for entry in HALLUCINATIONS {
            match classify_hallucination(entry) {
                HallucinationVerdict::Filtered { rule: "exact" } => {}
                HallucinationVerdict::Filtered { rule: "empty" } => via_empty.push(entry),
                other => panic!("entry {:?} is not filtered at all: {:?}", entry, other),
            }
        }
        assert_eq!(via_empty, vec![".", "..", "...", "\u{2026}"]);
    }

    #[test]
    fn no_substring_entry_normalizes_to_nothing() {
        // A substring entry that canonicalizes to "" would make
        // `normalized.contains(entry)` true for EVERY dictation — the whole
        // corpus deleted by one bad list edit. `canonical_substrings` drops
        // empties for that reason; this test is what stops the drop from
        // being silent.
        assert_eq!(
            canonical_substrings().len(),
            HALLUCINATION_SUBSTRINGS.len(),
            "an entry canonicalized to nothing and was dropped from the \
             matcher; it would otherwise match every dictation ever made"
        );
        assert!(canonical_substrings().iter().all(|s| !s.trim().is_empty()));
    }

    // ── TIGHT_GAP / MIN_CHAIN, measured 2026-09-02 ─────────────────────
    //
    // These two constants shipped bare for months. The measurement behind them
    // is in `repetition_loop_grams`; these are the tests that hold it.

    #[test]
    fn tight_gap_spans_the_longest_loop_phrase_we_have_seen() {
        // A 3-gram inside a looped phrase recurs exactly one phrase-length
        // apart, so TIGHT_GAP is a bound on the PERIOD of the loop, i.e. on the
        // length of the repeated phrase. The longest entry in `HALLUCINATIONS`
        // is ten words. At TIGHT_GAP = 8 the two ten-word entries looped
        // verbatim were invisible to the chain detector — the gate that exists
        // precisely to catch loops could not see the longest loops on file.
        let ten_words =
            "merci d'avoir regardé cette vidéo n'hésitez pas à vous abonner ";
        let looped = ten_words.repeat(4);
        assert!(
            !repetition_loop_grams(&normalize_for_hallucination_match(&looped)).is_empty(),
            "a ten-word phrase looped four times must be caught; TIGHT_GAP is \
             too small to span its period"
        );

        let eight_words = "n'oubliez pas de liker et de vous abonner ";
        assert!(!repetition_loop_grams(&normalize_for_hallucination_match(
            &eight_words.repeat(4)
        ))
        .is_empty());
    }

    #[test]
    fn min_chain_of_three_is_what_saves_the_long_monologue() {
        // The binding constraint is MIN_CHAIN, not TIGHT_GAP. Across 442 real
        // dictations exactly one had vocabulary poor enough to pass the 0.5
        // diversity gate (trace 0047-9238, a 383-word monologue, ratio 0.467),
        // and its tightest 3-gram chain was TWO occurrences four words apart.
        // MIN_CHAIN = 3 spared it by exactly one occurrence. At MIN_CHAIN = 2
        // it is destroyed at every TIGHT_GAP from 3 to 16.
        //
        // So: two tight occurrences are speech, three are a loop. This test
        // pins that boundary on a minimal fixture; the monologue's own words
        // are private and stay out of the repository.
        let two = "je ne sais pas trop ".repeat(2);
        let normalized = normalize_for_hallucination_match(&two);
        let words: Vec<&str> = normalized.split_whitespace().collect();
        let unique: std::collections::HashSet<&&str> = words.iter().collect();
        assert!(
            unique.len() as f32 / words.len() as f32 <= 0.5,
            "fixture must actually reach the chain detector, ratio was {}",
            unique.len() as f32 / words.len() as f32
        );
        assert!(
            repetition_loop_grams(&normalized).is_empty(),
            "two tight occurrences of a 3-gram is a person repeating themselves"
        );

        let three = "je ne sais pas trop ".repeat(3);
        assert!(
            !repetition_loop_grams(&normalize_for_hallucination_match(&three)).is_empty(),
            "three tight occurrences is a loop"
        );
    }

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

    // One trace per dictation. Every `return Err` below is a path where the
    // user pressed the hotkey, spoke, and got nothing — and until this
    // existed, all of them were indistinguishable from the outside. The
    // trace is written to its own file regardless of the main log level;
    // see `crate::trace`.
    let trace = crate::trace::Trace::start("recording");

    // The settings that govern THIS dictation, recorded once at the top.
    //
    // docs/tracing.md used to list "settings changes" as a blind spot: the
    // trace showed the consequences of a setting (`whisper.request.lang`,
    // `polish.decision`) but never the configuration itself, so a dictation
    // that behaved oddly could not be compared against one that did not. A
    // dictation is a function of its settings; the inputs belong in the record.
    // All slugs and booleans — nothing here is user text.
    {
        let s = get_settings();
        trace.stage(
            "settings.snapshot",
            serde_json::json!({
                "ai_polish": s.ai_polish_enabled,
                "transcription_language": s.transcription_language.as_deref().unwrap_or("auto"),
                "vad_auto_stop": s.vad_auto_stop_enabled,
                "vad_silence_secs": s.vad_silence_secs,
                "hands_free": s.hands_free_mode,
                "history": s.history_enabled,
                "diagnostics": s.diagnostics_enabled,
                "audio_device_pinned": s.audio_device_name.is_some(),
                "companion_face": s.companion_face_enabled,
            }),
        );
    }

    // Set state to Processing
    set_state(app, RecordingState::Processing);

    // Check if audio file exists
    let audio_file = Path::new(&audio_path);
    let file_size = match std::fs::metadata(audio_file) {
        Ok(meta) => meta.len(),
        Err(e) => {
            trace.abort("audio_file_missing", serde_json::json!({ "error": e.to_string() }));
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
        trace.abort(
            "wav_invalid",
            serde_json::json!({ "error": msg.clone(), "wav_bytes": file_size }),
        );
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
            trace.abort(
                "recording_empty",
                serde_json::json!({ "secs": secs, "wav_bytes": file_size }),
            );
            set_state(app, RecordingState::Idle);
            return Err(format!("Empty recording: {:.3}s of audio", secs));
        }
        Ok(secs) => {
            trace.stage(
                "audio.duration",
                serde_json::json!({ "secs": secs, "wav_bytes": file_size }),
            );
        }
        Err(e) => {
            // Defensive: if we can't even read the duration, treat as corrupt
            // (validate_wav already passed, so this should be rare).
            log_error(&format!("Could not read WAV duration: {}", e));
            trace.stage("audio.duration", serde_json::json!({ "error": e.to_string() }));
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
    match super::backup::wav_signal_stats(&audio_path) {
        // Digital silence — every sample exactly zero. NOT the same as "the
        // user didn't speak": a live microphone in a quiet room always has a
        // noise floor. All zeros means the audio callback delivered nothing,
        // so the honest message is the one about a dead capture device, not
        // "no speech detected". Telling someone who just dictated for eight
        // seconds that they hadn't spoken is how an app loses trust.
        Ok(stats) if stats.is_dead_capture() => {
            let capture_device = crate::audio_capture::last_capture_device();
            log_error(&format!(
                "Dead capture on device {:?}: {} samples, every one zero, in {} bytes. \
                 Likely cause: a Bluetooth input that connected but never streamed, \
                 a revoked mic permission, or the microphone held exclusively by \
                 another process.",
                capture_device.as_deref().unwrap_or("<unknown>"),
                stats.samples,
                file_size
            ));
            let _ = std::fs::remove_file(&audio_path);
            emit_progress(app, "error", "error.recording_empty", None);
            notify(app, &crate::i18n::tr("notification.recordingEmpty"));
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
                "error_category": "dead_capture",
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
                "samples": stats.samples,
                "wav_bytes": file_size,
            })));
            trace.abort(
                "dead_capture",
                serde_json::json!({
                    "device": capture_device,
                    "samples": stats.samples,
                    "peak": stats.peak,
                    "nonzero_ratio": stats.nonzero_ratio,
                    "wav_bytes": file_size,
                }),
            );
            set_state(app, RecordingState::Idle);
            return Err("Microphone delivered no audio".to_string());
        }
        Ok(stats) if stats.rms_after_silence < SILENCE_RMS_FLOOR => {
            crate::logging::log_info(&format!(
                "Silent recording detected (RMS {:.4} over the audio that arrived, \
                 {:.4} overall, floor {:.4}), skipping Whisper",
                stats.rms_after_silence, stats.rms, SILENCE_RMS_FLOOR
            ));
            let _ = std::fs::remove_file(&audio_path);
            emit_progress(app, "error", "error.no_speech", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
                "error_category": "silent_audio",
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
                "avg_rms": stats.rms,
            })));
            trace.abort(
                "silent_audio",
                serde_json::json!({
                    "device": crate::audio_capture::last_capture_device(),
                    "avg_rms": stats.rms,
                    "rms_after_silence": stats.rms_after_silence,
                    "leading_silence": stats.leading_silence,
                    "peak": stats.peak,
                    "nonzero_ratio": stats.nonzero_ratio,
                    "floor": SILENCE_RMS_FLOOR,
                }),
            );
            set_state(app, RecordingState::Idle);
            return Err("Silent recording — skipped Whisper".to_string());
        }
        Ok(stats) => {
            // `leading_silence` is the measurable form of "my Bluetooth mic
            // ate the start of my sentence". Recorded on every dictation, not
            // just failures, so the phenomenon can be quantified rather than
            // argued about.
            trace.stage(
                "audio.signal",
                serde_json::json!({
                    "avg_rms": stats.rms,
                    "rms_after_silence": stats.rms_after_silence,
                    "leading_silence": stats.leading_silence,
                    "peak": stats.peak,
                    "nonzero_ratio": stats.nonzero_ratio,
                    "samples": stats.samples,
                    "floor": SILENCE_RMS_FLOOR,
                }),
            );
        }
        Err(e) => {
            // Defensive: any read error is non-fatal — fall through to
            // Whisper. validate_wav already vouched for the file structure.
            crate::logging::log_warn(&format!("Could not compute WAV signal stats: {}", e));
            trace.stage("audio.signal", serde_json::json!({ "error": e.to_string() }));
        }
    }

    // Convert stereo 48kHz WAV → mono 16kHz WAV (reduces size ~6x)
    let convert_span = crate::trace::Span::start();
    let converted_path = match convert_to_mono_16khz(&audio_path) {
        Ok(path) => path,
        Err(e) => {
            crate::logging::log_warn(&format!("[Pipeline] Conversion failed: {} - sending original", e));
            // Degrading, not failing: we upload the original instead. Six
            // times the bytes, six times the upload, and a much closer brush
            // with the 25 MB ceiling — none of which was visible in the trace
            // before, because the fallback path wrote no line at all.
            trace.degraded(
                "audio.convert",
                serde_json::json!({ "error": e.to_string(), "fallback": "original_wav" }),
            );
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
        trace.abort("audio_too_large", serde_json::json!({ "mb": original_mb, "converted": false }));
        set_state(app, RecordingState::Idle);
        log_error(&format!("Conversion failed and original too large: {:.1}MB exceeds {}MB limit", original_mb, MAX_AUDIO_SIZE / 1_000_000));
        return Err(format!("Audio too large: {:.1}MB exceeds API limit", original_mb));
    }

    // Check size AFTER conversion (raw WAV can be large but converts down)
    let final_size = match std::fs::metadata(&final_upload_path).map(|m| m.len()) {
        Ok(len) => len,
        Err(e) => {
            // Every size check below now runs against the PRE-conversion
            // size. That can pass a file the API will reject, and used to do
            // so without leaving a trace of why the number was wrong.
            trace.degraded(
                "audio.size",
                serde_json::json!({ "error": e.to_string(), "fell_back_to": file_size }),
            );
            file_size
        }
    };
    let final_mb = final_size as f64 / 1_000_000.0;

    trace.timed(
        "audio.convert",
        &convert_span,
        serde_json::json!({
            "converted": use_converted,
            "in_bytes": file_size,
            "out_bytes": final_size,
        }),
    );

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
        trace.abort("audio_too_large", serde_json::json!({ "mb": final_mb, "converted": use_converted }));
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
            // Contention on the AppState mutex during a dictation is the same
            // condition that makes hotkey presses vanish (`hotkey.event_dropped`).
            // "unknown" in the analytics payload hid it; this names it.
            trace.degraded("input_mode", serde_json::json!({ "reason": "state_lock_busy" }));
            "unknown"
        }
    } else {
        trace.degraded("input_mode", serde_json::json!({ "reason": "state_unmanaged" }));
        "unknown"
    };

    // Get Groq API key.
    //
    // This is a keychain read, and a keychain read is not bounded: macOS
    // re-evaluates the ACL whenever the requesting binary's code signature
    // changes and blocks while it does — for seconds, and in the worst case
    // until someone clicks an authorization dialog they never saw. It sits
    // directly between the user finishing their sentence and Whisper being
    // called, and it was completely untimed. The `?` below was also the one
    // early return in this function that produced NO `dictation.finish` line
    // at all, so a keychain error ended a dictation invisibly.
    let key_span = crate::trace::Span::start();
    let groq_key = match get_groq_api_key_internal(app) {
        Ok(k) => {
            let k = k.filter(|k| !k.is_empty());
            trace.timed(
                "keychain.api_key",
                &key_span,
                serde_json::json!({ "found": k.is_some() }),
            );
            k
        }
        Err(e) => {
            trace.timed("keychain.api_key", &key_span, serde_json::json!({ "error": e.clone() }));
            trace.abort("api_key_read_failed", serde_json::json!({ "error": e.clone() }));
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

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
            trace.abort("no_api_key", serde_json::Value::Null);
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
            // Analytics only leaves the machine when telemetry is on, and
            // telemetry is off by default. Locally this was a log_warn nobody
            // reads. It matters: with no backup, a later API failure loses the
            // audio the user would have wanted to retry.
            trace.degraded(
                "backup.audio",
                serde_json::json!({ "category": category, "retryable": false }),
            );
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

    trace.stage(
        "whisper.request",
        serde_json::json!({
            "bytes": final_size,
            "lang": whisper_lang.unwrap_or("auto"),
            "prompt": whisper_prompt.is_some(),
            "input_mode": input_mode,
        }),
    );
    let whisper_start = std::time::Instant::now();

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
            trace.abort(
                "whisper_error",
                serde_json::json!({
                    "error_category": error_category,
                    "status_code": status_code,
                    "whisper_ms": whisper_start.elapsed().as_millis() as u64,
                }),
            );
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

    // This line used to carry `attempt: 1`, hardcoded, and it read `1` on all
    // 550 responses in the corpus — including the dictation whose internal
    // loop burned three attempts before failing (`0185-a9c0`). The real count
    // lives on `whisper.attempt`, emitted per attempt by `whisper.rs`. What
    // this line can honestly say is which of the two `whisper.response` lines
    // a dictation may emit this one is: the first call, or the resubmit after
    // an empty body.
    trace.text_stage(
        "whisper.response",
        &raw_text,
        serde_json::json!({
            "empty_body_retry": false,
            "ms": whisper_start.elapsed().as_millis() as u64,
        }),
    );

    // Groq whisper-large-v3 occasionally returns 200 OK with an empty body on
    // long single-speaker monologues — a known silent-failure mode. One
    // resubmit almost always recovers the transcription. Cheaper than the
    // user-facing "no sound" error.
    let raw_text = if raw_text.trim().is_empty() {
        crate::logging::log_warn(
            "Empty transcription from Groq, retrying once before surfacing no_speech",
        );
        trace.stage("whisper.retry", serde_json::json!({ "reason": "empty_body" }));
        let retried = transcribe_audio(&api_key, transcription_path, whisper_prompt.as_deref(), whisper_lang)
            .await
            .unwrap_or_default();
        trace.text_stage(
            "whisper.response",
            &retried,
            serde_json::json!({
                "empty_body_retry": true,
                // Measured from `whisper_start`, so this is both calls: the
                // empty first one plus this one. `whisper.attempt` has the
                // per-request timings.
                "ms": whisper_start.elapsed().as_millis() as u64,
            }),
        );
        retried
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
        trace.abort("no_speech", serde_json::json!({ "after_retry": true }));
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
        let words: Vec<&str> = raw_text.trim().split_whitespace().collect();
        // Whether the filter was even eligible to run, kept separate from
        // whether it fired. "No dictionary" and "long recording" are the two
        // reasons this filter routinely does nothing, and a reader who cannot
        // tell those from "it ran and passed the text" cannot rule the filter
        // out as the thing that ate a transcription.
        let considered =
            !dict_entries.is_empty() && approx_duration_secs < 10.0 && words.len() <= 2;
        let all_in_dict = if considered {
            let dict_words: Vec<String> = dict_entries.iter()
                .map(|e| e.correction.to_lowercase())
                .collect();
            words.iter().all(|w| {
                let w_lower = w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                dict_words.iter().any(|d| d == &w_lower)
            })
        } else {
            false
        };
        trace.decision(
            "filter.glossary_ghost",
            all_in_dict,
            serde_json::json!({
                "considered": considered,
                "dict_entries": dict_entries.len(),
                "words": words.len(),
                "audio_secs": approx_duration_secs,
            }),
        );
        if all_in_dict {
            let _ = std::fs::remove_file(&audio_path);
            if use_converted { let _ = std::fs::remove_file(&converted_path); }
            if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
            emit_progress(app, "error", "error.no_speech", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
            trace.abort(
                "glossary_ghost",
                serde_json::json!({
                    "words": words.len(),
                    "audio_secs": approx_duration_secs,
                }),
            );
            set_state(app, RecordingState::Idle);
            return Err("No speech detected (glossary ghost)".to_string());
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
        let considered = approx_duration_secs < 6.0 && word_count <= 8;
        let leaked = considered && intro_words.iter().any(|&w| leading == w);
        trace.decision(
            "filter.prompt_introducer",
            leaked,
            serde_json::json!({
                "considered": considered,
                "words": word_count,
                "audio_secs": approx_duration_secs,
            }),
        );
        if leaked {
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
            trace.abort(
                "prompt_introducer_leak",
                serde_json::json!({ "leading": leading, "words": word_count }),
            );
            set_state(app, RecordingState::Idle);
            return Err("No speech detected (prompt-introducer leak)".to_string());
        }
    }

    // Filter out common Whisper hallucinations on silent audio.
    //
    // This filter once deleted real speech, and the only evidence it left was
    // an absence. `matched:false` is the line that makes the absence legible:
    // with it, "no hallucination line" means the pipeline never got here, and
    // an empty target app after a `matched:false` is somebody else's fault.
    //
    // A repetition loop no known phrase corroborates does NOT get here as a
    // match. It is recorded as `matched:false` with `repetition_loop:true,
    // corroborated:false` and falls through to `cleanup` like any other
    // transcription — see `HallucinationVerdict`.
    let verdict = classify_hallucination(&raw_text);
    let hallucinated = matches!(verdict, HallucinationVerdict::Filtered { .. });
    let mut hallucination_fields = serde_json::json!({ "chars": raw_text.chars().count() });
    match verdict {
        HallucinationVerdict::Filtered { rule } => {
            hallucination_fields["rule"] = rule.into();
            if rule == "repetition_loop" {
                hallucination_fields["repetition_loop"] = true.into();
                hallucination_fields["corroborated"] = true.into();
            }
        }
        HallucinationVerdict::UncorroboratedLoop => {
            hallucination_fields["repetition_loop"] = true.into();
            hallucination_fields["corroborated"] = false.into();
        }
        HallucinationVerdict::Clean => {}
    }
    trace.decision("filter.hallucination", hallucinated, hallucination_fields);
    if hallucinated {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "error.no_speech", None);
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        // The single most opaque drop in the pipeline: Whisper returned real
        // characters and we deleted all of them. The `whisper.response` line
        // above holds what was dropped (text included when diagnostics are on).
        trace.abort(
            "hallucination",
            serde_json::json!({ "chars": raw_text.chars().count() }),
        );
        set_state(app, RecordingState::Idle);
        return Err("No speech detected (filtered)".to_string());
    }

    // Phase-1 deterministic cleanup BEFORE the LLM. Handles punctuation
    // commands, tech term normalization, acronyms, repetitions, standalone
    // fillers. Idempotent. Roughly 80% of cleanups happen here without
    // touching the LLM — faster, cheaper, safer.
    let cleanup_span = crate::trace::Span::start();
    let cleaned_text = cleanup(&raw_text);
    trace.transform(
        "cleanup",
        &raw_text,
        &cleaned_text,
        serde_json::json!({ "ms": cleanup_span.ms() }),
    );

    // Stage 2: Polish text (if enabled)
    //
    // No quota. AI Polish used to be capped at 30 calls a month for anyone
    // without a licence, which meant the app quietly got worse on the 31st
    // dictation of the month — and it was doing exactly that on the
    // maintainer's own machine while we were debugging something else. The
    // product is free and complete; nothing that makes a transcription better
    // is held back.
    let polish_quota_ok = settings.ai_polish_enabled;

    trace.stage(
        "polish.decision",
        serde_json::json!({
            "setting_enabled": settings.ai_polish_enabled,
            "quota_ok": polish_quota_ok,
        }),
    );

    // What actually happened to the polish call, as opposed to whether we
    // were allowed to attempt it. Set on every branch below.
    let mut polish_outcome = "skipped";
    // Why it went that way, and how long it took. Both used to exist only as
    // a Sentry breadcrumb — which is off by default, leaves the machine when
    // it is on, and is not in the file the user attaches to a bug report.
    let mut polish_detail = serde_json::json!({});
    let mut polish_total_ms: u64 = 0;

    let polish_span = crate::trace::Span::start();
    let final_text = if polish_quota_ok {
        emit_progress(app, "polishing", "progress.polishing", None);

        let polish_start = std::time::Instant::now();
        match polish_text(&api_key, &cleaned_text).await {
            Ok(result) => {
                // Off the critical path, and deliberately not awaited — the
                // same treatment `record_transcription` got below, for the
                // same reason, because this call was left behind.
                //
                // `record_polish_success` is `load_usage` + `save_usage`, and
                // both sides sign with the per-machine HMAC secret, which
                // comes from the OS keychain. A keychain round-trip is
                // unbounded: macOS re-evaluates the ACL whenever the binary's
                // code signature changes, and `ttp-trace.log` for 2026-08-31
                // records one such read taking **62,304 ms**, another 13,612,
                // and a third 397,600.
                //
                // Sitting here, that cost lands between the Groq response and
                // the user's text — a minute of nothing, after they finished
                // speaking, with an empty text field in front of them. The
                // polish counter is local bookkeeping for a monthly nudge; it
                // is read by nobody in this dictation and nothing downstream
                // waits on it. The user's words must never wait on a keychain.
                let polish_usage_trace = trace.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let started = std::time::Instant::now();
                    crate::usage::record_polish_success();
                    // Timed, so a keychain stall stays visible instead of
                    // merely moving somewhere the user cannot feel it. This
                    // line is also the proof the move happened: it carries a
                    // timestamp later than `paste.result` on the same
                    // dictation, which is only possible off the path.
                    polish_usage_trace.stage(
                        "usage.polish_recorded",
                        serde_json::json!({ "ms": started.elapsed().as_millis() as u64 }),
                    );
                });
                let polish_ms = polish_start.elapsed().as_millis() as u64;

                // Post-LLM guards: length ratio, content-word Jaccard,
                // refusal markers. On rejection → fall back to phase-1
                // cleaned text + log to Sentry. Silent fallback (no UI
                // popup mid-dictation, just slightly less-polished text).
                match guard_polish(&cleaned_text, &result.polished) {
                    GuardVerdict::Accept => {
                        polish_detail = serde_json::json!({ "intent": format!("{:?}", result.intent) });
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
                        polish_outcome = "applied";
                        POLISH_CONSECUTIVE_FAILURES.store(0, std::sync::atomic::Ordering::Relaxed);
                        POLISH_OUTAGE_NOTIFIED.store(false, std::sync::atomic::Ordering::Relaxed);
                        result.polished
                    }
                    GuardVerdict::Reject(reason) => {
                        // The single most opaque polish outcome: the model
                        // answered, we threw the answer away, and the user got
                        // the cleaned text with no indication anything happened.
                        polish_detail = serde_json::json!({
                            "guard_reason": reason,
                            "intent": format!("{:?}", result.intent),
                        });
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
                        polish_outcome = "guard_rejected";
                        cleaned_text.clone()
                    }
                }
            }
            Err(e) => {
                crate::logging::log_warn(&format!("[Pipeline] Polish failed, using cleaned text: {}", e));
                let category = classify_polish_error(&e);
                // `rate_limited` here is the exhausted-quota defect and
                // `invalid_api_key` is the decommissioned-model / no-access
                // one. They were distinguished in analytics and not in the
                // trace, which is backwards: the trace is the local artefact.
                polish_detail = serde_json::json!({ "error_category": category });
                if category == "polish_failed" {
                    emit_progress(app, "pasting", "error.polish_unavailable", None);
                }
                crate::telemetry::analytics::track(app, "polish_failed", Some(serde_json::json!({
                    "category": category
                })));
                polish_outcome = "failed";
                cleaned_text.clone()
            }
        }
    } else {
        cleaned_text.clone()
    };
    if polish_quota_ok {
        polish_total_ms = polish_span.ms();
    }

    let mut polish_fields = serde_json::json!({
        "outcome": polish_outcome,
        "model": crate::transcription::polish::MODEL,
        "quota_ok": polish_quota_ok,
        "ms": polish_total_ms,
    });
    if let (Some(dst), Some(src)) = (polish_fields.as_object_mut(), polish_detail.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    trace.transform("polish", &cleaned_text, &final_text, polish_fields);

    // A polish that fails on every call is invisible from the user's seat:
    // the text still arrives, just unpolished, while the setting keeps
    // claiming the feature is on. That is how a decommissioned model went
    // unnoticed for eight days. WARN so release builds keep it.
    if polish_outcome == "failed" {
        use std::sync::atomic::Ordering;
        let streak = POLISH_CONSECUTIVE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
        crate::logging::log_warn(&format!(
            "[Dictation {}] AI polish failed against model '{}' ({} in a row) — the \
             transcription was pasted unpolished. A sustained run means the model is \
             gone or the API key has no access to it.",
            trace.id(),
            crate::transcription::polish::MODEL,
            streak
        ));
        trace.stage(
            "polish.outage",
            serde_json::json!({
                "consecutive_failures": streak,
                "model": crate::transcription::polish::MODEL,
            }),
        );

        // Tell the user once per session, not once per dictation.
        if streak >= POLISH_OUTAGE_THRESHOLD
            && !POLISH_OUTAGE_NOTIFIED.swap(true, Ordering::Relaxed)
        {
            notify(app, &crate::i18n::tr("notification.polishUnavailable"));
        }
    }

    // Apply dictionary corrections as hard post-processing.
    // Even with the new polish pipeline, the personal dictionary is a hard
    // guarantee — applied after polish (and after cleanup) so user-specific
    // term spellings always win.
    let pre_dictionary_text = final_text;
    let dict_span = crate::trace::Span::start();
    let final_text = apply_dictionary(&pre_dictionary_text);
    trace.transform(
        "dictionary",
        &pre_dictionary_text,
        &final_text,
        serde_json::json!({
            "ms": dict_span.ms(),
            "entries": crate::dictionary::store::get_dictionary().len(),
        }),
    );

    // Stage 3: Paste into active app
    emit_progress(app, "pasting", "", None);

    // Create clipboard guard to save original content.
    // We always write the transcription to the clipboard first so that if AX
    // is denied — or anything in the paste path fails — the user can still
    // recover the text with a manual Cmd+V. The transcription stays in the
    // clipboard until either (a) we successfully paste/type and restore, or
    // (b) we error out (in which case we deliberately leave it).
    let clipboard_guard = ClipboardGuard::new(app);

    let clip_span = crate::trace::Span::start();
    if let Err(e) = clipboard_guard.write_text(&final_text) {
        // Transcription succeeded, polish succeeded, and the text is gone —
        // not even on the clipboard. This return wrote nothing to the trace,
        // so the dictation simply stopped mid-timeline with no finish line.
        trace.timed("clipboard.write", &clip_span, serde_json::json!({ "ok": false }));
        trace.abort("clipboard_write_failed", serde_json::json!({ "error": e.clone() }));
        emit_progress(app, "error", "error.clipboard_write_failed", None);
        notify(app, &crate::i18n::tr("notification.clipboardFailed"));
        set_state(app, RecordingState::Idle);
        return Err(e);
    }
    trace.timed("clipboard.write", &clip_span, serde_json::json!({ "ok": true }));

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
        let ax_span = crate::trace::Span::start();
        let trusted_flag = check_accessibility();
        let actually_works = probe_accessibility();
        // `AXUIElementCopyAttributeValue` against an unresponsive target app
        // blocks until its own timeout. That wait lands between the user
        // finishing their sentence and the text appearing, and it was untimed.
        trace.timed(
            "paste.accessibility",
            &ax_span,
            serde_json::json!({ "tcc_trusted": trusted_flag, "ax_probe_ok": actually_works }),
        );
        if trusted_flag && !actually_works {
            crate::logging::log_warn("[Pipeline] Accessibility TCC entry is stale - resetting so user can re-grant.");
            if let Err(e) = reset_accessibility_tcc() {
                crate::logging::log_error(&format!("[Pipeline] reset_accessibility_tcc failed: {}", e));
            }
        }
        actually_works
    };
    #[cfg(not(target_os = "macos"))]
    let has_accessibility = {
        let trusted_flag = check_accessibility();
        trace.stage(
            "paste.accessibility",
            serde_json::json!({ "tcc_trusted": trusted_flag }),
        );
        trusted_flag
    };

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

    // `app` is the bundle identifier of the frontmost application — where the
    // text is about to go. Nothing on this path ever recorded it, which is why
    // 211 unverifiable pastes across the corpus could not be attributed to a
    // single application. Bundle id only: a window title is user content.
    let target_app = crate::paste::frontmost_bundle_id();

    trace.stage(
        "paste.decision",
        serde_json::json!({
            "strategy": if use_direct_typing { "type" } else { "clipboard" },
            "chars": final_text.chars().count(),
            "has_accessibility": has_accessibility,
            "app": target_app,
        }),
    );

    // Snapshot the focused text field BEFORE injecting, so we can tell
    // afterwards whether anything actually landed. This is the only evidence
    // available: CGEventPost returns void and reports success even when the
    // window server drops every event we hand it — which is exactly what
    // happens when a stuck modifier reroutes the characters into the Globe
    // shortcut layer. Returns None on targets whose text we cannot read
    // (most Electron apps) and on non-macOS; the trace records which case.
    let focused_before = crate::paste::probe_focused_text();

    // Whether this paste is verifiable *at all* is decided here, before we
    // inject, and it is decided by whether we have a baseline to compare
    // against. That matters for honesty rather than for the verifier: it means
    // `dictation.finish` can say `pasted_unverified` for the 39% of dictations
    // whose target cannot be read, instead of claiming an observation that was
    // never going to arrive.
    let verifiable = focused_before.observable();

    // Use spawn_blocking to run sync paste code safely in async context.
    // We deliberately use tauri::async_runtime::spawn_blocking instead of the
    // raw tokio variant because it binds the worker to the same runtime Tauri
    // is using — bare tokio::task::spawn_blocking has been seen to panic with
    // "there is no reactor running" on macOS when the call site is somehow
    // detached from the active runtime (see sounds.rs / audio_monitor.rs
    // for the same fix pattern).
    // The verdict the verification task will reach, readable from here without
    // blocking. See `spawn_paste_verification`.
    let verdict_slot: PasteVerdictSlot = std::sync::Arc::new(std::sync::Mutex::new(None));

    // No `catch_unwind` around the injection.
    //
    // `[profile.release]` sets `panic = "abort"`, so there is no unwinding in
    // any shipped binary and `catch_unwind` can never return `Err`. The arm
    // that used to sit below it — logging "Paste simulation panicked" and
    // writing `paste.result {"ok":false,"kind":"panic"}` — was unreachable in
    // the product and live only under `cargo test`, where `[profile.dev]`
    // still unwinds. A test written against it would have passed while proving
    // nothing about what users run. Aborting on a panic inside CGEvent
    // injection is the right call: the process state is suspect at that point.
    //
    // What replaces the handler is a documented log shape rather than code: a
    // dictation with a `paste.decision` and no `paste.result` after it, in the
    // session before an `app.launched`, *is* a mid-injection abort. See
    // "Aborted dictations" in `docs/tracing.md`.
    let paste_span = crate::trace::Span::start();
    let paste_success = if has_accessibility {
        let paste_result = if use_direct_typing {
            let text_for_typing = final_text.clone();
            tauri::async_runtime::spawn_blocking(move || simulate_typing(&text_for_typing)).await
        } else {
            tauri::async_runtime::spawn_blocking(simulate_paste).await
        };

        match paste_result {
            Ok(Ok(())) => {
                // Direct typing walks the string one synthetic keystroke at a
                // time, so this scales with the transcription. `paste.decision`
                // records the strategy; this records what the strategy cost.
                trace.timed("paste.result", &paste_span, serde_json::json!({ "ok": true }));

                // Modifier bits still held when the events went out. This used
                // to be written by the injection itself through the standalone
                // trace writer, so it appeared as `[········]` with no
                // dictation id — invisible to `grep <trace-id>`, which is the
                // first command `docs/tracing.md` teaches. It is the single
                // most useful line next to a suspect `paste.verify`, so it now
                // belongs to the dictation.
                let held = crate::paste::last_injection_modifiers();
                if held != 0 {
                    trace.stage(
                        "paste.modifiers",
                        serde_json::json!({
                            "held": crate::paste::describe_held_modifiers(held),
                            "bits": format!("0x{:06X}", held),
                        }),
                    );
                }
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

                // Did the characters actually land? `paste.result ok=true`
                // only means the events were posted. Comparing the focused
                // field against the pre-injection snapshot is what separates
                // "typed successfully" from "silently swallowed" — the exact
                // ambiguity that made the stuck-modifier bug unfalsifiable.
                //
                // Runs off the critical path: the target needs run-loop time
                // to consume the events, and the user should not wait for our
                // bookkeeping. The task carries a clone of the trace, so its
                // line lands under the same dictation id.
                spawn_paste_verification(
                    app.clone(),
                    trace.clone(),
                    focused_before.clone(),
                    final_text.chars().count(),
                    verdict_slot.clone(),
                );

                // Restore original clipboard content
                let restore_result = clipboard_guard.restore();
                if let Err(ref e) = restore_result {
                    crate::logging::log_warn(&format!("[Pipeline] Failed to restore clipboard: {}", e));
                    // The user's pre-record clipboard is gone and their
                    // transcription is sitting in it instead. Small, silent,
                    // and exactly the kind of thing they later describe as
                    // "TTP ate my clipboard".
                    trace.degraded("clipboard.restore", serde_json::json!({ "error": e.to_string() }));
                }
                trace.stage(
                    "clipboard.restore",
                    serde_json::json!({ "ok": restore_result.is_ok() }),
                );

                // Start correction detection window (10 seconds to detect user corrections).
                // Skip when the final text is empty — the detection task would otherwise
                // poll Accessibility API for 15s for no reason (phantom F5 race, empty API).
                let armed = !final_text.trim().is_empty();
                if armed {
                    start_correction_window(app, final_text.clone());
                } else {
                    crate::logging::log_info("[Pipeline] start_correction_window skipped - empty final_text");
                }
                // Was written unconditionally, including on the path that
                // skipped arming — so the line claimed a watcher that did not
                // exist, and a missing dictionary correction had no explanation.
                trace.stage(
                    "correction_window.started",
                    serde_json::json!({ "armed": armed }),
                );

                true
            }
            Ok(Err(e)) => {
                crate::logging::log_error(&format!("[Pipeline] Paste simulation failed: {}", e));
                trace.timed(
                    "paste.result",
                    &paste_span,
                    serde_json::json!({ "ok": false, "error": e, "kind": "simulate_failed" }),
                );
                false
            }
            Err(e) => {
                crate::logging::log_error(&format!("[Pipeline] Paste task failed: {}", e));
                trace.timed(
                    "paste.result",
                    &paste_span,
                    serde_json::json!({ "ok": false, "error": e.to_string(), "kind": "join_failed" }),
                );
                false
            }
        }
    } else {
        crate::logging::log_info("[Pipeline] No accessibility permission - using clipboard fallback");
        trace.stage(
            "paste.skipped",
            serde_json::json!({ "reason": "no_accessibility" }),
        );

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

        let hist_span = crate::trace::Span::start();
        let history_result = add_history_entry(&final_text, raw_for_history);
        if let Err(ref e) = history_result {
            crate::logging::log_warn(&format!("[Pipeline] Failed to save to history: {}", e));
            trace.degraded("history.save", serde_json::json!({ "error": e.to_string() }));
        }
        trace.timed(
            "history.saved",
            &hist_span,
            serde_json::json!({ "ok": history_result.is_ok() }),
        );
    } else {
        // Not a failure — but "no history.saved line" previously meant either
        // "the setting is off" or "we never got here", and those are opposite
        // conclusions when you are chasing a lost transcription.
        trace.stage(
            "history.saved",
            serde_json::json!({ "skipped": "history_disabled" }),
        );
    }

    // What the verifier knows *right now*, without waiting for it.
    //
    // The blind case is decided before injection (`verifiable`), so it is
    // always in hand here. The observed case usually is not: measured over the
    // 548-verification corpus, `paste.verify` lands a median of 44 ms after
    // `dictation.finish`, a 90th percentile of 616 ms, and later than it in
    // 78% of dictations. Waiting for it is not an option — `set_state(Idle)`
    // is below this and the state machine stays in Processing until it runs,
    // so every millisecond spent here is a millisecond of dead hotkey.
    //
    // So the honest report is a *third* state rather than a delayed second
    // one: `pending` means the evidence is coming and carries the same trace
    // id, `ax_unreadable` means it is never coming.
    let settled = read_verdict(&verdict_slot);
    let verification = describe_verification(paste_success, verifiable, settled);

    // Complete with appropriate message.
    //
    // Deliberately *below* the verdict read. The pill has to know which of the
    // four outcomes this was, and `verification` is only in hand here; emitting
    // above would have sent `pending` on nearly everything, which is a new lie
    // pointing the other way. The frontend treats a missing `outcome` as "say
    // nothing", so this payload is the only thing that makes the honest states
    // visible.
    if paste_success {
        emit_progress(
            app,
            "complete",
            "",
            Some(serde_json::json!({
                "outcome": finish_outcome(paste_success, verification),
                "verification": verification,
            })),
        );
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

    // `pasted` is kept for readers that only ever wanted the boolean, and it
    // still means exactly what it always meant: we posted the events and
    // nothing errored. `verification` is what says whether anybody saw them
    // land.
    trace.stage(
        "ui.completed",
        serde_json::json!({ "pasted": paste_success, "verification": verification }),
    );

    let word_count = final_text.split_whitespace().count();
    let char_count = final_text.chars().count();

    // Off the critical path, and deliberately not awaited.
    //
    // This is local stats for the in-app Analytics panel — nothing downstream
    // reads it — but it was costing the user their hotkey. `record_transcription`
    // signs and verifies the usage file, and both sides of that reach the OS
    // keychain for the per-machine HMAC secret. A keychain round-trip can block
    // for seconds when macOS decides to re-evaluate the ACL, which it does after
    // any change to the app's code signature.
    //
    // Observed 2026-08-28: 7.6 seconds between `ui.completed` and
    // `usage.recorded`, with no `timer_stall` over the same window — so the main
    // run loop was turning normally and this was not App Nap. It was one
    // synchronous blocking call sitting on a tokio worker. The state machine
    // stays in Processing until this function returns, so for those 7.6 seconds
    // every hotkey press was a silent no-op: "TTP stopped responding", in
    // miniature, caused by bookkeeping.
    // Record the successful transcription in the local daily-stats bucket
    // so the in-app Analytics section can show "this week / this month".
    // Local-only — no network. Capped at u32 to keep the on-disk payload
    // bounded; even a power user shouldn't dent that ceiling per day.
    let stats_trace = trace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        crate::usage::record_transcription(
            word_count.try_into().unwrap_or(u32::MAX),
            char_count.try_into().unwrap_or(u32::MAX),
        );
        // Timed so a keychain stall stays visible instead of merely moving
        // somewhere the user cannot feel it.
        stats_trace.stage(
            "usage.recorded",
            serde_json::json!({ "ms": started.elapsed().as_millis() as u64 }),
        );
    });

    // Clean up audio files after processing
    let _ = std::fs::remove_file(&audio_path);
    if use_converted { let _ = std::fs::remove_file(&converted_path); }

    // AUDI-02: Delete backup only after successful transcription
    if let Some(ref bp) = backup_path {
        super::backup::remove_backup(bp);
    }
    trace.stage("files.cleaned", serde_json::Value::Null);

    // Re-read: the verifier may have concluded during the bookkeeping above.
    let settled = read_verdict(&verdict_slot);
    let verification = describe_verification(paste_success, verifiable, settled);
    let outcome = finish_outcome(paste_success, verification);

    let detail = serde_json::json!({
        "chars": char_count,
        "words": word_count,
        "has_accessibility": has_accessibility,
        "verification": verification,
        "app": target_app,
    });

    if outcome == "pasted_unverified" || outcome == "paste_swallowed" {
        // Emitted here rather than through `Trace::finish`, whose WARN line
        // reads "the text was never inserted into the focused app, only left
        // on the clipboard". That sentence is true for `clipboard_fallback`
        // and false for both of these states — for `pasted_unverified` the
        // text very probably did land and we just did not see it, and for
        // `paste_swallowed` the clipboard has already been restored to the
        // user's own contents, so the transcription is not sitting there
        // either. Repeating it would swap one confident wrong claim for
        // another. `Trace::finish` should learn the third state; `trace.rs` is
        // outside this workstream's blast radius.
        let mut fields = serde_json::json!({
            "outcome": outcome,
            "ms": trace.elapsed_ms(),
        });
        if let (Some(f), Some(d)) = (fields.as_object_mut(), detail.as_object()) {
            for (k, v) in d {
                f.insert(k.clone(), v.clone());
            }
        }
        trace.stage("dictation.finish", fields);

        // `pending` gets the trace line and no WARN.
        //
        // It is the honest terminal state — at the instant this line is
        // written nobody has confirmed anything — but it is also the *usual*
        // one: measured over the corpus, `paste.verify` lands a median of
        // 44 ms after `dictation.finish` and later than it in 78% of
        // dictations, so warning on it would put a WARN in `ttp.log` for most
        // successful dictations. A warning that fires on the majority of runs
        // is not a warning, it is wallpaper, and the evidence is one grep away
        // under the same trace id. The two states that do warn are the ones
        // where the answer is never coming, or has come and is bad.
        if verification != "pending" {
            crate::logging::log_warn(&format!(
                "[Dictation {}] finished as {} ({}) — the keystrokes were posted and \
                 nothing confirmed they landed in the focused app. See ttp-trace.log",
                trace.id(),
                outcome,
                verification
            ));
        }
    } else {
        trace.finish(outcome, detail);
    }

    set_state(app, RecordingState::Idle);
    Ok(final_text)
}

/// Watch the focused element until it reflects an injection, then record the
/// verdict — on `trace`, and in `slot` for the finish path to read.
///
/// Spawned rather than awaited. Verification is diagnostics, not product
/// behaviour: making the user wait up to `PASTE_VERIFY_TIMEOUT_MS` for the
/// completion pill so we can write a log line would be a bad trade, and the
/// state machine stays in `Processing` for exactly as long as this function's
/// caller runs. Nothing here is on the dictation's critical path and nothing
/// here may ever become so.
///
/// Bails without a single Accessibility round-trip when the *baseline* was
/// unreadable. That is not an optimisation, it is the honest answer: with no
/// `before` there is nothing a later read could be compared against, so
/// polling for 600 ms would buy an `after` that proves nothing. This is the
/// whole of the corpus blind spot — all 219 blind verifications across 548
/// have a null baseline, and none of them is a lost read-back — and the line
/// it emits says `verdict:"unverified" reason:"no_baseline"` so no reader ever
/// mistakes "we could not check" for "it did not land".
fn spawn_paste_verification(
    app: AppHandle,
    trace: crate::trace::Trace,
    focused_before: FocusSnapshot,
    expected_chars: usize,
    slot: crate::paste::PasteVerdictSlot,
) {
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let timeout = Duration::from_millis(PASTE_VERIFY_TIMEOUT_MS);

        let delta_of = |after: &FocusSnapshot| -> Option<i64> {
            match (focused_before.chars, after.chars) {
                (Some(b), Some(a)) => Some(a as i64 - b as i64),
                _ => None,
            }
        };

        // `None` means we never looked, which is a different fact from "we
        // looked and saw nothing" and is rendered as a null `ax_after`.
        let mut after: Option<FocusSnapshot> = None;
        let mut first_change_ms: Option<u64> = None;
        let mut settled_ms: u64 = 0;
        let mut reads: u32 = 0;
        let mut retries: u32 = 0;

        if focused_before.observable() {
            let mut snapshot = crate::paste::probe_focused_text();
            reads += 1;

            // Poll until the target has consumed everything we sent, not until
            // it first reacts. We inject in chunks, so the first read after the
            // first chunk lands shows a delta of exactly one chunk — stopping
            // there reported "16 characters arrived" for a 500-character paste,
            // which reads as a truncation bug that is not happening. Keep
            // watching until the delta covers what we sent, or the window
            // closes.
            while started.elapsed() < timeout {
                if snapshot.observable()
                    && delta_of(&snapshot).is_some_and(|d| d >= expected_chars as i64)
                {
                    break;
                }

                // The second means of verification, and the reason this loop no
                // longer gives up on the first blind read the way the previous
                // implementation did (`if focused_after.is_some()`, once, and
                // out). A target whose focus is momentarily unresolvable right
                // after injection — the window server is mid-transition, the
                // app is rebuilding its AX tree — used to turn a perfectly
                // readable baseline into `ax_readable:false`. Retry at a
                // coarser cadence, because each attempt is an AX round-trip
                // against an app that has just declined to answer.
                let gap = if snapshot.observable() {
                    PASTE_VERIFY_POLL_MS
                } else {
                    retries += 1;
                    PASTE_VERIFY_RETRY_MS
                };
                sleep(Duration::from_millis(gap)).await;

                let next = crate::paste::probe_focused_text();
                reads += 1;
                if next != snapshot {
                    settled_ms = started.elapsed().as_millis() as u64;
                    first_change_ms.get_or_insert(settled_ms);
                    snapshot = next;
                }
            }
            after = Some(snapshot);
        }

        // With no baseline the second argument is irrelevant — `classify`
        // returns `no_baseline` before it looks at it — so passing the
        // baseline itself is honest rather than clever: it says "there was
        // never a pair here".
        let verification = crate::paste::classify(
            &focused_before,
            after.as_ref().unwrap_or(&focused_before),
        );
        crate::paste::record_verdict(&slot, verification);

        // The verdict lands a median 44 ms after `dictation.finish`, so the
        // completion frame above almost always said `pending`. This is the
        // event that upgrades it. Without it the pill would have to either
        // wait (dead hotkey) or call every paste unverified (the same
        // concealment, inverted).
        app.emit(
            "paste-verified",
            serde_json::json!({
                "outcome": crate::paste::finish_outcome(true, verification.verdict.as_str()),
                "verification": verification.verdict.as_str(),
            }),
        )
        .ok();

        trace.stage(
            "paste.verify",
            serde_json::json!({
                // The verdict, and the kind of evidence it rests on. There is
                // deliberately no `changed` boolean any more: it was computed
                // by comparing `Option<String>`s, so an unreadable before plus
                // a readable after rendered as `changed:true` — a self-report
                // wearing an observation's clothes.
                "verdict": verification.verdict.as_str(),
                "evidence": verification.evidence,
                "reason": verification.reason,
                // Which read strategy answered, on each side. One boolean used
                // to collapse four different failures — no focused element, an
                // app that does not serve AX at all, an element with no
                // readable attribute, and not-macOS — into `ax_readable:false`,
                // and they need four different fixes.
                "ax_before": focused_before.source.as_str(),
                "ax_before_err": focused_before.ax_err,
                "ax_after": after.as_ref().map(|a| a.source.as_str()),
                "ax_after_err": after.as_ref().map(|a| a.ax_err),
                // Kept: same meaning it always had, so a year of corpus stays
                // comparable.
                "ax_readable": after.as_ref().is_some_and(|a| a.observable()),
                "before_chars": focused_before.chars,
                "after_chars": after.as_ref().and_then(|a| a.chars),
                "delta_chars": after.as_ref().and_then(delta_of),
                "expected_chars": expected_chars,
                // How fast the target reacted at all, vs when it stopped
                // changing. A large gap between them means a slow consumer;
                // first_change absent means it never reacted.
                "first_change_ms": first_change_ms,
                "settled_ms": settled_ms,
                // What the observation cost, and how much of it was spent on a
                // target that was not answering. `reads:1` with a decided
                // verdict is the cheap happy path; a high `retries` names the
                // apps worth a third read strategy.
                "reads": reads,
                "retries": retries,
            }),
        );
    });
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
    // Every one of these paths throws away audio the user has already
    // spoken, before `process_recording` opens a trace — so without this the
    // dictation simply vanishes between `capture.stop` and nothing at all.
    let err_idle = |app: &AppHandle, key: &str| -> String {
        crate::trace::event(
            "dictation.rejected",
            serde_json::json!({ "reason": key }),
        );
        crate::logging::log_warn(&format!(
            "[Pipeline] Recording rejected before transcription: {}",
            key
        ));
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

