"""Summary statistics.

Latency bugs hide in event logs that nobody aggregates. The keychain sitting
on the dictation critical path was one: every individual trace looked fine,
and the p95 of one stage transition was where it showed.
"""

from __future__ import annotations

import collections
import math

from .model import Corpus


def percentile(values: list[float], p: float):
    if not values:
        return None
    v = sorted(values)
    k = (len(v) - 1) * p / 100.0
    f, c = math.floor(k), math.ceil(k)
    if f == c:
        return v[int(k)]
    return v[f] + (v[c] - v[f]) * (k - f)


def outcome_distribution(corpus: Corpus) -> list[tuple[str, int, float]]:
    total = len(corpus.dictations)
    c = collections.Counter()
    for d in corpus.dictations:
        o = d.outcome or "<no finish>"
        c[f"{o}/{d.reason}" if d.reason else o] += 1
    return [(k, n, 100.0 * n / total if total else 0.0)
            for k, n in c.most_common()]


def stage_latencies(corpus: Corpus) -> dict[str, list[int]]:
    """Milliseconds between each pair of adjacent stages, per transition.

    Keyed on the *pair* rather than the stage, because the trace records when
    a stage finished, not how long it took. The gap between two adjacent
    stages is the only latency the log actually contains.
    """
    gaps: dict[str, list[int]] = collections.defaultdict(list)
    for d in corpus.dictations:
        ev = sorted((e for e in d.events if e.elapsed_ms is not None),
                    key=lambda e: e.index)
        for a, b in zip(ev, ev[1:]):
            gaps[f"{a.stage} -> {b.stage}"].append(b.elapsed_ms - a.elapsed_ms)
    return gaps


def totals(corpus: Corpus) -> list[int]:
    return [d.finish.get("ms") for d in corpus.dictations
            if d.finish and isinstance(d.finish.get("ms"), (int, float))]


def remote_latencies(corpus: Corpus) -> dict[str, list[float]]:
    out: dict[str, list[float]] = collections.defaultdict(list)
    for e in corpus.events:
        if e.stage == "whisper.response" and e.get("ms") is not None:
            out["whisper (transcription)"].append(e.get("ms"))
        elif e.stage == "polish.attempt" and e.get("ms") is not None:
            out[f"polish ({e.get('model')})"].append(e.get("ms"))
    return out


def device_mix(corpus: Corpus) -> collections.Counter:
    c = collections.Counter()
    for e in corpus.events:
        if e.stage == "capture.start" and e.get("device"):
            c[e.get("device")] += 1
    return c


def audio_seconds(corpus: Corpus) -> list[float]:
    return [d.stage("audio.duration").get("secs")
            for d in corpus.dictations
            if d.stage("audio.duration")
            and d.stage("audio.duration").get("secs") is not None]
