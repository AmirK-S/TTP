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

from .model import Corpus, Dictation, Event

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

DOCUMENTED_OUTCOMES = {"pasted", "aborted", "clipboard_fallback"}

# Stages named in docs/tracing.md's stage table, plus the ones the corpus
# shows. Informational only — see rule 1 above.
KNOWN_STAGES = {
    "app.launched", "hotkey.press", "hotkey.release", "hotkey.event_dropped",
    "hotkey.double_tap", "hotkey.hands_free_stop", "state.transition",
    "capture.start_failed", "capture.stop_failed", "dictation.rejected",
    "hotkey.tap_armed", "hotkey.tap_rearmed", "hotkey.tap_rebuilt",
    "hotkey.tap_abandoned", "hotkey.tap_create_failed",
    "hotkey.stale_fn_cleared", "hotkey.timer_stall", "capture.start",
    "capture.stop", "audio.duration", "audio.signal", "audio.rms",
    "audio.convert", "whisper.request", "whisper.response", "cleanup",
    "polish", "polish.decision", "polish.attempt", "polish.outage",
    "dictionary", "paste.accessibility", "paste.decision", "paste.modifiers",
    "paste.result", "paste.verify", "clipboard.restore",
    "correction_window.started", "history.saved", "usage.recorded",
    "files.cleaned", "ui.completed", "dictation.start", "dictation.finish",
    # docs/trace-api.md's families. Listed so they do not clutter the
    # "not taught about" section; the checks below still ignore what they
    # do not name.
    "keychain.slow", "keychain.api_key", "degraded",
}

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

# hotkey.tap_rearmed carries a streak. The Rust side escalates to a rebuild at
# 5, so a streak that reaches 5 means re-arming has stopped working — the
# condition docs/tracing.md describes as "a climbing streak means re-arming is
# not working".
TAP_STREAK_LIMIT = 5

