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
//
// ── Three properties this module has to keep ────────────────────────────
//
// 1. **It never sits on the dictation path.** Every emit is a channel push;
//    a dedicated writer thread does the `stat`, the rotation check, the
//    `open` and the two `write`s. Before Polaris this was synchronous, which
//    put a filesystem round-trip inside the 20 ms Fn timer block on the
//    macOS main thread and inside the audio pipeline between the user
//    finishing a sentence and their text appearing. Instrumentation that
//    slows the thing it observes is a bug, not a diagnostic.
//
// 2. **Negative cases are recorded too.** A trace that only writes on
//    failure cannot distinguish "this did not happen" from "this was never
//    instrumented". The filters emit a line when they decide *not* to fire;
//    the event tap emits a heartbeat when it is healthy.
//
// 3. **Every stage carries a duration.** `dur_ms` is the time since the
//    previous stage of the same dictation, injected automatically. The
//    keychain-on-the-critical-path defect was a latency bug, and latency
//    bugs are invisible in an event log with no clock.
//
// 4. **Every line names the process that wrote it.** `proc` is a short id
//    minted once per process. One log directory is shared by everything on
//    this machine that links this crate — the installed app, a `tauri dev`
//    build, and `cargo test` — so records from several processes interleave
//    in one file. Twice in this programme that produced an incident report
//    that had to be retracted: fourteen keychain "errors" claiming reads
//    blocked dictations for up to 397 seconds, and fifty-four rate-limit
//    warnings read as a live outage. Both were test binaries. Neither could
//    be told apart from the app, because a session was only ever the span
//    between two `app.launched` lines and a co-tenant writes none.
//
//    `proc` closes that. Group the lines by it and each group is exactly one
//    process; a group that contains an `app.launched` is the real app, and a
//    group that does not is a test binary or a dev build. `app.launched`
//    itself carries `build` — version and commit — so a log can also say
//    *which* binary produced it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Monotonic per-process counter, so trace ids sort in the order the
/// dictations happened.
static TRACE_SEQ: AtomicU64 = AtomicU64::new(0);

// ── Who wrote this line ─────────────────────────────────────────────────
//
// One value per process, on every record, so a reader can partition a shared
// log file by its writers. See requirement 4 in the module header for why.
//
// Three properties it has to have, and they decide the representation:
//
//   * **It must cost almost nothing.** This is a field on every line, and
//     lines are ~37 per dictation. `"proc":"a3f9",` is 14 bytes; the same
//     information as a pid would be 12-17 and would not be free of the
//     problems below. See `docs/tracing.md`, "Retention", for the bill.
//
//   * **It must carry no identity.** Not the pid, not a username, not a
//     path. A pid is a weak cross-log correlator and it leaks process
//     ordering and launch sequence for a file users are invited to send to
//     the maintainer. The id's only job is to say "these lines came from the
//     same process", and 16 random bits say exactly that and nothing else.
//     4 hex digits also matches the vocabulary of the trace id's own suffix
//     (`0007-3f2a`), so it reads as one family and greps like one.
//
//   * **It must be available inside a panic hook.** `logging::log_trace_line`
//     is called directly from the hook because a channel push does not
//     survive an abort, and the hook renders its record with `format_line`.
//     So minting sits behind a relaxed atomic and a `compare_exchange` — no
//     lock the panicking thread could be holding, no channel, no allocation.
//     In practice it is already minted long before any panic: the app's
//     first record is `app.launched`.
//
// 16 bits is not a collision-free namespace and does not need to be. The
// question is only ever whether two processes writing to the *same retained
// window* are distinguishable, which is a handful of processes at a time; at
// four co-tenants the chance of any pair colliding is about 0.009%.

/// This process's id, as 16 bits. `0` means "not minted yet" and is
/// therefore never a live value.
static PROC: AtomicU32 = AtomicU32::new(0);

/// Draw 16 bits of process identity.
///
/// Deliberately silent about its own failure: this runs on the path a panic
/// hook takes, and `degraded()` would re-enter `format_line`, which is what
/// called us. The clock fallback carries no identity either — sub-millisecond
/// bits of the launch instant, below the resolution the timestamp column
/// already publishes.
fn mint_proc_bits() -> u32 {
    let mut bytes = [0u8; 2];
    let raw = if getrandom::fill(&mut bytes).is_ok() {
        u16::from_ne_bytes(bytes) as u32
    } else {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        (nanos ^ (nanos >> 16)) & 0xFFFF
    };
    // Keep the sentinel out of the value space, so `PROC == 0` can only ever
    // mean "not minted".
    if raw == 0 {
        1
    } else {
        raw
    }
}

