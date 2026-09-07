"""Rendering. A violation the developer cannot go and read is half a finding.

Every reported violation carries its dictation id, its timestamp, and (with
--context) the surrounding events, so the next step is always
`grep <id> ttp-trace.log*` rather than a hunt.
"""

from __future__ import annotations

import collections
import json

from . import invariants, stats
from .invariants import ERROR, INFO, KNOWN_STAGES, REGISTRY, WARN, Finding
from .model import ORIGIN_HARNESS, Corpus

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
    # Printed whether or not it is zero. The parser repairs both symptoms
    # silently, which is exactly how the lost-newline race survived weeks of
    # people reading this log; a zero here is the fix reporting itself.
    out.append(f"  writer   {corpus.merged_records} merged records, "
               f"{corpus.blank_lines} blank lines "
               f"({'clean' if not (corpus.merged_records or corpus.blank_lines) else 'see writer-newline-lost'})")
    if corpus.unparsed:
        out.append(f"  note     {len(corpus.unparsed)} lines could not be "
                   f"parsed at all")
    # Printed in the header, next to the writer counts, for the same reason
    # those are: a caveat that only appears at the bottom of a long report is
    # a caveat nobody reads. The full statement is the `log-co-tenancy`
    # finding; this is the line that stops a reader from taking an
    # unattributed record for the app.
    co = invariants._cotenancy_evidence(corpus)
    # Process identity, where the writer supplies it. Printed above the
    # inference line, because it answers the question that line exists to say
    # is unanswerable — and printed as "none of these records says" when it is
    # absent, so a reader is never left to assume one or the other.
    if corpus.has_proc:
        app = corpus.app_processes()
        harness = corpus.harness_processes()
        undecided = [p for p in corpus.processes if p.head_truncated]
        builds = ", ".join(co["builds"]) if co["builds"] else "build unstated"
        out.append(f"  procs    {len(corpus.processes)} process(es): "
                   f"{len(app)} app ({builds}), {len(harness)} harness "
                   f"({co['harness_records']} records), "
                   f"{len(undecided)} undecidable")
        if corpus.records_without_proc:
            out.append(f"           {corpus.records_without_proc} record(s) "
                       f"carry no proc and fall back to window containment")
    else:
        out.append("  procs    no record in this corpus names the process "
                   "that wrote it (pre-`proc` trace); harness and app cannot "
                   "be told apart")
    witnesses = (len(co["impossible_spans"]) + co["unattributed_polish"]
                 + co["merged_mixed"])
    if witnesses:
        tail = ("Every record here names its writer"
                if corpus.fully_procced else
                "The format carries no process identity")
        out.append(f"  writers  more than one process appended to this log "
                   f"({len(co['impossible_spans'])} impossible timed spans, "
                   f"{co['unattributed_polish']} unattributed polish.attempt, "
                   f"{co['merged_mixed']} mixed merged lines). {tail}")
        out.append("           — see log-co-tenancy. Records that name no "
                   "writer are graded on whether a dictation can be shown to "
                   "have been waiting, never on a guess about who wrote "
                   "them.")

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
    #
    # Grouped by (severity, invariant), not by invariant. Several checks now
    # grade individual findings on the evidence behind them — a keychain read
    # attributable to a dictation is an ERROR and one that is not is
    # information from the same check — and a block headed [ERROR] whose rows
    # were mostly warnings would restate, in the layout, exactly the
    # over-claim those checks were fixed to stop making.
    #
    # Split first by ORIGIN. A finding whose evidence was written by a process
    # that never emitted `app.launched` is a finding about a test binary, and
    # printing it in the same block as the app's own is the merge that let 46
    # harness 429s sit beside a real invalid-key abort. The harness block is
    # printed last, under its own heading, and its counts are stated
    # separately in the summary line rather than folded into it.
    mine = [f for f in findings if f.origin != ORIGIN_HARNESS]
    theirs = [f for f in findings if f.origin == ORIGIN_HARNESS]

    def grouped(fs: list[Finding]) -> dict:
        g: dict[tuple, list[Finding]] = collections.OrderedDict()
        for f in sorted(fs, key=lambda f: (SEV_ORDER.get(f.severity, 9),
                                           f.invariant, f.ts)):
            g.setdefault((f.severity, f.invariant), []).append(f)
        return g

    def emit(g: dict) -> None:
        for (sev, inv), fs in g.items():
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

    by_inv = grouped(mine)
    counts = collections.Counter(f.severity for f in mine)
    out.append(_rule("Invariant results"))
    checked = {fn.iid for fn in REGISTRY}
    fired = {inv for _, inv in by_inv}
    out.append(f"  {len(checked)} invariants checked, {len(fired)} fired: "
               f"{counts.get(ERROR, 0)} error, {counts.get(WARN, 0)} warn, "
               f"{counts.get(INFO, 0)} info")
    if theirs:
        out.append(f"  plus {len(theirs)} finding(s) whose evidence was "
                   f"written by a process that never launched the app — "
                   f"listed separately at the end, and NOT counted above")
    quiet = sorted(checked - fired - {f.invariant for f in theirs})
    if quiet:
        out.append("  silent: " + ", ".join(quiet))
    split = sorted({inv for _, inv in by_inv
                    if sum(1 for s, i in by_inv if i == inv) > 1})
    if split:
        out.append("  graded by evidence, so they appear more than once: "
                   + ", ".join(split))

    emit(by_inv)

    if theirs:
        hcounts = collections.Counter(f.severity for f in theirs)
        out.append(_rule("Emitted by a harness, not by the app"))
        out.append("  These rest on records whose `proc` names a process that "
                   "wrote no")
        out.append("  app.launched: a test binary or a dev build sharing the "
                   "log directory.")
        out.append("  They are not findings about the product. Kept because a "
                   "harness that")
        out.append("  exhausts the API tier is still worth knowing about — "
                   "and because the")
        out.append("  46 records this section exists for were once reported "
                   "as an incident.")
        out.append(f"  {hcounts.get(ERROR, 0)} error, {hcounts.get(WARN, 0)} "
                   f"warn, {hcounts.get(INFO, 0)} info")
        emit(grouped(theirs))

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
            "blank_lines": corpus.blank_lines,
            "unknown_stages": sorted(corpus.stages_seen() - KNOWN_STAGES),
            "processes": [
                {
                    "proc": p.proc,
                    "origin": p.origin,
                    "records": len(p.events),
                    "build": p.build,
                    "first_ts": p.first_ts,
                    "last_ts": p.last_ts,
                }
                for p in corpus.processes
            ],
            "records_without_proc": corpus.records_without_proc,
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
                    # "app", "harness", or "unknown" — a consumer that treats
                    # "unknown" as "app" is reading a pre-`proc` corpus as if
                    # the field had been there.
                    "origin": f.origin,
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
