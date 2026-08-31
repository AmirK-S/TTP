"""Parsing and grouping for `ttp-trace.log`.

The parser is deliberately permissive. The trace vocabulary is still growing
(see `docs/trace-api.md`), so an unrecognised stage name must never cause a
parse error, a dropped line, or a reported violation. Everything here works on
the *shape* of a record — timestamp, id, elapsed, stage, JSON payload — and
never on a whitelist of stage names.

Privacy: `dictation.start {"verbose":true}` sessions record the dictated text
in `text` fields. Those are stripped at parse time and replaced by a character
count, so no downstream code — report, fixture, or check — can leak speech.
"""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from datetime import datetime

# A record header: [ts] [id] optional "+   Nms" then the stage name.
# The id column is either a trace id or U+00B7 filler for a standalone event.
HEADER_RE = re.compile(
    r"\[(?P<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3})\] "
    r"\[(?P<tid>[^\]]*)\]"
    r"(?:\s*\+\s*(?P<ms>\d+)ms)?"
    r"\s+(?P<stage>[A-Za-z0-9_.]+)\s*"
)

STANDALONE_ID = "·" * 8

TS_FMT = "%Y-%m-%d %H:%M:%S.%f"

# Keys whose values are raw dictated speech. Never kept.
TEXT_KEYS = {"text"}

_DECODER = json.JSONDecoder()


def _redact(value):
    """Recursively replace dictated text with its length."""
    if isinstance(value, dict):
        out = {}
        for k, v in value.items():
            if k in TEXT_KEYS and isinstance(v, str):
                out[k] = f"<redacted:{len(v)}chars>"
            else:
                out[k] = _redact(v)
        return out
    if isinstance(value, list):
        return [_redact(v) for v in value]
    return value


@dataclass
class Event:
    ts: datetime
    tid: str | None  # None for a standalone event
    elapsed_ms: int | None
    stage: str
    payload: dict
    source: str  # "file:line"
    index: int = -1  # position in the global ordered stream
    session: int = -1

    @property
    def is_standalone(self) -> bool:
        return self.tid is None

    def get(self, *path, default=None):
        cur = self.payload
        for p in path:
            if not isinstance(cur, dict) or p not in cur:
                return default
            cur = cur[p]
        return cur

    def line(self) -> str:
        """One-line rendering, safe to print (text already redacted)."""
        tid = self.tid or STANDALONE_ID
        el = f"+{self.elapsed_ms:>6}ms" if self.elapsed_ms is not None else " " * 9
        body = json.dumps(self.payload, ensure_ascii=False, sort_keys=True)
        if len(body) > 220:
            body = body[:217] + "..."
        return f"[{self.ts.strftime(TS_FMT)[:-3]}] [{tid}] {el} {self.stage} {body}"


@dataclass
class Dictation:
    tid: str
    session: int
    events: list[Event] = field(default_factory=list)

    @property
    def key(self) -> str:
        return f"s{self.session}/{self.tid}"

    @property
    def start(self) -> Event | None:
        return self.stage("dictation.start")

    @property
    def finish(self) -> Event | None:
        return self.stage("dictation.finish")

    def stage(self, name: str) -> Event | None:
        for e in self.events:
            if e.stage == name:
                return e
        return None

    def all_stages(self, name: str) -> list[Event]:
        return [e for e in self.events if e.stage == name]

    def has(self, name: str) -> bool:
        return self.stage(name) is not None

    @property
    def outcome(self) -> str | None:
        f = self.finish
        return f.get("outcome") if f else None

    @property
    def reason(self) -> str | None:
        f = self.finish
        return f.get("reason") if f else None


@dataclass
class Session:
    index: int
    launched: Event | None
    events: list[Event] = field(default_factory=list)


@dataclass
class Corpus:
    events: list[Event]
    sessions: list[Session]
    dictations: list[Dictation]
    merged_records: int = 0
    unparsed: list[tuple[str, str]] = field(default_factory=list)
    files: list[str] = field(default_factory=list)

    def by_key(self, key: str) -> Dictation | None:
        for d in self.dictations:
            if d.key == key:
                return d
        return None

    def context(self, index: int, before: int = 4, after: int = 4) -> list[Event]:
        lo = max(0, index - before)
        hi = min(len(self.events), index + after + 1)
        return self.events[lo:hi]

    def stages_seen(self) -> set[str]:
        return {e.stage for e in self.events}