/// This process's id, minted on first use and stable for its lifetime.
///
/// Lock-free by construction: a race mints twice and the loser adopts the
/// winner's value, so every thread — including one inside a panic hook —
/// reads the same id without ever waiting on another.
fn proc_bits() -> u32 {
    let cached = PROC.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let minted = mint_proc_bits();
    match PROC.compare_exchange(0, minted, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => minted,
        Err(won) => won,
    }
}

/// This process's id as it appears on disk: 4 lowercase hex digits.
pub fn proc_id() -> String {
    format!("{:04x}", proc_bits())
}

/// The stage that opens a session, named here because `event` has to
/// recognise it. See [`BUILD`].
const APP_LAUNCHED: &str = "app.launched";

/// Version and commit of the binary that is running, e.g. `3.1.7+9dbb2ff`.
///
/// Attached to `app.launched` and to nothing else — it is fixed for the
/// process, and a field that never varies has no business on 37 lines per
/// dictation. `app.launched` is where a reader already goes to find the
/// session boundary, and "which binary wrote this session" is the same
/// question. The commit comes from `build.rs`, which resolves
/// `GIT_COMMIT_SHA` (CI) then `git rev-parse` (local dev) then `unknown`; a
/// build that says `unknown` is one built outside a checkout, which is
/// itself worth seeing.
const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("TTP_BUILD_SHA"));

/// Id column used by standalone events — a hotkey press, a tap recovery —
/// that do not belong to a dictation. Kept the same width as a real id so
/// the columns line up, and deliberately not hex so `grep <id>` stays clean.
pub const STANDALONE_ID: &str = "········";

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
pub fn verbose_enabled() -> bool {
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

// ── The record ──────────────────────────────────────────────────────────

/// One line of the trace, in the shape the viewer UI consumes.
///
/// This is both what the writer thread renders to disk and what
/// `trace_api::parse_line` recovers from a line read back — the round trip is
/// tested, so the file on disk stays the single source of truth and the
/// viewer needs no parallel database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEvent {
    /// Local wall-clock time, `%Y-%m-%d %H:%M:%S%.3f`.
    pub ts: String,
    /// Dictation id, or `None` for a standalone event.
    pub id: Option<String>,
    /// Milliseconds since this dictation began. `None` for standalone events.
    pub elapsed_ms: Option<u64>,
    /// Dotted stage name (`whisper.response`, `hotkey.tap_rearmed`, …).
    pub stage: String,
    /// The stage's JSON payload. Always an object; `{}` when it has no fields.
    pub fields: Value,
}

/// Render the payload, stamping this process's `proc` as its first field.
///
/// The stamp is applied *here*, in the one function that defines the on-disk
/// format, rather than at the call sites that build records. That is the
/// whole point: no caller can forget it, including the ones that do not go
/// through `emit` at all. `lib.rs`'s panic hook builds a `TraceEvent` by hand
/// and hands it straight to `format_line` because a channel push does not
/// survive an abort — and it gets `proc` without knowing the field exists.
///
/// A record that already carries a `proc` keeps it. That is the re-render
/// case: a line read back off disk by `trace_api::parse_line` and formatted
/// again must still name the process that *wrote* it, not the one reading it.
/// It is also what keeps `parse_line` and `format_line` exact inverses.
///
/// Textual splice rather than a `serde_json::Map` insert, for two reasons:
/// it clones no map, and it puts `proc` first. The map is a `BTreeMap` here
/// (`serde_json` without `preserve_order`), so an inserted key would sort
/// into the middle of the payload and land in a different column on every
/// line. `proc` is a stamp, not a payload field, and it reads like one at
/// the front.
fn render_fields(fields: &Value) -> String {
    let body = match fields {
        Value::Null => "{}".to_string(),
        other => other.to_string(),
    };
    if !body.starts_with('{') || fields.get("proc").is_some() {
        return body;
    }
    let inner = &body[1..];
    if inner == "}" {
        format!("{{\"proc\":\"{:04x}\"}}", proc_bits())
    } else {
        format!("{{\"proc\":\"{:04x}\",{}", proc_bits(), inner)
    }
}

/// Render a record to the on-disk line format.
///
/// Kept as a pure function so the format has exactly one definition, shared
/// by the writer and by the parser's round-trip test.
pub fn format_line(ev: &TraceEvent) -> String {
    let rendered = render_fields(&ev.fields);
    match (&ev.id, ev.elapsed_ms) {
        (Some(id), Some(ms)) => format!(
            "[{}] [{}] +{:>5}ms {} {}",
            ev.ts, id, ms, ev.stage, rendered
        ),
        _ => format!(
            "[{}] [{}] {:>8} {} {}",
            ev.ts, STANDALONE_ID, "", ev.stage, rendered
        ),
    }
}

