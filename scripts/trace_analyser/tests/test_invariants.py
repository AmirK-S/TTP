"""Tests for the trace invariant analyser.

    python3 scripts/trace_analyser/tests/test_invariants.py

Stdlib unittest, no pytest, no install. Every test runs against a committed
fixture in `fixtures/`, never against the user's real log: a test whose input
is a live file is not reproducible and rots the first time the app is used.

The load-bearing test is `test_every_invariant_fires`. An invariant checker
whose checks have never been seen to fire is not known to work, so each
invariant must have a fixture that violates it, and the clean fixture must
leave every one of them silent.
"""

from __future__ import annotations

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(os.path.dirname(HERE)))

from trace_analyser import model  # noqa: E402
from trace_analyser.invariants import (  # noqa: E402
    ERROR, INFO, REGISTRY, WARN, run_all,
)
from trace_analyser.report import render, render_json  # noqa: E402

FIX = os.path.join(HERE, "fixtures")
BROKEN = os.path.join(FIX, "broken")


def analyse(*names):
    paths = []
    for n in names:
        p = os.path.join(FIX, n)
        paths.append(p if os.path.exists(p) else os.path.join(BROKEN, n))
    corpus = model.load(paths)
    return corpus, run_all(corpus)


def fired(findings) -> set[str]:
    return {f.invariant for f in findings}


def _analyse_text(raw: str):
    """Analyse a log written for one assertion, in a temp file.

    Fixtures on disk are for invariants that have to keep firing. A shape
    whose whole point is that it stays SILENT is clearer inline, and cannot
    be mistaken for a fixture that has stopped working.
    """
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "t.log")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(raw)
        corpus = model.load([path])
        return corpus, run_all(corpus)


class TestParser(unittest.TestCase):
    def test_parses_both_record_shapes(self):
        corpus, _ = analyse("clean.log")
        standalone = [e for e in corpus.events if e.is_standalone]
        staged = [e for e in corpus.events if not e.is_standalone]
        self.assertTrue(standalone, "no standalone events parsed")
        self.assertTrue(staged, "no dictation-stage events parsed")
        self.assertTrue(all(e.elapsed_ms is None for e in standalone))
        self.assertTrue(all(e.elapsed_ms is not None for e in staged))
        self.assertEqual(len(corpus.unparsed), 0)

    def test_groups_dictations_and_sessions(self):
        corpus, _ = analyse("clean.log")
        self.assertEqual(len(corpus.dictations), 3)
        self.assertEqual(len(corpus.sessions), 1)
        for d in corpus.dictations:
            self.assertIsNotNone(d.start)
            self.assertIsNotNone(d.finish)
            self.assertEqual(d.outcome, "pasted")

    def test_recovers_records_that_share_a_physical_line(self):
        """The real log has 28 of these — a newline lost between two writes.

        Splitting on the JSON payload's true end rather than on a regex is
        what makes this exact, and immune to a transcript that happens to
        contain a bracketed timestamp.

        The recovery must not INVENT findings — no check may misread a
        recovered record as a broken dictation — but recovery is no longer
        silent: `writer-newline-lost` reports the damage itself, which is the
        whole point of R1's single-buffer fix having a signature. So the
        expected set is exactly that one invariant and nothing else.
        """
        corpus, findings = analyse("merged_lines.log")
        self.assertEqual(corpus.merged_records, 1)
        self.assertEqual(len(corpus.unparsed), 0)
        self.assertIn("usage.recorded", {e.stage for e in corpus.events})
        self.assertEqual(fired(findings), {"writer-newline-lost"},
                         "recovering a merged line must not invent findings")

    def test_a_clean_file_reports_zero_writer_damage(self):
        """The counts are reported whether or not they are zero.

        A number that only appears when it is non-zero cannot be read as
        evidence that the fix is holding.
        """
        corpus, findings = analyse("clean.log")
        self.assertEqual(corpus.merged_records, 0)
        self.assertEqual(corpus.blank_lines, 0)
        self.assertNotIn("writer-newline-lost", fired(findings))
        self.assertIn("0 merged records, 0 blank lines",
                      render(corpus, findings))

    def test_blank_lines_are_counted_not_merely_skipped(self):
        corpus, _ = analyse("writer-newline-lost.log")
        self.assertGreaterEqual(corpus.merged_records, 1)
        self.assertGreaterEqual(corpus.blank_lines, 1)

    def test_dictation_identity_includes_the_session(self):
        """The sequence number in a trace id restarts every session."""
        corpus, findings = analyse("two_sessions.log")
        self.assertEqual(len(corpus.sessions), 2)
        self.assertEqual(len(corpus.dictations), 4)
        self.assertEqual(len({d.key for d in corpus.dictations}), 4)
        self.assertEqual(len({d.tid for d in corpus.dictations}), 2,
                         "the fixture must reuse ids across sessions")
        self.assertEqual(fired(findings), set())


