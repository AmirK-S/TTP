#!/usr/bin/env python3
"""Trace-log invariant analyser — see docs/trace-invariants.md.

    python3 scripts/check_trace.py                 # the whole rotated set
    python3 scripts/check_trace.py --context 4     # with surrounding events
    python3 scripts/check_trace.py --list          # what each invariant means

Stdlib only, no install step.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from trace_analyser.__main__ import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