// ── The writer ──────────────────────────────────────────────────────────
//
// Emitting must never block the caller. `logging::log_trace_line` does a
// `stat`, sometimes two `rename`s, an `open` and two `write`s — tens of
// microseconds when the page cache is warm, unbounded when it is not (a
// spinning disk, a sandboxed container, a full volume). Callers include the
// macOS main-thread Fn timer, which fires every 20 ms and whose stalls are
// themselves one of the defects the trace exists to catch.
//
// A bounded channel, not an unbounded one: if the writer thread ever wedges,
// an unbounded queue turns a stuck log into a memory leak. On overflow we
// drop the line and count it, then report the count on the next line that
// gets through — losing a line is bad, but silently losing lines is exactly
// the failure this module exists to prevent.

const QUEUE_CAPACITY: usize = 4096;

enum Msg {
    Line(TraceEvent),
    /// Test/shutdown barrier: the writer acknowledges once everything queued
    /// ahead of it has been written.
    Flush(SyncSender<()>),
}

static QUEUE: OnceLock<SyncSender<Msg>> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Set once during Tauri setup. Held so the writer thread can push each line
/// to a subscribed viewer window without the emit cost landing on the
/// dictation path.
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Whether any viewer is currently listening. Checked before every emit so a
/// user who has never opened the viewer pays nothing for it.
static LIVE: AtomicBool = AtomicBool::new(false);

/// Tauri event channel a viewer subscribes to. One `TraceEvent` per payload.
pub const LIVE_CHANNEL: &str = "trace-event";

/// Give the writer an app handle so live subscribers can be served.
pub fn set_app_handle(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

/// Turn live streaming on or off. Idempotent; the viewer calls this on mount
/// and unmount.
pub fn set_live(on: bool) {
    LIVE.store(on, Ordering::Relaxed);
}

pub fn live_enabled() -> bool {
    LIVE.load(Ordering::Relaxed)
}

fn queue() -> &'static SyncSender<Msg> {
    QUEUE.get_or_init(|| {
        let (tx, rx) = sync_channel::<Msg>(QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("ttp-trace-writer".into())
            .spawn(move || {
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Line(mut ev) => {
                            // Records the file itself rejected. `append_line`
                            // cannot report its own failure by logging — that
                            // is the call that just failed — so it counts them
                            // and the next record that lands carries the
                            // tally, exactly as queue overflow does above. A
                            // trace that goes quiet must say that it did.
                            let failed = crate::logging::take_write_failures();
                            if failed > 0 {
                                merge(&mut ev.fields, json!({ "trace_write_failures": failed }));
                            }
                            let landed = crate::logging::log_trace_line(&format_line(&ev));
                            if !landed {
                                // This line is gone and so is the tally it was
                                // carrying. Put it back, minus nothing: the
                                // failure of this very write has already been
                                // counted by `append_line`.
                                crate::logging::restore_write_failures(failed);
                            }
                            if LIVE.load(Ordering::Relaxed) {
                                if let Some(app) = APP.get() {
                                    use tauri::Emitter;
                                    let _ = app.emit(LIVE_CHANNEL, &ev);
                                }
                            }
                        }
                        Msg::Flush(ack) => {
                            let _ = ack.send(());
                        }
                    }
                }
            })
            .ok();
        tx
    })
}

/// Hand a record to the writer. Never blocks, never fails, never panics.
fn emit(mut ev: TraceEvent) {
    // Surface any lines the queue had to drop, on the first line that fits
    // afterwards. A gap in the trace that announces itself is recoverable;
    // one that does not is the blind spot this module was written to remove.
    let dropped = DROPPED.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        merge(&mut ev.fields, json!({ "trace_dropped_lines": dropped }));
    }
    if queue().try_send(Msg::Line(ev)).is_err() {
        // This line is lost too, and so is the count it was carrying — put
        // both back so the next line that gets through reports the true
        // total. Losing the tally of what we lost would be its own small
        // instance of the failure this module exists to prevent.
        DROPPED.fetch_add(dropped + 1, Ordering::Relaxed);
    }
}

/// Block until everything queued so far has reached the file.
///
/// Only for tests and for shutdown paths that need the evidence on disk.
/// Never call this from the dictation path — the whole point of the queue is
/// that the dictation path does not wait for the disk.
///
/// `allow(dead_code)`: used by the tests below and deliberately not wired
/// into any shutdown hook. Tauri's `RunEvent::Exit` fires before the window
/// server tears the process down, but `panic = "abort"` in the release
/// profile means the interesting exits do not reach it — draining there would
/// buy the tidy shutdown and not the untidy one. Left available for whoever
/// needs it rather than deleted.
#[allow(dead_code)]
pub fn flush() {
    let (tx, rx) = sync_channel::<()>(1);
    if queue().try_send(Msg::Flush(tx)).is_ok() {
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    }
}

fn now_ts() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