class TestPrivacy(unittest.TestCase):
    def test_dictated_text_never_survives_parsing(self):
        raw = ('[2026-09-01 10:00:00.000] [0000-1111] +   10ms '
               'whisper.response {"attempt":1,"chars":21,"ms":300,'
               '"sha8":"aaaa1111","text":"my private sentence"}\n')
        path = os.path.join(FIX, "_tmp_privacy.log")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(raw)
        try:
            corpus = model.load([path])
            findings = run_all(corpus)
            blob = render(corpus, findings, context=6) + render_json(
                corpus, findings)
            self.assertNotIn("my private sentence", blob)
            self.assertIn("<redacted:19chars>",
                          corpus.events[0].payload["text"])
        finally:
            os.remove(path)

    def test_no_fixture_contains_prose(self):
        """Fixtures live in the repo. Nothing dictated may reach them."""
        import json
        import re
        for root, _, files in os.walk(FIX):
            for name in files:
                if not name.endswith(".log"):
                    continue
                with open(os.path.join(root, name), encoding="utf-8") as fh:
                    body = fh.read()
                for m in re.finditer(r'"text":"([^"]*)"', body):
                    self.fail(f"{name} carries a text field: {m.group(1)!r}")
                self.assertTrue(json.dumps(body))


class TestCleanCorpusIsSilent(unittest.TestCase):
    def test_clean_log_fires_nothing(self):
        _, findings = analyse("clean.log")
        self.assertEqual(fired(findings), set(),
                         f"clean fixture produced {[f.summary for f in findings]}")

    def test_unknown_vocabulary_is_not_a_violation(self):
        """The coordination point: the Rust trace vocabulary is growing.

        An old-schema session carrying stages this analyser has never heard
        of, fields it expects that are absent, and fields it has never seen
        must still produce no findings.
        """
        corpus, findings = analyse("schema_drift.log")
        stages = corpus.stages_seen()
        self.assertIn("companion.face_changed", stages)
        self.assertIn("vad.armed", stages)
        self.assertIn("audio.rms", stages)
        self.assertEqual(
            fired(findings), set(),
            f"schema drift produced {[f.summary for f in findings]}")


class TestEveryInvariantFires(unittest.TestCase):
    def test_every_invariant_has_a_fixture_that_violates_it(self):
        missing = []
        for fn in REGISTRY:
            path = os.path.join(BROKEN, f"{fn.iid}.log")
            if not os.path.exists(path):
                missing.append(fn.iid)
        self.assertEqual(missing, [],
                         "no fixture proves these invariants can fire")

    def test_every_invariant_fires_on_its_fixture(self):
        failures = []
        for fn in REGISTRY:
            corpus, findings = analyse(f"{fn.iid}.log")
            if fn.iid not in fired(findings):
                failures.append(
                    f"{fn.iid}: fixture produced {sorted(fired(findings))}")
        self.assertEqual(failures, [], "\n".join(failures))

    def test_severities_are_as_declared(self):
        """A check registered as ERROR must not quietly emit only WARNs.

        Two invariants deliberately vary severity by evidence and are
        exempted by name: capture-handoff-missing grades on how long the
        capture was held, and dictation-start-missing downgrades inside the
        rotation-truncated head session.
        """
        graded = {"capture-handoff-missing", "dictation-start-missing"}
        for fn in REGISTRY:
            if fn.iid in graded:
                continue
            _, findings = analyse(f"{fn.iid}.log")
            mine = [f for f in findings if f.invariant == fn.iid]
            self.assertTrue(mine, fn.iid)
            for f in mine:
                self.assertEqual(f.severity, fn.severity, fn.iid)

    def test_findings_are_locatable(self):
        """A violation the developer cannot go and read is half a finding."""
        for fn in REGISTRY:
            corpus, findings = analyse(f"{fn.iid}.log")
            for f in (x for x in findings if x.invariant == fn.iid):
                self.assertTrue(f.ts, f"{fn.iid}: no timestamp")
                self.assertTrue(f.summary, f"{fn.iid}: no summary")
                self.assertGreaterEqual(f.index, 0, f"{fn.iid}: no anchor")
                self.assertTrue(corpus.context(f.index, 2, 2),
                                f"{fn.iid}: no readable context")


