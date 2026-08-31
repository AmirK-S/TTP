// TTP - Reading the dictation trace back
//
// `trace.rs` writes. This module reads, and defines the contract a viewer UI
// consumes. The full contract — command names, argument and return shapes,
// event channel, JSON examples — is written down in `docs/trace-api.md`, and
// that file is the thing to change first if anything here changes.
//
// ── Why parse the log instead of keeping a second copy ───────────────────
//
// The obvious design is a ring buffer of structured records in memory that
// the commands read from. It is also the wrong one here:
//
//   * The file already survives restarts. An in-memory buffer answers "what
//     happened since launch", and the dictation people ask about is usually
//     from before the restart that made them notice.
//   * Two stores drift. If the viewer showed a record the file did not, the
//     trace would have stopped being the single artefact a user can attach
//     to a bug report — which is most of its value.
//   * The retained window is already bounded and paid for (see
//     `logging::trace_rotation`). A parallel buffer is a second budget for
//     the same evidence.
//
// So: the on-disk line format is the wire format, `parse_line` is its exact
// inverse, and a round-trip test holds the two definitions together. Reading
// costs a few hundred milliseconds across ~10 MB, which is fine for a UI
// action and is never on the dictation path.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::trace::{TraceEvent, STANDALONE_ID};

/// Default number of dictations `trace_recent_dictations` returns.
const DEFAULT_LIMIT: usize = 50;

/// Hard ceiling on any one query, so a viewer bug cannot ask for the whole
/// retained window and serialise 10 MB through the IPC bridge.
const MAX_LIMIT: usize = 500;

/// One dictation, reassembled from its stage lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DictationTrace {
    /// The greppable trace id, e.g. `0007-3f2a`.
    pub id: String,
    /// Timestamp of this dictation's first line.
    pub started_at: String,
    /// `pasted`, `clipboard_fallback`, `aborted`, or `None` when the
    /// dictation has no `dictation.finish` line — either still in flight, or
    /// the process died mid-dictation, which is itself the finding.
    pub outcome: Option<String>,
    /// The abort slug (`silent_audio`, `hallucination`, …) when
    /// `outcome == "aborted"`.
    pub reason: Option<String>,
    /// Total wall-clock milliseconds, from `dictation.finish`.
    pub total_ms: Option<u64>,
    /// Characters and words finally inserted, when the dictation produced text.
    pub chars: Option<u64>,
    pub words: Option<u64>,
    /// The stage with the largest `dur_ms`, so a viewer can show "where the
    /// time went" without walking the timeline itself. This is the question
    /// every latency investigation opens with.
    pub slowest_stage: Option<String>,
    pub slowest_stage_ms: Option<u64>,
    /// Every line carrying this id, in the order it was written.
    pub stages: Vec<TraceEvent>,
}

/// What the trace currently costs and how far back it reaches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStatus {
    /// Whether full transcription text is being written (`diagnostics_enabled`
    /// or `TTP_DIAGNOSTICS`). A viewer should say so plainly: it is the
    /// difference between a diagnostic file and a transcript of everything
    /// the user has said.
    pub verbose: bool,
    /// Whether the live channel is currently streaming.
    pub live: bool,
    /// Tauri event name carrying live records.
    pub channel: String,
    /// Trace files on disk, newest first.
    pub files: Vec<TraceFile>,
    pub total_bytes: u64,
    /// Rotation policy, so the viewer can show the real retained window.
    pub rotate_at_bytes: u64,
    pub keep_rotations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFile {
    pub name: String,
    pub bytes: u64,
}

// ── Parsing ─────────────────────────────────────────────────────────────

