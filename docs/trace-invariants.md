# The trace-log invariant analyser

`docs/tracing.md` describes what a failure looks like in `ttp-trace.log` — "a
`capture.stop` with no `dictation.start` after it", "`ax_readable:true` with
`changed:false` means the keystrokes were swallowed", "a climbing `streak`
means re-arming is not working". Those sentences are correct and nobody
applies them, because applying them means reading fifteen thousand lines by
eye.

This tool makes them executable. Every documented shape is a check; every
check runs against the real log the app is writing right now. That turns
`docs/tracing.md` from a document into an integration suite over production
data — the one test surface a unit test cannot reach, and where all seven of
this project's defects actually lived.

It reads the log. It changes nothing. It needs no build, no dependencies, and
no running app.

## Running it

```sh
python3 scripts/check_trace.py                  # the whole rotated set
python3 scripts/check_trace.py --context 4      # with surrounding events
python3 scripts/check_trace.py --list           # what each invariant means
python3 scripts/check_trace.py --json           # machine-readable
python3 scripts/check_trace.py --only dead-capture --context 6
python3 scripts/check_trace.py path/to/a.log path/to/b.log   # explicit files
python3 scripts/check_trace.py --fail-on-error  # exit 1 if any ERROR fired
```

With no arguments it reads **the whole rotated set** — `ttp-trace.log.3`
through `ttp-trace.log`, oldest first. That matters: at the time of writing,
the live file held 17 lines of a 15,569-line corpus. A tool that silently
analysed 0.1% of the evidence would be worse than none.

Python 3.10+, standard library only.

## What it prints

1. **Corpus summary** — files, events, sessions, dictations, time span, and a
   `writer` line giving the merged-record and blank-line counts. Those two are
   printed whether or not they are zero: the parser repairs both silently, and
   a number that only appears when something is wrong cannot be read as
   evidence that nothing is.
2. **Stages it has not been taught about** — informational, never a
   violation. See "Adding an invariant" below.
3. **Outcome distribution** — every dictation by outcome and abort reason.
4. **Stage latency** — p50/p95/p99/max of the gap between each pair of
   adjacent stages, plus the remote calls. The keychain defect was a *latency*
   bug; latency bugs hide in event logs nobody aggregates, and a p95 is where
   the next one shows up first.
5. **Findings**, grouped by invariant, each with its dictation id and
   timestamp so `grep <id> ttp-trace.log*` is the next step.

## Severity

| | Meaning |
|---|---|
| **ERROR** | A documented invariant was violated. Something is wrong, or was. |
| **WARN** | A shape worth a human look. Often benign; sometimes the first sign. |
| **INFO** | New vocabulary, not a defect. A stage or reason slug the docs do not yet list. |

An ERROR is not automatically a live bug — the harvest window is running an
unmodified 3.1.6, so the signatures of already-fixed defects are still in the
corpus and still fire. Read the timestamp before reading the severity.

## The invariants

Thirty-seven checks. `--list` prints the full reasoning for each; this is the
map.

Three of them — `capture-arbiter-left-live`, `keychain-not-single-flighted`
and `writer-newline-lost` — are a different kind from the rest. Every other
check asserts the *absence* of a failure that has happened. Those three assert
that a fix which has shipped is still engaged, so a hit means a regression
rather than a historical scar. They are marked **(regression)** below.

### Lifecycle — does every dictation have a shape

| id | Asserts |
|---|---|
| `dictation-finish-missing` | Every `dictation.start` has a `dictation.finish`. Tail-truncated dictations at the end of the corpus are exempt. |
| `dictation-start-missing` | Every `dictation.finish` has a `dictation.start`. Info inside the rotation-truncated first session. |
| `capture-handoff-missing` | Every `capture.stop` is followed by a `dictation.start`. **This is `tracing.md`'s "one shape the trace can only bound, not explain".** |
| `capture-stop-missing` | Every `capture.start` is followed by a `capture.stop` — or by one of the arbiter's closes (`capture.orphan_prevented`, `capture.orphan_reclaimed`, `capture.stale_dropped`). Otherwise the microphone was left live. |
| `capture-arbiter-left-live` | **(regression)** When the arbiter refuses or reclaims a stream, the microphone actually goes off. See below. |
| `capture-stop-without-start` | `capture.stop` only fires against an open capture. |
| `paste-result-missing` | Every `paste.decision` is followed by a `paste.result`. This is the signature audit item A4 names for a panic under `panic = "abort"`. |
| `paste-verify-missing` | Every successful `paste.result` is followed by a `paste.verify`. |
| `state-chain-broken` | `state.transition.from` matches the previous transition's `.to`. |
| `state-parked-processing` | No session ends parked in Processing, which makes every later press a silent no-op. |