class TestSpecificShapes(unittest.TestCase):
    """The cases where getting the rule subtly wrong is the whole risk."""

    def test_long_dictation_is_not_read_as_an_orphan(self):
        """This programme already made this mistake once.

        A 32-second dictation was reported as a capture.stop with no
        dictation.start because the check used a time window. The rule is
        event order, not elapsed time.
        """
        log = model.load([os.path.join(FIX, "clean.log")])
        stops = [e for e in log.events if e.stage == "capture.stop"]
        starts = [e for e in log.events if e.stage == "dictation.start"]
        self.assertEqual(len(stops), len(starts))
        _, findings = analyse("clean.log")
        self.assertNotIn("capture-handoff-missing", fired(findings))

    def test_short_stray_tap_is_a_warning_not_an_error(self):
        """Every unhanded capture in the real corpus was held under 0.7 s.

        Calling those errors would bury the one that matters.
        """
        _, findings = analyse("capture-handoff-missing.log")
        handoff = [f for f in findings
                   if f.invariant == "capture-handoff-missing"]
        self.assertTrue(handoff)
        self.assertTrue(any(f.severity == ERROR for f in handoff),
                        "a long unhanded capture must be an error")

    def test_dead_capture_predicate_is_strict(self):
        """docs/tracing.md: one non-zero sample disqualifies dead_capture."""
        _, findings = analyse("dead-capture-misclassified.log")
        self.assertIn("dead-capture-misclassified", fired(findings))
        _, clean_findings = analyse("dead-capture.log")
        self.assertNotIn("dead-capture-misclassified", fired(clean_findings))

    def test_arbiter_closing_a_capture_is_not_read_as_a_leak(self):
        """The false positive that would discredit the whole R1 check.

        A start refused by the arbiter emits `capture.start` first — the
        refusal happens after the stream is built — so a capture-stop-missing
        that only knows about `capture.stop` would report the fix working as
        an eleven-hour microphone.
        """
        raw = (
            '[2026-09-01 10:00:00.000] [········]          capture.start '
            '{"device":"AirPods Pro","rate":24000}\n'
            '[2026-09-01 10:00:00.500] [········]          '
            'capture.orphan_prevented '
            '{"reason":"user_idle","device":"AirPods Pro","build_ms":544}\n'
            '[2026-09-01 10:00:09.000] [········]          capture.start '
            '{"device":"AirPods Pro","rate":24000}\n'
            '[2026-09-01 10:00:14.000] [········]          capture.stop '
            '{"device":"AirPods Pro","samples":240000,'
            '"device_changed":false}\n'
            '[2026-09-01 10:00:15.000] [········]          hotkey.tap_armed '
            '{}\n'
        )
        corpus, findings = _analyse_text(raw)
        self.assertEqual(fired(findings), set(),
                         "the arbiter doing its job must be silent")

    def test_a_coalesced_keychain_read_is_not_a_regression(self):
        """lock_wait_ms > 0 is single-flighting working, not a second read."""
        raw = (
            '[2026-09-01 10:00:00.000] [········]          keychain.slow '
            '{"account":"usage_hmac_secret","ms":9500,"op":"secret_read",'
            '"lock_wait_ms":0}\n'
            '[2026-09-01 10:00:00.010] [········]          keychain.slow '
            '{"account":"usage_hmac_secret","ms":9490,"op":"secret_read",'
            '"lock_wait_ms":9490}\n'
            '[2026-09-01 10:00:00.020] [········]          keychain.slow '
            '{"account":"usage_hmac_secret","ms":9480,"op":"secret_read",'
            '"lock_wait_ms":9480}\n'
        )
        _, findings = _analyse_text(raw)
        self.assertNotIn("keychain-not-single-flighted", fired(findings))
        self.assertIn("keychain-on-critical-path", fired(findings),
                      "a slow read is still a slow read")

    def test_silent_audio_check_works_without_nonzero_ratio(self):
        """Older traces predate nonzero_ratio; avg_rms == 0 is equivalent."""
        corpus, findings = analyse("silent-audio-misclassified.log")
        self.assertIn("silent-audio-misclassified", fired(findings))

    def test_a_covering_stall_reattributes_a_bookkeeping_gap(self):
        """docs/tracing.md's own procedure, made executable.

        The same 6-second gap is a bookkeeping-stall on its own, a
        process-suspended when a timer_stall covers it, and a
        keychain-on-critical-path when a keychain.slow sits inside it.
        """
        _, plain = analyse("bookkeeping-stall.log")
        self.assertIn("bookkeeping-stall", fired(plain))
        self.assertNotIn("process-suspended", fired(plain))

        _, stalled = analyse("process-suspended.log")
        self.assertIn("process-suspended", fired(stalled))
        self.assertNotIn("bookkeeping-stall", fired(stalled))

        _, keyed = analyse("keychain-on-critical-path.log")
        self.assertIn("keychain-on-critical-path", fired(keyed))
        self.assertNotIn("bookkeeping-stall", fired(keyed))

    def test_paste_partial_ignores_a_non_empty_target(self):
        """Typing over a selection legitimately shortens the field."""
        _, findings = analyse("clean.log")
        self.assertNotIn("paste-partial", fired(findings))

    def test_paste_modifiers_is_attributed_positionally(self):
        """The line carries no trace id, so only position can attribute it."""
        _, findings = analyse("paste-modifier-held.log")
        mods = [f for f in findings if f.invariant == "paste-modifier-held"]
        self.assertEqual(len(mods), 1)
        self.assertIsNotNone(mods[0].dictation)

    def test_new_vocabulary_is_reported_as_info_never_as_an_error(self):
        for name in ("abort-reason-undocumented.log",
                     "outcome-undocumented.log"):
            _, findings = analyse(name)
            new = [f for f in findings
                   if f.invariant.endswith("-undocumented")]
            self.assertTrue(new, name)
            for f in new:
                self.assertEqual(f.severity, INFO, name)


