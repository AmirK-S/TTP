#!/usr/bin/env python3
"""Regenerate the fixture logs.

    python3 scripts/trace_analyser/tests/make_fixtures.py

Every fixture is synthetic. No line here was copied from a real trace, and no
`text` field carries anything but a placeholder — the real log has
`diagnostics_enabled` on and contains full dictated speech, which must never
reach the repository.

The fixtures are committed, not generated at test time: a test whose input is
built by the same file that builds the tool is testing itself. This script
exists so that adding an invariant means adding a fixture here and running it
once, and so the diff of a fixture is reviewable.
"""

from __future__ import annotations

import os
from datetime import datetime, timedelta

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures")

T0 = datetime(2026, 9, 1, 10, 0, 0)


class Log:
    """A synthetic trace file.

    `proc` is opt-in. Almost every fixture leaves it off, because almost every
    line in the real corpus lacks it and the checks have to keep working on
    those — a fixture set that all carried process identity would test the
    analyser only in the regime it will be in a month from now, and none of
    the eleven days of harvested log it has to read today.
    """

    def __init__(self, launched: bool = True, proc: str | None = None,
                 build: str | None = None):
        self.lines: list[str] = []
        self.t = T0
        # Stamped onto every record this Log writes, exactly as the writer
        # does: one id, minted once per process, on every line.
        self.proc = proc
        if launched:
            fields = {"arch": "aarch64", "os": "macos", "version": "3.1.6"}
            if build:
                # `app.launched` carries the build on top of `proc`. Version
                # and commit: the pair that says which binary this was.
                fields["build"] = build
                fields["version"] = build.split("+")[0]
            self.free("app.launched", fields)

    def _ts(self, ms: int) -> str:
        self.t += timedelta(milliseconds=ms)
        return self.t.strftime("%Y-%m-%d %H:%M:%S.") + f"{self.t.microsecond // 1000:03d}"

    def _body(self, fields: dict | None) -> str:
        """Render a payload the way `trace::render_fields` does.

        `proc` goes FIRST, not in sort order. The writer splices it onto the
        front of the rendered object precisely so it lands in the same column
        on every line — the payload map is a BTreeMap, so an inserted key
        would sort into the middle. The parser does not care where it is; this
        fixture is also the reference for the on-disk shape, so it matches.
        """
        body = _json(fields or {})
        if self.proc is None:
            return body
        inner = body[1:]
        if inner == "}":
            return f'{{"proc":"{self.proc}"}}'
        return f'{{"proc":"{self.proc}",{inner}'

    def free(self, stage: str, fields: dict | None = None, ms: int = 10):
        """A standalone event — no trace id, no elapsed column."""
        body = self._body(fields)
        self.lines.append(
            f"[{self._ts(ms)}] [{'·' * 8}]          {stage} {body}")
        return self

    def stage(self, tid: str, elapsed: int, stage: str,
              fields: dict | None = None, ms: int = 10):
        body = self._body(fields)
        self.lines.append(
            f"[{self._ts(ms)}] [{tid}] +{elapsed:>6}ms {stage} {body}")
        return self

    def text(self) -> str:
        return "\n".join(self.lines) + "\n"

    def write(self, name: str):
        path = os.path.join(OUT, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(self.text())
        return path


def _json(d: dict) -> str:
    import json
    return json.dumps(d, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=False)


FLOOR = 0.005


def hotkey_cycle(log: Log, device="MacBook Air Microphone", samples=240000,
                 hold_ms=5000):
    """press -> record -> release -> stop, the standalone half of a dictation."""
    log.free("hotkey.press", {"flags": "0x800000", "held_ms": 160})
    log.free("state.transition", {"from": "Idle", "to": "Recording"}, ms=1)
    log.free("capture.start", {"channels": 1, "device": device,
                               "format": "F32", "is_os_default": True,
                               "preferred": None, "rate": 48000}, ms=40)
    log.free("hotkey.release", {"flags": "0x0"}, ms=hold_ms)
    log.free("state.transition", {"from": "Recording", "to": "Processing"},
             ms=1)
    log.free("capture.stop", {"default_now": device, "device": device,
                              "device_changed": False, "samples": samples},
             ms=400)


def dictation(log: Log, tid: str, *, chars=64, secs=5.0, rms=0.02,
              outcome="pasted", reason=None, verbose=False,
              verify=None, skip=(), extra_after=(), device=None,
              nonzero_ratio=1.0, whisper_chars=None, quota_ok=True,
              bookkeeping_gap_ms=0, extra_polish_attempts=(),
              halluc_rule="repetition_loop", halluc_corroborated=True):
    """A whole healthy dictation, with knobs for each thing that can go wrong."""
    sha_a, sha_b = "aaaa1111", "bbbb2222"
    whisper_chars = chars if whisper_chars is None else whisper_chars
    e = 0

    def s(stage, fields, step=5):
        nonlocal e
        e += step
        if stage in skip:
            return
        log.stage(tid, e, stage, fields, ms=step)

    log.stage(tid, 0, "dictation.start",
              {"kind": "recording", "verbose": verbose}, ms=2)
    s("audio.duration", {"secs": secs, "wav_bytes": int(secs * 96000)}, 0)
    s("audio.signal", {"avg_rms": rms, "floor": FLOOR,
                       "nonzero_ratio": nonzero_ratio,
                       "peak": max(rms * 8, 0.0),
                       "samples": int(secs * 48000)}, 3)

    if outcome == "aborted":
        fin = {"ms": e + 2, "outcome": "aborted"}
        if reason:
            fin["reason"] = reason
        if reason in ("silent_audio", "dead_capture"):
            fin.update({"avg_rms": rms, "floor": FLOOR,
                        "nonzero_ratio": nonzero_ratio,
                        "device": device or "MacBook Air Microphone",
                        "samples": int(secs * 48000), "peak": 0.0})
        if reason == "whisper_error":
            fin.update({"error_category": "invalid_api_key",
                        "status_code": 403, "whisper_ms": 400})
        if reason == "hallucination":
            fin["chars"] = whisper_chars
            s("audio.convert", {"converted": True, "in_bytes": 1, "out_bytes": 1}, 3)
            s("whisper.request", {"bytes": 1, "input_mode": "push_to_talk",
                                  "lang": "auto", "prompt": False}, 2)
            s("whisper.response", {"empty_body_retry": False,
                                   "chars": whisper_chars,
                                   "ms": 300, "sha8": sha_a}, 300)
            # The filter's own record, with the vocabulary the corroboration
            # work added: `rule` names the predicate that matched, and for a
            # repetition loop `corroborated` says whether the known-phrase
            # list vouched for it. Quoted by the finding, never tested by it.
            f = {"chars": whisper_chars, "matched": True,
                 "rule": halluc_rule}
            if halluc_rule == "repetition_loop":
                f["repetition_loop"] = True
                f["corroborated"] = halluc_corroborated
            s("filter.hallucination", f, 1)
        s("dictation.finish", fin, 2)
        return

    s("audio.convert", {"converted": True, "in_bytes": 1, "out_bytes": 1}, 6)
    s("whisper.request", {"bytes": 1, "input_mode": "push_to_talk",
                          "lang": "auto", "prompt": False}, 3)
    s("whisper.response", {"empty_body_retry": False,
                           "chars": whisper_chars, "ms": 400,
                           "sha8": sha_a}, 400)
    s("cleanup", {"changed": False, "from": {"chars": whisper_chars, "sha8": sha_a},
                  "to": {"chars": whisper_chars, "sha8": sha_a}}, 20)
    s("polish.decision", {"quota_ok": quota_ok, "setting_enabled": True}, 1)
    # polish.attempt is a standalone record — no trace id — written between
    # the decision and the polish stage. That containment is the only thing
    # that can attribute it to this dictation.
    for fields in extra_polish_attempts:
        log.free("polish.attempt", fields, ms=100)
    s("polish", {"changed": True, "from": {"chars": whisper_chars, "sha8": sha_a},
                 "model": "openai/gpt-oss-120b",
                 "outcome": "applied" if quota_ok else "skipped",
                 "quota_ok": quota_ok,
                 "to": {"chars": chars, "sha8": sha_b}}, 600)
    s("dictionary", {"changed": False, "from": {"chars": chars, "sha8": sha_b},
                     "to": {"chars": chars, "sha8": sha_b}}, 1)
    s("paste.accessibility", {"ax_probe_ok": True, "tcc_trusted": True}, 2)
    s("paste.decision", {"chars": chars, "has_accessibility": True,
                         "strategy": "type"}, 1)
    for stage, fields in extra_after:
        s(stage, fields, 2)
    s("paste.result", {"ok": True}, 110)
    s("clipboard.restore", {"ok": True}, 1)
    s("correction_window.started", {}, 1)
    s("history.saved", {"ok": True}, 1)
    s("ui.completed", {"pasted": True}, 1)
    s("usage.recorded", {"ms": 0}, 1 + bookkeeping_gap_ms)
    s("files.cleaned", {}, 1)
    s("dictation.finish", {"chars": chars, "has_accessibility": True,
                           "ms": e + 2, "outcome": outcome,
                           "words": max(1, chars // 5)}, 2)
    v = {"after_chars": chars, "ax_readable": True, "before_chars": 0,
         "changed": True, "delta_chars": chars, "expected_chars": chars,
         "first_change_ms": 30, "settled_ms": 48}
    v.update(verify or {})
    s("paste.verify", v, 400)


def healthy(log: Log, tid: str, **kw):
    hotkey_cycle(log)
    dictation(log, tid, **kw)
    log.free("state.transition", {"from": "Processing", "to": "Idle"}, ms=1)
    log.free("state.transition", {"from": "Idle", "to": "Idle"}, ms=1)


def tail(log: Log, from_state: str | None = "Processing"):
    """Trailing standalone events.

    Long enough that the last dictation is not treated as in-flight at EOF
    and exempted, and closing the state machine back to Idle so the chain
    check has nothing to complain about. Pass from_state=None for a fixture
    with no state machine, or when the parked-in-Processing state is the
    point.
    """
    if from_state:
        log.free("state.transition", {"from": from_state, "to": "Idle"}, ms=5)
        log.free("state.transition", {"from": "Idle", "to": "Idle"}, ms=1)
    log.free("hotkey.tap_armed", {}, ms=1000)
    log.free("hotkey.tap_armed", {}, ms=1000)
    log.free("hotkey.tap_armed", {}, ms=1000)
    log.free("hotkey.tap_armed", {}, ms=1000)


# ---------------------------------------------------------------- clean

def clean():
    log = Log()
    for tid in ("0000-1111", "0001-2222", "0002-3333"):
        healthy(log, tid)
    tail(log, from_state=None)
    log.write("clean.log")


def two_sessions():
    """The same trace ids in two sessions.

    The sequence number in a trace id restarts at every app.launched, so
    dictation identity has to include the session or six dictations collapse
    into three.
    """
    log = Log()
    healthy(log, "0000-1111")
    healthy(log, "0001-2222")
    log.free("app.launched", {"arch": "aarch64", "os": "macos",
                              "version": "3.1.6"}, ms=60000)
    healthy(log, "0000-1111")
    healthy(log, "0001-2222")
    tail(log, from_state=None)
    log.write("two_sessions.log")


def proc_identity():
    """Two processes sharing one log, each naming itself.

    The contract the trace writer now keeps: every record carries `proc`, a
    short id minted once per process, and `app.launched` additionally carries
    `build` — version and commit. That is what makes harness contamination
    detectable *positively* for the first time: `app.launched` is written by
    the real application at startup and by nothing else, so a process that
    emitted one is the app and a process that never did is a test binary or a
    dev build.

    This file is the shape of the 2026-09-02 incident with the field in place:
    one app process serving a dictation, and one harness process hammering
    polish until the API tier gives 429s. Before `proc`, those 429s were
    indistinguishable from the app's own and were reported to the maintainer
    as a live production incident. Here they are attributable by lookup, and
    the report keeps them out of the app's blocks entirely.

    Committed rather than inline because it is also the reference for the
    field's on-disk shape: `proc` is a payload key, like every other field,
    not a new header column — the header is positional and adding to it would
    break `trace_api::parse_line`'s round trip.
    """
    app = Log(proc="a1b2", build="3.1.7+ab12cd3")
    healthy(app, "0000-1111")
    tail(app, from_state=None)

    harness = Log(launched=False, proc="ff01")
    # A test binary: it links the crate and writes trace records, but it never
    # starts the app, so there is no app.launched anywhere in its output.
    harness.t = T0 + timedelta(seconds=3)
    for _ in range(4):
        harness.free("polish.attempt", {"model": "openai/gpt-oss-120b",
                                        "ms": 52, "n": 1, "status": 429},
                     ms=200)

    merged = sorted(app.lines + harness.lines)
    path = os.path.join(OUT, "proc_identity.log")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(merged) + "\n")


def schema_drift():
    """An old-schema session plus stages the analyser has never heard of.

    Must produce no findings at all: unknown vocabulary is not a violation,
    and a check that cannot see the field it needs abstains.
    """
    log = Log()
    log.free("companion.face_changed", {"face": "bowl"})
    log.free("vad.armed", {"threshold": 0.01})
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start",
              {"kind": "recording", "verbose": False}, ms=2)
    log.stage("0000-1111", 0, "audio.duration",
              {"secs": 4.0, "wav_bytes": 384000})
    # Old name, and no nonzero_ratio at all.
    log.stage("0000-1111", 2, "audio.rms", {"avg_rms": 0.02, "floor": FLOOR})
    log.stage("0000-1111", 8, "audio.convert",
              {"converted": True, "in_bytes": 1, "out_bytes": 1})
    log.stage("0000-1111", 11, "whisper.request",
              {"bytes": 1, "input_mode": "push_to_talk", "lang": "auto",
               "prompt": False})
    log.stage("0000-1111", 400, "whisper.response",
              {"empty_body_retry": False, "chars": 30, "ms": 380,
               "sha8": "aaaa1111"}, 389)
    log.stage("0000-1111", 420, "cleanup",
              {"changed": False, "from": {"chars": 30, "sha8": "aaaa1111"},
               "to": {"chars": 30, "sha8": "aaaa1111"}}, 20)
    log.stage("0000-1111", 421, "polish.decision",
              {"quota_ok": True, "setting_enabled": True}, 1)
    # Old polish schema: `applied`, not `outcome`.
    log.stage("0000-1111", 460, "polish",
              {"applied": True, "changed": False,
               "from": {"chars": 30, "sha8": "aaaa1111"},
               "to": {"chars": 30, "sha8": "aaaa1111"}}, 39)
    log.stage("0000-1111", 461, "dictionary",
              {"changed": False, "from": {"chars": 30, "sha8": "aaaa1111"},
               "to": {"chars": 30, "sha8": "aaaa1111"}}, 1)
    log.stage("0000-1111", 470, "paste.accessibility",
              {"ax_probe_ok": True, "tcc_trusted": True}, 9)
    log.stage("0000-1111", 471, "paste.decision",
              {"chars": 30, "has_accessibility": True, "strategy": "type"}, 1)
    log.stage("0000-1111", 590, "paste.result", {"ok": True}, 119)
    log.stage("0000-1111", 591, "clipboard.restore", {"ok": True}, 1)
    log.stage("0000-1111", 592, "correction_window.started", {}, 1)
    log.stage("0000-1111", 593, "history.saved", {"ok": True}, 1)
    log.stage("0000-1111", 594, "ui.completed", {"pasted": True}, 1)
    log.stage("0000-1111", 595, "usage.recorded", {}, 1)
    log.stage("0000-1111", 596, "files.cleaned", {}, 1)
    log.stage("0000-1111", 597, "dictation.finish",
              {"chars": 30, "has_accessibility": True, "ms": 597,
               "outcome": "pasted", "words": 6}, 1)
    # Old paste.verify: `grew`, no `changed`, unreadable target.
    log.stage("0000-1111", 600, "paste.verify",
              {"after_chars": None, "ax_readable": False,
               "before_chars": None, "expected_chars": 30, "grew": None}, 3)
    log.free("degraded", {"subsystem": "polish", "state": "recovered"})
    tail(log)
    log.write("schema_drift.log")


def merged_lines():
    """Two records sharing a physical line, as the real log contains."""
    log = Log()
    healthy(log, "0000-1111")
    tail(log, from_state=None)
    text = log.text().split("\n")
    for i, line in enumerate(text):
        if "usage.recorded" in line:
            text[i] = line + text[i + 1]
            del text[i + 1]
            break
    with open(os.path.join(OUT, "merged_lines.log"), "w",
              encoding="utf-8") as fh:
        fh.write("\n".join(text))
    return


# --------------------------------------------------------------- broken

def broken():
    out: dict[str, Log] = {}

    def new(launched=True) -> Log:
        return Log(launched)

    # -- lifecycle -------------------------------------------------------
    log = new()
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start",
              {"kind": "recording", "verbose": False})
    log.stage("0000-1111", 0, "audio.duration", {"secs": 4.0, "wav_bytes": 1})
    log.stage("0000-1111", 5, "whisper.request", {"bytes": 1})
    tail(log)
    out["dictation-finish-missing"] = log

    log = new()
    log.stage("0000-1111", 900, "dictation.finish",
              {"chars": 10, "ms": 900, "outcome": "pasted", "words": 2})
    tail(log, from_state=None)
    out["dictation-start-missing"] = log

    log = new()
    hotkey_cycle(log, samples=480000, hold_ms=10000)   # a 10-second hold
    hotkey_cycle(log)                                  # ... then the next one
    healthy(log, "0000-1111")
    tail(log)
    out["capture-handoff-missing"] = log

    log = new()
    log.free("capture.start", {"channels": 1, "device": "AirPods Pro",
                               "format": "F32", "is_os_default": True,
                               "preferred": None, "rate": 24000})
    log.free("capture.start", {"channels": 1, "device": "AirPods Pro",
                               "format": "F32", "is_os_default": True,
                               "preferred": None, "rate": 24000}, ms=600000)
    tail(log, from_state=None)
    out["capture-stop-missing"] = log

    log = new()
    log.free("capture.stop_failed", {"error": "No recording in progress"})
    tail(log, from_state=None)
    out["capture-stop-without-start"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", skip=("paste.result", "clipboard.restore",
                                      "correction_window.started",
                                      "history.saved", "ui.completed",
                                      "usage.recorded", "files.cleaned",
                                      "paste.verify"))
    tail(log)
    out["paste-result-missing"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", skip=("paste.verify",))
    tail(log)
    out["paste-verify-missing"] = log

    log = new()
    log.free("state.transition", {"from": "Idle", "to": "Recording"})
    log.free("state.transition", {"from": "Processing", "to": "Idle"})
    tail(log, from_state=None)
    out["state-chain-broken"] = log

    log = new()
    log.free("state.transition", {"from": "Idle", "to": "Recording"})
    log.free("state.transition", {"from": "Recording", "to": "Processing"})
    log.free("hotkey.press", {"flags": "0x800000", "held_ms": 160}, ms=5000)
    log.free("hotkey.press", {"flags": "0x800000", "held_ms": 160}, ms=5000)
    log.free("app.launched", {"arch": "aarch64", "os": "macos",
                              "version": "3.1.6"}, ms=1000)
    tail(log, from_state=None)
    out["state-parked-processing"] = log

    # -- vocabulary ------------------------------------------------------
    log = new()
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start", {"kind": "recording"})
    log.stage("0000-1111", 5, "dictation.finish", {"ms": 5,
                                                   "outcome": "aborted"})
    tail(log)
    out["abort-reason-missing"] = log

    log = new()
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start", {"kind": "recording"})
    log.stage("0000-1111", 5, "dictation.finish",
              {"ms": 5, "outcome": "aborted", "reason": "moon_phase_wrong"})
    tail(log)
    out["abort-reason-undocumented"] = log

    log = new()
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start", {"kind": "recording"})
    log.stage("0000-1111", 5, "dictation.finish",
              {"ms": 5, "outcome": "teleported", "chars": 3})
    tail(log)
    out["outcome-undocumented"] = log

    # -- audio -----------------------------------------------------------
    log = new()
    hotkey_cycle(log, device="AirPods Pro")
    dictation(log, "0000-1111", outcome="aborted", reason="dead_capture",
              secs=21.0, rms=0.0, nonzero_ratio=0.0, device="AirPods Pro")
    tail(log)
    out["dead-capture"] = log

    log = new()
    hotkey_cycle(log, device="AirPods Pro")
    dictation(log, "0000-1111", outcome="aborted", reason="dead_capture",
              secs=3.0, rms=0.0004, nonzero_ratio=0.42, device="AirPods Pro")
    tail(log)
    out["dead-capture-misclassified"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", outcome="aborted", reason="silent_audio",
              secs=8.0, rms=0.0, nonzero_ratio=0.0)
    tail(log)
    out["silent-audio-misclassified"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", outcome="aborted", reason="silent_audio",
              secs=16.7, rms=0.001, nonzero_ratio=0.96)
    tail(log)
    out["long-recording-silent"] = log

    log = new()
    log.free("capture.start", {"channels": 1, "device": "AirPods Pro",
                               "format": "F32", "is_os_default": True,
                               "preferred": None, "rate": 24000})
    log.free("capture.stop", {"default_now": "MacBook Air Microphone",
                              "device": "AirPods Pro",
                              "device_changed": True, "samples": 240000},
             ms=5000)
    healthy(log, "0000-1111")
    tail(log)
    out["device-changed-mid-recording"] = log

    # -- text ------------------------------------------------------------
    log = new()
    hotkey_cycle(log)
    # Whisper heard 87 characters and a transformation deleted every one.
    dictation(log, "0000-1111", chars=0, whisper_chars=87)
    tail(log)
    out["text-emptied"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", outcome="aborted", reason="hallucination",
              secs=16.0, rms=0.0098, whisper_chars=299)
    tail(log)
    out["hallucination-dropped-speech"] = log

    log = new()
    hotkey_cycle(log)
    log.stage("0000-1111", 0, "dictation.start", {"kind": "recording"})
    log.stage("0000-1111", 400, "cleanup",
              {"changed": False, "from": {"chars": 30, "sha8": "aaaa1111"},
               "to": {"chars": 30, "sha8": "aaaa1111"}})
    log.stage("0000-1111", 420, "dictionary",
              {"changed": False, "from": {"chars": 30, "sha8": "cccc3333"},
               "to": {"chars": 30, "sha8": "cccc3333"}})
    log.stage("0000-1111", 430, "dictation.finish",
              {"chars": 30, "ms": 430, "outcome": "pasted", "words": 6})
    tail(log)
    out["text-chain-broken"] = log

    # -- remote ----------------------------------------------------------
    # Three consecutive failures against one model: POLISH_OUTAGE_STREAK, the
    # point at which "repeatedly" in the invariant's own title is satisfied.
    # A peak of 1 is a network and is graded WARN.
    log = new()
    for n in range(1, 4):
        log.free("polish.attempt", {"model": "llama-3.3-70b-versatile",
                                    "ms": 200, "n": 1, "status": 404})
        log.free("polish.outage", {"consecutive_failures": n,
                                   "model": "llama-3.3-70b-versatile"})
    tail(log, from_state=None)
    out["polish-outage"] = log

    # THE SPLIT. `remote-call-failed` had one fixture for two checks with two
    # evidence standards, which is how it came to have one severity for them
    # as well. Each half now has its own, and each half's fixture must leave
    # the other silent — that is the property the merge destroyed.
    #
    # polish-call-failed: the 404 is inside this dictation's polish window,
    # between its polish.decision and its polish stage, which is the only
    # attribution polish.attempt admits (the record carries no trace id). One
    # dictation was waiting on it, so it is a WARN about the app; the
    # dictation still pastes, because the pipeline falls back to unpolished
    # text, which is why it is not more than a WARN.
    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111",
              extra_polish_attempts=[{"model": "llama-3.3-70b-versatile",
                                      "ms": 200, "n": 1, "status": 404}])
    tail(log)
    out["polish-call-failed"] = log

    # whisper-call-failed: a dictation aborted with `whisper_error`. The
    # evidence is `dictation.finish` carrying this dictation's own id, its
    # status code and its error category — the app's own statement that this
    # dictation produced nothing. No window arithmetic, no co-tenant reading,
    # and the user lost the words they spoke. ERROR.
    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", outcome="aborted", reason="whisper_error")
    tail(log)
    out["whisper-call-failed"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", quota_ok=False)
    tail(log)
    out["polish-quota-exhausted"] = log

    # -- input layer -----------------------------------------------------
    log = new()
    # Past TAP_STREAK_ESCALATED, with the rebuilds that were supposed to fix
    # it. Reaching TAP_STREAK_LIMIT alone is the escalation firing as
    # designed and is graded WARN; this is the streak climbing THROUGH the
    # rebuild, which is the dead-event-tap defect.
    for n in range(1, 15):
        log.free("hotkey.tap_rearmed", {"reason": "watchdog", "streak": n},
                 ms=2000)
        if n % 5 == 0:
            log.free("hotkey.tap_rebuilt", {"attempt": n // 5}, ms=10)
    tail(log, from_state=None)
    out["tap-rearm-streak"] = log

    log = new()
    log.free("hotkey.tap_abandoned", {"rebuilds": 3})
    tail(log, from_state=None)
    out["tap-abandoned"] = log

    log = new()
    # Spaced so the session runs well past TAP_FLAP_MINUTES. Under half an
    # hour, "the tap is flapping and swallowing presses" and "nobody pressed
    # anything" fit the same evidence, and the check grades accordingly.
    for n in range(25):
        log.free("hotkey.tap_rearmed",
                 {"reason": "watchdog", "streak": (n % 5) + 1}, ms=120000)
    tail(log, from_state=None)
    out["tap-flapping-without-input"] = log

    log = new()
    log.free("hotkey.stale_fn_cleared", {})
    tail(log, from_state=None)
    out["stale-fn-latched"] = log

    log = new()
    log.free("hotkey.event_dropped", {"kind": "press"})
    tail(log, from_state=None)
    out["hotkey-event-dropped"] = log

    # -- paste -----------------------------------------------------------
    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", chars=14,
              verify={"changed": False, "after_chars": 0, "delta_chars": 0,
                      "first_change_ms": None, "settled_ms": 0})
    tail(log)
    out["paste-swallowed"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111",
              extra_after=[("paste.modifiers",
                            {"bits": "0x800000", "held": "Fn/Globe"})])
    tail(log)
    out["paste-modifier-held"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", chars=80,
              verify={"after_chars": 12, "delta_chars": 12})
    tail(log)
    out["paste-partial"] = log

    # -- latency ---------------------------------------------------------
    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", bookkeeping_gap_ms=6000)
    tail(log)
    out["bookkeeping-stall"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", bookkeeping_gap_ms=6000,
              extra_after=[])
    # A keychain.slow inside the gap re-attributes it. This is a genuine
    # blocking read and the check must still fire at ERROR on it: the record
    # is inside the dictation's own stage sequence, and the 5,900 ms it
    # reports began and ended within a dictation that was alive for longer —
    # so the dictation could have made this call and waited for it. Contrast
    # `log-co-tenancy.log`, where the same stage carries the same account and
    # the same op, spills past the dictation it overlaps, and is graded a
    # warning about two writers rather than an ERROR about a blocked user.
    lines = log.lines
    for i, line in enumerate(lines):
        if "usage.recorded" in line:
            ts = line[1:24]
            lines.insert(i, f"[{ts}] [{'·' * 8}]          keychain.slow "
                            '{"account":"usage_hmac_secret","ms":5900,'
                            '"op":"secret_read"}')
            break
    tail(log)
    out["keychain-on-critical-path"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", bookkeeping_gap_ms=6000)
    lines = log.lines
    for i, line in enumerate(lines):
        if "usage.recorded" in line:
            ts = line[1:24]
            lines.insert(i + 1,
                         f"[{ts}] [{'·' * 8}]          hotkey.timer_stall "
                         '{"gap_ms":6100}')
            break
    tail(log)
    out["process-suspended"] = log

    # -- R1's fixes, seen from the other side ----------------------------
    #
    # These three do not describe an old failure; each is the shape that
    # would mean a shipped fix had stopped working.

    log = new()
    # The arbiter says it tore the stream down, and then something else
    # closes a capture with no capture.start in between — so the stream was
    # still published and the microphone was still live.
    log.free("state.transition", {"from": "Idle", "to": "Recording"})
    log.free("capture.start", {"channels": 1, "device": "AirPods Pro",
                               "format": "F32", "is_os_default": True,
                               "preferred": None, "rate": 24000}, ms=40)
    log.free("state.transition", {"from": "Recording", "to": "Idle"}, ms=60)
    log.free("capture.orphan_prevented",
             {"build_ms": 544, "device": "AirPods Pro",
              "reason": "user_idle"}, ms=6)
    log.free("capture.stop", {"default_now": "AirPods Pro",
                              "device": "AirPods Pro",
                              "device_changed": False, "samples": 480000},
             ms=9000)
    # And the two other ways the guarantee can be seen to fail.
    log.free("degraded", {"error": "arbiter and STATE disagree", "live": True,
                          "site": "capture.reclaim"}, ms=50)
    log.free("capture.stop_waited_for_start", {"ms": 3000, "timed_out": True},
             ms=50)
    tail(log, from_state=None)
    out["capture-arbiter-left-live"] = log

    log = new()
    # Eight sequential secret_reads of one account in one session, none of
    # which waited on another. This is the pre-fix ladder, in milliseconds.
    for ms in (62304, 13612, 397600, 97407, 157, 56037, 106, 86):
        log.free("keychain.slow", {"account": "usage_hmac_secret",
                                   "lock_wait_ms": 0, "ms": ms,
                                   "op": "secret_read"}, ms=30000)
    tail(log, from_state=None)
    out["keychain-not-single-flighted"] = log

    log = new()
    healthy(log, "0000-1111")
    tail(log, from_state=None)
    out["writer-newline-lost"] = log

    # -- more than one writer, stated rather than resolved ---------------
    #
    # The shape that produced the analyser's first false alarm, kept as a
    # fixture so the statement of ambiguity keeps being made. A keychain read
    # reported as 60,000 ms lands just after a dictation that started, ran
    # its whole pipeline including usage.recorded, and finished in about a
    # second. The read therefore began long before that dictation existed and
    # ended after it: one process with a lifetime cache and read_once cannot
    # produce that, so two writers shared this file. Which records belong to
    # which of them is not recoverable, and the report says so.
    log = new()
    healthy(log, "0000-1111")
    log.free("keychain.slow", {"account": "usage_hmac_secret",
                               "lock_wait_ms": 0, "ms": 60000,
                               "op": "secret_read"}, ms=200)
    tail(log, from_state=None)
    out["log-co-tenancy"] = log

    for name, log in out.items():
        log.write(os.path.join("broken", f"{name}.log"))

    # writer-newline-lost is the one fixture whose defect is in the file's
    # physical layout rather than in any record, so it is damaged after
    # writing: one newline moved from between two records to a line of its
    # own, which is precisely what two racing write_all calls did.
    _lose_a_newline(os.path.join(OUT, "broken", "writer-newline-lost.log"))
    return sorted(out)


def _lose_a_newline(path: str):
    with open(path, encoding="utf-8") as fh:
        text = fh.read().split("\n")
    for i, line in enumerate(text):
        if "usage.recorded" in line:
            text[i] = line + text[i + 1]
            text[i + 1] = ""          # the stray newline lands here
            break
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(text))


if __name__ == "__main__":
    clean()
    two_sessions()
    schema_drift()
    merged_lines()
    proc_identity()
    names = broken()
    print(f"wrote clean.log, two_sessions.log, schema_drift.log, "
          f"merged_lines.log, proc_identity.log and "
          f"{len(names)} broken fixtures into {OUT}")