### Vocabulary — does the log say what the docs say it says

| id | Asserts |
|---|---|
| `abort-reason-missing` | Every `outcome:"aborted"` carries a reason slug. |
| `abort-reason-undocumented` | *(info)* The slug appears in `tracing.md` or `trace-api.md`. |
| `outcome-undocumented` | *(info)* The outcome appears in the docs. |

### The microphone — the AirPods defect

| id | Asserts |
|---|---|
| `dead-capture` | No dictation ends because the device delivered digital silence. |
| `dead-capture-misclassified` | `dead_capture` implies `nonzero_ratio == 0`. `tracing.md` makes this predicate deliberately strict: one non-zero sample disqualifies it, because telling a user their microphone is broken is a strong claim. |
| `silent-audio-misclassified` | `silent_audio` implies the device produced *something*. Two witnesses, so it works on traces older than `nonzero_ratio`: `nonzero_ratio == 0`, or an `avg_rms` of exactly 0 (a mean of absolute sample values can only be zero if every sample is). |
| `long-recording-silent` | A recording of 5 s or more does not abort as silence. Nobody holds push-to-talk that long without speaking. |
| `device-changed-mid-recording` | The OS default input does not move mid recording; an open cpal stream does not follow it. |

### The text — the hallucination-filter defect

| id | Asserts |
|---|---|
| `text-emptied` | No transformation reduces the transcription to zero characters. |
| `hallucination-dropped-speech` | The filter does not drop what looks like speech: ≥3 s, `avg_rms` above the floor, and ≥20 characters returned. |
| `text-chain-broken` | Each stage's `from.sha8` matches the previous stage's `to.sha8`, so nothing rewrites the text between two traced stages. |

### Remote calls — the decommissioned model and the exhausted quota

| id | Asserts |
|---|---|
| `polish-outage` | Polish does not fail repeatedly against one model. A climbing `consecutive_failures` against a fixed model name is a model that has stopped answering. |
| `remote-call-failed` | Non-200 `polish.attempt`, or a `whisper_error` abort, reported with status and category — 403, 429 and 404 are three different bugs that look identical to the user. |
| `polish-quota-exhausted` | Polish is not skipped for lack of quota. |

### The input layer — the dead event tap

| id | Asserts |
|---|---|
| `tap-rearm-streak` | The re-arm streak does not reach 5, the point at which the Rust side gives up on `CGEventTapEnable` and rebuilds. |
| `tap-abandoned` | The input layer never gives up. After `hotkey.tap_abandoned` the Fn key does nothing until relaunch. |
| `tap-flapping-without-input` | A session with 20+ re-arms still records at least one `hotkey.press`. Carries the session duration, because over two minutes this can just mean the user pressed nothing, and over four hours it cannot. |
| `stale-fn-latched` | The Globe key does not latch held. |
| `hotkey-event-dropped` | No hotkey event is discarded under lock contention. |

### Landing — did the text arrive

| id | Asserts |
|---|---|
| `paste-swallowed` | `ax_readable:true` with `changed:false`. The app reported `pasted` and the field did not change. |
| `paste-modifier-held` | No modifier is physically held at injection time. Attributed positionally, because `paste.modifiers` carries no trace id. |
| `paste-partial` | `delta_chars` reaches `expected_chars` — checked only when `before_chars` is 0, since typing over a selection legitimately shortens the field. |

### Latency — the keychain defect

| id | Asserts |
|---|---|
| `keychain-on-critical-path` | No `keychain.slow` event, and none inside a dictation's stages. |
| `keychain-not-single-flighted` | **(regression)** At most one uncoalesced read per (session, account, op). `lock_wait_ms > 0` is coalescing working and is not counted; two or more reads that each did their own securityd call is the check-then-act shape returning. |
| `writer-newline-lost` | **(regression)** Every record occupies its own line. Merged records and blank lines are counted and printed unconditionally, so a recurrence is visible in the corpus summary before any check runs. |
| `bookkeeping-stall` | No gap over 1 s between two adjacent post-`paste.result` stages. `tracing.md`: none of that region "can take seconds, let alone minutes". |
| `process-suspended` | No `hotkey.timer_stall` overlapping a dictation in flight. |