/// Recover a [`TraceEvent`] from one on-disk line.
///
/// The exact inverse of `trace::format_line`; `round_trips_through_the_line_format`
/// in the tests below is what keeps them honest. Returns `None` for anything
/// that is not a trace line at all (a blank line, a torn write at the end of
/// a rotated file) — the caller skips those rather than failing the query,
/// because one corrupt line must not cost you the other nine thousand.
pub fn parse_line(line: &str) -> Option<TraceEvent> {
    let rest = line.strip_prefix('[')?;
    let (ts, rest) = rest.split_once("] [")?;
    let (id_raw, rest) = rest.split_once(']')?;

    let rest = rest.trim_start();
    let (elapsed_ms, rest) = match rest.strip_prefix('+') {
        Some(after_plus) => {
            let (num, tail) = after_plus.split_once("ms")?;
            (Some(num.trim().parse::<u64>().ok()?), tail)
        }
        None => (None, rest),
    };

    let rest = rest.trim_start();
    let (stage, raw_fields) = match rest.split_once(' ') {
        Some((s, f)) => (s, f.trim()),
        None => (rest, "{}"),
    };
    if stage.is_empty() {
        return None;
    }

    // A payload we cannot parse is preserved rather than dropped. The line
    // exists, something wrote it, and "there was a line here I could not
    // read" is strictly more informative than silence — which is the whole
    // argument this subsystem is built on.
    let fields = serde_json::from_str::<Value>(raw_fields)
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({ "unparsed": raw_fields }));

    let id = if id_raw == STANDALONE_ID || id_raw.trim().is_empty() {
        None
    } else {
        Some(id_raw.to_string())
    };

    Some(TraceEvent {
        ts: ts.to_string(),
        id,
        elapsed_ms,
        stage: stage.to_string(),
        fields,
    })
}

/// Read trace files newest-first, stopping as soon as `want_dictations`
/// distinct dictation ids have been seen, and return every parsed record in
/// chronological order.
///
/// Reading whole files and discarding most of them is deliberate: the files
/// are small (2.5 MB), and seeking backwards through a line-oriented text
/// file for an unknown number of records is the kind of cleverness that ends
/// up with an off-by-one nobody notices for a month.
fn read_events(want_dictations: usize) -> Vec<TraceEvent> {
    let mut chunks: Vec<Vec<TraceEvent>> = Vec::new();
    let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path in crate::logging::trace_files_newest_first() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let parsed: Vec<TraceEvent> = text.lines().filter_map(parse_line).collect();
        for ev in &parsed {
            if let Some(id) = &ev.id {
                ids.insert(id.clone());
            }
        }
        chunks.push(parsed);
        if ids.len() >= want_dictations {
            break;
        }
    }

    // Files came newest-first; flatten oldest-first so the result reads
    // forwards in time.
    chunks.reverse();
    chunks.into_iter().flatten().collect()
}

/// Group a flat, chronological record stream into dictations.
///
/// Standalone events (`hotkey.*`, `state.transition`, `app.launched`) have no
/// id and are not part of any dictation, so they are skipped here. They are
/// still reachable through [`recent_events`], which is how a viewer shows the
/// input timeline alongside the dictations.
pub fn group_dictations(events: Vec<TraceEvent>) -> Vec<DictationTrace> {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, Vec<TraceEvent>> = HashMap::new();

    for ev in events {
        let Some(id) = ev.id.clone() else { continue };
        by_id.entry(id.clone()).or_insert_with(|| {
            order.push(id.clone());
            Vec::new()
        });
        if let Some(slot) = by_id.get_mut(&id) {
            slot.push(ev);
        }
    }

    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id).map(|stages| assemble(id, stages)))
        .collect()
}

fn u64_field(fields: &Value, key: &str) -> Option<u64> {
    fields.get(key).and_then(|v| v.as_u64())
}

