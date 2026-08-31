"""Rendering. A violation the developer cannot go and read is half a finding.

Every reported violation carries its dictation id, its timestamp, and (with
--context) the surrounding events, so the next step is always
`grep <id> ttp-trace.log*` rather than a hunt.
"""

from __future__ import annotations

import collections
import json

from . import stats
from .invariants import ERROR, INFO, KNOWN_STAGES, REGISTRY, WARN, Finding
from .model import Corpus

SEV_ORDER = {ERROR: 0, WARN: 1, INFO: 2}
SEV_LABEL = {ERROR: "ERROR", WARN: "WARN ", INFO: "INFO "}


def _rule(title: str, width: int = 78) -> str:
    return f"\n{title}\n{'-' * min(width, max(len(title), 20))}"


def render(corpus: Corpus, findings: list[Finding], context: int = 0,
           max_per_invariant: int = 8) -> str:
    out: list[str] = []
    out.append("TTP trace invariant analyser")
    out.append("=" * 78)
    for p in corpus.files:
        out.append(f"  source   {p}")
    span = ""
    if corpus.events:
        span = f"{corpus.events[0].ts} .. {corpus.events[-1].ts}"
    out.append(f"  events   {len(corpus.events)}   sessions "
               f"{len(corpus.sessions)}   dictations "
               f"{len(corpus.dictations)}")
    out.append(f"  window   {span}")
    if corpus.merged_records:
        out.append(f"  note     {corpus.merged_records} records shared a "
                   f"physical line with another (a lost newline between two "
                   f"concurrent writes); all were recovered")
    if corpus.unparsed:
        out.append(f"  note     {len(corpus.unparsed)} lines could not be "
                   f"parsed at all")

    unknown = sorted(corpus.stages_seen() - KNOWN_STAGES)
    if unknown:
        out.append(_rule("Stages this analyser has not been taught about"))
        out.append("  Not a violation — the vocabulary is meant to grow. Each")
        out.append("  is carried through the checks untouched; add an "
                   "invariant if one deserves it.")
        for s in unknown:
            n = sum(1 for e in corpus.events if e.stage == s)
            out.append(f"    {s:34s} {n:6d}")

    # ---- outcomes
    out.append(_rule("Outcome distribution"))
    for name, n, pct in stats.outcome_distribution(corpus):
        out.append(f"  {name:34s} {n:5d}  {pct:5.1f}%")

    secs = stats.audio_seconds(corpus)
    if secs:
        out.append(f"  {'audio seconds (p50/p95/max)':34s} "
                   f"{stats.percentile(secs, 50):.1f} / "
                   f"{stats.percentile(secs, 95):.1f} / {max(secs):.1f}")
    dm = stats.device_mix(corpus)
    if dm:
        out.append("  capture devices: "
                   + ", ".join(f"{k} ({v})" for k, v in dm.most_common()))

    # ---- latency
    out.append(_rule("Stage latency (ms between adjacent stages, per "
                     "dictation)"))
    out.append(f"  {'transition':52s} {'n':>5s} {'p50':>7s} {'p95':>8s} "
               f"{'p99':>8s} {'max':>9s}")
    gaps = stats.stage_latencies(corpus)
    rows = [(k, v) for k, v in gaps.items() if len(v) >= 5]
    rows.sort(key=lambda kv: -(stats.percentile(kv[1], 95) or 0))
    for k, v in rows:
        out.append(f"  {k:52s} {len(v):5d} "
                   f"{stats.percentile(v, 50):7.0f} "
                   f"{stats.percentile(v, 95):8.0f} "
                   f"{stats.percentile(v, 99):8.0f} {max(v):9.0f}")

    tot = stats.totals(corpus)
    if tot:
        out.append("")
        out.append(f"  {'dictation.finish total ms':52s} {len(tot):5d} "
                   f"{stats.percentile(tot, 50):7.0f} "
                   f"{stats.percentile(tot, 95):8.0f} "
                   f"{stats.percentile(tot, 99):8.0f} {max(tot):9.0f}")
    for name, vals in sorted(stats.remote_latencies(corpus).items()):
        out.append(f"  {name:52s} {len(vals):5d} "
                   f"{stats.percentile(vals, 50):7.0f} "
                   f"{stats.percentile(vals, 95):8.0f} "
                   f"{stats.percentile(vals, 99):8.0f} {max(vals):9.0f}")

    # ---- findings
    by_inv: dict[str, list[Finding]] = collections.OrderedDict()
    for f in sorted(findings, key=lambda f: (SEV_ORDER.get(f.severity, 9),
                                             f.invariant, f.ts)):
        by_inv.setdefault(f.invariant, []).append(f)

    counts = collections.Counter(f.severity for f in findings)
    out.append(_rule("Invariant results"))
    checked = {fn.iid for fn in REGISTRY}
    fired = set(by_inv)
    out.append(f"  {len(checked)} invariants checked, {len(fired)} fired: "
               f"{counts.get(ERROR, 0)} error, {counts.get(WARN, 0)} warn, "
               f"{counts.get(INFO, 0)} info")
    quiet = sorted(checked - fired)
    if quiet:
        out.append("  silent: " + ", ".join(quiet))

    for inv, fs in by_inv.items():
        sev = fs[0].severity
        title = next((fn.title for fn in REGISTRY if fn.iid == inv), "")
        out.append("")
        out.append(f"  [{SEV_LABEL.get(sev, sev)}] {inv}  ({len(fs)})")
        out.append(f"          {title}")
        for f in fs[:max_per_invariant]:
            loc = f" {f.dictation}" if f.dictation else ""
            out.append(f"      - {f.ts}{loc}")
            out.append(f"        {f.summary}")
            if context and f.index >= 0:
                for e in corpus.context(f.index, context, context):
                    mark = ">>" if e.index == f.index else "  "
                    out.append(f"          {mark} {e.line()}")
        if len(fs) > max_per_invariant:
            out.append(f"      ... {len(fs) - max_per_invariant} more")

    out.append("")
    return "\n".join(out)