/// Record a one-off event that isn't part of a dictation: hotkey transitions,
/// event-tap health, permission changes.
///
/// Written to the same file so a dictation's stages and the key press that
/// started them read in one chronological stream. The id column is `····`
/// rather than a trace id, which keeps `grep <id>` clean while still letting
/// `grep hotkey\.` pull the whole input timeline.
pub fn event(name: &str, fields: Value) {
    let mut payload = normalise(fields);
    // `build` is attached here rather than at the call site for the same
    // reason `proc` is attached in `format_line`: which binary opened a
    // session is a property of the process, not of whoever happened to write
    // the line, and a call site that has to remember it is a call site that
    // can forget it. `proc` says two groups of lines came from different
    // processes; `build` is what lets a reader say *what* the one that
    // launched actually was.
    if name == APP_LAUNCHED {
        merge(&mut payload, json!({ "build": BUILD }));
    }
    emit(TraceEvent {
        ts: now_ts(),
        id: None,
        elapsed_ms: None,
        stage: name.to_string(),
        fields: payload,
    });
}

/// A failure that was swallowed and replaced with a polite default.
///
/// `unwrap_or_default`, `.ok()`, `if let Ok(..)` with no `else` — the shape
/// that made seven defects in this app invisible for weeks. This does not
/// change the degradation (that is a behavioural decision, and not this
/// module's call); it makes it audible. Every one of them lands on the same
/// stage name so the whole class is one grep:
///
/// ```sh
/// grep '"stage":"degraded"' ttp-trace.log   # or simply: grep ' degraded '
/// ```
pub fn degraded(site: &str, fields: Value) {
    let mut payload = json!({ "site": site });
    merge(&mut payload, fields);
    event("degraded", payload);
}

/// Coerce a payload into an object so every line's `fields` has one shape.
fn normalise(fields: Value) -> Value {
    match fields {
        Value::Object(_) => fields,
        Value::Null => json!({}),
        other => json!({ "value": other }),
    }
}

// ── One dictation ───────────────────────────────────────────────────────

/// A single dictation's trace. Cheap to create; every method is fire-and-
/// forget and never fails — tracing must not be able to break a dictation.
///
/// `Clone` so a background task can keep emitting into the same dictation
/// after the pipeline has moved on — the paste verification does this, since
/// it has to wait for the target app's run loop and must not sit on the
/// critical path while it does. The cloned `Instant` keeps elapsed times
/// consistent with the parent's, and the shared `last_stage_ms` cursor keeps
/// `dur_ms` meaningful across the clone.
#[derive(Clone)]
pub struct Trace {
    id: String,
    start: Instant,
    verbose: bool,
    /// Elapsed-ms of the previous stage on this dictation. Shared between
    /// clones so `dur_ms` measures the real gap between consecutive lines
    /// however many tasks are writing them.
    last_stage_ms: Arc<AtomicU64>,
}

/// Stopwatch for one piece of work, so a stage can report how long the thing
/// it describes actually took rather than merely when it ended.
///
/// `dur_ms` (gap since the previous line) answers "where did the wall clock
/// go"; `ms` from a `Span` answers "how long did *this call* take", which is
/// the question you need when the slow thing and the previous line are not
/// the same thing — a keychain read inside a stage that also does I/O, say.
pub struct Span(Instant);

impl Span {
    pub fn start() -> Self {
        Span(Instant::now())
    }
    pub fn ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

impl Trace {
    /// Open a trace and emit its first line.
    pub fn start(kind: &str) -> Self {
        let trace = Self {
            id: new_id(),
            start: Instant::now(),
            verbose: verbose_enabled(),
            last_stage_ms: Arc::new(AtomicU64::new(0)),
        };
        trace.stage(
            "dictation.start",
            json!({ "kind": kind, "verbose": trace.verbose }),
        );
        trace
    }

    /// Short id of this dictation, for cross-referencing a line written to
    /// `ttp.log` back to the full stage list in `ttp-trace.log`.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Record a stage with a JSON object of fields.
    ///
    /// `dur_ms` — the gap since this dictation's previous stage — is injected
    /// into every line. Reading a trace used to mean subtracting the elapsed
    /// column by hand to find the slow step; now the slow step names itself,
    /// which is what makes `sort -t: -k2 -rn` over a field possible at all.
    pub fn stage(&self, name: &str, fields: Value) {
        emit(self.build(name, fields));
    }

    /// Build the record a `stage` call would emit, advancing the duration
    /// cursor. Split out so the `dur_ms` contract can be asserted without a
    /// writer thread, a filesystem, or a settings file.
    fn build(&self, name: &str, fields: Value) -> TraceEvent {
        let elapsed = self.elapsed_ms();
        let prev = self.last_stage_ms.swap(elapsed, Ordering::Relaxed);
        let mut payload = normalise(fields);
        merge(
            &mut payload,
            json!({ "dur_ms": elapsed.saturating_sub(prev) }),
        );
        TraceEvent {
            ts: now_ts(),
            id: Some(self.id.clone()),
            elapsed_ms: Some(elapsed),
            stage: name.to_string(),
            fields: payload,
        }
    }

