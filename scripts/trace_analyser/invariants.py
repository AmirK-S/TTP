"""The invariants.

Each check encodes either a property `docs/tracing.md` states in prose, or the
log shape of one of the seven defects this project has fixed. A check is a
function that takes the `Corpus` and yields `Finding`s.

Two rules every check obeys:

1. **Unknown stages are never a violation.** The trace vocabulary is growing
   (`docs/trace-api.md`). A check keys off the stages it names and ignores
   everything else. `KNOWN_STAGES` exists only to print an informational list
   of stages this file has not been taught about — it never gates a check.
2. **A missing field is not a violation.** The corpus already contains two
   schema generations (`audio.rms` before `audio.signal`, `polish {applied}`
   before `polish {outcome}`). A check that cannot see what it needs abstains.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from .model import (
    ORIGIN_APP, ORIGIN_HARNESS, ORIGIN_UNKNOWN, Corpus, Dictation, Event,
)

ERROR = "error"
WARN = "warn"
INFO = "info"

# Documented in docs/tracing.md § "Aborted dictations". Anything outside this
# set is reported as INFO (new vocabulary), never as a violation.
DOCUMENTED_ABORT_REASONS = {
    "recording_empty",
    "dead_capture",
    "silent_audio",
    "whisper_error",
    "no_speech",
    "hallucination",
    "glossary_ghost",
    "prompt_introducer_leak",
    "audio_too_large",
    "wav_invalid",
    "no_api_key",
    # Added by docs/trace-api.md, which is the newer contract.
    "api_key_read_failed",
    "clipboard_write_failed",
    "audio_file_missing",
}

# docs/tracing.md § "Aborted dictations" and § outcomes. `pasted_unverified`
# and `paste_swallowed` landed with the paste-verification workstream: since
# "pasted now means observed", the two states that used to be reported as
# `pasted` need names of their own, and both are documented.
DOCUMENTED_OUTCOMES = {
    "pasted", "aborted", "clipboard_fallback",
    "pasted_unverified", "paste_swallowed",
}

# The `PasteVerdict` vocabulary being added to `paste.verify` alongside this
# workstream (docs/tracing.md). Named here rather than inlined so that when a
# fourth slug appears, the one place to teach it is this block. Rule 1 still
# holds: a verdict this file does not know is not a violation — `check_paste_
# swallowed` abstains on it, and the reader learns about it from
# `outcome-undocumented` and the unknown-stage list.
PASTE_VERDICT_OBSERVED = "observed"      # the text was seen to land
PASTE_VERDICT_SWALLOWED = "swallowed"    # it was readable, and it did not
PASTE_VERDICT_UNVERIFIED = "unverified"  # the target could not be read at all

# `paste.verify`'s other two new fields, named for the same reason. `verdict`
# is the answer, `evidence` is what it was read off ("text" or "length") and
# `reason` is why an `unverified` verdict could not do better. The checks read
# `verdict` and quote the other two; nothing keys off their values, so a new
# slug in either is not a violation.
PASTE_EVIDENCE_KEY = "evidence"
PASTE_REASON_KEY = "reason"

# The hallucination filter's field vocabulary (`filter.hallucination`), added
# when the filter stopped deleting every repeated 3-gram. `rule` says which
# predicate matched — "empty", "exact", "substring", "repetition_loop" — and
# for a loop, `corroborated` says whether one of the known hallucination
# phrases backed it up. An UNcorroborated loop is now KEPT, which is the
# safeguard: the filter only deletes a loop the known list vouches for. These
# are quoted in `hallucination-dropped-speech`'s finding rather than gating
# it, so a trace generation without them still gets the same verdict.
HALLUCINATION_RULE_KEY = "rule"
HALLUCINATION_LOOP_KEY = "repetition_loop"
HALLUCINATION_CORROBORATED_KEY = "corroborated"

# Stages named in docs/tracing.md's stage table, plus the ones the corpus
# shows. Informational only — see rule 1 above.
KNOWN_STAGES = {
    "app.launched", "hotkey.press", "hotkey.release", "hotkey.event_dropped",
    "hotkey.double_tap", "hotkey.hands_free_stop", "state.transition",
    "capture.start_failed", "capture.stop_failed", "dictation.rejected",
    "hotkey.tap_armed", "hotkey.tap_rearmed", "hotkey.tap_rebuilt",
    "hotkey.tap_abandoned", "hotkey.tap_create_failed", "hotkey.tap_health",
    "hotkey.stale_fn_cleared", "hotkey.timer_stall", "capture.start",
    "capture.stop", "audio.duration", "audio.signal", "audio.rms",
    "audio.convert", "whisper.request", "whisper.response", "whisper.retry",
    "cleanup", "polish", "polish.decision", "polish.attempt", "polish.outage",
    "dictionary", "paste.accessibility", "paste.decision", "paste.modifiers",
    "paste.result", "paste.skipped", "paste.verify", "clipboard.restore",
    "clipboard.write", "correction_window.started", "history.saved",
    "usage.recorded", "usage.polish_recorded", "files.cleaned", "ui.completed",
    "dictation.start", "dictation.finish", "settings.snapshot",
    "filter.hallucination", "filter.glossary_ghost", "filter.prompt_introducer",
    "vad.armed", "vad.fired", "vad.disarmed",
    "companion.face", "companion.named", "companion.pill_hidden",
    "companion.state",
    # docs/trace-api.md's families. Listed so they do not clutter the
    # "not taught about" section; the checks below still ignore what they
    # do not name.
    "keychain.slow", "keychain.api_key", "keychain.warmed", "degraded",
    # R1 (wave 4). The capture arbiter's four vocabulary items, the two
    # permission families, and the dead-input watchdog. Documented in
    # docs/tracing.md § "The stages"; `capture-arbiter-left-live` and
    # `keychain-not-single-flighted` are the checks that key off them.
    "capture.orphan_prevented", "capture.orphan_reclaimed",
    "capture.stale_dropped", "capture.stop_waited_for_start",
    "capture.dead_input_detected",
    "permission.tcc_reset", "permission.tcc_reset_result",
    "permission.notify", "permission.notify_failed",
    # Reconciled against the emitting source rather than added one at a time,
    # which is how `whisper.attempt` sat in the unknown list for a wave. The
    # scan is every stage-name literal passed to `trace::event`, `Trace::
    # stage`/`text_stage`/`timed`/`transform`/`decision`, and the panic hook's
    # hand-built record, across src-tauri/src. Three names were missing:
    #
    #   whisper.attempt        transcription/whisper.rs — the per-attempt
    #                          record carrying the retry decision, added when
    #                          `attempt` stopped being hardcoded on
    #                          whisper.response;
    #   app.panic              lib.rs — the panic hook. `panic = "abort"` in
    #                          release means catch_unwind never runs, so this
    #                          record is the only thing a crash leaves behind
    #                          and it carries no dictation id by design;
    #   settings.unknown_field settings/store.rs — a field the settings store
    #                          did not recognise, which used to be eaten.
    #
    # `degraded` sites are NOT stages: trace::degraded routes every one of
    # them (capture.reclaim, keychain.secret, keychain.csprng,
    # keychain.migration_flag, settings.fsync, audio.size, backup.audio,
    # history.save, input_mode) onto the single stage `degraded` with the site
    # in `site`, which is already listed above. Adding them here would invent
    # nine stages the writer never emits.
    "whisper.attempt", "app.panic", "settings.unknown_field",
    # `audio.rms` below has no emitter left in the source and is kept on
    # purpose: it is the pre-`audio.signal` schema generation, 52 dictations
    # of it are in the corpus, and dropping it would list a stage the app once
    # wrote as one the analyser has never heard of.
}

# The four ways a live capture is closed. `capture.stop` is the healthy one;
# the other three are the capture arbiter (`src-tauri/src/capture_arbiter.rs`)
# closing a stream nobody is waiting for. A start refused by the arbiter still
# emits `capture.start` first — the refusal happens after the stream is built —
# so a check that only knows about `capture.stop` reports the arbiter *working*
# as a microphone left live. That would be exactly the confident-and-wrong
# finding this analyser exists not to produce.
CAPTURE_CLOSED_BY = {
    "capture.stop", "capture.stop_failed",
    "capture.orphan_prevented", "capture.orphan_reclaimed",
    "capture.stale_dropped",
}

# The arbiter's two "I closed a stream" events. After either one the
# microphone is off, and the next capture-layer event must be a fresh
# `capture.start`.
ARBITER_CLOSED = {"capture.orphan_prevented", "capture.orphan_reclaimed"}

# The keychain account read on the dictation critical path. A read that
# takes this long is the keychain defect: securityd can block for seconds,
# and it does so between paste.verify and usage.recorded.
KEYCHAIN_SLOW_MS = 250

# Stages after paste.result that docs/tracing.md calls "trivial and
# synchronous": a clipboard write, a JSON append, a few remove_file calls.
# "None of it can take seconds, let alone minutes."
BOOKKEEPING_STAGES = {
    "clipboard.restore", "correction_window.started", "history.saved",
    "ui.completed", "usage.recorded", "files.cleaned", "dictation.finish",
    "paste.verify",
}

# A gap between two adjacent bookkeeping stages above this is a violation.
# 1 s is three orders of magnitude above the p99 of every such transition in
# the 402-dictation corpus (p99 <= 3 ms for all of them except the ones this
# check exists to find), so it cannot fire on ordinary jitter.
BOOKKEEPING_GAP_MS = 1000

# Above this, the gap stops being a slow step and starts being a stopped
# process. Nothing in the bookkeeping region issues a call that can take a
# minute — not a clipboard write, not a JSON append, not the keychain read
# that produced the worst gap this project has measured. A minute-plus gap is
# a machine that slept, a process that was descheduled, or a wall clock that
# moved; hotkey.timer_stall proves the second of those and the trace has no
# witness for the other two. So above this the finding is a warning that
# names what it cannot distinguish, rather than an error asserting a slow
# step.
BOOKKEEPING_IMPLAUSIBLE_MS = 60_000

# hotkey.tap_rearmed carries a streak. The Rust side escalates to a rebuild at
# 5, so a streak that reaches 5 means re-arming has stopped working — the
# condition docs/tracing.md describes as "a climbing streak means re-arming is
# not working".
TAP_STREAK_LIMIT = 5

# Reaching TAP_STREAK_LIMIT is the escalation firing as designed. Getting to
# twice it means the rebuild did not help either, which is the defect rather
# than the response to it.
TAP_STREAK_ESCALATED = 2 * TAP_STREAK_LIMIT

# A session with this many re-arms and no hotkey.press at all is a session in
# which the Fn key was dead throughout.
TAP_FLAP_WITHOUT_INPUT = 20

# ... but only if it lasted long enough that "the user pressed nothing" stops
# being the simpler explanation. docs/trace-invariants.md's own wording: "over
# two minutes this can just mean the user pressed nothing, and over four hours
# it cannot". Half an hour of continuous flapping with no input is the point
# where the benign reading stops being reasonable.
TAP_FLAP_MINUTES = 30.0

# polish.outage carries consecutive_failures. One or two against a live model
# is a network; the decommissioned-model defect reached 17.
POLISH_OUTAGE_STREAK = 3

# A recording at least this long that aborts as silent_audio. Nobody holds a
# push-to-talk key for five seconds in silence; the likelier reading is a
# device streaming near-nothing. Chosen as ~3x the median silent_audio
# duration in the corpus (0.98 s) with margin.
LONG_SILENT_SECS = 5.0

# A capture held for less than this and dropped before dictation.start is a
# stray tap, not a lost recording. The 402-dictation corpus has no traced
# dictation shorter than 0.44 s, and every unhanded capture in it was held
# between 0.20 s and 0.70 s.
HANDOFF_TRIVIAL_SECS = 1.0

# Below this, a "hallucination" verdict on a short take is plausible. Above it,
# combined with a signal above the RMS floor, the filter deleted something that
# looked like speech. 20 chars is roughly four words.
HALLUCINATION_MIN_CHARS = 20
HALLUCINATION_MIN_SECS = 3.0


@dataclass
class Finding:
    invariant: str
    severity: str
    summary: str
    ts: str = ""
    dictation: str | None = None
    index: int = -1  # anchor into corpus.events for context
    detail: dict = field(default_factory=dict)
    # Who wrote the records this finding rests on: "app", "harness", or
    # "unknown" for a record that does not say. Stamped by `run_all` from the
    # anchor record's `proc` unless a check sets it explicitly, and used by the
    # report to keep harness-emitted findings out of the app's blocks. A
    # finding about a test binary is not a finding about the product, and
    # printing them together is the merge this workstream exists to undo.
    origin: str = ORIGIN_UNKNOWN


REGISTRY: list = []


def invariant(iid: str, severity: str, title: str, why: str):
    """Register a check. `why` is printed by --list, so it is the docs."""

    def deco(fn):
        fn.iid = iid
        fn.severity = severity
        fn.title = title
        fn.why = why
        REGISTRY.append(fn)
        return fn

    return deco


# ---------------------------------------------------------------- helpers


def _anchor(d: Dictation) -> int:
    return d.events[0].index if d.events else -1


def _signal(d: Dictation) -> Event | None:
    """The signal stage, under either of its two schema names."""
    return d.stage("audio.signal") or d.stage("audio.rms")


def _nonzero_ratio(d: Dictation):
    """nonzero_ratio, from wherever this schema generation put it."""
    for e in (d.finish, _signal(d)):
        if e is not None:
            v = e.get("nonzero_ratio")
            if v is not None:
                return v
    return None


def _in_flight_at_eof(corpus: Corpus, d: Dictation) -> bool:
    """True if the corpus simply ends while this dictation is still running.

    A tail-truncated dictation is not a violation. This is the guard that keeps
    the analyser from repeating the mistake this programme already made once:
    reading a long dictation as an orphan because the evidence ran out.
    """
    if not d.events:
        return True
    last = corpus.events[-1]
    return d.events[-1].index >= last.index - 2


def _truncated_head(corpus: Corpus, d: Dictation) -> bool:
    """True if this dictation is in the session whose head rotation ate."""
    s = corpus.sessions[0] if corpus.sessions else None
    return bool(s and s.launched is None and d.session == s.index)


def _stall_covering(corpus: Corpus, a: Event, b: Event):
    """A hotkey.timer_stall whose skipped window overlaps [a.ts, b.ts].

    docs/tracing.md's own procedure: a large gap in the bookkeeping region
    means the process stopped running, not that a step is slow — and a stall
    covering the same window is what tells the two apart.
    """
    for e in corpus.events:
        if e.stage != "hotkey.timer_stall":
            continue
        gap = e.get("gap_ms")
        if gap is None:
            continue
        # The stall is reported when the timer next fires, so its window is
        # [e.ts - gap, e.ts]. Allow a second of slack at the far end.
        start = e.ts.timestamp() - gap / 1000.0
        end = e.ts.timestamp() + 1.0
        if start <= b.ts.timestamp() and a.ts.timestamp() <= end:
            return e
    return None


def _keychain_slow_in(corpus: Corpus, a: Event, b: Event):
    """A keychain.slow event inside [a, b].

    docs/trace-api.md added the keychain.* family while this analyser was
    being written, and it fired on the live log within the hour. A blocking
    securityd read is the one explanation for a bookkeeping gap that is
    neither suspension nor a mystery.
    """
    for e in corpus.events[a.index:b.index + 1]:
        if e.stage.startswith("keychain.") and (
                e.stage == "keychain.slow"
                or (e.get("ms") or 0) >= KEYCHAIN_SLOW_MS):
            return e
    return None


def _capture_for(corpus: Corpus, d: Dictation):
    """The capture.start that fed this dictation.

    capture.* lines carry no trace id (docs/trace-api.md confirms this is by
    design), so the device has to be recovered positionally: the last
    capture.start before the dictation began.
    """
    if not d.start:
        return None
    for e in reversed(corpus.events[:d.start.index]):
        if e.stage == "capture.start":
            return e
        if e.stage == "app.launched":
            return None
    return None


def _events_between(corpus: Corpus, a: Event, b: Event) -> int:
    """How many events of any kind were logged during a gap.

    Zero means the process produced nothing at all — weak evidence on its own
    (a quiet app logs nothing either) but decisive over minutes.
    """
    return max(0, b.index - a.index - 1)


def _ordered(d: Dictation) -> list[Event]:
    return sorted(d.events, key=lambda e: e.index)


# ------------------------------------------------------------ attribution
#
# Rule 3, added after the analyser's first outing over the real corpus.
#
# `keychain-on-critical-path` reported fourteen ERRORs claiming keychain reads
# had blocked a dictation for up to 397 seconds. Every one of them carried
# `[········]` — no dictation id — and no dictation in that corpus ran longer
# than 3.5 s. The check had concluded from time proximity: the records sat
# near dictations in the file, so it called them blocking. They were `cargo
# test` runs appending to the same log directory as the installed app.
#
# **Proximity is not attribution.** A standalone record belongs to a dictation
# when the dictation's own id is on it, or when the *timed call it describes*
# demonstrably began and ended inside that dictation's life. A synchronous
# call made by a dictation's pipeline cannot start before the dictation did or
# finish after it ended; a record whose interval spills past the window is a
# different flow — very often a different process — and saying otherwise is
# arithmetic, not judgement.

# Wall-clock slack either side of a dictation's window when testing whether a
# timed call fits inside it. Records are written after the call returns and
# the timestamps are millisecond-resolution, so an exact-fit test would be
# brittle; a quarter of a second is far below the smallest gap this
# distinction has to resolve.
ATTRIBUTION_SLACK_MS = 250


def _dictation_spans(corpus: Corpus):
    """(start, end, dictation) for every dictation with a known life, sorted.

    The end is the last event carrying the id, not `dictation.finish`:
    `paste.verify` is written from a spawned task and legitimately lands after
    the finish line (docs/trace-api.md § DictationTrace).
    """
    spans = getattr(corpus, "_spans_cache", None)
    if spans is None:
        spans = []
        for d in corpus.dictations:
            if not d.start or not d.events:
                continue
            lo = d.start.ts.timestamp()
            hi = max(e.ts for e in d.events).timestamp()
            spans.append((lo, hi, d))
        spans.sort(key=lambda s: s[0])
        corpus._spans_cache = spans          # noqa: SLF001 - a memo, not state
    return spans


def _attribute_timed(corpus: Corpus, e: Event, ms):
    """Place a timed standalone record against the dictations around it.

    Returns (verdict, dictation, ms_overlapped):

    - ``"inside"``    the call began and ended inside one dictation's own
                      start-to-last-stage window. That dictation could have
                      made it, so it is attributable.
    - ``"spans"``     it overlaps a dictation but began before that dictation
                      started or ended after it finished. A synchronous call
                      on the dictation's critical path cannot do that, so the
                      dictation did not make it — and something else was
                      writing to this file at the same time.
    - ``"unrelated"`` no dictation was in flight at any point during the call.

    ``ms`` of ``None`` means the record does not say how long it took, so
    there is nothing to place: rule 2 applies and the answer is "unknown".
    """
    if ms is None:
        return "unknown", None, None
    hi = e.ts.timestamp()
    lo = hi - ms / 1000.0
    slack = ATTRIBUTION_SLACK_MS / 1000.0
    best = None
    for a, b, d in _dictation_spans(corpus):
        if a > hi:
            break
        if b < lo:
            continue
        if a - slack <= lo and hi <= b + slack:
            return "inside", d, ms
        if best is None:
            best = (d, round((min(hi, b) - max(lo, a)) * 1000))
    if best is not None:
        return "spans", best[0], best[1]
    return "unrelated", None, None


def _life_ms(d: Dictation) -> float:
    if not d.events or not d.start:
        return 0.0
    return (max(e.ts for e in d.events)
            - d.start.ts).total_seconds() * 1000.0


def _polish_windows(corpus: Corpus):
    """(first_index, last_index, dictation) for each dictation that polished.

    `polish.attempt` carries no trace id, so the only honest attribution is
    containment in the window a dictation opened when it decided to polish and
    closed when its `polish` stage landed. An attempt outside every such
    window was made by something that is not a traced dictation of this app —
    on 2026-09-02 that was the golden test suite, 83 attempts of which 46
    came back 429 and were reported to the maintainer as a production incident
    that had to be retracted.
    """
    wins = getattr(corpus, "_polish_windows_cache", None)
    if wins is None:
        wins = []
        for d in corpus.dictations:
            dec = d.stage("polish.decision")
            if dec is None:
                continue
            end = d.stage("polish") or d.finish or _ordered(d)[-1]
            wins.append((dec.index, end.index, d))
        corpus._polish_windows_cache = wins  # noqa: SLF001
    return wins


def _owning_dictation(corpus: Corpus, e: Event):
    for lo, hi, d in _polish_windows(corpus):
        if lo <= e.index <= hi:
            return d
    return None


def _cotenancy_evidence(corpus: Corpus) -> dict:
    """What the corpus can and cannot say about how many processes wrote it.

    Two regimes, and which one applies is a property of the corpus in front of
    you, not a setting.

    **With `proc`.** Every record names the process that wrote it and
    `app.launched` names the build. A process that emitted an `app.launched`
    is the app; one that never did is a test binary or a dev build. Then "how
    many writers" is a count, "which records are whose" is a lookup, and the
    inferences below become corroboration rather than the only evidence there
    is.

    **Without it** — every record harvested before the field shipped — the
    format carries no process identity at all, so a record can never be
    *assigned* to the app or to a test binary. What can be established is the
    negative, and it is the half that matters for grading: whether a record is
    attributable to a dictation the app actually served.

    Three shapes constitute positive evidence that more than one writer was
    appending, none of which says which records belong to whom:

    1. a timed standalone call whose interval spans a whole dictation that
       finished in less time than the call took;
    2. `polish.attempt` records outside every dictation's polish window;
    3. merged physical lines whose two records belong to *different*
       dictations (within one dictation two threads of one process explain it
       just as well — and in this corpus 34 of 38 are that).

    Both regimes are computed every time. A partly-migrated corpus — the state
    every machine passes through on the way from one to the other — has some
    records that name their writer and some that do not, and it gets both
    answers, each about the records that support it.
    """
    ev = {"impossible_spans": [], "unattributed_polish": 0,
          "polish_days": {}, "merged_same_dictation": 0,
          "merged_mixed": 0, "long_sessions_without_launch": 0,
          # Process identity, where the writer supplies it.
          "procs": len(corpus.processes),
          "app_procs": [p.proc for p in corpus.app_processes()],
          "harness_procs": [p.proc for p in corpus.harness_processes()],
          "harness_records": sum(len(p.events)
                                 for p in corpus.harness_processes()),
          "records_without_proc": corpus.records_without_proc,
          "builds": sorted({str(p.build) for p in corpus.app_processes()
                            if p.build is not None})}
    for e in corpus.events:
        if e.stage == "keychain.slow" and e.tid is None:
            verdict, d, over = _attribute_timed(corpus, e, e.get("ms"))
            if verdict == "spans" and over is not None and d is not None:
                if (e.get("ms") or 0) > _life_ms(d):
                    ev["impossible_spans"].append((e, d))
    for e in corpus.events:
        if e.stage == "polish.attempt" and _owning_dictation(corpus, e) is None:
            ev["unattributed_polish"] += 1
            k = str(e.ts.date())
            ev["polish_days"][k] = ev["polish_days"].get(k, 0) + 1
    by_source: dict[str, list[Event]] = {}
    for e in corpus.events:
        by_source.setdefault(e.source, []).append(e)
    for evs in by_source.values():
        if len(evs) < 2:
            continue
        tids = {e.tid for e in evs}
        if len(tids) == 1 and None not in tids:
            ev["merged_same_dictation"] += 1
        else:
            ev["merged_mixed"] += 1
    for s in corpus.sessions:
        if s.launched is None or not s.events:
            continue
        span_h = (s.events[-1].ts - s.events[0].ts).total_seconds() / 3600
        if span_h > 24:
            ev["long_sessions_without_launch"] += 1
    return ev


# ------------------------------------------------------- lifecycle shape


@invariant(
    "dictation-finish-missing", ERROR,
    "Every dictation.start has a dictation.finish",
    "docs/engineering-standards.md §3.5. A start with no finish is a dictation "
    "that fell out of the pipeline with no record of where. Tail-truncated "
    "dictations at the end of the corpus are exempt.",
)
def check_finish_missing(corpus: Corpus):
    for d in corpus.dictations:
        if d.start and not d.finish and not _in_flight_at_eof(corpus, d):
            yield Finding(
                "dictation-finish-missing", ERROR,
                f"{d.key} started and never finished "
                f"({len(d.events)} stages, last: {_ordered(d)[-1].stage})",
                ts=str(d.start.ts), dictation=d.key, index=_anchor(d),
                detail={"stages": [e.stage for e in _ordered(d)]},
            )


@invariant(
    "dictation-start-missing", WARN,
    "Every dictation.finish has a dictation.start",
    "A finish with no start means the head of the dictation was lost. In the "
    "rotation-truncated first session this is expected and reported as info.",
)
def check_start_missing(corpus: Corpus):
    for d in corpus.dictations:
        if d.finish and not d.start:
            sev = INFO if _truncated_head(corpus, d) else WARN
            yield Finding(
                "dictation-start-missing", sev,
                f"{d.key} finished with no dictation.start"
                + (" (rotation-truncated session)" if sev == INFO else ""),
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            )


@invariant(
    "capture-handoff-missing", ERROR,
    "Every capture.stop is followed by a dictation.start",
    "docs/tracing.md § 'What is not covered': an exception in the JS "
    "stop_recording -> process_audio handoff leaves capture.stop with no "
    "dictation.start after it. Audio was recorded and then vanished with no "
    "reason line. Scoped by event order, not by a time window, so a long "
    "dictation is never mistaken for an orphan. Graded by how long the "
    f"capture was held: under {HANDOFF_TRIVIAL_SECS}s is almost certainly a "
    "stray tap being discarded by a minimum-length guard (a gap in the "
    "instrumentation, worth a line but not an incident); at or above it, a "
    "real recording was lost with no reason recorded.",
)
def check_capture_handoff(corpus: Corpus):
    open_start: Event | None = None
    for i, e in enumerate(corpus.events):
        if e.stage == "capture.start":
            open_start = e
        if e.stage != "capture.stop":
            continue
        held = ((e.ts - open_start.ts).total_seconds()
                if open_start is not None else None)
        sev = (WARN if held is not None and held < HANDOFF_TRIVIAL_SECS
               else ERROR)
        held_s = f"{held:.2f}s" if held is not None else "unknown"
        for f in corpus.events[i + 1:]:
            if f.stage == "dictation.start":
                break
            if f.stage in ("capture.start", "app.launched"):
                yield Finding(
                    "capture-handoff-missing", sev,
                    f"capture held {held_s} ({e.get('samples')} samples, "
                    f"{e.get('device')}) with no dictation.start before the "
                    f"next {f.stage}",
                    ts=str(e.ts), index=e.index,
                    detail={"samples": e.get("samples"),
                            "device": e.get("device"),
                            "held_secs": held,
                            "terminated_by": f.stage},
                )
                break
        else:
            if e.index < len(corpus.events) - 3:
                yield Finding(
                    "capture-handoff-missing", sev,
                    f"capture held {held_s} ({e.get('samples')} samples) with "
                    f"no dictation.start before the end of the corpus",
                    ts=str(e.ts), index=e.index,
                    detail={"samples": e.get("samples"), "held_secs": held},
                )


@invariant(
    "capture-stop-missing", ERROR,
    "Every capture.start is followed by a capture.stop",
    "The start/stop race: a capture.start that lands after the stop path has "
    "already run leaves a cpal stream open with nobody to close it — the "
    "microphone stays live. Terminated by the next capture.start, an "
    "app.launched, or the end of the corpus. Since R1 a capture is also "
    "closed by the arbiter — capture.orphan_prevented, "
    "capture.orphan_reclaimed, capture.stale_dropped — and each of those "
    "counts as a stop here, because the refused start emitted its "
    "capture.start before the refusal. Reading the arbiter working as a "
    "microphone left live would be a false positive of the worst kind.",
)
def check_capture_stop_missing(corpus: Corpus):
    open_start: Event | None = None
    for e in corpus.events:
        if e.stage == "capture.start":
            if open_start is not None:
                held = (e.ts - open_start.ts).total_seconds()
                yield Finding(
                    "capture-stop-missing", ERROR,
                    f"capture.start on {open_start.get('device')} never "
                    f"stopped; stream open for {held / 60:.1f} min until the "
                    f"next capture.start",
                    ts=str(open_start.ts), index=open_start.index,
                    detail={"device": open_start.get("device"),
                            "open_seconds": round(held, 1)},
                )
            open_start = e
        elif e.stage in CAPTURE_CLOSED_BY:
            open_start = None
        elif e.stage == "app.launched" and open_start is not None:
            held = (e.ts - open_start.ts).total_seconds()
            # Graded lower than the other branch on purpose. When the next
            # thing in the log is app.launched, the process that owned the
            # stream exited, and macOS releases a capture device when its
            # process dies — so the microphone was live for at most the rest
            # of that process's life, and the trace cannot say how long that
            # was. The eleven-hour-microphone failure is the other branch:
            # a stream still open while the SAME process starts another one.
            yield Finding(
                "capture-stop-missing", WARN,
                f"capture.start on {open_start.get('device')} never stopped "
                f"before the app relaunched ({held / 60:.1f} min of log "
                f"between them). The process exited, so the OS reclaimed the "
                f"device; how long the microphone was actually live is not "
                f"in the trace",
                ts=str(open_start.ts), index=open_start.index,
                detail={"device": open_start.get("device"),
                        "open_seconds": round(held, 1),
                        "terminated_by": "app.launched"},
            )
            open_start = None


@invariant(
    "capture-stop-without-start", WARN,
    "capture.stop only fires against an open capture",
    "The other half of the start/stop race: the stop path ran while no "
    "recording was open. Its capture.start usually arrives milliseconds "
    "later, orphaned.",
)
def check_stop_without_start(corpus: Corpus):
    open_start = False
    for e in corpus.events:
        if e.stage == "capture.start":
            open_start = True
        elif e.stage in CAPTURE_CLOSED_BY:
            if not open_start and e.stage in ("capture.stop",
                                              "capture.stop_failed"):
                yield Finding(
                    "capture-stop-without-start", WARN,
                    f"{e.stage} with no open capture "
                    f"({e.get('error') or e.get('samples')})",
                    ts=str(e.ts), index=e.index,
                    detail=dict(e.payload),
                )
            open_start = False


@invariant(
    "capture-arbiter-left-live", ERROR,
    "When the capture arbiter refuses or reclaims a stream, the microphone "
    "goes off",
    "The positive half of the 11-hour-microphone fix, and the only check that "
    "can say the fix WORKED rather than that the old failure is absent. "
    "capture-stop-missing catches the old shape — a capture.start with no "
    "capture.stop ever following. R1's arbiter "
    "(src-tauri/src/capture_arbiter.rs) is supposed to make that shape "
    "impossible by refusing a start that lands after its stop concluded "
    "(capture.orphan_prevented) or tearing down a stream published with "
    "nobody to collect it (capture.orphan_reclaimed). In both the Rust drops "
    "the cpal stream before it writes the line, so by the time the line "
    "exists the microphone is off and the capture slot is empty. Three things "
    "in the log would say otherwise, and each is reported here: (1) the next "
    "capture-layer event is another close — a capture.stop, "
    "capture.stale_dropped or capture.orphan_reclaimed with no capture.start "
    "between — which means the stream the arbiter said it tore down was still "
    "published and something else found it; (2) degraded{site:capture.reclaim} "
    "— the arbiter and audio_capture::STATE disagree about whether a capture "
    "is live, which is the bookkeeping that the whole guarantee rests on; "
    "(3) capture.stop_waited_for_start with timed_out:true, the hole the "
    "3-second settle constant's own comment says cannot happen — not a leak "
    "by itself, but it means the arbiter is the only thing between the user "
    "and a hot microphone, so it is reported and the wait is named.",
)
def check_arbiter_left_live(corpus: Corpus):
    for i, e in enumerate(corpus.events):
        if e.stage in ARBITER_CLOSED:
            for nxt in corpus.events[i + 1:]:
                if nxt.stage == "capture.start" or nxt.stage == "app.launched":
                    break
                if nxt.stage in CAPTURE_CLOSED_BY:
                    yield Finding(
                        "capture-arbiter-left-live", ERROR,
                        f"{e.stage} on {e.get('device')} claimed to close the "
                        f"capture, then {nxt.stage} closed one again "
                        f"{(nxt.ts - e.ts).total_seconds():.1f}s later with no "
                        f"capture.start between — the stream was still live",
                        ts=str(e.ts), index=e.index,
                        detail={"closed_by": e.stage,
                                "then": nxt.stage,
                                "device": e.get("device"),
                                "gap_secs": round(
                                    (nxt.ts - e.ts).total_seconds(), 1)},
                    )
                    break
        elif e.stage == "degraded" and e.get("site") == "capture.reclaim":
            yield Finding(
                "capture-arbiter-left-live", ERROR,
                f"the Idle backstop could not reclaim: {e.get('error')} — the "
                f"arbiter and audio_capture::STATE disagree about whether a "
                f"microphone is live",
                ts=str(e.ts), index=e.index, detail=dict(e.payload),
            )
        elif e.stage == "capture.stop_waited_for_start" and e.get("timed_out"):
            yield Finding(
                "capture-arbiter-left-live", ERROR,
                f"stop gave up waiting for an in-flight start after "
                f"{e.get('ms')} ms (timed_out) — the settle window is the "
                f"hole the arbiter now has to cover alone",
                ts=str(e.ts), index=e.index, detail=dict(e.payload),
            )


@invariant(
    "paste-result-missing", ERROR,
    "Every paste.decision is followed by a paste.result",
    "docs/engineering-standards.md §3.5, and the signature A4 names: with "
    "panic = \"abort\" the catch_unwind around injection cannot run, so a "
    "panic in the paste path leaves exactly this gap.",
)
def check_paste_result(corpus: Corpus):
    for d in corpus.dictations:
        if d.has("paste.decision") and not d.has("paste.result"):
            if _in_flight_at_eof(corpus, d):
                continue
            yield Finding(
                "paste-result-missing", ERROR,
                f"{d.key} decided to paste "
                f"({d.stage('paste.decision').get('chars')} chars) and never "
                f"reported a result",
                ts=str(d.stage("paste.decision").ts), dictation=d.key,
                index=_anchor(d),
            )


@invariant(
    "paste-verify-missing", WARN,
    "Every successful paste.result is followed by a paste.verify",
    "paste.result only means the events reached the window server. Without "
    "paste.verify there is no evidence the text landed.",
)
def check_paste_verify_present(corpus: Corpus):
    for d in corpus.dictations:
        r = d.stage("paste.result")
        if r and r.get("ok") is True and not d.has("paste.verify"):
            if _in_flight_at_eof(corpus, d):
                continue
            yield Finding(
                "paste-verify-missing", WARN,
                f"{d.key} pasted with no paste.verify — landing unproven",
                ts=str(r.ts), dictation=d.key, index=_anchor(d),
            )


@invariant(
    "state-chain-broken", WARN,
    "state.transition.from matches the previous transition's .to",
    "The state machine is the thing that decides whether a hotkey press does "
    "anything. A break in the chain means a transition happened that nobody "
    "traced.",
)
def check_state_chain(corpus: Corpus):
    for s in corpus.sessions:
        prev = None
        for e in s.events:
            if e.stage != "state.transition":
                continue
            frm, to = e.get("from"), e.get("to")
            if frm is None or to is None:
                continue
            if prev is not None and frm != prev:
                yield Finding(
                    "state-chain-broken", WARN,
                    f"state jumped to '{frm}' without a transition into it "
                    f"(previous state was '{prev}')",
                    ts=str(e.ts), index=e.index,
                    detail={"from": frm, "to": to, "expected_from": prev},
                )
            prev = to


@invariant(
    "state-parked-processing", ERROR,
    "A session never ends parked in Processing",
    "docs/tracing.md: 'A session parked in Processing makes all later presses "
    "silent no-ops.' The user presses and nothing happens, forever.",
)
def check_parked_processing(corpus: Corpus):
    for s in corpus.sessions:
        state = None
        last = None
        for e in s.events:
            if e.stage == "state.transition" and e.get("to"):
                state, last = e.get("to"), e
        if state == "Processing" and last is not None:
            presses = [e for e in s.events
                       if e.stage == "hotkey.press" and e.index > last.index]
            # An ERROR is meant to mean a user was affected. A session that
            # ends in Processing with nobody pressing anything afterwards is
            # a process that exited mid-dictation, which is a different and
            # much smaller thing than a live app that has gone deaf. The
            # presses are the evidence of harm, so they decide the grade.
            yield Finding(
                "state-parked-processing",
                ERROR if presses else WARN,
                f"session {s.index} ended parked in Processing; "
                f"{len(presses)} later hotkey press(es) were no-ops"
                if presses else
                f"session {s.index} ended parked in Processing, but no "
                f"hotkey press followed it in this session — consistent with "
                f"the process exiting mid-dictation as well as with the app "
                f"going deaf, and the trace does not separate the two",
                ts=str(last.ts), index=last.index,
                detail={"later_presses": len(presses)},
            )


# ---------------------------------------------------- outcome vocabulary


@invariant(
    "abort-reason-missing", ERROR,
    "Every aborted dictation carries a reason",
    "docs/tracing.md: 'Every path that ends without text writes "
    "dictation.finish with an outcome:\"aborted\" and a stable reason slug.' "
    "An abort with no slug is a silent drop wearing a label.",
)
def check_abort_reason(corpus: Corpus):
    for d in corpus.dictations:
        if d.outcome == "aborted" and not d.reason:
            yield Finding(
                "abort-reason-missing", ERROR,
                f"{d.key} aborted with no reason slug",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            )


@invariant(
    "abort-reason-undocumented", INFO,
    "Abort reasons are documented in docs/tracing.md",
    "Reported as info, not as a violation: the vocabulary is meant to grow. "
    "A reason appearing here is a prompt to add a row to the table.",
)
def check_abort_reason_known(corpus: Corpus):
    seen = set()
    for d in corpus.dictations:
        r = d.reason
        if r and r not in DOCUMENTED_ABORT_REASONS and r not in seen:
            seen.add(r)
            yield Finding(
                "abort-reason-undocumented", INFO,
                f"abort reason '{r}' is not in docs/tracing.md's table "
                f"(first seen {d.key})",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            )


@invariant(
    "outcome-undocumented", INFO,
    "dictation.finish outcomes are documented",
    "Also info-only. A new outcome is new vocabulary, not a defect.",
)
def check_outcome_known(corpus: Corpus):
    seen = set()
    for d in corpus.dictations:
        o = d.outcome
        if o and o not in DOCUMENTED_OUTCOMES and o not in seen:
            seen.add(o)
            yield Finding(
                "outcome-undocumented", INFO,
                f"outcome '{o}' is not documented (first seen {d.key})",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            )


# -------------------------------------------------- audio, and the mic


@invariant(
    "dead-capture", ERROR,
    "No dictation ends because the device delivered digital silence",
    "The AirPods defect. docs/tracing.md: dead_capture on a Bluetooth device "
    "means the headset was connected but never actually streaming. Every "
    "occurrence is a recording the user made and lost.",
)
def check_dead_capture(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "dead_capture":
            continue
        dur = d.stage("audio.duration")
        secs = (dur.get("secs") if dur else None) or d.finish.get("secs")
        yield Finding(
            "dead-capture", ERROR,
            f"{d.key}: {secs:.1f}s of digital silence from "
            f"{d.finish.get('device')} "
            f"({d.finish.get('samples')} samples, all zero)"
            if secs else f"{d.key}: digital silence from "
                         f"{d.finish.get('device')}",
            ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            detail={"device": d.finish.get("device"),
                    "samples": d.finish.get("samples"), "secs": secs},
        )


@invariant(
    "dead-capture-misclassified", ERROR,
    "dead_capture implies nonzero_ratio == 0",
    "docs/tracing.md states the predicate and why it is strict: 'a single "
    "non-zero sample anywhere in the recording disqualifies dead_capture. "
    "Telling a user their microphone is broken is a strong claim.' If this "
    "fires, the classifier has drifted from the documented rule.",
)
def check_dead_capture_strict(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "dead_capture":
            continue
        nz = _nonzero_ratio(d)
        if nz is not None and nz != 0:
            yield Finding(
                "dead-capture-misclassified", ERROR,
                f"{d.key} called dead_capture with nonzero_ratio={nz} — the "
                f"device was producing samples",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
                detail={"nonzero_ratio": nz},
            )


@invariant(
    "silent-audio-misclassified", ERROR,
    "silent_audio implies nonzero_ratio > 0",
    "The other half of the same rule. A recording of pure zeros told the user "
    "'no speech detected' — the message docs/tracing.md calls 'wrong and "
    "actively misleading'. Two independent witnesses, so the check still "
    "works on traces older than nonzero_ratio: an exactly-zero avg_rms is a "
    "mean of absolute sample values, and it can only be zero if every sample "
    "is.",
)
def check_silent_audio_strict(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "silent_audio":
            continue
        nz = _nonzero_ratio(d)
        sig = _signal(d)
        rms = d.finish.get("avg_rms")
        if rms is None and sig:
            rms = sig.get("avg_rms")
        witness = None
        if nz is not None and nz == 0:
            witness = "nonzero_ratio=0"
        elif rms == 0:
            witness = "avg_rms=0.0 (every sample zero)"
        if witness is None:
            continue
        dur = d.stage("audio.duration")
        secs = dur.get("secs") if dur else None
        yield Finding(
            "silent-audio-misclassified", ERROR,
            f"{d.key}: "
            + (f"{secs:.1f}s " if secs else "")
            + f"called silent_audio with {witness} — that is a dead device, "
              f"not a quiet room, and the user was told 'no speech detected'",
            ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            detail={"witness": witness, "secs": secs, "avg_rms": rms,
                    "nonzero_ratio": nz},
        )


@invariant(
    "long-recording-silent", WARN,
    "A long recording does not abort as silence",
    f"Nobody holds push-to-talk for {LONG_SILENT_SECS:.0f}+ seconds without "
    "speaking. A long take below the RMS floor is far likelier to be a device "
    "streaming near-nothing than a user staring at the microphone.",
)
def check_long_silent(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "silent_audio":
            continue
        dur = d.stage("audio.duration")
        secs = dur.get("secs") if dur else None
        if secs is None or secs < LONG_SILENT_SECS:
            continue
        sig = _signal(d)
        rms = d.finish.get("avg_rms")
        if rms is None and sig:
            rms = sig.get("avg_rms")
        dev = d.finish.get("device")
        cap = _capture_for(corpus, d)
        if dev is None and cap:
            dev = cap.get("device")
        rms_s = f"{rms:.5f}" if isinstance(rms, (int, float)) else "unknown"
        yield Finding(
            "long-recording-silent", WARN,
            f"{d.key}: {secs:.1f}s recording dropped as silent "
            f"(avg_rms={rms_s}, device={dev or 'unknown'})",
            ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            detail={"secs": secs, "device": dev, "avg_rms": rms,
                    "nonzero_ratio": _nonzero_ratio(d)},
        )


@invariant(
    "device-changed-mid-recording", WARN,
    "The OS default input does not move mid recording",
    "docs/tracing.md: 'An already-open cpal stream does not follow it, so it "
    "keeps reading from a device that has stopped producing audio.'",
)
def check_device_changed(corpus: Corpus):
    for e in corpus.events:
        if e.stage == "capture.stop" and e.get("device_changed") is True:
            yield Finding(
                "device-changed-mid-recording", WARN,
                f"default input moved from {e.get('device')} to "
                f"{e.get('default_now')} while recording",
                ts=str(e.ts), index=e.index, detail=dict(e.payload),
            )


# ------------------------------------------------------- the text chain


@invariant(
    "text-emptied", ERROR,
    "No text stage reduces the transcription to zero characters",
    "The hallucination-filter defect. docs/tracing.md: 'A to.chars of 0 names "
    "the stage that emptied the transcription.' The user spoke, Whisper "
    "heard, and a transformation deleted it.",
)
def check_text_emptied(corpus: Corpus):
    for d in corpus.dictations:
        for e in _ordered(d):
            frm, to = e.get("from", "chars"), e.get("to", "chars")
            if frm and to == 0:
                yield Finding(
                    "text-emptied", ERROR,
                    f"{d.key}: {e.stage} emptied {frm} characters",
                    ts=str(e.ts), dictation=d.key, index=e.index,
                    detail={"stage": e.stage, "from_chars": frm},
                )


@invariant(
    "hallucination-dropped-speech", ERROR,
    "The hallucination filter does not drop what looks like speech",
    f"An abort for 'hallucination' where the audio was at least "
    f"{HALLUCINATION_MIN_SECS:.0f}s, its avg_rms was above the silence floor, "
    f"and Whisper returned at least {HALLUCINATION_MIN_CHARS} characters. "
    "That combination is the shape of real speech being deleted, which is "
    "one of the seven defects and the one users notice least.",
)
def check_hallucination(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "hallucination":
            continue
        dur = d.stage("audio.duration")
        sig = _signal(d)
        wr = d.stage("whisper.response")
        secs = dur.get("secs") if dur else None
        rms = sig.get("avg_rms") if sig else None
        floor = sig.get("floor") if sig else None
        chars = (wr.get("chars") if wr else None) or d.finish.get("chars")
        if None in (secs, rms, floor, chars):
            continue
        if (secs >= HALLUCINATION_MIN_SECS and rms > floor
                and chars >= HALLUCINATION_MIN_CHARS):
            # Which predicate deleted it, where the trace says. Quoted, never
            # tested: the verdict above is decided by the three measurements,
            # and a generation of the trace without these fields must produce
            # the same finding as one with them.
            f = d.stage("filter.hallucination")
            rule = f.get(HALLUCINATION_RULE_KEY) if f else None
            corroborated = (f.get(HALLUCINATION_CORROBORATED_KEY)
                            if f else None)
            why = f" by rule '{rule}'" if rule else ""
            if rule == HALLUCINATION_LOOP_KEY and corroborated is False:
                # The filter is documented not to delete these any more, so a
                # dictation aborted on one is worth saying out loud.
                why += " on an UNCORROBORATED repetition loop"
            elif corroborated is True:
                why += " (loop corroborated by the known-phrase list)"
            yield Finding(
                "hallucination-dropped-speech", ERROR,
                f"{d.key}: {chars} chars from {secs:.1f}s at avg_rms "
                f"{rms:.4f} (floor {floor:.4f}) dropped as hallucination"
                f"{why}",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
                detail={"chars": chars, "secs": secs, "avg_rms": rms,
                        "floor": floor, "peak": sig.get("peak"),
                        "rule": rule, "corroborated": corroborated},
            )


@invariant(
    "text-chain-broken", WARN,
    "Each text stage starts from the previous stage's output digest",
    "docs/tracing.md: 'matching digests across two stages mean the text "
    "passed through untouched'. A from.sha8 that does not match the previous "
    "to.sha8 means something rewrote the text between two traced stages.",
)
def check_text_chain(corpus: Corpus):
    for d in corpus.dictations:
        prev = None
        prev_stage = None
        for e in _ordered(d):
            frm = e.get("from", "sha8")
            to = e.get("to", "sha8")
            if frm is None and to is None:
                continue
            if prev and frm and frm != prev:
                yield Finding(
                    "text-chain-broken", WARN,
                    f"{d.key}: {e.stage} starts from {frm} but "
                    f"{prev_stage} ended at {prev}",
                    ts=str(e.ts), dictation=d.key, index=e.index,
                    detail={"stage": e.stage, "from": frm, "prev_to": prev},
                )
            if to:
                prev, prev_stage = to, e.stage


# ------------------------------------------------- remote models & quota


@invariant(
    "polish-outage", ERROR,
    "Polish does not fail repeatedly against the same model",
    "The decommissioned-model defect. polish.outage climbing against one "
    "model name is a model that has stopped answering — the failure is in the "
    "model id, not the network. WHAT THIS USED TO CLAIM: it reported an ERROR "
    f"for any polish.outage record at all. That fired on a peak "
    f"consecutive_failures of 1 — a single failed call, which the next call "
    f"recovered from — and reported it in the same breath as a "
    f"decommissioned model that failed 25 times in a row. The word in the "
    f"title is 'repeatedly': below {POLISH_OUTAGE_STREAK} consecutive "
    "failures against one model this is a warning, because one or two "
    "failures against a live model is a network, and burying a real outage "
    "under them is how a checker stops being read.",
)
def check_polish_outage(corpus: Corpus):
    runs: dict[str, list[Event]] = {}
    for e in corpus.events:
        if e.stage == "polish.outage":
            runs.setdefault(e.get("model") or "?", []).append(e)
    for model, evs in runs.items():
        worst = max(evs, key=lambda e: e.get("consecutive_failures") or 0)
        peak = worst.get("consecutive_failures") or 0
        sev = ERROR if peak >= POLISH_OUTAGE_STREAK else WARN
        tail = ("" if sev == ERROR else
                f" — a peak of {peak} is not an outage; one failed call that "
                f"the next call recovered from looks exactly like this")
        yield Finding(
            "polish-outage", sev,
            f"polish failed against '{model}' {len(evs)} times "
            f"(peak streak {peak}), "
            f"{evs[0].ts.date()} to {evs[-1].ts.date()}{tail}",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"model": model, "events": len(evs),
                    "peak_streak": peak},
        )


# THE SPLIT. `remote-call-failed` used to be both of the two checks below,
# under one id, at one severity.
#
# They are not one check. They share only the word "remote":
#
#                        whisper_error abort        non-200 polish.attempt
#   attribution          the dictation's own id     no trace id at all;
#                        on dictation.finish        window containment only
#   scope                one named dictation        a standalone record
#   consequence          the user lost their        the pipeline pastes the
#                        words; nothing pasted      unpolished text
#   who else emits it    nothing — a test binary    the golden test suite,
#                        writes no dictation of     46 times on 2026-09-02
#                        the user's
#
# Merging them is what let 46 test-harness 429s sit in the same block as a
# real 403 invalid-key abort, at the same grade, sorted only by timestamp. A
# reader scanning that block had no way to see that one of the twelve rows was
# a user losing a dictation. Two ids, two severities, two evidence standards.
# The `--only` surface changes accordingly: `remote-call-failed` no longer
# exists, and `--only whisper-call-failed --only polish-call-failed` is the
# old behaviour spelled out.


@invariant(
    "whisper-call-failed", ERROR,
    "Transcription does not fail against the remote model",
    "A dictation aborted with reason `whisper_error`: the user pressed, "
    "spoke, and got nothing, because the transcription call failed. Reported "
    "with the status code and error category, because 403 (bad key), 429 "
    "(quota) and 404 (decommissioned model) are three different bugs that "
    "look identical to the user. ERROR, and the evidence supports it without "
    "qualification: the record is `dictation.finish`, it carries the "
    "dictation's own id, and its `outcome` is the app's own statement that "
    "this dictation produced nothing. There is no window arithmetic here and "
    "no co-tenant reading — a test binary does not abort a dictation of the "
    "user's, because it never started one. WHY THIS IS ITS OWN CHECK: it used "
    "to be half of `remote-call-failed`, sharing an id and a WARN with "
    "non-200 polish.attempt records. On the 2026-09-02 corpus that put one "
    "real 403 invalid-key abort in the same block as ten quota warnings, 46 "
    "of whose siblings were a cargo test run. The two halves have different "
    "attribution, different blast radius and different readers; the only "
    "thing they had in common was the word 'remote'.",
)
def check_whisper_failed(corpus: Corpus):
    for d in corpus.dictations:
        if d.reason != "whisper_error" or d.finish is None:
            continue
        status = d.finish.get("status_code")
        category = d.finish.get("error_category")
        # Rule 2: a schema generation that recorded neither still gets a
        # finding — the abort itself is the evidence — but the summary says
        # so rather than printing "None (None)" as if it were a reading.
        if status is None and category is None:
            detail_txt = "no status_code or error_category recorded"
        elif status is None:
            detail_txt = f"{category} (no status code recorded)"
        else:
            detail_txt = f"{status} ({category or 'no category recorded'})"
        yield Finding(
            "whisper-call-failed", ERROR,
            f"{d.key}: transcription failed and the dictation was aborted — "
            f"{detail_txt}",
            ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
            detail={"status_code": status, "error_category": category,
                    "whisper_ms": d.finish.get("whisper_ms"),
                    "attribution": "trace_id"},
        )


@invariant(
    "polish-call-failed", WARN,
    "Polish calls return success",
    "A `polish.attempt` with a non-200 status. WARN at most, never ERROR, "
    "and the reasons are stacked: the record carries NO trace id, so it can "
    "only be attributed by containment in a dictation's polish window "
    "(`polish.decision` -> `polish`); polish has a working fallback, so a "
    "failed call costs the user their polish and not their words — the "
    "pipeline pastes the unpolished text and the dictation still lands; and "
    "this is the one stage in the whole vocabulary that the project's own "
    "test suite provably emits in bulk, 46 times in one 2026-09-02 window. "
    "Graded by attribution, three ways. (1) The record names a process that "
    "never emitted `app.launched` — a test binary or a dev build — INFO, "
    "aggregated, and now stated positively rather than hedged. (2) It falls "
    "inside a dictation's polish window: that dictation was waiting on this "
    "call and got unpolished text, WARN. (3) It falls outside every window "
    "and does not say who wrote it: INFO, aggregated per status per model per "
    "day, saying only that no dictation this analyser can see was waiting — "
    "never 'a test run', because without `proc` the log cannot support that "
    "sentence. WHAT THIS USED TO CLAIM: as half of `remote-call-failed` it "
    "reported every non-200 as a warning about the app, which on 2026-09-02 "
    "produced 54 warnings about a quota wall, 46 of them the golden suite in "
    "src-tauri/tests exhausting the API tier from a cargo test run sharing "
    "this log directory. That was reported to the maintainer as a live "
    "incident and had to be retracted.",
)
def check_polish_failed(corpus: Corpus):
    # (status, model, day) -> events, for the two unattributed aggregations.
    stray: dict[tuple, list[Event]] = {}
    harness: dict[tuple, list[Event]] = {}
    for e in corpus.events:
        if e.stage != "polish.attempt":
            continue
        st = e.get("status")
        if st is None or st == 200:
            continue
        # Process identity first, where the writer supplies it: it is a
        # lookup, and containment is an inference. A record whose process
        # never launched the app was not serving a user, whatever window it
        # happens to sit inside.
        if corpus.origin_of(e.proc) == ORIGIN_HARNESS:
            harness.setdefault(
                (str(e.ts.date()), st, e.get("model"), e.proc), []).append(e)
            continue
        owner = _owning_dictation(corpus, e)
        if owner is None:
            stray.setdefault(
                (str(e.ts.date()), st, e.get("model")), []).append(e)
            continue
        yield Finding(
            "polish-call-failed", WARN,
            f"{owner.key}: polish.attempt returned {st} from "
            f"{e.get('model')} — the dictation was waiting on this call and "
            f"pasted unpolished text",
            ts=str(e.ts), dictation=owner.key, index=e.index,
            detail=dict(e.payload, attributed_to=owner.key,
                        attribution="polish_window"),
            origin=corpus.origin_of(e.proc),
        )
    for (day, st, model, proc), evs in sorted(
            harness.items(), key=lambda kv: str(kv[0])):
        yield Finding(
            "polish-call-failed", INFO,
            f"{len(evs)} polish.attempt record(s) returned {st} from {model} "
            f"on {day}, written by process '{proc}', which emitted no "
            f"app.launched — a test binary or a dev build, not the app "
            f"serving a user. Not a finding about the product",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"status": st, "model": model, "day": day,
                    "count": len(evs), "proc": proc,
                    "attribution": "process_identity"},
            origin=ORIGIN_HARNESS,
        )
    for (day, st, model), evs in sorted(stray.items()):
        # Three different sentences, because the corpus supports three
        # different claims about the same shape.
        procs = sorted({e.proc for e in evs if e.proc})
        app_procs = [p for p in procs
                     if corpus.origin_of(p) == ORIGIN_APP]
        undecided = [p for p in procs if p not in app_procs]
        if app_procs and not undecided:
            tail = (f"written by app process(es) {', '.join(app_procs)}, "
                    f"which did emit an app.launched, so this is the app "
                    f"polishing outside any dictation this analyser can see "
                    f"— not a harness")
        elif procs:
            # A proc whose app.launched would have been rotated away. It is
            # named, and neither claim is made about it.
            tail = (f"written by process(es) {', '.join(procs)}, whose "
                    f"records begin at the top of this corpus, so a "
                    f"rotated-away app.launched and a test binary are the "
                    f"same shape here and neither is claimed")
        else:
            tail = ("These records do not say which process wrote them "
                    "(pre-`proc` trace), so this cannot be shown to be a test "
                    "run; what it does establish is that no dictation this "
                    "analyser can see was waiting on them")
        yield Finding(
            "polish-call-failed", INFO,
            f"{len(evs)} polish.attempt record(s) returned {st} from {model} "
            f"on {day} with no dictation making the call — outside every "
            f"traced dictation's polish window. {tail}",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"status": st, "model": model, "day": day,
                    "count": len(evs), "attribution": "unattributed",
                    "procs": sorted(procs)},
        )


@invariant(
    "polish-quota-exhausted", WARN,
    "Polish is not skipped for lack of quota",
    "The exhausted-quota defect. quota_ok:false silently downgrades the "
    "output — the dictation still pastes, so nothing tells the user the "
    "text they got is the unpolished one.",
)
def check_quota(corpus: Corpus):
    hits = [d for d in corpus.dictations
            if (d.stage("polish.decision")
                and d.stage("polish.decision").get("quota_ok") is False)]
    for d in hits:
        e = d.stage("polish.decision")
        yield Finding(
            "polish-quota-exhausted", WARN,
            f"{d.key}: polish skipped, quota exhausted",
            ts=str(e.ts), dictation=d.key, index=_anchor(d),
        )


# ------------------------------------------------------ the input layer


@invariant(
    "tap-rearm-streak", ERROR,
    "The event-tap re-arm streak does not climb",
    "The dead-event-tap defect. docs/tracing.md: 'A climbing streak means "
    f"re-arming is not working.' Every Fn press between the disable and a "
    f"successful re-arm was lost. Graded by how far the streak got. Reaching "
    f"{TAP_STREAK_LIMIT} is the point at which the Rust side stops trying "
    f"CGEventTapEnable and rebuilds the tap — so a streak that touches "
    f"{TAP_STREAK_LIMIT} and stops is the escalation firing as designed, and "
    f"is a warning. A streak past {TAP_STREAK_ESCALATED} means the rebuild "
    f"did not help either, which is the defect. WHAT THIS USED TO CLAIM: "
    f"every streak of {TAP_STREAK_LIMIT} or more was an ERROR, which on the "
    "real corpus reported thirteen, eleven of them a streak of exactly 5 or 6 "
    "in a two-minute session during a relaunch storm — burying the two "
    "sessions whose streaks reached 37 and 47.",
)
def check_tap_streak(corpus: Corpus):
    worst: dict[int, Event] = {}
    for e in corpus.events:
        if e.stage != "hotkey.tap_rearmed":
            continue
        streak = e.get("streak")
        if streak is None or streak < TAP_STREAK_LIMIT:
            continue
        cur = worst.get(e.session)
        if cur is None or (streak > (cur.get("streak") or 0)):
            worst[e.session] = e
    for sess, e in sorted(worst.items()):
        streak = e.get("streak") or 0
        rebuilds = sum(1 for f in corpus.events
                       if f.session == sess and f.stage == "hotkey.tap_rebuilt")
        if streak >= TAP_STREAK_ESCALATED:
            yield Finding(
                "tap-rearm-streak", ERROR,
                f"session {sess}: event tap re-arm streak reached "
                f"{streak} (limit {TAP_STREAK_LIMIT}, {rebuilds} rebuilds) — "
                f"CGEventTapEnable stopped restoring the tap and rebuilding "
                f"did not recover it",
                ts=str(e.ts), index=e.index,
                detail={"streak": streak, "reason": e.get("reason"),
                        "rebuilds": rebuilds},
            )
        else:
            # The streak resets on every rebuild, so in a session that
            # rebuilds hundreds of times the streak CANNOT climb — it is
            # held down by the very escalation it is supposed to measure.
            # Saying "the escalation firing once is the design working" to a
            # reader looking at 444 rebuilds would be false, and false in the
            # reassuring direction. The peak streak is only readable as a
            # health signal while the rebuild count is small; past that the
            # finding says which number to read instead, and
            # tap-flapping-without-input is the check that grades it.
            churn = rebuilds > TAP_STREAK_LIMIT
            tail = (
                f"The escalation firing once is the design working; read it "
                f"as a symptom, not an incident"
                if not churn else
                f"but this session rebuilt the tap {rebuilds} times, and "
                f"every rebuild resets the streak — so the streak is low "
                f"because the escalation kept firing, not because the tap "
                f"was healthy. Read the rebuild count, not the streak; "
                f"tap-flapping-without-input and tap-abandoned are the "
                f"checks that grade this session"
            )
            yield Finding(
                "tap-rearm-streak", WARN,
                f"session {sess}: event tap re-arm streak reached {streak}, "
                f"the point at which the Rust side gives up on "
                f"CGEventTapEnable and rebuilds ({rebuilds} rebuilds in this "
                f"session). {tail}",
                ts=str(e.ts), index=e.index,
                detail={"streak": streak, "reason": e.get("reason"),
                        "rebuilds": rebuilds,
                        "streak_masked_by_rebuilds": churn},
            )


@invariant(
    "tap-abandoned", ERROR,
    "The input layer never gives up on the event tap",
    "hotkey.tap_abandoned means re-arming failed, rebuilding failed, and the "
    "code stopped trying. From that line until the app is relaunched, the Fn "
    "key does nothing at all and the trace records no further presses.",
)
def check_tap_abandoned(corpus: Corpus):
    for e in corpus.events:
        if e.stage == "hotkey.tap_abandoned":
            after = [f for f in corpus.events
                     if f.index > e.index and f.session == e.session
                     and f.stage == "hotkey.press"]
            # Graded ERROR on the abandonment itself, not on the press count.
            # The count is reported but deliberately not used as evidence:
            # after tap_abandoned the tap is the thing that would have
            # RECORDED a press, so zero presses afterwards is exactly what
            # both "the user pressed and nothing was seen" and "the user
            # stopped using the app" look like. Reading zero as reassurance
            # would be reading the absence of the instrument as the absence
            # of the failure.
            yield Finding(
                "tap-abandoned", ERROR,
                f"session {e.session}: event tap abandoned after "
                f"{e.get('rebuilds')} rebuilds — the Fn key does nothing "
                f"until the app is relaunched. {len(after)} hotkey press(es) "
                f"recorded afterwards in this session, which is not evidence "
                f"either way: the abandoned tap is what would have recorded "
                f"them",
                ts=str(e.ts), index=e.index,
                detail={"rebuilds": e.get("rebuilds"),
                        "presses_after": len(after)},
            )


@invariant(
    "tap-flapping-without-input", ERROR,
    "A session that re-arms the tap repeatedly still sees hotkey presses",
    f"A session with {TAP_FLAP_WITHOUT_INPUT}+ re-arms and zero hotkey.press "
    "recorded. Over a short session this can simply mean the user pressed "
    "nothing; over a long one it is the shape of the original complaint — "
    "'I pressed and nothing happened' — with the tap flapping as the "
    f"mechanism. The check already said that in prose and then graded every "
    f"case as an ERROR anyway, which on the real corpus reported seven, five "
    f"of them sessions lasting one to three minutes during a relaunch storm "
    f"where 'the user pressed nothing' is the obvious reading. Only a session "
    f"of {TAP_FLAP_MINUTES:.0f} minutes or more is graded as an error; below "
    "that the finding is a warning and still carries the duration, because "
    "the duration is the evidence.",
)
def check_tap_flap(corpus: Corpus):
    for s in corpus.sessions:
        rearms = sum(1 for e in s.events if e.stage == "hotkey.tap_rearmed")
        presses = sum(1 for e in s.events if e.stage == "hotkey.press")
        if rearms >= TAP_FLAP_WITHOUT_INPUT and presses == 0:
            span = (s.events[-1].ts - s.events[0].ts).total_seconds() / 60
            rebuilds = sum(1 for e in s.events
                           if e.stage == "hotkey.tap_rebuilt")
            long_enough = span >= TAP_FLAP_MINUTES
            yield Finding(
                "tap-flapping-without-input",
                ERROR if long_enough else WARN,
                f"session {s.index}: {rearms} re-arms and {rebuilds} "
                f"rebuilds over {span:.0f} min, and not one hotkey.press "
                f"recorded in the whole session"
                + ("" if long_enough else
                   f" — under {TAP_FLAP_MINUTES:.0f} min this is equally "
                   f"consistent with nobody pressing anything"),
                ts=str(s.events[0].ts), index=s.events[0].index,
                detail={"rearms": rearms, "rebuilds": rebuilds,
                        "minutes": round(span), "dictations": 0},
            )


@invariant(
    "stale-fn-latched", WARN,
    "The Globe key does not latch held",
    "docs/tracing.md: 'Keystrokes injected before this were being routed to "
    "the Globe shortcut layer.' Every character typed while latched went "
    "somewhere else.",
)
def check_stale_fn(corpus: Corpus):
    for e in corpus.events:
        if e.stage == "hotkey.stale_fn_cleared":
            yield Finding(
                "stale-fn-latched", WARN,
                "Globe key was latched held and had to be forced down",
                ts=str(e.ts), index=e.index, detail=dict(e.payload),
            )


@invariant(
    "hotkey-event-dropped", WARN,
    "No hotkey event is discarded under lock contention",
    "docs/tracing.md: 'The press happened; nothing came of it.'",
)
def check_hotkey_dropped(corpus: Corpus):
    for e in corpus.events:
        if e.stage == "hotkey.event_dropped":
            yield Finding(
                "hotkey-event-dropped", WARN,
                "a hotkey event was discarded while the state lock was held",
                ts=str(e.ts), index=e.index, detail=dict(e.payload),
            )


# ------------------------------------------------------- paste landing


@invariant(
    "paste-swallowed", ERROR,
    "Text that was injected actually landed",
    "docs/tracing.md: 'ax_readable:true with changed:false — the keystrokes "
    "were swallowed.' This is the original complaint made visible: the app "
    "reports outcome:pasted and the target field is unchanged. Two evidence "
    "standards, because the trace has two vocabularies for the same question "
    "and the corpus contains both. Where paste.verify carries an explicit "
    f"verdict — the {PASTE_VERDICT_SWALLOWED}/{PASTE_VERDICT_OBSERVED}/"
    f"{PASTE_VERDICT_UNVERIFIED} vocabulary — that verdict decides, because "
    "the writer knows things the analyser is guessing at: only "
    f"'{PASTE_VERDICT_SWALLOWED}' is an ERROR, and "
    f"'{PASTE_VERDICT_UNVERIFIED}' is a WARN that says the landing is "
    "unproven rather than that it failed. Where the field is absent (every "
    "record written before the verdict existed) the check falls back to the "
    "ax_readable/changed pair, which is the same question asked with less "
    "information. A verdict slug this file has not been taught about is not "
    "a violation — rule 1 — and reaches the reader through "
    "`outcome-undocumented` and the unknown-stage list instead.",
)
def check_paste_swallowed(corpus: Corpus):
    for d in corpus.dictations:
        v = d.stage("paste.verify")
        if not v:
            continue

        verdict = v.get("verdict")
        if verdict is not None:
            if verdict not in (PASTE_VERDICT_SWALLOWED,
                               PASTE_VERDICT_UNVERIFIED):
                continue  # observed, or vocabulary this file does not know
            sev = (ERROR if verdict == PASTE_VERDICT_SWALLOWED else WARN)
            gloss = ("target unchanged after injection"
                     if verdict == PASTE_VERDICT_SWALLOWED else
                     "the target could not be read, so the landing is "
                     "unproven — this is not evidence the text was lost")
        elif v.get("ax_readable") is True and v.get("changed") is False:
            # The pre-verdict shape. Same claim, weaker instrument.
            sev, gloss = ERROR, "target unchanged"
        else:
            continue

        mods = _modifiers_near(corpus, d)
        yield Finding(
            "paste-swallowed", sev,
            f"{d.key}: {v.get('expected_chars')} chars injected, {gloss} "
            f"(before={v.get('before_chars')}, "
            f"after={v.get('after_chars')}, "
            f"settled_ms={v.get('settled_ms')})"
            + (f"; modifier held: {mods.get('held')}" if mods else ""),
            ts=str(v.ts), dictation=d.key, index=v.index,
            detail={"verify": dict(v.payload), "modifiers": mods,
                    "verdict": verdict or "(pre-verdict trace)"},
        )


def _modifiers_near(corpus: Corpus, d: Dictation):
    """paste.modifiers between this dictation's decision and its result.

    The line carries no trace id (it is a standalone event), so the only way
    to attribute it is positional — which is what docs/tracing.md's advice
    'look for a paste.modifiers line immediately before it' means in practice.
    """
    dec = d.stage("paste.decision")
    res = d.stage("paste.result")
    if not dec:
        return None
    hi = res.index if res else dec.index + 12
    for e in corpus.events[dec.index:hi + 1]:
        if e.stage == "paste.modifiers":
            return dict(e.payload)
    return None


@invariant(
    "paste-modifier-held", WARN,
    "No modifier is physically held when keystrokes are injected",
    "The mechanism the original complaint was diagnosed as and never "
    "observed: injecting while Fn/Globe is down routes the synthetic events "
    "into the Globe shortcut layer instead of the text field. Attributed "
    "positionally, because paste.modifiers carries no trace id.",
)
def check_paste_modifiers(corpus: Corpus):
    for d in corpus.dictations:
        mods = _modifiers_near(corpus, d)
        if mods:
            yield Finding(
                "paste-modifier-held", WARN,
                f"{d.key}: '{mods.get('held')}' held at injection time "
                f"(bits {mods.get('bits')})",
                ts=str(d.stage("paste.decision").ts), dictation=d.key,
                index=d.stage("paste.decision").index, detail=mods,
            )


@invariant(
    "paste-partial", WARN,
    "All the characters arrive",
    "delta_chars short of expected_chars. Only checked when before_chars is "
    "0: typing over a selection legitimately shortens the field, so a "
    "non-empty target cannot distinguish a partial paste from a replacement.",
)
def check_paste_partial(corpus: Corpus):
    for d in corpus.dictations:
        v = d.stage("paste.verify")
        if not v or v.get("changed") is not True:
            continue
        before, delta, exp = (v.get("before_chars"), v.get("delta_chars"),
                              v.get("expected_chars"))
        if before != 0 or delta is None or not exp:
            continue
        if delta < exp:
            yield Finding(
                "paste-partial", WARN,
                f"{d.key}: {delta} of {exp} characters landed in an empty "
                f"field",
                ts=str(v.ts), dictation=d.key, index=v.index,
                detail=dict(v.payload),
            )


# ------------------------------------------------------------- latency


@invariant(
    "bookkeeping-stall", ERROR,
    "The post-paste bookkeeping stages take milliseconds",
    "The keychain defect's shape. docs/tracing.md: everything after "
    "paste.result is 'trivial synchronous bookkeeping ... none of it can take "
    "seconds, let alone minutes'. The check reports the gap and the two "
    "things that tell the causes apart, and does not pick between them for "
    "you: whether a hotkey.timer_stall covers the window (the process was "
    "descheduled) and whether a keychain.slow lands inside it (a blocking "
    "securityd read on the critical path). If neither explains it, say so "
    "and go read the window.",
)
def check_bookkeeping_stall(corpus: Corpus):
    for d in corpus.dictations:
        ev = [e for e in _ordered(d) if e.elapsed_ms is not None]
        for a, b in zip(ev, ev[1:]):
            if a.stage not in BOOKKEEPING_STAGES or b.stage not in BOOKKEEPING_STAGES:
                continue
            gap = b.elapsed_ms - a.elapsed_ms
            if gap < BOOKKEEPING_GAP_MS:
                continue
            keychain = _keychain_slow_in(corpus, a, b)
            quiet = _events_between(corpus, a, b)
            stall = _stall_covering(corpus, a, b)
            if keychain is not None:
                yield Finding(
                    "keychain-on-critical-path", ERROR,
                    f"{d.key}: {gap} ms between {a.stage} and {b.stage}, "
                    f"containing keychain.slow "
                    f"{keychain.get('op')}({keychain.get('account')}) "
                    f"= {keychain.get('ms')} ms",
                    ts=str(a.ts), dictation=d.key, index=a.index,
                    detail={"gap_ms": gap, "from": a.stage, "to": b.stage,
                            "keychain_ms": keychain.get("ms"),
                            "account": keychain.get("account")},
                )
            elif stall is not None:
                yield Finding(
                    "process-suspended", WARN,
                    f"{d.key}: {gap} ms between {a.stage} and {b.stage}, "
                    f"covered by hotkey.timer_stall gap_ms="
                    f"{stall.get('gap_ms')} — the process was suspended, not "
                    f"the step slow",
                    ts=str(a.ts), dictation=d.key, index=a.index,
                    detail={"gap_ms": gap, "from": a.stage, "to": b.stage,
                            "stall_gap_ms": stall.get("gap_ms")},
                )
            else:
                # "No keychain.slow inside it" is only evidence of absence
                # where keychain.* records exist at all. The corpus spans
                # builds from before docs/trace-api.md added that family, and
                # in the file this gap lives in there may be none — in which
                # case the discriminator was blind, and the finding has to
                # say so rather than let the reader infer it looked and found
                # nothing.
                keychain_instrumented = any(
                    f.stage.startswith("keychain.")
                    and f.source.split(":")[0] == a.source.split(":")[0]
                    for f in corpus.events)
                blind = ("" if keychain_instrumented else
                         "; no keychain.* record appears anywhere in "
                         f"{a.source.split(':')[0]}, so 'no keychain.slow "
                         "inside it' is not evidence of absence")
                if gap >= BOOKKEEPING_IMPLAUSIBLE_MS:
                    yield Finding(
                        "bookkeeping-stall", WARN,
                        f"{d.key}: {gap} ms ({gap / 60000:.1f} min) between "
                        f"{a.stage} and {b.stage}. No synchronous "
                        f"bookkeeping step takes a minute, so this is a "
                        f"process that stopped running rather than a step "
                        f"that was slow — a slept machine, a descheduled "
                        f"process, or a moved clock. No timer_stall covers "
                        f"it, which rules out only the one of those three "
                        f"the trace can witness; {quiet} other events were "
                        f"logged during the window{blind}",
                        ts=str(a.ts), dictation=d.key, index=a.index,
                        detail={"gap_ms": gap, "from": a.stage, "to": b.stage,
                                "events_during": quiet,
                                "covered_by_stall": False,
                                "keychain_instrumented": keychain_instrumented,
                                "grade_reason": "longer than any synchronous "
                                                "step can be"},
                    )
                else:
                    yield Finding(
                        "bookkeeping-stall", ERROR,
                        f"{d.key}: {gap} ms ({gap / 1000:.1f}s) between "
                        f"{a.stage} and {b.stage}; no timer_stall covers it, "
                        f"no keychain.slow inside it, {quiet} other events "
                        f"logged during the window{blind}",
                        ts=str(a.ts), dictation=d.key, index=a.index,
                        detail={"gap_ms": gap, "from": a.stage, "to": b.stage,
                                "events_during": quiet,
                                "covered_by_stall": False,
                                "keychain_instrumented": keychain_instrumented},
                    )


@invariant(
    "keychain-on-critical-path", ERROR,
    "No keychain read blocks a dictation",
    "The keychain defect, now directly instrumented: docs/trace-api.md's "
    "keychain.slow names the account and the op. securityd can block for tens "
    "of seconds, and the account it blocks on (usage_hmac_secret) is read by "
    "usage.recorded — a step docs/tracing.md calls trivial and synchronous. "
    "WHAT THIS USED TO CLAIM, and why it was wrong: it reported every "
    "keychain.slow record as an ERROR that had 'blocked a dictation'. On the "
    "first real corpus that produced fourteen ERRORs, up to 397 seconds each, "
    "none of which blocked anything: all fourteen carried no dictation id, "
    "and no dictation in the corpus ran longer than 3,455 ms, so a 397-second "
    "block inside one is arithmetically impossible. They came from cargo test "
    "runs appending to the same log directory. The check had inferred "
    "causation from the records being NEAR dictations in the file. A "
    "keychain read is on the critical path only when it is ATTRIBUTABLE: the "
    "record carries a dictation id, or the read demonstrably began and ended "
    "inside one dictation's own start-to-last-stage window (a synchronous "
    "call made by that dictation cannot start before it or end after it), or "
    "it lands inside a gap between two of that dictation's own stages. A read "
    "that overlaps a dictation but spills past its window is reported as a "
    "warning WITH the arithmetic, because a read longer than the dictation it "
    "straddles is positive evidence that two processes were writing this file "
    "— not evidence that a user waited. A read that overlaps nothing is "
    "information: a slow securityd call happened, and nobody was waiting on "
    "it.",
)
def check_keychain(corpus: Corpus):
    # Unattributed reads are aggregated, not listed. Twelve consecutive rows
    # each saying "this blocked nothing" is the same failure of proportion
    # the fourteen ERRORs were, one severity down: a reader who has to scroll
    # past a dozen non-events to reach a finding stops reading the section.
    # One row per day per account per op, carrying the count and the worst
    # time, keeps every number a reader could act on.
    stray: dict[tuple, list[Event]] = {}

    for e in corpus.events:
        if e.stage != "keychain.slow":
            continue
        ms = e.get("ms")
        who = f"{e.get('op')}({e.get('account')})"

        if e.tid is not None:
            # The strongest attribution there is: the writer put the
            # dictation's own id on the record.
            yield Finding(
                "keychain-on-critical-path", ERROR,
                f"keychain {who} blocked dictation {e.tid} for {ms} ms",
                ts=str(e.ts), dictation=e.tid, index=e.index,
                detail=dict(e.payload),
            )
            continue

        verdict, d, overlap = _attribute_timed(corpus, e, ms)

        if verdict == "unknown":
            # No `ms`: nothing to place. Rule 2 — abstain.
            continue

        if verdict == "inside":
            yield Finding(
                "keychain-on-critical-path", ERROR,
                f"keychain {who} blocked for {ms} ms entirely inside "
                f"{d.key}, which lived {_life_ms(d):.0f} ms — the read is on "
                f"that dictation's critical path",
                ts=str(e.ts), dictation=d.key, index=e.index,
                detail=dict(e.payload, attribution="inside-dictation",
                            dictation_ms=round(_life_ms(d))),
            )
        elif verdict == "spans":
            life = _life_ms(d)
            yield Finding(
                "keychain-on-critical-path", WARN,
                f"keychain {who} took {ms} ms and overlaps {d.key} for "
                f"{overlap} ms, but it started before that dictation began or "
                f"ended after it finished — {d.key} lived {life:.0f} ms in "
                f"total, so it cannot have made this call and waited for it. "
                f"Two writers, not a blocked user",
                ts=str(e.ts), index=e.index,
                detail=dict(e.payload, attribution="spans-dictation",
                            overlap_ms=overlap, dictation=d.key,
                            dictation_ms=round(life)),
            )
        else:
            stray.setdefault(
                (str(e.ts.date()), e.get("op"), e.get("account")),
                []).append(e)

    for (day, op, account), evs in sorted(stray.items()):
        worst = max(evs, key=lambda x: x.get("ms") or 0)
        yield Finding(
            "keychain-on-critical-path", INFO,
            f"{len(evs)} slow keychain {op}({account}) read(s) on {day}, "
            f"worst {worst.get('ms')} ms — none of them had a trace id and "
            f"no dictation was in flight at any point during any of them, so "
            f"nothing a user was waiting for was blocked. A slow securityd "
            f"call is still worth knowing about; it is not an incident",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"day": day, "op": op, "account": account,
                    "count": len(evs), "worst_ms": worst.get("ms"),
                    "ms": [x.get("ms") for x in evs],
                    "attribution": "unattributed"},
        )


@invariant(
    "keychain-not-single-flighted", ERROR,
    "One uncoalesced keychain read per account per session",
    "The regression check for R1's single-flight keychain, written to catch "
    "the shape the OLD check-then-act produced rather than the one the fix "
    "produces. Both keychain caches live for the whole process and "
    "keychain::read_once now performs at most one call however many callers "
    "arrive together, so a process pays each account exactly once and at most "
    "one timed read per (session, account, op) can exist. The defect looked "
    "like this: one process, one lifetime cache, and EIGHT sequential "
    "secret_read lines for usage_hmac_secret — 62,304 / 13,612 / 397,600 / "
    "97,407 / 157 / 56,037 / 106 / 86 ms. Every one of those callers checked "
    "the cache, released the lock, and then made its own unbounded securityd "
    "call. lock_wait_ms is the discriminator and it is used as evidence, not "
    "as a gate: a read that waited behind someone else's (lock_wait_ms > 0) "
    "is coalescing working and is not counted, while a read with "
    "lock_wait_ms == 0 — or without the field at all, which is a trace from "
    "before the fix — did its own call and is. Two or more of those for one "
    "account in one session is the regression. Caveat worth reading before "
    "calling a hit a bug: a session here is the span between app.launched "
    "lines in ONE file, and a dev or test binary writing into the same "
    "ttp-trace.log contributes its own reads to that span with no launch "
    "line of its own — which is the same co-tenancy that causes the lost "
    "newline. Check the timestamps against a build before blaming the app.",
)
def check_keychain_single_flight(corpus: Corpus):
    groups: dict[tuple, list] = {}
    for e in corpus.events:
        if e.stage != "keychain.slow":
            continue
        op = e.get("op")
        account = e.get("account")
        if op is None or account is None:
            continue  # rule 2: a field we need is absent, so abstain
        if not str(op).endswith("read"):
            continue  # a write is not a read and does not single-flight
        if (e.get("lock_wait_ms") or 0) > 0:
            continue  # this caller waited for someone else's read: coalesced
        groups.setdefault((e.session, account, op), []).append(e)

    for (session, account, op), evs in groups.items():
        if len(evs) < 2:
            continue
        ladder = " / ".join(f"{e.get('ms')}" for e in evs[:10])
        if len(evs) > 10:
            ladder += " / ..."
        worst = max((e.get("ms") or 0) for e in evs)

        # The caveat this check was already written with, now decided by
        # evidence instead of left to the reader. If any read in the group
        # spans a whole dictation that finished in less time than the read
        # took, then during that read a dictation ran its own pipeline —
        # including the usage.recorded step that reads this very account —
        # and completed. One process with a lifetime cache and read_once
        # cannot do that. So at least two processes contributed reads to
        # this span, the group is not one process's ladder, and calling it a
        # regression would be the same over-claim the keychain check made.
        impossible = []
        for e in evs:
            verdict, d, _ = _attribute_timed(corpus, e, e.get("ms"))
            if (verdict == "spans" and d is not None
                    and (e.get("ms") or 0) > _life_ms(d)):
                impossible.append((e, d))

        if impossible:
            e, d = impossible[0]
            yield Finding(
                "keychain-not-single-flighted", WARN,
                f"session {session}: {len(evs)} uncoalesced {op}({account}) "
                f"reads — {ladder} ms (worst {worst / 1000.0:.1f}s) — but "
                f"they cannot all belong to one process: the {e.get('ms')} ms "
                f"read ending {e.ts} spans {d.key}, which started, ran "
                f"usage.recorded and finished inside it in "
                f"{_life_ms(d):.0f} ms. At least two writers shared this log "
                f"during this span, so this is not evidence the single-flight "
                f"regressed. Check the timestamps against a build.",
                ts=str(evs[0].ts), index=evs[0].index,
                detail={"session": session, "account": account, "op": op,
                        "reads": len(evs),
                        "ms": [e.get("ms") for e in evs],
                        "worst_ms": worst,
                        "co_tenant_proof": [
                            {"read_ms": x.get("ms"), "spanned": y.key,
                             "dictation_ms": round(_life_ms(y))}
                            for x, y in impossible[:4]],
                        },
            )
            continue

        yield Finding(
            "keychain-not-single-flighted", ERROR,
            f"session {session}: {len(evs)} uncoalesced {op}({account}) "
            f"reads, none of which waited on another — {ladder} ms "
            f"(worst {worst / 1000.0:.1f}s). A lifetime cache plus "
            f"read_once permits exactly one.",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"session": session, "account": account, "op": op,
                    "reads": len(evs),
                    "ms": [e.get("ms") for e in evs],
                    "worst_ms": worst},
        )


@invariant(
    "process-suspended", WARN,
    "The process is not descheduled during a dictation",
    "A hotkey.timer_stall whose skipped window overlaps a dictation in "
    "flight. docs/tracing.md says the activity assertion added in "
    "crate::activity should make this impossible, so 'one that still appears "
    "is worth investigating'.",
)
def check_stall_spanning_dictation(corpus: Corpus):
    stalls = [e for e in corpus.events if e.stage == "hotkey.timer_stall"
              and e.get("gap_ms")]
    for d in corpus.dictations:
        if not d.start or not d.finish:
            continue
        lo, hi = d.start.ts.timestamp(), d.finish.ts.timestamp()
        if hi - lo < 1.0:
            continue
        for s in stalls:
            end = s.ts.timestamp()
            start = end - (s.get("gap_ms") or 0) / 1000.0
            if start < hi and lo < end:
                yield Finding(
                    "process-suspended", WARN,
                    f"{d.key}: hotkey.timer_stall gap_ms={s.get('gap_ms')} "
                    f"overlaps a dictation that was in flight for "
                    f"{hi - lo:.1f}s",
                    ts=str(s.ts), dictation=d.key, index=s.index,
                    detail={"gap_ms": s.get("gap_ms"),
                            "dictation_secs": round(hi - lo, 1)},
                )


@invariant(
    "writer-newline-lost", ERROR,
    "Every trace record occupies its own line",
    "The lost-newline race, made visible instead of being silently repaired. "
    "logging::append_line used to issue two write_all calls — the record, "
    "then the newline — and two writes are not one append: a second writer "
    "landing between them produces one physical line carrying two records, "
    "and a matching blank line where the stray newline went. The corpus "
    "signature was exact: 20 merged records against exactly 20 blank lines. "
    "R1 fixed it by framing the record and its newline into one buffer and "
    "writing that. The parser recovers merged records — it splits on the JSON "
    "payload's true end — and skips blank lines, which is why this went "
    "unnoticed for weeks: the evidence was being repaired before anyone could "
    "read it. So the counts are reported unconditionally in the corpus "
    "summary, and any occurrence is a finding here. What a hit means: two "
    "writers were appending to this file concurrently. That is not "
    "necessarily two threads of one app — an installed build running "
    "alongside a dev build is the normal state on this machine, and a second "
    "process is precisely what the single writer thread cannot serialise. "
    "Read the dates on the merged lines against which builds were running "
    "before concluding the fix regressed.",
)
def check_writer_newline(corpus: Corpus):
    if not corpus.merged_records and not corpus.blank_lines:
        return
    by_source: dict[str, list[Event]] = {}
    for e in corpus.events:
        by_source.setdefault(e.source, []).append(e)
    shared = [evs for evs in by_source.values() if len(evs) > 1]
    shared.sort(key=lambda evs: evs[0].index)
    anchor = shared[0][0].index if shared else (
        corpus.events[0].index if corpus.events else -1)
    if anchor < 0:
        return
    first = shared[0][0] if shared else corpus.events[0]
    where = ", ".join(evs[0].source for evs in shared[:6])
    if len(shared) > 6:
        where += f", ... ({len(shared) - 6} more)"
    # The docstring above asks the reader to decide whether a hit is the app
    # racing itself or a second process. That question has an answer in the
    # data, so the check now gives it instead of delegating it: when both
    # records on a merged line carry the SAME dictation id, they were written
    # by two threads inside one process's pipeline — a second process has no
    # way to produce that dictation's id. When they carry different ids, or
    # one is standalone, co-tenancy is on the table. In the August/September
    # 2026 corpus the split is 34 same-dictation to 4 mixed, which settles it:
    # this is the app, not a dev build alongside it.
    same = mixed = 0
    for evs in shared:
        tids = {e.tid for e in evs}
        if len(tids) == 1 and None not in tids:
            same += 1
        else:
            mixed += 1
    yield Finding(
        "writer-newline-lost", ERROR,
        f"{corpus.merged_records} record(s) shared a physical line with "
        f"another and {corpus.blank_lines} blank line(s) were written — two "
        f"appends raced and one lost its newline. All records were recovered "
        f"by the parser; nothing was lost, and that is the problem. "
        f"{same} of the {len(shared)} merged lines carry two records of the "
        f"SAME dictation, which only threads inside one process can produce, "
        f"so this is the app racing itself and not a co-tenanting build "
        f"({mixed} mixed). First at {where}",
        ts=str(first.ts), index=anchor,
        detail={"merged_records": corpus.merged_records,
                "blank_lines": corpus.blank_lines,
                "merged_same_dictation": same,
                "merged_mixed": mixed,
                "merged_lines": [evs[0].source for evs in shared]},
    )


@invariant(
    "log-co-tenancy", INFO,
    "The analyser states what it cannot attribute, rather than resolving it",
    "Not a defect in the app: a standing statement of what this corpus can "
    "and cannot support, printed whenever the evidence for more than one "
    "writer is present. It exists because the analyser's first outing over "
    "the maintainer's real log produced two confident findings that were "
    "neither — fourteen keychain ERRORs claiming blocks of up to 397 seconds "
    "inside dictations that never ran longer than 3.5 seconds, and 54 "
    "warnings about a 429 quota wall — both of them a cargo test run "
    "appending to the same log directory as the installed app. The record "
    "format carries NO process identity: no pid, no build id, no instance "
    "token (docs/trace-api.md § TraceEvent). So no record can ever be "
    "assigned to the app or to a harness, and any check that needs to know "
    "which one wrote a line cannot be made sound. What CAN be established is "
    "the negative, and it is the half that decides severity: whether a "
    "record is attributable to a dictation the app actually served. Three "
    "shapes are positive evidence that a second writer existed, and none of "
    "them says which records are whose: a timed call whose interval spans a "
    "whole dictation that finished in less time than the call took; "
    "polish.attempt records outside every dictation's polish window; and "
    "merged physical lines carrying records of two different dictations. "
    "Read this block before reading any unattributed finding below it. "
    "WHAT CHANGED: the writer now mints a `proc` id once per process and puts "
    "it on every record, and `app.launched` carries a `build`. Where a record "
    "has one, the paragraph above stops applying to it: a process that "
    "emitted an app.launched is the app, one that never did is a test binary "
    "or a dev build, and this finding says which processes wrote the corpus "
    "and how many records each contributed instead of stating an ambiguity. "
    "Where a record has none — every line written before the field shipped, "
    "which is the whole harvest corpus — nothing changes and the three "
    "inferences above are still the only evidence there is. One caveat the "
    "field does not remove: a process whose app.launched was rotated out of "
    "the window has the same shape as a harness, so a proc whose first record "
    "is at the very start of the corpus is reported as undecidable rather "
    "than as a harness.",
)
def check_cotenancy(corpus: Corpus):
    ev = _cotenancy_evidence(corpus)
    reasons = []
    # Process identity first, where it exists: it is the answer the three
    # inferences below were standing in for.
    if corpus.has_proc:
        yield from _proc_identity_finding(corpus, ev)
    if ev["impossible_spans"]:
        e, d = ev["impossible_spans"][0]
        reasons.append(
            f"{len(ev['impossible_spans'])} timed keychain read(s) span a "
            f"whole dictation that outlived them by arithmetic — e.g. "
            f"{e.get('ms')} ms ending {e.ts} across {d.key}, which lived "
            f"{_life_ms(d):.0f} ms")
    if ev["unattributed_polish"]:
        days = ", ".join(f"{k} ({v})" for k, v in
                         sorted(ev["polish_days"].items()))
        reasons.append(
            f"{ev['unattributed_polish']} polish.attempt record(s) fall "
            f"outside every traced dictation's polish window: {days}")
    if ev["merged_mixed"]:
        reasons.append(
            f"{ev['merged_mixed']} merged physical line(s) carry records of "
            f"two different dictations")
    if not reasons:
        return
    # The inference-only sentence is about records that do not name a writer.
    # On a corpus where every record does, saying "which records belong to
    # which writer cannot be determined" would be false — the three shapes are
    # then corroboration of something already known by lookup.
    if corpus.fully_procced:
        closing = (". Every record here names its writer, so these are "
                   "corroboration rather than the evidence of last resort — "
                   "see the process list above for who wrote what")
    elif corpus.has_proc:
        closing = (f". {corpus.records_without_proc} of these records do not "
                   f"name their writer, and for those the question stays "
                   f"open: they are graded on whether a dictation can be "
                   f"shown to have been waiting, never on a guess about who "
                   f"wrote them")
    else:
        closing = (". The format carries no process identity, so which "
                   "records belong to which writer cannot be determined — "
                   "findings below are graded on whether a record is "
                   "attributable to a dictation, never on a guess about who "
                   "wrote it")
    yield Finding(
        "log-co-tenancy", INFO,
        "more than one writer appended to this log. "
        + "; ".join(reasons) + closing,
        ts=str(corpus.events[0].ts) if corpus.events else "",
        index=corpus.events[0].index if corpus.events else -1,
        detail={"unattributed_polish_attempts": ev["unattributed_polish"],
                "impossible_spans": len(ev["impossible_spans"]),
                "merged_same_dictation": ev["merged_same_dictation"],
                "merged_mixed": ev["merged_mixed"],
                "records_without_proc": ev["records_without_proc"]},
    )


def _proc_identity_finding(corpus: Corpus, ev: dict):
    """State who wrote this corpus, from the records' own `proc` ids.

    This is the finding the previous audit said could not exist: "harness
    contamination cannot be detected positively, and no amount of cleverness
    in this tool changes that — the fix belongs in the writer". The writer
    fixed it. `app.launched` is emitted once per real app process and by
    nothing else, so it partitions the processes into the app and everything
    else, and every record inherits its process's answer.
    """
    app = corpus.app_processes()
    harness = corpus.harness_processes()
    # Neither: their records start at the rotation boundary, so an
    # `app.launched` they did write would have been cut off. Model.Process
    # marks these `head_truncated` and reports origin "unknown" for them, and
    # so does this line.
    undecidable = [p for p in corpus.processes if p.head_truncated]
    bits = [f"{len(corpus.processes)} process(es) wrote this corpus"]
    if app:
        builds = ", ".join(ev["builds"]) if ev["builds"] else "build unstated"
        bits.append(f"{len(app)} emitted app.launched and "
                    f"{'is' if len(app) == 1 else 'are'} the app "
                    f"({', '.join(p.proc for p in app)}; {builds})")
    if harness:
        n = sum(len(p.events) for p in harness)
        bits.append(f"{len(harness)} never emitted app.launched and wrote "
                    f"{n} record(s) — a test binary or a dev build, not the "
                    f"app serving a user ({', '.join(p.proc for p in harness)})")
    if undecidable:
        bits.append(f"{len(undecidable)} began at the corpus's first record "
                    f"({', '.join(p.proc for p in undecidable)}), so a "
                    f"rotated-away app.launched and a harness are the same "
                    f"shape and neither is claimed")
    if corpus.records_without_proc:
        bits.append(f"{corpus.records_without_proc} record(s) name no process "
                    f"at all and fall back to window containment")
    yield Finding(
        "log-co-tenancy", INFO,
        "; ".join(bits),
        ts=str(corpus.events[0].ts) if corpus.events else "",
        index=corpus.events[0].index if corpus.events else -1,
        detail={"processes": ev["procs"], "app": ev["app_procs"],
                "harness": ev["harness_procs"],
                "harness_records": ev["harness_records"],
                "undecidable": [p.proc for p in undecidable],
                "builds": ev["builds"],
                "records_without_proc": ev["records_without_proc"]},
        origin=ORIGIN_APP,
    )


def run_all(corpus: Corpus, only: set[str] | None = None) -> list[Finding]:
    """Run every check, then filter the FINDINGS by id.

    Filtering the registry instead was a quiet bug: check_bookkeeping_stall
    yields findings labelled `keychain-on-critical-path` and
    `process-suspended` — that is docs/tracing.md's discrimination procedure,
    where one gap becomes one of three findings depending on what else is in
    the window — so `--only keychain-on-critical-path` skipped the check that
    produces the best-attributed keychain findings there are. Every check is
    read-only over the corpus, so running them all costs nothing but time.
    """
    out: list[Finding] = []
    for fn in REGISTRY:
        out.extend(fn(corpus))
    for f in out:
        _stamp_origin(corpus, f)
    if only:
        out = [f for f in out if f.invariant in only]
    out.sort(key=lambda f: (f.ts, f.invariant))
    return out


def _stamp_origin(corpus: Corpus, f: Finding) -> None:
    """Attribute a finding to the process that wrote its evidence.

    Checks that already know better set `origin` themselves — the harness
    aggregation in `check_polish_failed` is one record per process by
    construction. For everything else the anchor is the evidence: a
    dictation-scoped finding takes the dictation's process, and a standalone
    one takes its own record's.

    Degrades to `unknown` for every corpus written before `proc` existed,
    which leaves the report exactly where it was: one undifferentiated block,
    graded by window containment, saying so.
    """
    if f.origin != ORIGIN_UNKNOWN:
        return
    if f.dictation:
        d = corpus.by_key(f.dictation)
        if d is not None:
            f.origin = corpus.origin_of(d.proc)
            if f.origin != ORIGIN_UNKNOWN:
                return
    f.origin = corpus.origin_at(f.index)