def parse_line(raw: str, source: str) -> tuple[list[Event], bool]:
    """Parse one physical line into one or more records.

    Records can share a physical line: the corpus contains lines where a second
    record was appended with no newline between them (concurrent writers losing
    a newline). Splitting on the JSON payload's true end rather than on a
    regex keeps that from swallowing the second record — and is immune to a
    dictated transcript that happens to contain a bracketed timestamp.

    Returns (events, was_merged).
    """
    events: list[Event] = []
    pos = 0
    n = len(raw)
    while pos < n:
        m = HEADER_RE.match(raw, pos)
        if not m:
            break
        pos = m.end()
        payload: dict = {}
        if pos < n and raw[pos] == "{":
            try:
                obj, end = _DECODER.raw_decode(raw, pos)
                payload = obj if isinstance(obj, dict) else {"value": obj}
                pos = end
            except ValueError:
                # Truncated JSON (a torn write at the tail of a rotated file).
                payload = {"_unparsed_payload": True}
                pos = n
        while pos < n and raw[pos] == " ":
            pos += 1
        tid_raw = m.group("tid")
        tid = None if set(tid_raw) <= {"·", " ", "."} else tid_raw
        events.append(
            Event(
                ts=datetime.strptime(m.group("ts"), TS_FMT),
                tid=tid,
                elapsed_ms=int(m.group("ms")) if m.group("ms") else None,
                stage=m.group("stage"),
                payload=_redact(payload),
                source=source,
            )
        )
    return events, len(events) > 1


def load(paths: list[str]) -> Corpus:
    """Load one or more trace files into a single ordered corpus.

    Rotated files are read oldest-first so the stream is chronological. Order
    is by (timestamp, file order, line number): the timestamps are millisecond
    resolution and ties are common, so a stable sort on file position is what
    keeps a dictation's stages in the order they were written.
    """
    records: list[tuple[datetime, int, int, Event]] = []
    merged = 0
    unparsed: list[tuple[str, str]] = []
    for fi, path in enumerate(paths):
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            for lineno, raw in enumerate(fh, 1):
                raw = raw.rstrip("\n")
                if not raw.strip():
                    continue
                src = f"{os.path.basename(path)}:{lineno}"
                evs, was_merged = parse_line(raw, src)
                if not evs:
                    unparsed.append((src, raw[:160]))
                    continue
                if was_merged:
                    merged += len(evs) - 1
                for e in evs:
                    records.append((e.ts, fi, lineno, e))

    records.sort(key=lambda r: (r[0], r[1], r[2]))
    events = [r[3] for r in records]
    for i, e in enumerate(events):
        e.index = i

    # Sessions: split on app.launched. Anything before the first one belongs to
    # session 0, a session whose head was lost to rotation.
    sessions: list[Session] = [Session(index=0, launched=None)]
    for e in events:
        if e.stage == "app.launched":
            sessions.append(Session(index=len(sessions), launched=e))
        e.session = sessions[-1].index
        sessions[-1].events.append(e)
    if not sessions[0].events:
        sessions.pop(0)

    # Dictations: keyed by (session, trace id). The sequence number in the id
    # restarts each session, so the session index is part of the identity.
    dicts_by_key: dict[tuple[int, str], Dictation] = {}
    order: list[Dictation] = []
    for e in events:
        if e.tid is None:
            continue
        k = (e.session, e.tid)
        d = dicts_by_key.get(k)
        if d is None:
            d = Dictation(tid=e.tid, session=e.session)
            dicts_by_key[k] = d
            order.append(d)
        d.events.append(e)

    return Corpus(
        events=events,
        sessions=sessions,
        dictations=order,
        merged_records=merged,
        unparsed=unparsed,
        files=list(paths),
    )


def default_log_paths() -> list[str]:
    """The rotated set, oldest first.

    A tool that reads only `ttp-trace.log` analyses whatever the last few
    minutes happened to hold — 17 lines of a 15,500-line corpus at the time
    this was written. Always take the whole set.
    """
    base = os.path.expanduser(
        "~/Library/Application Support/com.ttp.desktop/ttp-trace.log"
    )
    if os.name == "nt":  # pragma: no cover - documented, not exercised here
        appdata = os.environ.get("APPDATA", "")
        base = os.path.join(appdata, "com.ttp.desktop", "ttp-trace.log")
    found = []
    for n in (3, 2, 1):
        p = f"{base}.{n}"
        if os.path.exists(p):
            found.append(p)
    if os.path.exists(base):
        found.append(base)
    return found