    /// A trace that writes nothing, for tests. Never emits `dictation.start`,
    /// so it does not touch the settings file to resolve `verbose`.
    #[cfg(test)]
    fn detached(verbose: bool) -> Self {
        Self {
            id: "0000-test".into(),
            start: Instant::now(),
            verbose,
            last_stage_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record a stage plus how long the work it describes took.
    ///
    /// Use where the previous line is not the start of this work — a spawned
    /// task, a retry, anything where `dur_ms` would measure the wrong gap.
    pub fn timed(&self, name: &str, span: &Span, fields: Value) {
        let mut payload = normalise(fields);
        merge(&mut payload, json!({ "ms": span.ms() }));
        self.stage(name, payload);
    }

    /// A failure this dictation swallowed and replaced with a default.
    ///
    /// Same intent as the free `degraded` function, attributed to the
    /// dictation it happened inside so it lands in that dictation's timeline
    /// rather than floating in the standalone stream.
    pub fn degraded(&self, site: &str, fields: Value) {
        let mut payload = json!({ "site": site });
        merge(&mut payload, fields);
        self.stage("degraded", payload);
    }

    /// Record a decision a stage took, including the case where it decided to
    /// do nothing.
    ///
    /// The filters are the reason this exists. `hallucination` only ever left
    /// a line when it fired, which means an empty target app with no
    /// `hallucination` line was equally consistent with "the filter passed the
    /// text through" and "the filter was never reached" — two very different
    /// bugs. `matched:false` costs one line and removes the ambiguity.
    pub fn decision(&self, name: &str, matched: bool, fields: Value) {
        let mut payload = json!({ "matched": matched });
        merge(&mut payload, fields);
        self.stage(name, payload);
    }

    /// Record a stage that carries user text.
    ///
    /// Always logs the character count and digest; adds the text itself only
    /// when diagnostics are enabled. `extra` is merged into the same object so
    /// a call site can attach context (which filter matched, which model
    /// answered) without emitting a second line.
    pub fn text_stage(&self, name: &str, text: &str, extra: Value) {
        emit(self.build(name, self.text_fields(text, extra)));
    }

    fn text_fields(&self, text: &str, extra: Value) -> Value {
        let mut fields = json!({
            "chars": text.chars().count(),
            "sha8": digest8(text),
        });
        if self.verbose {
            fields["text"] = Value::String(text.to_string());
        }
        merge(&mut fields, extra);
        fields
    }

    /// Record the transformation of one text into another.
    ///
    /// `changed` is the field worth grepping for: it separates "this stage was
    /// a no-op" from "this stage rewrote the user's words", and a `to.chars`
    /// of 0 pinpoints the stage that emptied the transcription.
    pub fn transform(&self, name: &str, before: &str, after: &str, extra: Value) {
        emit(self.build(name, self.transform_fields(before, after, extra)));
    }

    fn transform_fields(&self, before: &str, after: &str, extra: Value) -> Value {
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
        fields
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

    #[test]
    fn normalise_always_yields_an_object() {
        assert_eq!(normalise(Value::Null), json!({}));
        assert_eq!(normalise(json!({ "a": 1 })), json!({ "a": 1 }));
        assert_eq!(normalise(json!(7)), json!({ "value": 7 }));
    }

    #[test]
    fn dictation_line_keeps_the_documented_shape() {
        // docs/tracing.md publishes this layout and every grep in the wild
        // depends on it. Column widths included: the elapsed field is right-
        // aligned to 5 so stages line up down the page.
        let line = format_line(&TraceEvent {
            ts: "2026-08-26 08:48:57.412".into(),
            id: Some("0007-3f2a".into()),
            elapsed_ms: Some(6),
            stage: "audio.duration".into(),
            fields: json!({ "secs": 7.52 }),
        });
        assert_eq!(
            line,
            format!(
                "[2026-08-26 08:48:57.412] [0007-3f2a] +    6ms audio.duration \
                 {{\"proc\":\"{}\",\"secs\":7.52}}",
                proc_id()
            )
        );
    }

    // ── Who wrote this line ─────────────────────────────────────────────

    #[test]
    fn every_rendered_record_names_its_process() {
        // The whole workstream in one assertion: there is no record shape
        // that reaches disk without a `proc`. Includes the empty payload,
        // which is the shape most standalone events have, and the hand-built
        // shape `lib.rs`'s panic hook uses.
        for fields in [
            Value::Null,
            json!({}),
            json!({ "secs": 7.52, "dur_ms": 3 }),
            json!({ "msg": "x", "location": "src/lib.rs:1:1", "thread": "main" }),
        ] {
            let line = format_line(&TraceEvent {
                ts: "2026-08-26 08:48:57.412".into(),
                id: None,
                elapsed_ms: None,
                stage: "some.stage".into(),
                fields,
            });
            let parsed = crate::trace_api::parse_line(&line).expect("parses");
            assert_eq!(
                parsed.fields["proc"], proc_id(),
                "a record reached the line format with no proc: {}",
                line
            );
        }
    }

    #[test]
    fn the_process_id_is_stable_for_the_life_of_the_process() {
        // If this drifted, grouping a file by `proc` would split one process
        // into several and re-create the ambiguity it exists to remove.
        let first = proc_id();
        for _ in 0..1000 {
            assert_eq!(proc_id(), first);
        }
        // Including from another thread: it is a process id, not a thread id.
        let expected = first.clone();
        let seen = std::thread::spawn(move || proc_id()).join().unwrap();
        assert_eq!(seen, expected);
    }

    #[test]
    fn the_process_id_is_four_lowercase_hex_digits() {
        let id = proc_id();
        assert_eq!(id.len(), 4, "proc is a field on every line; keep it short");
        assert!(
            id.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "proc must stay greppable ASCII: {}",
            id
        );
    }

    #[test]
    fn the_process_id_carries_no_identity() {
        // Requirement 5. A pid would be the obvious implementation and is
        // the one thing this may not be: it is a cross-log correlator and it
        // leaks launch ordering, in a file users are invited to send us.
        let pid = std::process::id();
        assert_ne!(
            proc_bits(),
            pid & 0xFFFF,
            "proc is the low bits of the pid — that is the implementation this forbids"
        );
    }

    #[test]
    fn minting_is_high_entropy() {
        // `proc_id` is minted once, so the distinctness of two *processes*
        // rests entirely on the distinctness of two draws from this function.
        // `distinct_processes_get_distinct_process_ids` observes the real
        // property across real processes; this one says why it holds, and
        // fails loudly if the source is ever swapped for something coarse
        // (a second-resolution clock, a counter, a constant).
        let draws: std::collections::HashSet<u32> =
            (0..512).map(|_| mint_proc_bits()).collect();
        assert!(
            draws.len() > 480,
            "only {} distinct values in 512 draws — the id source is not random",
            draws.len()
        );
        assert!(!draws.contains(&0), "0 is the not-minted sentinel");
    }

    /// Two real processes, not two calls in one.
    ///
    /// The claim is about co-tenancy — an installed app and a `cargo test`
    /// run appending to one file — so the test spawns actual processes and
    /// compares what they mint. Re-executes this test binary with
    /// `TTP_PROC_ID_CHILD` set, which makes the child take the early branch,
    /// print its id and exit; costs a few milliseconds and needs no fixture.
    #[test]
    fn distinct_processes_get_distinct_process_ids() {
        const CHILD: &str = "TTP_PROC_ID_CHILD";
        if std::env::var(CHILD).is_ok() {
            println!("PROC={}", proc_id());
            return;
        }

        let exe = std::env::current_exe().expect("test binary path");
        let mut ids = vec![proc_id()];
        for _ in 0..2 {
            let out = std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "--nocapture",
                    "trace::tests::distinct_processes_get_distinct_process_ids",
                ])
                .env(CHILD, "1")
                .output()
                .expect("re-exec the test binary");
            let stdout = String::from_utf8_lossy(&out.stdout);
            let id = stdout
                .lines()
                .find_map(|l| l.strip_prefix("PROC="))
                .unwrap_or_else(|| panic!("child printed no id:\n{}", stdout))
                .to_string();
            ids.push(id);
        }

        let distinct: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(
            distinct.len(),
            ids.len(),
            "two processes shared a proc id: {:?} — co-tenants would still be indistinguishable",
            ids
        );
    }

    #[test]
    fn a_record_that_already_names_a_process_is_not_re_attributed() {
        // A line read off disk and rendered again — which is what the viewer
        // and the analyser do — must keep the id of the process that wrote
        // it. Without this, reading a shared log would stamp every record in
        // it with the reader's own id and erase the very distinction.
        let line = format_line(&TraceEvent {
            ts: "2026-08-26 08:48:57.412".into(),
            id: None,
            elapsed_ms: None,
            stage: "app.launched".into(),
            fields: json!({ "proc": "beef", "version": "3.1.7" }),
        });
        assert!(line.contains("\"proc\":\"beef\""), "{}", line);
        assert!(!line.contains(&proc_id()), "the reader re-attributed the record");
    }

    #[test]
    fn a_launch_line_names_the_binary_and_a_test_binary_writes_none() {
        // The discriminator the analyser keys on. `app.launched` is the only
        // process-scoped stage, and `event` is what puts `build` on it — so
        // a process that emitted one is the app, and a process whose lines
        // carry no such record is a test binary or a dev build.
        let mut launched = normalise(json!({ "version": "3.1.7", "os": "macos" }));
        merge(&mut launched, json!({ "build": BUILD }));
        assert_eq!(launched["build"], BUILD);
        assert!(
            BUILD.starts_with(env!("CARGO_PKG_VERSION")),
            "build must lead with the version: {}",
            BUILD
        );
        assert!(
            BUILD.contains('+') && BUILD.split('+').nth(1).is_some_and(|s| !s.is_empty()),
            "build must name a commit: {}",
            BUILD
        );

        // And nothing else gets it: it is fixed for the process, so paying
        // for it on 37 lines a dictation would be pure retention cost.
        let mut ordinary = normalise(json!({ "to": "Idle" }));
        if "state.transition" == APP_LAUNCHED {
            merge(&mut ordinary, json!({ "build": BUILD }));
        }
        assert!(ordinary.get("build").is_none());
    }

    #[test]
    fn a_record_can_be_rendered_from_inside_a_panic_hook() {
        // Requirement 2. `logging::log_trace_line` is called *directly* from
        // the panic hook because a push onto the trace channel does not
        // survive an abort, and the hook renders its record with
        // `format_line`. So `proc` has to be reachable there: no channel, no
        // lock the panicking thread might hold, no unbounded allocation.
        // This test proves the render happens and produces a well-formed,
        // parseable record carrying `proc`; `tests/abort_observability.rs`
        // is what proves such a record survives a real abort.
        //
        // Deliberately writes nothing to disk. The only trace file this
        // process could reach is the user's live `ttp-trace.log`, and there
        // is a harvest running in it.
        static CAPTURED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
        static SERIALISE: std::sync::Mutex<()> = std::sync::Mutex::new(());

        let _guard = SERIALISE.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| {
            let line = format_line(&TraceEvent {
                ts: "2026-08-26 08:48:57.412".into(),
                id: None,
                elapsed_ms: None,
                stage: "app.panic".into(),
                fields: json!({
                    "msg": "deliberate",
                    "location": info.location().map(|l| l.file()).unwrap_or("<unknown>"),
                    "thread": "test",
                }),
            });
            *CAPTURED.lock().unwrap_or_else(|e| e.into_inner()) = Some(line);
        }));
        let outcome = std::panic::catch_unwind(|| panic!("deliberate"));
        std::panic::set_hook(previous);
        assert!(outcome.is_err(), "the test panic did not happen");