The last three are one mechanism seen three ways, and the check follows
`tracing.md`'s own procedure rather than guessing. Given a gap in the
bookkeeping region it reports the two discriminators and lets them decide:

- a `keychain.slow` **inside** the window → `keychain-on-critical-path`, a
  blocking securityd read;
- a `hotkey.timer_stall` **covering** the window → `process-suspended`, the
  app was descheduled;
- neither → `bookkeeping-stall`, stated as evidence ("no stall covers it, no
  keychain.slow inside it, N other events logged during the window") rather
  than as a verdict. Go and read the window.


## The three that prove a fix, rather than a failure

`docs/tracing.md` describes what each defect looked like. These three describe
what the *fix* looks like, which is the harder and more useful direction:
absence of the old shape is consistent with the fix working and with the code
path never being exercised, and only one of those is worth knowing.

**`capture-arbiter-left-live`.** The old 11-hour-microphone failure is a
`capture.start` with no `capture.stop` after it, and `capture-stop-missing`
already catches that. But a start refused by the arbiter emits its
`capture.start` first — the refusal happens after the stream is built — so
`capture-stop-missing` had to be taught that `capture.orphan_prevented`,
`capture.orphan_reclaimed` and `capture.stale_dropped` each close a capture.
Without that it reports the fix *working* as an eleven-hour microphone, which
is the most expensive kind of false positive this tool can produce; the test
`test_arbiter_closing_a_capture_is_not_read_as_a_leak` exists to keep it
taught. The positive check is then: the Rust drops the cpal stream **before**
writing either line, so by the time the line exists the microphone is off and
the capture slot is empty. Three log shapes say otherwise, and each is
reported —

1. a second close (`capture.stop`, `capture.stale_dropped`,
   `capture.orphan_reclaimed`) with no `capture.start` between: the stream the
   arbiter said it dropped was still published and something else found it;
2. `degraded {"site":"capture.reclaim"}`: the arbiter and
   `audio_capture::STATE` disagree about whether a capture is live, and their
   agreement is what the whole guarantee rests on;
3. `capture.stop_waited_for_start {"timed_out":true}`: the 3-second settle
   window elapsed, which its own comment says cannot happen. Not a leak by
   itself — it means the arbiter is now the only thing between the user and a
   live microphone, and the wait is named so you can see how far off the
   assumption was.

**`keychain-not-single-flighted`.** Both keychain caches live for the whole
process and `read_once` makes at most one call however many callers arrive
together, so exactly one timed read per (session, account, op) can exist. The
defect produced eight in one process — 62,304 / 13,612 / **397,600** / 97,407
/ 157 / 56,037 / 106 / 86 ms — because each caller checked the cache, released
the lock, and then made its own unbounded securityd call. `lock_wait_ms` is
used as evidence rather than as a gate: a read with `lock_wait_ms > 0` waited
behind somebody else's and is coalescing working, so it is not counted; a read
with `lock_wait_ms == 0`, **or without the field at all** — a trace from
before the fix — did its own call and is. Two or more of those for one account
in one session is the regression. Before calling a hit a bug, read the
timestamps: a session here is a span between `app.launched` lines in one file,
and a dev or test binary appending to the same log contributes its reads to
that span with no launch line of its own.

**`writer-newline-lost`.** `append_line` used to issue two `write_all` calls,
the record and then the newline, and two writes are not one append: a second
writer landing between them produces one physical line carrying two records
and a matching blank line where the stray newline went. The corpus signature
was exact — 20 merged records against exactly 20 blank lines. The awkward part
is that **the analyser was repairing the evidence**: the parser splits merged
records on the JSON payload's true end and skips blank lines, so the damage
was invisible to everything downstream, which is a small instance of the
polite-degradation failure this whole programme is about. Both counts are now
printed in the corpus summary whether or not they are zero — a number that
only appears when it is non-zero cannot be read as evidence that the fix is
holding — and any occurrence is a finding. A hit means two writers were
appending concurrently, which is not necessarily two threads of one app: an
installed build running alongside a dev build is the normal state on this
machine, and a second process is precisely what a single writer thread cannot
serialise.

## Two things it deliberately does not do

**It does not guess at unknown stages.** The Rust trace vocabulary is growing
(`docs/trace-api.md`). Every check keys off the stages it names and ignores
the rest; unknown stages are listed once, as information. A check that cannot
see the field it needs abstains rather than reporting a violation — the corpus
already spans two schema generations (`audio.rms` before `audio.signal`,
`polish {applied}` before `polish {outcome}`) and a check that fired on the
older one would be wrong 52 times.

**It does not print dictated speech.** `diagnostics_enabled` is on, so the log
contains full transcriptions. `text` fields are replaced with a character count
at parse time, before any check or report can see them. No fixture in this
repository contains dictated speech, and a test enforces it.

## Adding an invariant

The point of this file. When a defect is found, the question is always "what
shape did it make in the log, and why did nobody see it" — and the answer
belongs here, as a check, before the fix ships.

1. **Find the shape.** Read the trace of the failure. Name the property that
   would have been false. Not "polish was broken" but "`polish.outage`'s
   `consecutive_failures` climbed against a fixed `model`".

2. **Write the check** in `scripts/trace_analyser/invariants.py`:

   ```python
   @invariant(
       "my-invariant-id", ERROR,
       "The one-line property, stated positively",
       "Why this matters, which defect it comes from, and what the reader "
       "should do about a hit. This string is printed by --list, so it is "
       "the documentation.",
   )
   def check_my_thing(corpus: Corpus):
       for d in corpus.dictations:
           if <the property is false>:
               yield Finding(
                   "my-invariant-id", ERROR,
                   f"{d.key}: what happened, with the numbers",
                   ts=str(d.finish.ts), dictation=d.key, index=_anchor(d),
                   detail={...},
               )
   ```

   Obey the two rules at the top of that file: never treat an unknown stage as
   a violation, and abstain when a field you need is absent.

3. **Add a fixture** in `scripts/trace_analyser/tests/make_fixtures.py`, in
   the `broken()` function, named exactly after the invariant id. The builders
   (`hotkey_cycle`, `dictation`, `healthy`, `tail`) assemble a well-formed
   session; most invariants need one keyword argument set wrong. Then:

   ```sh
   python3 scripts/trace_analyser/tests/make_fixtures.py
   ```

4. **Run the tests.**

   ```sh
   python3 scripts/trace_analyser/tests/test_invariants.py
   ```

   Three of them enforce the contract without you writing anything:
   `test_every_invariant_has_a_fixture_that_violates_it`,
   `test_every_invariant_fires_on_its_fixture`, and
   `test_clean_log_fires_nothing`. **An invariant checker whose checks have
   never been seen to fire is not known to work** — so a new invariant must
   both fire on a log that violates it and stay silent on one that does not,
   and the suite will fail until both are true.

5. **Add a row to the table above**, and to `docs/tracing.md` if the shape is
   new to it as well.

### When a check turns out to be wrong

Fix the check, and leave a comment saying what it used to claim. This has
already happened once in this programme: a confident "reproduced bug" was a
32-second dictation misread as an orphan because the rule used a time window.
`capture-handoff-missing` therefore scopes by *event order* — the next
`capture.start`, `app.launched`, or end of corpus — and never by elapsed time.
The test named `test_long_dictation_is_not_read_as_an_orphan` exists to keep
it that way.

A false positive reported confidently is more expensive than a missed
finding, because it costs the tool its credibility. When something fires, go
read the surrounding events with `--context` before calling it a bug.

## Files

```
scripts/check_trace.py                          entry point
scripts/trace_analyser/model.py                 parsing, sessions, dictations
scripts/trace_analyser/invariants.py            the checks
scripts/trace_analyser/stats.py                 percentiles, distributions
scripts/trace_analyser/report.py                text and JSON rendering
scripts/trace_analyser/tests/make_fixtures.py   regenerates the fixtures
scripts/trace_analyser/tests/test_invariants.py the suite
scripts/trace_analyser/tests/fixtures/          clean.log, and one per invariant
scripts/trace_analyser/tests/fixtures/.gitignore  re-includes them (see below)
```

**The fixtures were not actually committed.** The repository root `.gitignore`
has `*.log` on line 3, which swallowed every file in `fixtures/` — so the
sentence above about fixtures being committed rather than generated was true
of the intent and false of the tree, and on a fresh clone
`test_every_invariant_has_a_fixture_that_violates_it` would have failed for
all thirty-seven checks. A nested `fixtures/.gitignore` containing `!*.log`
re-includes them; the directory itself was never excluded, so the negation
works. They are synthetic logs, not runtime output, and they belong in git.