fn assemble(id: String, stages: Vec<TraceEvent>) -> DictationTrace {
    let started_at = stages
        .first()
        .map(|e| e.ts.clone())
        .unwrap_or_else(String::new);

    let finish = stages.iter().find(|e| e.stage == "dictation.finish");
    let outcome = finish
        .and_then(|e| e.fields.get("outcome"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let reason = finish
        .and_then(|e| e.fields.get("reason"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let total_ms = finish.and_then(|e| u64_field(&e.fields, "ms"));
    let chars = finish.and_then(|e| u64_field(&e.fields, "chars"));
    let words = finish.and_then(|e| u64_field(&e.fields, "words"));

    // "Which stage ate the time" — computed here rather than in the viewer so
    // every consumer agrees on the answer. `dictation.start` is excluded: its
    // dur_ms is measured from a zero cursor and is always 0, and including it
    // would make a zero-length dictation report itself as its own bottleneck.
    let slowest = stages
        .iter()
        .filter(|e| e.stage != "dictation.start")
        .filter_map(|e| u64_field(&e.fields, "dur_ms").map(|ms| (ms, e.stage.clone())))
        .max_by_key(|(ms, _)| *ms);

    DictationTrace {
        id,
        started_at,
        outcome,
        reason,
        total_ms,
        chars,
        words,
        slowest_stage: slowest.as_ref().map(|(_, s)| s.clone()),
        slowest_stage_ms: slowest.as_ref().map(|(ms, _)| *ms),
        stages,
    }
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

// ── Commands ────────────────────────────────────────────────────────────

/// Most recent dictations, newest first, each with its full stage timeline.
#[tauri::command]
pub fn trace_recent_dictations(limit: Option<usize>) -> Vec<DictationTrace> {
    let limit = clamp_limit(limit);
    let mut all = group_dictations(read_events(limit));
    if all.len() > limit {
        all.drain(..all.len() - limit);
    }
    all.reverse();
    all
}

/// One dictation by id, with every line that carries it — including the
/// `paste.verify` line, which is written after `dictation.finish` from a
/// spawned task and is the one people most often miss when reading by eye.
#[tauri::command]
pub fn trace_get_dictation(id: String) -> Option<DictationTrace> {
    // Scan the whole retained window: the id someone pastes in is by
    // definition one they had to go looking for.
    let events = read_events(usize::MAX);
    let stages: Vec<TraceEvent> = events
        .into_iter()
        .filter(|e| e.id.as_deref() == Some(id.as_str()))
        .collect();
    if stages.is_empty() {
        return None;
    }
    Some(assemble(id, stages))
}

/// The raw record stream, newest first, optionally filtered by a stage-name
/// prefix.
///
/// This is how a viewer renders the standalone timeline — `hotkey.`,
/// `state.transition`, `capture.`, `companion.` — that belongs to no single
/// dictation but explains most of what happened between them.
#[tauri::command]
pub fn trace_recent_events(limit: Option<usize>, stage_prefix: Option<String>) -> Vec<TraceEvent> {
    let limit = clamp_limit(limit);
    // A prefix query is a hunt for something rare — `companion.`,
    // `hotkey.tap_`, `degraded` — and the newest file may hold none of it, so
    // scan the whole retained window rather than reporting an empty result
    // that reads as "it never happened".
    let mut events = read_events(if stage_prefix.is_some() { usize::MAX } else { DEFAULT_LIMIT });
    if let Some(prefix) = stage_prefix.as_deref() {
        events.retain(|e| e.stage.starts_with(prefix));
    }
    if events.len() > limit {
        events.drain(..events.len() - limit);
    }
    events.reverse();
    events
}

/// Turn the live channel on or off.
///
/// Off by default and off again on unmount: a viewer nobody is looking at
/// should not cost the writer thread an IPC emit per line.
#[tauri::command]
pub fn trace_set_live(enabled: bool) {
    crate::trace::set_live(enabled);
}

/// Retention, verbosity and channel name, for a viewer's header.
#[tauri::command]
pub fn trace_status() -> TraceStatus {
    let (rotate_at_bytes, keep_rotations) = crate::logging::trace_rotation();
    let files: Vec<TraceFile> = crate::logging::trace_files_newest_first()
        .into_iter()
        .map(|p| TraceFile {
            name: p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ttp-trace.log")
                .to_string(),
            bytes: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
        })
        .collect();
    TraceStatus {
        verbose: crate::trace::verbose_enabled(),
        live: crate::trace::live_enabled(),
        channel: crate::trace::LIVE_CHANNEL.to_string(),
        total_bytes: files.iter().map(|f| f.bytes).sum(),
        files,
        rotate_at_bytes,
        keep_rotations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::format_line;

    fn ev(id: Option<&str>, elapsed: Option<u64>, stage: &str, fields: Value) -> TraceEvent {
        TraceEvent {
            ts: "2026-08-31 09:12:03.101".into(),
            id: id.map(str::to_string),
            elapsed_ms: elapsed,
            stage: stage.into(),
            fields,
        }
    }

    #[test]
    fn round_trips_through_the_line_format() {
        // The load-bearing test of this module. The file on disk is the only
        // store; if the writer and the parser ever disagree the viewer shows
        // something the log does not say.
        for original in [
            ev(Some("0007-3f2a"), Some(1709), "dictation.finish",
               json!({ "outcome": "pasted", "ms": 1709, "chars": 87, "dur_ms": 12 })),
            ev(None, None, "hotkey.press", json!({ "held_ms": 152 })),
            ev(Some("0000-0000"), Some(0), "dictation.start",
               json!({ "kind": "recording", "verbose": false, "dur_ms": 0 })),
            ev(None, None, "app.launched", json!({})),
        ] {
            let parsed = parse_line(&format_line(&original)).expect("parses");
            assert_eq!(parsed, original, "round trip changed the record");
        }
    }

    #[test]
    fn parses_a_line_from_the_published_documentation() {
        // Copied out of docs/tracing.md. If this stops parsing, either the
        // format moved or the documentation is now wrong; both are bugs.
        let line = "[2026-08-26 08:48:57.412] [0007-3f2a] +    0ms dictation.start {\"kind\":\"recording\",\"verbose\":false}";
        let ev = parse_line(line).expect("parses");
        assert_eq!(ev.id.as_deref(), Some("0007-3f2a"));
        assert_eq!(ev.elapsed_ms, Some(0));
        assert_eq!(ev.stage, "dictation.start");
        assert_eq!(ev.fields["kind"], "recording");
    }

    #[test]
    fn standalone_lines_have_no_dictation_id() {
        let line = format_line(&ev(None, None, "state.transition", json!({ "to": "Idle" })));
        let parsed = parse_line(&line).expect("parses");
        assert_eq!(parsed.id, None);
        assert_eq!(parsed.elapsed_ms, None);
    }

    #[test]
    fn junk_lines_are_skipped_not_fatal() {
        assert!(parse_line("").is_none());
        assert!(parse_line("not a trace line at all").is_none());
        assert!(parse_line("[half a line").is_none());
        // A torn write at the tail of a rotated file: truncated mid-payload.
        let torn = "[2026-08-26 08:48:57.412] [0007-3f2a] +    0ms whisper.resp";
        assert!(parse_line(torn).is_some(), "a truncated payload still names its stage");
    }

    #[test]
    fn an_unreadable_payload_is_preserved_rather_than_dropped() {
        let line = "[2026-08-26 08:48:57.412] [0007-3f2a] +    5ms polish {not json";
        let ev = parse_line(line).expect("parses");
        assert_eq!(ev.stage, "polish");
        assert_eq!(ev.fields["unparsed"], "{not json");
    }

    #[test]
    fn grouping_keeps_stage_order_and_reads_the_outcome() {
        let events = vec![
            ev(Some("0001-aaaa"), Some(0), "dictation.start", json!({ "dur_ms": 0 })),
            ev(None, None, "hotkey.press", json!({})),
            ev(Some("0001-aaaa"), Some(900), "whisper.response", json!({ "dur_ms": 900 })),
            ev(Some("0002-bbbb"), Some(0), "dictation.start", json!({ "dur_ms": 0 })),
            ev(Some("0001-aaaa"), Some(1000), "dictation.finish",
               json!({ "outcome": "aborted", "reason": "hallucination", "ms": 1000, "dur_ms": 100 })),
        ];
        let grouped = group_dictations(events);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].id, "0001-aaaa");
        assert_eq!(grouped[0].stages.len(), 3);
        assert_eq!(grouped[0].outcome.as_deref(), Some("aborted"));
        assert_eq!(grouped[0].reason.as_deref(), Some("hallucination"));
        assert_eq!(grouped[0].total_ms, Some(1000));
        // Order of first appearance, not lexical order of ids.
        assert_eq!(grouped[1].id, "0002-bbbb");
        // No finish line yet: an in-flight (or abandoned) dictation reports
        // no outcome rather than a made-up one.
        assert_eq!(grouped[1].outcome, None);
    }

    #[test]
    fn slowest_stage_ignores_the_opening_line() {
        let events = vec![
            ev(Some("0001-aaaa"), Some(0), "dictation.start", json!({ "dur_ms": 0 })),
            ev(Some("0001-aaaa"), Some(40), "audio.signal", json!({ "dur_ms": 40 })),
            ev(Some("0001-aaaa"), Some(7700), "keychain.api_key", json!({ "dur_ms": 7660 })),
            ev(Some("0001-aaaa"), Some(8000), "dictation.finish",
               json!({ "outcome": "pasted", "ms": 8000, "dur_ms": 300 })),
        ];
        let grouped = group_dictations(events);
        // This is the shape of the real keychain defect: everything is fast
        // except one stage nobody was timing.
        assert_eq!(grouped[0].slowest_stage.as_deref(), Some("keychain.api_key"));
        assert_eq!(grouped[0].slowest_stage_ms, Some(7660));
    }

    #[test]
    fn limit_is_clamped_to_a_sane_range() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(10_000)), MAX_LIMIT);
        assert_eq!(clamp_limit(Some(7)), 7);
    }
}