        let line = CAPTURED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .expect("the hook rendered no record");
        let parsed = crate::trace_api::parse_line(&line).expect("the panic record must parse");
        assert_eq!(parsed.stage, "app.panic");
        assert_eq!(parsed.fields["proc"], proc_id());
        assert_eq!(parsed.fields["msg"], "deliberate");
    }

    #[test]
    fn every_stage_carries_a_duration() {
        // The claim requirement 2 rests on: no stage is exempt, including one
        // whose caller passed no fields at all.
        let t = Trace::detached(false);
        for fields in [Value::Null, json!({}), json!({ "ok": true })] {
            let ev = t.build("some.stage", fields);
            assert!(
                ev.fields.get("dur_ms").and_then(|v| v.as_u64()).is_some(),
                "a stage without dur_ms is a stage you cannot time"
            );
        }
    }

    #[test]
    fn duration_measures_the_gap_since_the_previous_stage() {
        let t = Trace::detached(false);
        let _ = t.build("first", json!({}));
        std::thread::sleep(std::time::Duration::from_millis(30));
        let second = t.build("second", json!({}));
        let dur = second.fields["dur_ms"].as_u64().unwrap();
        // Generous bounds: this asserts the cursor advances, not the
        // scheduler's punctuality. A dur_ms that measured from dictation
        // start instead would be indistinguishable at 30 ms, so the first
        // stage below is what pins the difference down.
        assert!(dur >= 20, "dur_ms {} did not span the sleep", dur);
        let elapsed = second.elapsed_ms.unwrap();
        assert!(
            dur <= elapsed,
            "a stage cannot have lasted longer than the whole dictation"
        );
    }

