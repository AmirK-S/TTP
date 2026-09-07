"""CLI: python3 -m trace_analyser [logs...]"""

from __future__ import annotations

import argparse
import sys

from . import report
from .invariants import ERROR, run_all
from .model import ORIGIN_HARNESS, default_log_paths, load


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        prog="check_trace",
        description="Assert docs/tracing.md's documented failure shapes "
                    "against a real ttp-trace.log corpus.",
    )
    p.add_argument("logs", nargs="*",
                   help="trace files, oldest first. Default: the whole "
                        "rotated set in Application Support.")
    p.add_argument("--json", action="store_true", help="machine-readable")
    p.add_argument("--context", type=int, default=0, metavar="N",
                   help="print N events either side of each violation")
    p.add_argument("--only", action="append", metavar="ID",
                   help="run only this invariant (repeatable)")
    p.add_argument("--list", action="store_true",
                   help="describe every invariant and exit")
    p.add_argument("--max-per-invariant", type=int, default=8, metavar="N")
    p.add_argument("--fail-on-error", action="store_true",
                   help="exit 1 if any ERROR-severity invariant fired on a "
                        "record the app wrote (harness-emitted findings are "
                        "reported but never fail)")
    a = p.parse_args(argv)

    if a.list:
        sys.stdout.write(report.render_invariant_list())
        return 0

    paths = a.logs or default_log_paths()
    if not paths:
        sys.stderr.write("no trace log found; pass paths explicitly\n")
        return 2

    corpus = load(paths)
    findings = run_all(corpus, set(a.only) if a.only else None)

    if a.json:
        sys.stdout.write(report.render_json(corpus, findings) + "\n")
    else:
        sys.stdout.write(report.render(corpus, findings, a.context,
                                       a.max_per_invariant) + "\n")

    # A harness-emitted ERROR is not a defect in the product: the records it
    # rests on carry a `proc` that never wrote an `app.launched`, so they came
    # from a test binary or a dev build sharing the log directory. Failing a
    # build on one would be the retracted 2026-09-02 incident with an exit
    # code attached.
    if a.fail_on_error and any(f.severity == ERROR and f.origin != ORIGIN_HARNESS
                               for f in findings):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