# A session with this many re-arms and no hotkey.press at all is a session in
# which the Fn key was dead throughout.
TAP_FLAP_WITHOUT_INPUT = 20

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
    "app.launched, or the end of the corpus.",
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
        elif e.stage in ("capture.stop", "capture.stop_failed"):
            open_start = None
        elif e.stage == "app.launched" and open_start is not None:
            held = (e.ts - open_start.ts).total_seconds()
            yield Finding(
                "capture-stop-missing", ERROR,
                f"capture.start on {open_start.get('device')} never stopped "
                f"before the app relaunched ({held / 60:.1f} min)",
                ts=str(open_start.ts), index=open_start.index,
                detail={"device": open_start.get("device"),
                        "open_seconds": round(held, 1)},
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
        elif e.stage in ("capture.stop", "capture.stop_failed"):
            if not open_start:
                yield Finding(
                    "capture-stop-without-start", WARN,
                    f"{e.stage} with no open capture "
                    f"({e.get('error') or e.get('samples')})",
                    ts=str(e.ts), index=e.index,
                    detail=dict(e.payload),
                )
            open_start = False


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
            yield Finding(
                "state-parked-processing", ERROR,
                f"session {s.index} ended parked in Processing; "
                f"{len(presses)} later hotkey press(es) were no-ops",
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
            yield Finding(
                "hallucination-dropped-speech", ERROR,
                f"{d.key}: {chars} chars from {secs:.1f}s at avg_rms "
                f"{rms:.4f} (floor {floor:.4f}) dropped as hallucination",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
                detail={"chars": chars, "secs": secs, "avg_rms": rms,
                        "floor": floor, "peak": sig.get("peak")},
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
    "model id, not the network.",
)
def check_polish_outage(corpus: Corpus):
    runs: dict[str, list[Event]] = {}
    for e in corpus.events:
        if e.stage == "polish.outage":
            runs.setdefault(e.get("model") or "?", []).append(e)
    for model, evs in runs.items():
        worst = max(evs, key=lambda e: e.get("consecutive_failures") or 0)
        yield Finding(
            "polish-outage", ERROR,
            f"polish failed against '{model}' {len(evs)} times "
            f"(peak streak {worst.get('consecutive_failures')}), "
            f"{evs[0].ts.date()} to {evs[-1].ts.date()}",
            ts=str(evs[0].ts), index=evs[0].index,
            detail={"model": model, "events": len(evs),
                    "peak_streak": worst.get("consecutive_failures")},
        )


@invariant(
    "remote-call-failed", WARN,
    "Remote calls return success",
    "Covers both remote hops: a polish.attempt with a non-200 status, and a "
    "dictation aborted with whisper_error. Reported with the status code and "
    "error category, because 403 (bad key), 429 (quota) and 404 "
    "(decommissioned model) are three different bugs that look identical to "
    "the user.",
)
def check_remote_failures(corpus: Corpus):
    for e in corpus.events:
        if e.stage == "polish.attempt":
            st = e.get("status")
            if st is not None and st != 200:
                yield Finding(
                    "remote-call-failed", WARN,
                    f"polish.attempt returned {st} from "
                    f"{e.get('model')}",
                    ts=str(e.ts), index=e.index, detail=dict(e.payload),
                )
    for d in corpus.dictations:
        if d.reason == "whisper_error":
            yield Finding(
                "remote-call-failed", WARN,
                f"{d.key}: whisper failed with "
                f"{d.finish.get('status_code')} "
                f"({d.finish.get('error_category')})",
                ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
                detail={"status_code": d.finish.get("status_code"),
                        "error_category": d.finish.get("error_category")},
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
    "re-arming is not working.' Every Fn press between the disable and a "
    "successful re-arm was lost.",
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
        yield Finding(
            "tap-rearm-streak", ERROR,
            f"session {sess}: event tap re-arm streak reached "
            f"{e.get('streak')} (limit {TAP_STREAK_LIMIT}) — "
            f"CGEventTapEnable stopped restoring the tap",
            ts=str(e.ts), index=e.index,
            detail={"streak": e.get("streak"), "reason": e.get("reason")},
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
            yield Finding(
                "tap-abandoned", ERROR,
                f"session {e.session}: event tap abandoned after "
                f"{e.get('rebuilds')} rebuilds; {len(after)} hotkey presses "
                f"seen afterwards in this session",
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
    "mechanism. The finding carries the duration so the reader can tell "
    "which they are looking at.",
)
def check_tap_flap(corpus: Corpus):
    for s in corpus.sessions:
        rearms = sum(1 for e in s.events if e.stage == "hotkey.tap_rearmed")
        presses = sum(1 for e in s.events if e.stage == "hotkey.press")
        if rearms >= TAP_FLAP_WITHOUT_INPUT and presses == 0:
            span = (s.events[-1].ts - s.events[0].ts).total_seconds() / 60
            rebuilds = sum(1 for e in s.events
                           if e.stage == "hotkey.tap_rebuilt")
            yield Finding(
                "tap-flapping-without-input", ERROR,
                f"session {s.index}: {rearms} re-arms and {rebuilds} "
                f"rebuilds over {span:.0f} min, and not one hotkey.press "
                f"recorded in the whole session",
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
    "reports outcome:pasted and the target field is unchanged.",
)
def check_paste_swallowed(corpus: Corpus):
    for d in corpus.dictations:
        v = d.stage("paste.verify")
        if not v:
            continue
        if v.get("ax_readable") is True and v.get("changed") is False:
            mods = _modifiers_near(corpus, d)
            yield Finding(
                "paste-swallowed", ERROR,
                f"{d.key}: {v.get('expected_chars')} chars injected, target "
                f"unchanged (before={v.get('before_chars')}, "
                f"after={v.get('after_chars')}, "
                f"settled_ms={v.get('settled_ms')})"
                + (f"; modifier held: {mods.get('held')}" if mods else ""),
                ts=str(v.ts), dictation=d.key, index=v.index,
                detail={"verify": dict(v.payload), "modifiers": mods},
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
                yield Finding(
                    "bookkeeping-stall", ERROR,
                    f"{d.key}: {gap} ms ({gap / 1000:.1f}s) between "
                    f"{a.stage} and {b.stage}; no timer_stall covers it, no "
                    f"keychain.slow inside it, {quiet} other events logged "
                    f"during the window",
                    ts=str(a.ts), dictation=d.key, index=a.index,
                    detail={"gap_ms": gap, "from": a.stage, "to": b.stage,
                            "events_during": quiet,
                            "covered_by_stall": False},
                )


@invariant(
    "keychain-on-critical-path", ERROR,
    "No keychain read blocks a dictation",
    "The keychain defect, now directly instrumented: docs/trace-api.md's "
    "keychain.slow names the account and the op. securityd can block for tens "
    "of seconds, and the account it blocks on (usage_hmac_secret) is read by "
    "usage.recorded — a step docs/tracing.md calls trivial and synchronous. "
    "Reported both standalone and as the explanation for a bookkeeping gap "
    "that contains one.",
)
def check_keychain(corpus: Corpus):
    for e in corpus.events:
        if e.stage != "keychain.slow":
            continue
        ms = e.get("ms")
        yield Finding(
            "keychain-on-critical-path", ERROR,
            f"keychain {e.get('op')}({e.get('account')}) blocked for "
            f"{ms} ms",
            ts=str(e.ts), index=e.index, detail=dict(e.payload),
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


def run_all(corpus: Corpus, only: set[str] | None = None) -> list[Finding]:
    out: list[Finding] = []
    for fn in REGISTRY:
        if only and fn.iid not in only:
            continue
        out.extend(fn(corpus))
    out.sort(key=lambda f: (f.ts, f.invariant))
    return out