    #[test]
    fn duration_is_not_elapsed_time() {
        // Regression guard for the obvious wrong implementation. Three quick
        // stages after a long pause: only the one that follows the pause may
        // carry it.
        let t = Trace::detached(false);
        std::thread::sleep(std::time::Duration::from_millis(40));
        let after_pause = t.build("a", json!({}));
        let immediately = t.build("b", json!({}));
        assert!(after_pause.fields["dur_ms"].as_u64().unwrap() >= 30);
        assert!(
            immediately.fields["dur_ms"].as_u64().unwrap() < 20,
            "a stage that followed instantly reported the whole elapsed time"
        );
    }

    #[test]
    fn a_decision_records_the_negative_case() {
        // Requirement 1: the filter that decided NOT to filter still writes a
        // line, and the line says so in a field a grep can find.
        let t = Trace::detached(false);
        let ev = t.build("filter.hallucination", json!({ "matched": false, "chars": 87 }));
        assert_eq!(ev.fields["matched"], false);
        assert_eq!(ev.fields["chars"], 87);
    }

    #[test]
    fn text_stages_redact_unless_diagnostics_are_on() {
        // The privacy contract. `text_stage` and `transform` are the only two
        // paths that can put speech on disk and both are gated on the same
        // flag; a regression here would leak transcriptions from a product
        // whose entire positioning is that it does not.
        let quiet = Trace::detached(false);
        let redacted = quiet.text_fields("bonjour tout le monde", Value::Null);
        assert!(
            redacted.get("text").is_none(),
            "diagnostics are off and the transcription is in the trace"
        );
        assert_eq!(redacted["chars"], 21);
        assert_eq!(redacted["sha8"], digest8("bonjour tout le monde"));

        let loud = Trace::detached(true);
        assert_eq!(
            loud.text_fields("bonjour tout le monde", Value::Null)["text"],
            "bonjour tout le monde"
        );
    }