def render_json(corpus: Corpus, findings: list[Finding]) -> str:
    gaps = stats.stage_latencies(corpus)
    return json.dumps(
        {
            "files": corpus.files,
            "events": len(corpus.events),
            "sessions": len(corpus.sessions),
            "dictations": len(corpus.dictations),
            "merged_records": corpus.merged_records,
            "unknown_stages": sorted(corpus.stages_seen() - KNOWN_STAGES),
            "outcomes": [
                {"outcome": k, "count": n, "pct": round(p, 2)}
                for k, n, p in stats.outcome_distribution(corpus)
            ],
            "latency_ms": {
                k: {
                    "n": len(v),
                    "p50": stats.percentile(v, 50),
                    "p95": stats.percentile(v, 95),
                    "p99": stats.percentile(v, 99),
                    "max": max(v),
                }
                for k, v in gaps.items() if len(v) >= 5
            },
            "findings": [
                {
                    "invariant": f.invariant,
                    "severity": f.severity,
                    "ts": f.ts,
                    "dictation": f.dictation,
                    "summary": f.summary,
                    "detail": f.detail,
                }
                for f in findings
            ],
        },
        indent=2,
        ensure_ascii=False,
        default=str,
    )


def render_invariant_list() -> str:
    out = ["Invariants checked by scripts/check_trace.py", "=" * 78]
    for fn in REGISTRY:
        out.append("")
        out.append(f"{fn.iid}   [{fn.severity}]")
        out.append(f"  {fn.title}")
        for line in _wrap(fn.why, 74):
            out.append(f"    {line}")
    return "\n".join(out) + "\n"


def _wrap(text: str, width: int) -> list[str]:
    words, lines, cur = text.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > width:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}".strip()
    if cur:
        lines.append(cur)
    return lines
