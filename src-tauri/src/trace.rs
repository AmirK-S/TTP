// TTP - Dictation tracing
//
// `ttp.log` answers "did something throw?". It cannot answer the question
// that actually costs users their trust: "I pressed, I spoke, and nothing was
// written — where did my words go?". That failure is silent by construction.
// Every stage of the pipeline has a legitimate reason to produce no text:
//
//   * the RMS gate decides the recording was silent,
//   * Whisper returns 200 with an empty body,
//   * the hallucination filter matches and drops the whole transcription,
//   * the glossary-ghost filter drops a short dictionary-only result,
//   * polish returns something the guard rejects,
//   * Accessibility is trusted-but-broken and CGEventPost goes nowhere,
//   * a stuck modifier turns every injected character into a system shortcut.
//
// All seven end the same way from the user's seat — an empty target app — and
// in a release build (log level Warn) all seven are invisible. This module
// makes each one leave a line.
//
// One `Trace` per dictation, written to its own `ttp-trace.log` so it is
// neither filtered by the main log's level nor rotated away by unrelated
// chatter. Each line carries the trace id, the elapsed time since the
// dictation began, a dotted stage name, and a compact JSON object:
//
//   [2026-08-26 08:48:57.412] [0007-3f2a] +    0ms dictation.start {"kind":"recording"}
//   [2026-08-26 08:48:57.418] [0007-3f2a] +    6ms audio.metrics {"secs":7.52,"avg_rms":0.0413}
//   [2026-08-26 08:48:58.902] [0007-3f2a] + 1490ms whisper.response {"chars":87,"sha8":"9f2c1ab0"}
//   [2026-08-26 08:48:58.903] [0007-3f2a] + 1491ms paste.modifiers {"held":"Fn/Globe"}
//   [2026-08-26 08:48:59.121] [0007-3f2a] + 1709ms dictation.finish {"outcome":"pasted","ms":1709}
//
// Text payloads are redacted by default: every stage that carries text logs
// its length and an 8-hex-digit digest, which is enough to tell "the filter
// dropped it" from "the filter rewrote it" without writing anyone's
// transcriptions to disk. Turning on the `diagnostics_enabled` setting (or
// exporting `TTP_DIAGNOSTICS=1`) adds the full text to those same lines.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Monotonic per-process counter, so trace ids sort in the order the
/// dictations happened.
static TRACE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Short, greppable identifier for one dictation: a sequence number (ordering
/// at a glance) plus 4 hex digits of the creation timestamp (so ids stay
/// distinct across restarts, when the sequence resets to zero).
fn new_id() -> String {
    let seq = TRACE_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    format!("{:04}-{:04x}", seq % 10_000, nanos & 0xFFFF)
}

/// First 8 hex digits of the SHA-256 of `text`.
///
/// Lets a redacted trace still answer "is this the same string the previous
/// stage produced?" — which is the whole question when hunting for the stage
/// that ate a transcription.
fn digest8(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let out = hasher.finalize();
    format!("{:02x}{:02x}{:02x}{:02x}", out[0], out[1], out[2], out[3])
}

/// Whether full text payloads should be written to the trace.
///
/// `TTP_DIAGNOSTICS` is checked first so the setting can be overridden for a
/// single launch from a terminal without touching persisted preferences.
fn verbose_enabled() -> bool {
    if let Ok(raw) = std::env::var("TTP_DIAGNOSTICS") {
        return matches!(raw.trim(), "1" | "true" | "yes" | "on");
    }
    crate::settings::get_settings().diagnostics_enabled
}

/// Merge the fields of `extra` into `target`. Silently ignores a non-object
/// `extra` (including `Value::Null`), which is the "no extra fields" case.
fn merge(target: &mut Value, extra: Value) {
    if let (Some(map), Value::Object(source)) = (target.as_object_mut(), extra) {
        for (k, v) in source {
            map.insert(k, v);
        }
    }
}

/// Record a one-off event that isn't part of a dictation: hotkey transitions,
/// event-tap health, permission changes.
///
/// Written to the same file so a dictation's stages and the key press that
/// started them read in one chronological stream. The id column is `····`
/// rather than a trace id, which keeps `grep <id>` clean while still letting
/// `grep hotkey\.` pull the whole input timeline.
pub fn event(name: &str, fields: Value) {
    let rendered = match &fields {
        Value::Null => "{}".to_string(),
        other => other.to_string(),
    };
    crate::logging::log_trace_line(&format!(
        "[{}] [{}] {:>8} {} {}",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        "········",
        "",
        name,
        rendered
    ));
}

/// A single dictation's trace. Cheap to create; every method is fire-and-
/// forget and never fails — tracing must not be able to break a dictation.
pub struct Trace {
    id: String,
    start: Instant,
    verbose: bool,
}

impl Trace {
    /// Open a trace and emit its first line.
    pub fn start(kind: &str) -> Self {
        let trace = Self {
            id: new_id(),
            start: Instant::now(),
            verbose: verbose_enabled(),
        };
        trace.stage(
            "dictation.start",
            json!({ "kind": kind, "verbose": trace.verbose }),
        );
        trace
    }

    #[allow(dead_code)]
    pub fn verbose(&self) -> bool {
        self.verbose
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Record a stage with a JSON object of fields.
    pub fn stage(&self, name: &str, fields: Value) {
        let rendered = match &fields {
            Value::Null => "{}".to_string(),
            other => other.to_string(),
        };
        crate::logging::log_trace_line(&format!(
            "[{}] [{}] +{:>5}ms {} {}",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            self.id,
            self.elapsed_ms(),
            name,
            rendered
        ));
    }

    /// Record a stage that carries user text.
    ///
    /// Always logs the character count and digest; adds the text itself only
    /// when diagnostics are enabled. `extra` is merged into the same object so
    /// a call site can attach context (which filter matched, which model
    /// answered) without emitting a second line.
    pub fn text_stage(&self, name: &str, text: &str, extra: Value) {
        let mut fields = json!({
            "chars": text.chars().count(),
            "sha8": digest8(text),
        });
        if self.verbose {
            fields["text"] = Value::String(text.to_string());
        }
        merge(&mut fields, extra);
        self.stage(name, fields);
    }

    /// Record the transformation of one text into another.
    ///
    /// `changed` is the field worth grepping for: it separates "this stage was
    /// a no-op" from "this stage rewrote the user's words", and a `to.chars`
    /// of 0 pinpoints the stage that emptied the transcription.
    pub fn transform(&self, name: &str, before: &str, after: &str, extra: Value) {
        let mut fields = json!({
            "changed": before != after,
            "from": { "chars": before.chars().count(), "sha8": digest8(before) },
            "to": { "chars": after.chars().count(), "sha8": digest8(after) },
        });
        if self.verbose && before != after {
            fields["from"]["text"] = Value::String(before.to_string());
            fields["to"]["text"] = Value::String(after.to_string());
        }
        merge(&mut fields, extra);
        self.stage(name, fields);
    }

    /// Terminal line for a dictation that produced no text.
    ///
    /// `reason` is a stable slug (`silent_audio`, `hallucination`,
    /// `glossary_ghost`, `no_speech`, …) so the whole class of "I dictated and
    /// nothing came out" reports collapses to one grep.
    pub fn abort(&self, reason: &str, detail: Value) {
        let mut fields =
            json!({ "outcome": "aborted", "reason": reason, "ms": self.elapsed_ms() });
        merge(&mut fields, detail);
        self.stage("dictation.finish", fields);

        // Mirror into the main log. `ttp.log` is what a user attaches to a bug
        // report and what they read when something feels broken, and until now
        // a dropped dictation left nothing there at all. WARN so it survives
        // the release-default level filter, and it names the trace id so the
        // detailed stages are one grep away.
        crate::logging::log_warn(&format!(
            "[Dictation {}] produced no text: {} — see ttp-trace.log",
            self.id, reason
        ));
    }

    /// Terminal line for a dictation that ran to completion.
    pub fn finish(&self, outcome: &str, detail: Value) {
        let mut fields = json!({ "outcome": outcome, "ms": self.elapsed_ms() });
        merge(&mut fields, detail);
        self.stage("dictation.finish", fields);

        if outcome != "pasted" {
            crate::logging::log_warn(&format!(
                "[Dictation {}] finished as {} — the text was never inserted into the \
                 focused app, only left on the clipboard. See ttp-trace.log",
                self.id, outcome
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_ordered_within_a_process() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        // Sequence prefix is zero-padded so lexical order matches creation
        // order for the first 10 000 dictations of a session.
        assert!(a[..4] < b[..4]);
    }

    #[test]
    fn id_is_greppable_ascii() {
        let id = new_id();
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        assert_eq!(id.len(), 9);
    }

    #[test]
    fn digest_is_stable_and_discriminating() {
        assert_eq!(digest8("bonjour"), digest8("bonjour"));
        assert_ne!(digest8("bonjour"), digest8("bonjours"));
        assert_eq!(digest8("bonjour").len(), 8);
    }

    #[test]
    fn empty_text_still_hashes() {
        // The stage that empties a transcription must still produce a line.
        assert_eq!(digest8("").len(), 8);
    }

    #[test]
    fn merge_adds_fields_and_ignores_non_objects() {
        let mut v = json!({ "a": 1 });
        merge(&mut v, json!({ "b": 2 }));
        assert_eq!(v, json!({ "a": 1, "b": 2 }));
        merge(&mut v, Value::Null);
        assert_eq!(v, json!({ "a": 1, "b": 2 }));
    }
}