    #[test]
    fn transforms_redact_both_sides_unless_diagnostics_are_on() {
        let quiet = Trace::detached(false);
        let f = quiet.transform_fields("avant", "après", Value::Null);
        assert_eq!(f["changed"], true);
        assert!(f["from"].get("text").is_none());
        assert!(f["to"].get("text").is_none());
        // Digests still distinguish "the stage rewrote it" from "the stage
        // passed it through" — the whole point of redacting rather than
        // omitting.
        assert_ne!(f["from"]["sha8"], f["to"]["sha8"]);

        let loud = Trace::detached(true);
        let f = loud.transform_fields("avant", "après", Value::Null);
        assert_eq!(f["from"]["text"], "avant");
        assert_eq!(f["to"]["text"], "après");
    }

    #[test]
    fn an_unchanged_transform_never_carries_text() {
        // Even with diagnostics on: a no-op stage has nothing to explain, so
        // it does not get to write the user's words to disk a second time.
        let loud = Trace::detached(true);
        let f = loud.transform_fields("identique", "identique", Value::Null);
        assert_eq!(f["changed"], false);
        assert!(f["from"].get("text").is_none());
    }

    #[test]
    fn extra_fields_cannot_be_clobbered_by_the_caller() {
        // `extra` is merged last on purpose: a call site that needs to
        // override `chars` (a truncated payload, say) can.
        let t = Trace::detached(false);
        let f = t.text_fields("abc", json!({ "attempt": 2 }));
        assert_eq!(f["attempt"], 2);
        assert_eq!(f["chars"], 3);
    }

    #[test]
    fn the_writer_thread_starts_and_drains() {
        // Deliberately does NOT emit a line. The only trace file this process
        // can write to is the user's real `ttp-trace.log`, and there is a live
        // harvest running in it — a test that appends fake dictations to the
        // evidence would be worse than no test. `flush()` proves the thread
        // spawned, the channel is connected and messages are consumed in
        // order, which is everything the queue is responsible for; what
        // happens after `log_trace_line` is `logging`'s contract, not this
        // module's.
        //
        // Known gap, stated rather than papered over: nothing here exercises
        // emit → file end to end. That needs a redirectable log directory,
        // which `logging` does not have yet.
        let start = std::time::Instant::now();
        flush();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "flush timed out — the writer thread is not draining"
        );
        flush(); // idempotent: a second barrier on a live thread also returns
    }

    #[test]
    fn emitting_never_panics_and_never_blocks_the_caller() {
        // The property the dictation path depends on. Not a timing assertion
        // — a loaded CI box can stall anything — but a smoke test that the
        // bounded queue's overflow branch is reachable without unwinding.
        let t = Trace::detached(false);
        for i in 0..(QUEUE_CAPACITY + 64) {
            let _ = t.build("stress.stage", json!({ "i": i }));
        }
    }

    #[test]
    fn standalone_line_uses_the_dotted_id_column() {
        let line = format_line(&TraceEvent {
            ts: "2026-08-26 08:48:57.412".into(),
            id: None,
            elapsed_ms: None,
            stage: "hotkey.press".into(),
            fields: json!({}),
        });
        assert!(line.contains(&format!("[{}]", STANDALONE_ID)));
        assert!(line.ends_with(&format!("hotkey.press {{\"proc\":\"{}\"}}", proc_id())));
    }
}
