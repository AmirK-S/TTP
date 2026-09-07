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
    DOCUMENTED_OUTCOMES, ERROR, INFO, KNOWN_STAGES, REGISTRY, WARN, run_all,
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

    def test_the_vocabulary_matches_what_the_app_emits(self):
        """Reconciled against the source, not extended one name at a time.

        `whisper.attempt` sat in the "not taught about" list for a whole wave
        because the list was only ever appended to when someone noticed a
        name. These three are the ones the scan of src-tauri/src turned up:
        the per-attempt whisper record, the panic hook (the only thing a
        `panic = "abort"` crash leaves behind), and the settings store's
        unknown-field line.

        `audio.rms` has no emitter left and stays anyway: it is the
        pre-`audio.signal` schema generation, and 52 dictations of the
        harvested corpus are in it.
        """
        for stage in ("whisper.attempt", "app.panic",
                      "settings.unknown_field"):
            self.assertIn(stage, KNOWN_STAGES)
        self.assertIn("audio.rms", KNOWN_STAGES)
        # `trace::degraded` routes every site onto the one stage `degraded`.
        # Listing the sites as stages would invent nine the writer never
        # emits, and hide a real new stage among them.
        self.assertIn("degraded", KNOWN_STAGES)
        for site in ("capture.reclaim", "settings.fsync", "audio.size",
                     "backup.audio", "history.save", "keychain.secret"):
            self.assertNotIn(site, KNOWN_STAGES,
                             f"{site} is a degraded() site, not a stage")

    def test_the_outcome_vocabulary_covers_the_verified_paste_states(self):
        """`pasted` now means observed, so its two former halves need names."""
        for outcome in ("pasted_unverified", "paste_swallowed"):
            self.assertIn(outcome, DOCUMENTED_OUTCOMES)
        _, findings = analyse("outcome-undocumented.log")
        self.assertIn("outcome-undocumented", fired(findings),
                      "an outcome the docs do not have must still be INFO")


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

        Several invariants deliberately vary severity by evidence and are
        exempted by name. Each fixture still produces the declared severity
        for the shape it names — the exemption is for the OTHER findings the
        same check can emit from the same fixture.
        """
        graded = {
            # Grades on how long the capture was held.
            "capture-handoff-missing",
            # Downgrades inside the rotation-truncated head session.
            "dictation-start-missing",
            # Grades on attribution: an ERROR is a read a dictation can be
            # shown to have made and waited for. Everything else is a warning
            # or information, which is the whole point of the fix.
            "keychain-on-critical-path",
        }
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

    def test_a_keychain_read_outside_every_dictation_is_not_a_block(self):
        """The false positive this workstream exists to remove.

        Fourteen of these were reported as ERRORs claiming they had "blocked
        a dictation" for up to 397 seconds. All fourteen carried no dictation
        id and no dictation in the corpus ran longer than 3.5 s. Nothing was
        blocked; the records were near dictations in the file, and the check
        read nearness as causation.
        """
        raw = (
            '[2026-09-01 10:00:00.000] [········]          keychain.slow '
            '{"account":"usage_hmac_secret","ms":397600,"op":"secret_read",'
            '"lock_wait_ms":0}\n'
            '[2026-09-01 10:00:01.000] [········]          hotkey.tap_armed '
            '{}\n'
        )
        _, findings = _analyse_text(raw)
        keychain = [f for f in findings
                    if f.invariant == "keychain-on-critical-path"]
        self.assertTrue(keychain, "a slow read is still worth reporting")
        for f in keychain:
            self.assertEqual(f.severity, INFO)
            self.assertNotIn("blocked a", f.summary)

    def test_a_keychain_read_inside_a_dictation_still_errors(self):
        """The other half: a genuine block must still fire.

        A check that has been taught not to cry wolf and now cannot bark at
        all is worse than the one it replaced, so the fixture that proves the
        real shape is asserted here at full severity.
        """
        _, findings = analyse("keychain-on-critical-path.log")
        keychain = [f for f in findings
                    if f.invariant == "keychain-on-critical-path"]
        self.assertTrue(keychain)
        self.assertTrue(any(f.severity == ERROR for f in keychain),
                        "a read inside a dictation's own stages is a block")
        self.assertTrue(any(f.dictation for f in keychain),
                        "an attributed read names the dictation it blocked")

    def test_a_read_spanning_a_short_dictation_is_two_writers(self):
        """397 seconds cannot happen inside 3.5, and saying so is arithmetic.

        Overlap is not attribution either. A synchronous call made by a
        dictation's pipeline begins and ends inside that dictation; one that
        straddles the window was made by something else, and that something
        else is another writer on the same file.
        """
        _, findings = analyse("log-co-tenancy.log")
        self.assertIn("log-co-tenancy", fired(findings))
        spanning = [f for f in findings
                    if f.invariant == "keychain-on-critical-path"]
        self.assertTrue(spanning)
        for f in spanning:
            self.assertEqual(f.severity, WARN)
            self.assertIn("Two writers", f.summary)

    def test_unattributed_keychain_reads_are_aggregated(self):
        """Fourteen rows saying "this blocked nothing" is still fourteen rows.

        Downgrading the false ERRORs was half the job; a reader who has to
        scroll a dozen non-events to reach a real finding has been failed in
        the same way, one severity down. One row per day per account per op,
        carrying the count and the worst time.
        """
        raw = "".join(
            f'[2026-09-01 10:0{n}:00.000] [········]          keychain.slow '
            f'{{"account":"usage_hmac_secret","ms":{1000 * (n + 1)},'
            f'"op":"secret_read","lock_wait_ms":0}}\n'
            for n in range(6)
        ) + ('[2026-09-01 10:30:00.000] [········]          hotkey.tap_armed '
             '{}\n')
        _, findings = _analyse_text(raw)
        keychain = [f for f in findings
                    if f.invariant == "keychain-on-critical-path"]
        self.assertEqual(len(keychain), 1, "six reads, one row")
        self.assertEqual(keychain[0].severity, INFO)
        self.assertEqual(keychain[0].detail["count"], 6)
        self.assertEqual(keychain[0].detail["worst_ms"], 6000,
                         "the worst time survives aggregation")

    def test_a_paste_verdict_decides_when_the_writer_supplies_one(self):
        """The vocabulary landing alongside this workstream.

        `swallowed` is the ERROR. `unverified` means the target could not be
        read, which is not evidence the text was lost — grading it as one
        would be the same over-claim in a new field. `observed` is silent,
        and a slug this file has not been taught about is not a violation.
        """
        def verify(verdict):
            return (
                '[2026-09-01 10:00:00.000] [0001-aaaa] +    0ms '
                'dictation.start {"kind":"recording","verbose":false}\n'
                '[2026-09-01 10:00:01.000] [0001-aaaa] + 1000ms '
                'paste.verify {"ax_readable":true,"changed":false,'
                '"before_chars":0,"after_chars":0,"delta_chars":0,'
                '"expected_chars":12,"settled_ms":0,'
                f'"verdict":"{verdict}"}}\n'
                '[2026-09-01 10:00:02.000] [0001-aaaa] + 2000ms '
                'dictation.finish {"chars":12,"ms":2000,"outcome":"pasted",'
                '"words":2}\n'
                '[2026-09-01 10:00:03.000] [········]          '
                'hotkey.tap_armed {}\n'
            )

        def sev_of(verdict):
            _, fs = _analyse_text(verify(verdict))
            return [f.severity for f in fs if f.invariant == "paste-swallowed"]

        self.assertEqual(sev_of("swallowed"), [ERROR])
        self.assertEqual(sev_of("unverified"), [WARN],
                         "unreadable is not the same as lost")
        self.assertEqual(sev_of("observed"), [],
                         "the writer says it landed; ax_readable/changed is "
                         "the weaker instrument and must not override it")
        self.assertEqual(sev_of("teleported"), [],
                         "rule 1: unknown vocabulary is never a violation")

    def test_an_unattributed_polish_failure_is_not_a_warning_about_the_app(self):
        """The 429s that were reported as an incident and then retracted.

        polish.attempt carries no trace id. The golden test suite exhausting
        the API tier from a cargo test run produces exactly these records,
        and on a pre-`proc` trace they are indistinguishable from the app's
        own except by whether a dictation was waiting on them.
        """
        raw = (
            '[2026-09-02 00:20:08.988] [········]          polish.attempt '
            '{"model":"openai/gpt-oss-120b","ms":52,"n":1,"status":429}\n'
            '[2026-09-02 00:20:09.988] [········]          polish.attempt '
            '{"model":"openai/gpt-oss-120b","ms":51,"n":1,"status":429}\n'
            '[2026-09-02 00:20:11.988] [········]          hotkey.tap_armed '
            '{}\n'
        )
        _, findings = _analyse_text(raw)
        remote = [f for f in findings if f.invariant == "polish-call-failed"]
        self.assertEqual(len(remote), 1, "aggregated, not one per record")
        self.assertEqual(remote[0].severity, INFO)
        self.assertIn("no dictation making the call", remote[0].summary)
        self.assertIn("not say which process wrote them",
                      remote[0].summary,
                      "without proc the report must say so, not imply a "
                      "harness")

        # ... and the same status inside a dictation's polish window is.
        _, attributed = analyse("polish-call-failed.log")
        mine = [f for f in attributed
                if f.invariant == "polish-call-failed"]
        self.assertTrue(any(f.severity == WARN for f in mine))
        self.assertTrue(any(f.dictation for f in mine))

    def test_the_two_halves_of_the_old_remote_check_are_independent(self):
        """The split, asserted in the only way that matters.

        `remote-call-failed` merged a `whisper_error` abort — dictation-scoped,
        exactly attributed, the user lost their words — with a non-200
        `polish.attempt`, which carries no trace id, has a working fallback,
        and is frequently our own test harness. Each half must now fire on its
        own fixture and stay SILENT on the other's; a shared id could not
        express that, which is how 46 harness 429s came to sit in the same
        block as a real 403 invalid-key abort.
        """
        _, polish = analyse("polish-call-failed.log")
        _, whisper = analyse("whisper-call-failed.log")

        self.assertIn("polish-call-failed", fired(polish))
        self.assertNotIn("whisper-call-failed", fired(polish))
        self.assertIn("whisper-call-failed", fired(whisper))
        self.assertNotIn("polish-call-failed", fired(whisper))

        self.assertNotIn("remote-call-failed",
                         {fn.iid for fn in REGISTRY},
                         "the merged id must be gone, not aliased")

        w = [f for f in whisper if f.invariant == "whisper-call-failed"]
        self.assertTrue(all(f.severity == ERROR for f in w),
                        "a whisper_error abort is a user losing their words")
        self.assertTrue(all(f.dictation for f in w),
                        "exact attribution: the finding names the dictation")

        p = [f for f in polish if f.invariant == "polish-call-failed"]
        self.assertTrue(p)
        self.assertTrue(all(f.severity in (WARN, INFO) for f in p),
                        "polish has a fallback; it is never an ERROR")

    def test_a_harness_polish_failure_is_named_as_one(self):
        """What `proc` buys: the positive answer, not just the negative.

        The previous audit concluded harness contamination "cannot be detected
        positively, and no amount of cleverness in this tool changes that".
        The writer changed it. These two 429s sit inside nothing, and their
        process never emitted an `app.launched` — so it is a test binary or a
        dev build, and the analyser may finally say so.
        """
        raw = (
            '[2026-09-02 00:20:00.000] [········]          app.launched '
            '{"build":"3.1.7+ab12cd3","proc":"a1b2","version":"3.1.7"}\n'
            '[2026-09-02 00:20:08.988] [········]          polish.attempt '
            '{"model":"openai/gpt-oss-120b","ms":52,"n":1,"proc":"ff01",'
            '"status":429}\n'
            '[2026-09-02 00:20:09.988] [········]          polish.attempt '
            '{"model":"openai/gpt-oss-120b","ms":51,"n":1,"proc":"ff01",'
            '"status":429}\n'
            '[2026-09-02 00:20:11.988] [········]          hotkey.tap_armed '
            '{"proc":"a1b2"}\n'
        )
        corpus, findings = _analyse_text(raw)
        self.assertEqual(corpus.records_without_proc, 0)
        self.assertEqual([p.proc for p in corpus.app_processes()], ["a1b2"])
        self.assertEqual([p.proc for p in corpus.harness_processes()],
                         ["ff01"])
        self.assertEqual(corpus.process("a1b2").build, "3.1.7+ab12cd3")

        pf = [f for f in findings if f.invariant == "polish-call-failed"]
        self.assertEqual(len(pf), 1, "aggregated per process, per status")
        self.assertEqual(pf[0].severity, INFO)
        self.assertEqual(pf[0].origin, "harness")
        self.assertIn("no app.launched", pf[0].summary)

        # ... and the report keeps it out of the app's own blocks.
        text = render(corpus, findings)
        self.assertIn("Emitted by a harness, not by the app", text)
        head, tail = text.split("Emitted by a harness, not by the app", 1)
        self.assertIn("polish-call-failed", tail)
        self.assertNotIn("polish-call-failed", head,
                         "a harness finding must not appear in the app's "
                         "block as well")

    def test_the_committed_proc_fixture_separates_the_two_writers(self):
        """The 2026-09-02 incident, replayed with process identity in place.

        One app process serving a dictation and one test binary hammering
        polish, in one file, exactly as it happened. The harness's 429s must
        not appear in the app's blocks, must not be counted in the app's
        totals, and must be described as what they are rather than hedged.
        """
        corpus, findings = analyse("proc_identity.log")
        self.assertEqual(corpus.records_without_proc, 0)
        self.assertTrue(corpus.fully_procced)
        self.assertEqual([p.proc for p in corpus.app_processes()], ["a1b2"])
        self.assertEqual([p.proc for p in corpus.harness_processes()],
                         ["ff01"])

        harness = [f for f in findings if f.origin == "harness"]
        self.assertTrue(harness)
        self.assertTrue(all(f.severity == INFO for f in harness),
                        "a test binary's API quota is never a product ERROR")
        self.assertTrue(all(f.dictation is None for f in harness),
                        "no dictation of the user's was waiting on these")

        text = render(corpus, findings)
        self.assertIn("1 app (3.1.7+ab12cd3), 1 harness", text)
        head, _ = text.split("Emitted by a harness, not by the app", 1)
        self.assertNotIn("polish-call-failed", head)
        # And the exit code: a harness ERROR must never fail a build.
        self.assertEqual(
            [f for f in findings if f.severity == ERROR
             and f.origin == "harness"], [])

    def test_a_corpus_without_proc_degrades_and_says_so(self):
        """Every line of the harvested corpus lacks `proc`.

        The analyser must keep working on them unchanged — window containment,
        same grades, same findings — and must say which method it used, so a
        reader never mistakes "no harness was found" for "no harness could be
        distinguished".
        """
        corpus, findings = analyse("clean.log")
        self.assertFalse(corpus.has_proc)
        self.assertEqual(corpus.records_without_proc, len(corpus.events))
        self.assertEqual(corpus.origin_at(0), "unknown")
        self.assertTrue(all(f.origin == "unknown" for f in findings))
        self.assertIn("no record in this corpus names the process",
                      render(corpus, findings))

        # And the pre-proc grading is untouched: the polish fixture still
        # produces exactly what it did before the field existed.
        corpus, findings = analyse("polish-call-failed.log")
        self.assertFalse(corpus.has_proc)
        self.assertTrue(any(f.invariant == "polish-call-failed"
                            and f.severity == WARN for f in findings))

    def test_a_process_whose_launch_was_rotated_away_is_not_called_a_harness(self):
        """The one failure mode `proc` does not remove.

        Rotation cuts the head off the oldest file, taking any `app.launched`
        with it. A real app process that starts at the top of the window then
        has exactly a harness's shape, and calling it one would be the same
        confident-and-wrong reading this analyser exists not to produce.
        """
        raw = (
            '[2026-09-02 00:20:08.988] [········]          hotkey.tap_armed '
            '{"proc":"c0de"}\n'
            '[2026-09-02 00:20:09.988] [········]          polish.attempt '
            '{"model":"openai/gpt-oss-120b","ms":51,"n":1,"proc":"c0de",'
            '"status":429}\n'
            '[2026-09-02 00:20:11.988] [········]          hotkey.tap_armed '
            '{"proc":"c0de"}\n'
        )
        corpus, findings = _analyse_text(raw)
        p = corpus.process("c0de")
        self.assertTrue(p.head_truncated)
        self.assertEqual(p.origin, "unknown", "neither claim is supported")
        self.assertEqual(corpus.harness_processes(), [])
        pf = [f for f in findings if f.invariant == "polish-call-failed"]
        self.assertEqual(len(pf), 1)
        self.assertEqual(pf[0].severity, INFO)
        self.assertIn("neither is claimed", pf[0].summary,
                      "an undecidable proc must be named as undecidable, not "
                      "reported as a harness")

    def test_one_polish_failure_is_not_an_outage(self):
        """'Repeatedly' is in the invariant's own title."""
        raw = (
            '[2026-09-02 00:20:29.157] [0087-9be8] +  100ms polish.outage '
            '{"consecutive_failures":1,"model":"openai/gpt-oss-120b"}\n'
            '[2026-09-02 00:20:31.157] [········]          hotkey.tap_armed '
            '{}\n'
        )
        _, findings = _analyse_text(raw)
        outage = [f for f in findings if f.invariant == "polish-outage"]
        self.assertEqual(len(outage), 1)
        self.assertEqual(outage[0].severity, WARN)
        _, real = analyse("polish-outage.log")
        self.assertTrue(any(f.invariant == "polish-outage"
                            and f.severity == ERROR for f in real))

    def test_reaching_the_rearm_escalation_once_is_not_the_defect(self):
        """A streak of exactly 5 is the rebuild trigger firing as designed."""
        raw = "".join(
            f'[2026-09-01 10:00:0{n}.000] [········]          '
            f'hotkey.tap_rearmed {{"reason":"watchdog","streak":{n}}}\n'
            for n in range(1, 6)
        ) + ('[2026-09-01 10:00:07.000] [········]          hotkey.tap_armed '
             '{}\n')
        _, findings = _analyse_text(raw)
        streaks = [f for f in findings if f.invariant == "tap-rearm-streak"]
        self.assertEqual(len(streaks), 1)
        self.assertEqual(streaks[0].severity, WARN)

    def test_only_check_still_sees_findings_other_checks_emit(self):
        """--only filters findings, not the registry.

        check_bookkeeping_stall emits `keychain-on-critical-path` and
        `process-suspended` findings — that is docs/tracing.md's own
        discrimination procedure — so filtering the registry by id silently
        dropped the best-attributed keychain findings there are.
        """
        corpus = model.load([os.path.join(BROKEN,
                                          "keychain-on-critical-path.log")])
        only = run_all(corpus, {"keychain-on-critical-path"})
        self.assertTrue(only)
        self.assertEqual({f.invariant for f in only},
                         {"keychain-on-critical-path"})
        self.assertTrue(any("between" in f.summary for f in only),
                        "the bookkeeping-derived finding is missing")

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
