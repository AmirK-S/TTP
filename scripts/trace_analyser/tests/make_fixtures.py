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
    def __init__(self, launched: bool = True):
        self.lines: list[str] = []
        self.t = T0
        if launched:
            self.free("app.launched",
                      {"arch": "aarch64", "os": "macos", "version": "3.1.6"})

    def _ts(self, ms: int) -> str:
        self.t += timedelta(milliseconds=ms)
        return self.t.strftime("%Y-%m-%d %H:%M:%S.") + f"{self.t.microsecond // 1000:03d}"

    def free(self, stage: str, fields: dict | None = None, ms: int = 10):
        """A standalone event — no trace id, no elapsed column."""
        body = _json(fields or {})
        self.lines.append(
            f"[{self._ts(ms)}] [{'·' * 8}]          {stage} {body}")
        return self

    def stage(self, tid: str, elapsed: int, stage: str,
              fields: dict | None = None, ms: int = 10):
        body = _json(fields or {})
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
              bookkeeping_gap_ms=0):
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
            s("whisper.response", {"attempt": 1, "chars": whisper_chars,
                                   "ms": 300, "sha8": sha_a}, 300)
        s("dictation.finish", fin, 2)
        return

    s("audio.convert", {"converted": True, "in_bytes": 1, "out_bytes": 1}, 6)
    s("whisper.request", {"bytes": 1, "input_mode": "push_to_talk",
                          "lang": "auto", "prompt": False}, 3)
    s("whisper.response", {"attempt": 1, "chars": whisper_chars, "ms": 400,
                           "sha8": sha_a}, 400)
    s("cleanup", {"changed": False, "from": {"chars": whisper_chars, "sha8": sha_a},
                  "to": {"chars": whisper_chars, "sha8": sha_a}}, 20)
    s("polish.decision", {"quota_ok": quota_ok, "setting_enabled": True}, 1)
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
              {"attempt": 1, "chars": 30, "ms": 380, "sha8": "aaaa1111"}, 389)
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
    log = new()
    for n in range(1, 4):
        log.free("polish.attempt", {"model": "llama-3.3-70b-versatile",
                                    "ms": 200, "n": 1, "status": 404})
        log.free("polish.outage", {"consecutive_failures": n,
                                   "model": "llama-3.3-70b-versatile"})
    tail(log, from_state=None)
    out["polish-outage"] = log
    out["remote-call-failed"] = log

    log = new()
    hotkey_cycle(log)
    dictation(log, "0000-1111", quota_ok=False)
    tail(log)
    out["polish-quota-exhausted"] = log

    # -- input layer -----------------------------------------------------
    log = new()
    for n in range(1, 7):
        log.free("hotkey.tap_rearmed", {"reason": "watchdog", "streak": n},
                 ms=2000)
    tail(log, from_state=None)
    out["tap-rearm-streak"] = log

    log = new()
    log.free("hotkey.tap_abandoned", {"rebuilds": 3})
    tail(log, from_state=None)
    out["tap-abandoned"] = log

    log = new()
    for n in range(25):
        log.free("hotkey.tap_rearmed",
                 {"reason": "watchdog", "streak": (n % 5) + 1}, ms=2000)
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
    # A keychain.slow inside the gap re-attributes it.
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

    for name, log in out.items():
        log.write(os.path.join("broken", f"{name}.log"))
    return sorted(out)


if __name__ == "__main__":
    clean()
    two_sessions()
    schema_drift()
    merged_lines()
    names = broken()
    print(f"wrote clean.log, two_sessions.log, schema_drift.log, "
          f"merged_lines.log and "
          f"{len(names)} broken fixtures into {OUT}")