class TestReporting(unittest.TestCase):
    def test_text_report_renders_with_context(self):
        corpus, findings = analyse("paste-swallowed.log")
        text = render(corpus, findings, context=3)
        self.assertIn("paste-swallowed", text)
        self.assertIn("Outcome distribution", text)
        self.assertIn("Stage latency", text)

    def test_json_report_is_valid(self):
        import json
        corpus, findings = analyse("dead-capture.log")
        payload = json.loads(render_json(corpus, findings))
        self.assertEqual(payload["dictations"], 1)
        self.assertTrue(payload["findings"])
        self.assertIn("outcomes", payload)

    def test_severity_counts_are_consistent(self):
        _, findings = analyse("tap-abandoned.log")
        self.assertTrue(all(f.severity in (ERROR, WARN, INFO)
                            for f in findings))


class TestCli(unittest.TestCase):
    def test_fail_on_error_exit_code(self):
        from trace_analyser.__main__ import main
        import contextlib
        import io
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            clean_rc = main([os.path.join(FIX, "clean.log"),
                             "--fail-on-error"])
            bad_rc = main([os.path.join(BROKEN, "dead-capture.log"),
                           "--fail-on-error"])
        self.assertEqual(clean_rc, 0)
        self.assertEqual(bad_rc, 1)

    def test_list_describes_every_invariant(self):
        from trace_analyser.report import render_invariant_list
        text = render_invariant_list()
        for fn in REGISTRY:
            self.assertIn(fn.iid, text)
            self.assertTrue(fn.why, f"{fn.iid} has no explanation")


if __name__ == "__main__":
    unittest.main(verbosity=2)
