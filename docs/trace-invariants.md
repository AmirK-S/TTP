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
python3 scripts/check_trace.py --only whisper-call-failed --only polish-call-failed
python3 scripts/check_trace.py path/to/a.log path/to/b.log   # explicit files
python3 scripts/check_trace.py --fail-on-error  # exit 1 if any ERROR fired
```

The second `--only` line is what `--only remote-call-failed` used to be. That
id no longer exists: it was two checks with two evidence standards sharing one
name, and the split gives each its own. Nothing else takes the old name, so a
script still passing it gets an empty result rather than a wrong one.

`--fail-on-error` fails on an ERROR **the app emitted**. A finding whose
records carry a `proc` that never wrote an `app.launched` came from a test
binary; it is reported, in its own block, and never sets the exit code.

With no arguments it reads **the whole rotated set** — `ttp-trace.log.3`
through `ttp-trace.log`, oldest first. That matters: at the time of writing,
the live file held 17 lines of a 15,569-line corpus. A tool that silently
analysed 0.1% of the evidence would be worse than none.

Python 3.10+, standard library only.

## What it prints

1. **Corpus summary** — files, events, sessions, dictations, time span, a
   `writer` line giving the merged-record and blank-line counts, and a `procs`
   line. The first two are printed whether or not they are zero: the parser
   repairs both silently, and a number that only appears when something is
   wrong cannot be read as evidence that nothing is. The `procs` line says how
   many processes wrote this corpus, how many of them are the app (with their
   builds) and how many are a harness — or, on a corpus written before the
   `proc` field existed, that no record names its writer at all. It is printed
   in both cases for the same reason: "no harness was found" and "no harness
   could be distinguished" are different facts and must not look alike.
2. **Stages it has not been taught about** — informational, never a
   violation. See "Adding an invariant" below.
3. **Outcome distribution** — every dictation by outcome and abort reason.
4. **Stage latency** — p50/p95/p99/max of the gap between each pair of
   adjacent stages, plus the remote calls. The keychain defect was a *latency*
   bug; latency bugs hide in event logs nobody aggregates, and a p95 is where
   the next one shows up first.
5. **Findings**, grouped by (severity, invariant), each with its dictation id
   and timestamp so `grep <id> ttp-trace.log*` is the next step.
6. **Findings emitted by a harness, not by the app** — a separate block at the
   end, present only when some record's `proc` names a process that never
   wrote an `app.launched`. These are not findings about the product: they are
   not counted in the totals on the "invariants checked" line, and they cannot
   set the `--fail-on-error` exit code. They are still printed, because a test
   run that exhausts the API tier is worth knowing about — and because the 46
   records this block exists for were once reported to the maintainer as a
   production incident.

## Severity

| | Meaning |
|---|---|
| **ERROR** | **A user was affected, or the code is wrong.** Not "a rule tripped" — the evidence has to support the claim of harm. |
| **WARN** | A shape worth a human look. Anything that might equally be a dev artefact, a suspended laptop, or an unattributable coincidence lives here. |
| **INFO** | Something happened and nobody was waiting on it. New vocabulary; a slow call outside every dictation; a stated ambiguity. |

An ERROR is not automatically a live bug — the harvest window is running an
unmodified 3.1.6, so the signatures of already-fixed defects are still in the
corpus and still fire. Read the timestamp before reading the severity.

**Severity is decided per finding, not per invariant.** The registration line
declares the severity of the shape the invariant is named for; individual
findings are graded on what the evidence in front of them actually supports,
and the report groups them by (severity, invariant) so a block headed `[ERROR]`
never contains a row that is only a warning.

Twelve checks grade individual findings on their evidence:
`bookkeeping-stall`, `capture-handoff-missing`, `capture-stop-missing`,
`dictation-start-missing`, `keychain-not-single-flighted`,
`keychain-on-critical-path`, `paste-swallowed`, `polish-call-failed`,
`polish-outage`, `state-parked-processing`,
`tap-flapping-without-input`, `tap-rearm-streak`. The report names the ones
that actually split on the corpus in front of you, under
"graded by evidence, so they appear more than once" — so you never have to
guess whether a `[WARN]` block is the whole story for that check.

Because of this, the registration severity in `invariants.py` declares the
grade of the *shape the check is named for*, not of everything it can emit.
`test_severities_are_as_declared` still holds every check to that: its fixture
must produce the declared severity. Only the three whose fixture legitimately
emits a lower grade alongside it — `capture-handoff-missing`,
`dictation-start-missing`, `keychain-on-critical-path` — are exempted, by
name and with the reason written next to each.

## Attribution, and what this log cannot tell you

The first run of this tool over the maintainer's five-day corpus produced two
confident findings that were neither.

- `keychain-on-critical-path` reported **14 ERRORs** claiming keychain reads
  had blocked a dictation for up to **397 seconds**. All fourteen carried
  `[········]` — no dictation id — and no dictation in that corpus ran longer
  than **3,455 ms**. A 397-second block inside a 3.5-second dictation is
  arithmetically impossible. They came from `cargo test` runs appending to the
  same log directory between 22:21 and 22:41 on 2026-08-31.
- `remote-call-failed` (now split; see `polish-call-failed`) reported **54
  warnings** about a 429 quota wall on 2026-09-02, and `polish-outage` an
  ERROR alongside them. Forty-six of the 429s were the golden test suite in
  `src-tauri/tests` exhausting the API tier. That was reported to the
  maintainer as a live production incident and had to be retracted.

Both checks had inferred causation from **time proximity**: the records sat
near dictations in the file, so the checks called them related. Proximity is
not attribution, and a false positive reported confidently costs the tool the
credibility that makes it worth running.

### Process identity: `proc`

**This section used to say the question was unanswerable.** It said: "Can the
analyser tell an app-emitted record from a harness-emitted one? No — and it
should stop pretending the question is answerable… The fix belongs in the
writer, not the reader — one field, a process or instance id on every record,
would turn every inference in this section into a lookup."

The writer shipped the field — `docs/tracing.md` § "Who wrote this line" is
the writer's side of it, and this is the reader's. The contract:

- **every record carries `proc`**, four hex digits minted once per process,
  spliced to the **front** of the record's JSON payload — not a new header
  column, because the header is positional and adding to it would break
  `trace_api::parse_line`'s round trip, and first rather than in sort order so
  it lands in the same column on every line;
- **`app.launched` additionally carries `build`** — `"3.1.7+9dbb2ff"`, the
  version and the 7-character commit — and it is the only stage that does,
  because it is the only one whose answer is fixed for the process.

`proc` is not a pid, not unique forever, and not a session id; the analyser
uses it for one thing only, which is the one thing 16 random bits support:
these lines came from the same process.

That is enough, because of one asymmetry: **`app.launched` is written once at
startup by the real application and by nothing else.** A `cargo test` binary
links the same crate and writes trace records, but it never starts the app, so
it never writes that line. Therefore:

> a process that emitted an `app.launched` is the app; a process that never
> did is a test binary or a dev build.

The analyser groups records by `proc`, attributes each to its emitting
process, and **reports harness-emitted findings in their own block** at the
end of the report — not counted in the app's error/warn/info totals, and never
able to fail `--fail-on-error`. The corpus summary gains a `procs` line giving
the process count, the builds, and how many records each writer contributed.

**Absence of `proc` is not evidence of anything.** Every line of the harvested
corpus — all 22,081 of them, eleven days of it — predates the field. On such a
corpus the analyser degrades to exactly the behaviour described below, prints
`no record in this corpus names the process that wrote it (pre-\`proc\`
trace); harness and app cannot be told apart`, and grades everything by window
containment as before. A partly-migrated corpus, which every machine passes
through, gets both: process identity for the records that carry it, window
containment for the records that do not, and a line saying how many are in
each camp. A consumer of `--json` reads `origin` on each finding — `"app"`,
`"harness"` or `"unknown"` — and must not read `"unknown"` as `"app"`.

One failure mode the field does **not** remove: rotation cuts the head off the
oldest file, taking any `app.launched` up there with it. A real app process
whose records begin at the very top of the window has exactly a harness's
shape. Those are reported as **undecidable** and neither claim is made about
them.

Everything below this line is what the analyser does with a record that does
**not** name its writer. It is still most of them.

What *can* be established without `proc` is the negative, and it is the half
that decides severity: **whether a record is attributable to a dictation the
app actually served.** A standalone record belongs to a dictation when

1. the dictation's own id is on it — the strongest attribution there is; or
2. the *timed call it describes* began and ended inside that dictation's own
   start-to-last-stage window. A synchronous call made by a dictation's
   pipeline cannot start before the dictation existed or finish after it
   ended; or
3. it lands between two of that dictation's own stages in event order.

Anything else is unattributed, and an unattributed record is never an ERROR.

Three shapes are positive evidence that **more than one writer** appended,
none of which says which records are whose. They are counted in the corpus
summary (`writers`) and stated in full by the `log-co-tenancy` finding:

- a timed call whose interval spans a whole dictation that finished in less
  time than the call took (one process with a lifetime cache and `read_once`
  cannot do that);
- `polish.attempt` records outside every dictation's polish window;
- merged physical lines carrying records of **two different** dictations —
  as opposed to two records of the same one, which threads inside one process
  explain just as well. In the August/September 2026 corpus that split is 34
  same-dictation to 4 mixed, which is why `writer-newline-lost` is still an
  ERROR: it is the app racing itself, not a dev build alongside it.

The report states this ambiguity rather than resolving it. It does not guess.

## Which invariants a second writer can reach

Every check was audited against two questions: **what evidence does it have,
versus what does it assert?** and **could a co-tenanting dev or test process
produce its shape?** The answer to the second is not a guess — it is measured,
by counting what a standalone (no-`[tid]`) record can be in this corpus:

| Family | Standalone records in the corpus | Reachable by a co-tenant |
|---|---|---|
| `dictation.*`, `audio.*`, `whisper.*`, `polish` (the stage), `cleanup`, `dictionary`, `paste.*` except `paste.modifiers`, `usage.*`, `history.saved`, `files.cleaned`, `ui.completed`, `clipboard.*`, `correction_window.started` | **none** — every one carries a trace id | **No.** A record with a dictation id was written by a process running the dictation pipeline. |
| `polish.attempt` | 532 | **Yes, and it happened.** Attributed by containment in a dictation's polish window; unattributed attempts are INFO. |
| `keychain.slow` | 14 | **Yes, and it happened.** Attributed by whether the timed call fits inside a dictation's life. |
| `capture.*`, `hotkey.*`, `state.transition`, `app.launched` | 8,300+ | **In principle, but not observably here.** In both known contamination windows the counts of `capture.start`, `capture.stop`, `hotkey.press` and `hotkey.release` match the dictation count exactly; the only excess records are `polish.attempt` (105 against 13 dictations) and `keychain.slow`. |

So the contaminated surface in this corpus is exactly two stages, and both are
attributed rather than assumed. The input-layer and capture checks were not
being contaminated in it — but they *could* be, because on a pre-`proc` trace
`app.launched` is the only process-scoped line there is and a test binary
writes none, so a co-tenant's records fold silently into whatever session was
open. On such a corpus that is stated in each affected check rather than
resolved, because it cannot be resolved from the log as it exists.

**This paragraph used to end: "the honest answer to 'can harness contamination
be detected at all' is: not positively, and no amount of cleverness in this
tool changes that. The fix belongs in the writer, not the reader."** It did.
`proc` is that field, and every row of this table is a lookup on a corpus that
carries it — including the last row, whose "in principle, but not observably
here" becomes an observation either way. The table stands as written for the
harvested corpus, which has no `proc` on any line; read it as the description
of the degraded mode, and "Process identity: `proc`" above as the description
of the other one.

## The invariants

Thirty-nine checks — the split of `remote-call-failed` into
`whisper-call-failed` and `polish-call-failed` took the count from
thirty-eight. `--list` prints the full reasoning for each; this is the map.

Each row carries its **evidence standard**: what has to be true for the check
to fire, and what grade that evidence supports. Where a check emits more than
one severity, the row says which evidence produces which — that is the
difference between a tool that reports rule trips and one that reports harm.

Three checks — `capture-arbiter-left-live`, `keychain-not-single-flighted`
and `writer-newline-lost` — are a different kind from the rest. Every other
check asserts the *absence* of a failure that has happened. Those three assert
that a fix which has shipped is still engaged, so a hit means a regression
rather than a historical scar. They are marked **(regression)** below.

### Lifecycle — does every dictation have a shape

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `dictation-finish-missing` | Every `dictation.start` has a `dictation.finish`. | Both records carry the dictation id, so attribution is exact. ERROR. Tail-truncated dictations at the end of the corpus are exempt — the absence is the rotation boundary, not a defect. |
| `dictation-start-missing` | Every `dictation.finish` has a `dictation.start`. | Same exact attribution, but the head of a rotated file is legitimately missing, so it drops to INFO inside the rotation-truncated first session and is WARN elsewhere. Never ERROR: a lost head is a lost record, not a lost dictation. |
| `capture-handoff-missing` | Every `capture.stop` is followed by a `dictation.start`. **`tracing.md`'s "one shape the trace can only bound, not explain".** | Scoped by *event order*, never by a time window, so a long dictation is never mistaken for an orphan. Graded on how long the capture was held, because that is the only measurement of what was lost: under 0.7 s is a stray tap discarded by a minimum-length guard — an instrumentation gap, WARN; at or above it a real recording vanished with no reason line, ERROR. The 36-hit regrade that set the precedent for this whole document. |
| `capture-stop-missing` | Every `capture.start` is followed by a `capture.stop` — or by one of the arbiter's closes (`capture.orphan_prevented`, `capture.orphan_reclaimed`, `capture.stale_dropped`). | Two branches, two grades. Terminated by the **next `capture.start` in the same process**: the stream was demonstrably live across that whole span — ERROR, and this is the 11-hour-microphone shape. Terminated by **`app.launched`**: the process exited, and macOS reclaims a capture device when its owner dies, so the trace cannot say how long the microphone was live — WARN, stating that limit rather than asserting the duration. |
| `capture-arbiter-left-live` | **(regression)** When the arbiter refuses or reclaims a stream, the microphone actually goes off. | The Rust drops the cpal stream *before* writing the line, so the line existing means the mic is off. Only three log shapes contradict that, all of them explicit records rather than inferences; each is ERROR. See below. |
| `capture-stop-without-start` | `capture.stop` only fires against an open capture. | An explicit `capture.stop_failed` record with its own error string — the app saying so, not the analyser inferring it. WARN: it means bookkeeping disagreed, not that a user lost anything. |
| `paste-result-missing` | Every `paste.decision` is followed by a `paste.result`. | Both carry the dictation id. The signature audit item A4 names for a panic under `panic = "abort"`, where `catch_unwind` cannot run. ERROR: the user's text went nowhere and nothing recorded why. In-flight-at-EOF dictations are exempt. |
| `paste-verify-missing` | Every successful `paste.result` is followed by a `paste.verify`. | Exact attribution, but the claim is only "landing unproven" — `paste.result` means the events reached the window server, which is not the same as arriving. WARN, because absence of proof is not proof of loss. |
| `state-chain-broken` | `state.transition.from` matches the previous transition's `.to`. | Compares adjacent records inside one session. WARN: a co-tenant's `state.transition` records would fold into the same session and produce exactly this shape, and the log cannot rule that out. |
| `state-parked-processing` | No session ends parked in Processing. | Graded on the evidence of harm, not on the parking. Later `hotkey.press` records in the same session are presses the app ignored — ERROR. **No** later presses is equally consistent with the process exiting mid-dictation, which is a much smaller thing — WARN, saying that the trace does not separate the two. |

### Vocabulary — does the log say what the docs say it says

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `abort-reason-missing` | Every `outcome:"aborted"` carries a reason slug. | The record either has the field or does not; nothing is inferred. ERROR — a silent drop wearing a label is the failure mode this whole programme exists to remove. |
| `abort-reason-undocumented` | The slug appears in `tracing.md` or `trace-api.md`. | INFO by construction. The vocabulary is *meant* to grow; a hit is a prompt to add a table row, and grading it higher would punish the app for improving. |
| `outcome-undocumented` | The outcome appears in the docs. | INFO, same reasoning. This is how the sibling workstream's new non-`pasted` outcome states reached a reader without being reported as defects — and they have now landed, so `pasted_unverified` and `paste_swallowed` are in the documented set alongside `pasted`, `aborted` and `clipboard_fallback`. Since "`pasted` now means observed", those two are the names for what `pasted` used to cover silently. |

### The microphone — the AirPods defect

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `dead-capture` | No dictation ends because the device delivered digital silence. | The app's own classification, on a dictation-scoped record. ERROR: every occurrence is a recording the user made and lost, and the id names which one. |
| `dead-capture-misclassified` | `dead_capture` implies `nonzero_ratio == 0`. | `tracing.md` makes the predicate deliberately strict — one non-zero sample disqualifies it, because telling a user their microphone is broken is a strong claim. ERROR: if it fires, the classifier has drifted from its own documented rule, which is the code being wrong. Abstains when the field is absent. |
| `silent-audio-misclassified` | `silent_audio` implies the device produced *something*. | Two independent witnesses so it still works on traces older than `nonzero_ratio`: `nonzero_ratio == 0`, or an `avg_rms` of exactly 0 — a mean of absolute sample values can only be zero if every sample is. ERROR: the user was told "no speech detected" about a dead device. |
| `long-recording-silent` | A recording of 5 s or more does not abort as silence. | Circumstantial by design: nobody holds push-to-talk that long in silence, but "somebody did" is not excluded. WARN, carrying the duration, device and RMS so the reader can judge. |
| `device-changed-mid-recording` | The OS default input does not move mid recording. | An explicit `device_changed:true` flag on `capture.stop`. WARN: the app records the fact, and whether audio was actually lost is not in the trace. |

### The text — the hallucination-filter defect

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `text-emptied` | No transformation reduces the transcription to zero characters. | `to.chars == 0` on a dictation-scoped stage names the exact stage that did it. ERROR: the user spoke, Whisper heard, and a transformation deleted it. |
| `hallucination-dropped-speech` | The filter does not drop what looks like speech. | A conjunction of three independent measurements — ≥3 s of audio, `avg_rms` above the recorded floor, ≥20 characters returned — all on records carrying the dictation id. ERROR. Abstains entirely if any of the three is missing rather than guessing from the rest. |
| `text-chain-broken` | Each stage's `from.sha8` matches the previous stage's `to.sha8`. | Digest comparison within one dictation. WARN not ERROR: a mismatch proves something rewrote the text between two traced stages, but not that the result was wrong, and a schema generation that omits a digest can produce it. |

### Remote calls — the decommissioned model and the exhausted quota

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `polish-outage` | Polish does not fail **repeatedly** against one model. | Graded on `consecutive_failures`, which is the word "repeatedly" made numeric. A peak of 3 or more against one model id is a model that has stopped answering — ERROR, and the decommissioned-model defect reached 17. A peak of 1 or 2 is a network, and one failed call the next call recovered from looks identical — WARN. Reporting those two in one breath is how the check buried its own real finding. |
| `whisper-call-failed` | A dictation aborted with `whisper_error`. | The evidence is `dictation.finish` carrying the dictation's own id, its `status_code` and its `error_category` — the app's own statement that this dictation produced nothing. No window arithmetic, no co-tenant reading: a test binary does not abort a dictation of the user's, because it never started one. **ERROR**, and the grade is what the evidence supports: the user pressed, spoke, and got nothing back. 403 (bad key), 429 (quota) and 404 (decommissioned model) look identical to the user and are three different bugs, so the code is always quoted; where a schema generation recorded neither, the summary says so rather than printing `None (None)` as if it were a reading. |
| `polish-call-failed` | A `polish.attempt` with a non-200 status. | **WARN at most, never ERROR, for three stacked reasons.** The record carries no trace id, so it can be attributed only by containment in a dictation's polish window (`polish.decision` → `polish`). Polish has a working fallback — the pipeline pastes the unpolished text and the dictation still lands — so a failed call costs the user their polish, not their words. And this is the one stage in the vocabulary the project's own test suite provably emits in bulk. Graded by attribution, three ways: (1) the record's `proc` names a process that never emitted `app.launched` → **INFO**, aggregated, and stated positively as a test binary or dev build; (2) it falls inside a dictation's polish window → **WARN**, that dictation was waiting and got unpolished text; (3) it falls outside every window and names no process → **INFO**, aggregated per status per model per day, saying only that no dictation this analyser can see was waiting — never "a test run", because without `proc` the log cannot support that sentence. |
| `polish-quota-exhausted` | Polish is not skipped for lack of quota. | `quota_ok:false` on the dictation's own `polish.decision`. WARN: the output was silently downgraded and nothing told the user, which is worth knowing and is not a defect in the code. |

### The input layer — the dead event tap

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `tap-rearm-streak` | The re-arm streak does not climb. | Graded on how far the streak got. Reaching **5** is the point at which the Rust side stops retrying `CGEventTapEnable` and rebuilds — the escalation firing as designed, WARN. Past **10** the rebuild did not help either, which is the defect: ERROR. Caveat the finding now states itself: every rebuild *resets* the streak, so in a session that rebuilt 444 times the streak is low because the escalation kept firing, not because the tap was healthy — read the rebuild count, and read `tap-flapping-without-input` for the grade. |
| `tap-abandoned` | The input layer never gives up. | An explicit `hotkey.tap_abandoned` record: the code saying it stopped trying. ERROR on that alone. The press count afterwards is reported but deliberately **not** used as evidence — the abandoned tap is the thing that would have recorded those presses, so zero is what both "the user pressed and nothing was seen" and "the user walked away" look like. Reading zero as reassurance would be mistaking the absence of the instrument for the absence of the failure. |
| `tap-flapping-without-input` | A session with 20+ re-arms still records at least one `hotkey.press`. | Graded on session duration, because duration is the only thing that separates the two readings. Under **30 minutes**, "the tap is swallowing presses" and "nobody pressed anything" fit the same evidence — WARN, saying so. At or above it, the benign reading stops being reasonable — ERROR. The check previously said this in prose and then graded everything ERROR anyway. |
| `stale-fn-latched` | The Globe key does not latch held. | An explicit `hotkey.stale_fn_cleared` record. WARN: the app detected and corrected it, so the record is the recovery, not the injury. |
| `hotkey-event-dropped` | No hotkey event is discarded under lock contention. | An explicit `hotkey.event_dropped` record. WARN: "the press happened; nothing came of it" — one lost press, which the user retries. |

### Landing — did the text arrive

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `paste-swallowed` | Text that was injected actually landed. | Two vocabularies, because the corpus contains both. Where `paste.verify` carries an explicit **`verdict`**, the writer's verdict decides — `swallowed` is ERROR, `unverified` is WARN (the target could not be read, which is not evidence the text was lost), `observed` is silent, and an unknown slug is silent per rule 1. Where the field is absent — every record written before the verdict existed — it falls back to `ax_readable:true` with `changed:false`, the same question asked with less information, at ERROR. |
| `paste-modifier-held` | No modifier is physically held at injection time. | Attributed **positionally** — `paste.modifiers` carries no trace id, so the only attribution is "between this dictation's `paste.decision` and its `paste.result`", which is what `tracing.md`'s "look for a `paste.modifiers` line immediately before it" means in practice. WARN: this is the mechanism the original complaint was diagnosed as and never observed, so it is corroboration for `paste-swallowed`, not a finding on its own. |
| `paste-partial` | `delta_chars` reaches `expected_chars`. | Checked **only** when `before_chars` is 0, because typing over a selection legitimately shortens the field and a non-empty target cannot distinguish a partial paste from a replacement. WARN: an under-count in an empty field is real, but IME composition and rich-text targets produce it too. |

### Latency — the keychain defect

| id | Asserts | Evidence standard, and what it grades |
|---|---|---|
| `keychain-on-critical-path` | No keychain read blocks a dictation. | **The check this workstream was opened for; see "Attribution" above.** Four evidence tiers. (1) The record carries a dictation id — ERROR, strongest attribution there is. (2) It carries none, but the timed call began *and* ended inside one dictation's start-to-last-stage window, so that dictation could have made it and waited — ERROR, naming the dictation. (3) It overlaps a dictation but spills past the window — a synchronous call on a dictation's critical path cannot start before it or end after it, so WARN, reported *with the arithmetic*, as positive evidence of two writers. (4) No dictation in flight at any point — INFO, aggregated to one row per day per account per op with the count and the worst time, because a dozen rows each saying "this blocked nothing" is the same failure of proportion one severity down. |
| `keychain-not-single-flighted` | **(regression)** At most one uncoalesced read per (session, account, op). | `lock_wait_ms` used as evidence, not as a gate: `> 0` means the read waited behind another and is coalescing working, so it is not counted; `== 0`, or the field absent (a pre-fix trace), means it made its own call. Two or more of those is the regression — ERROR. **Unless** any read in the group spans a whole dictation that started, ran `usage.recorded` and finished in less time than the read took: one process with a lifetime cache cannot do that, so the group is not one process's ladder and calling it a regression would repeat the neighbouring check's mistake — WARN, with the spanning read quoted. |
| `writer-newline-lost` | **(regression)** Every record occupies its own line. | Counted unconditionally in the corpus summary, so a recurrence is visible before any check runs. ERROR, and the grade is *earned by evidence*: 34 of the 38 merged lines carry two records of the **same** dictation, which only two threads inside one process can produce. That is the app racing itself, not a co-tenanting build. |
| `bookkeeping-stall` | No gap over 1 s between two adjacent post-`paste.result` stages. | `tracing.md`: none of that region "can take seconds, let alone minutes". Graded on plausibility. Under **60 s**, a slow synchronous step is a coherent explanation — ERROR. At or above it, no synchronous bookkeeping step can take a minute, so it is a process that stopped running: a slept machine, a descheduled process, or a moved clock, of which the trace can witness only the second — WARN, naming what it cannot distinguish. Both branches now say when the keychain discriminator was **blind** (no `keychain.*` record anywhere in that file), because "no `keychain.slow` inside it" is only evidence of absence where the family exists. |
| `process-suspended` | No `hotkey.timer_stall` overlapping a dictation in flight. | An explicit `hotkey.timer_stall` with a measured `gap_ms` covering the window — the app witnessing its own descheduling. WARN: the process was suspended, the step was not slow, and a suspended laptop is not a defect. |
| `log-co-tenancy` | *(not a defect in the app)* The analyser states who wrote this corpus, or what it cannot attribute. | INFO, two regimes. **With `proc`**: it names the processes, says which emitted an `app.launched` (the app, with its `build`) and which did not (a test binary or a dev build, with its record count), and marks as undecidable any whose records begin at the rotation boundary. **Without it** — the whole harvested corpus — it falls back to the three co-tenancy witnesses and states the ambiguity rather than resolving it. Either way it is echoed in the corpus summary's `procs` and `writers` lines so the caveat is not buried at the bottom of a long report. Read it before reading any unattributed finding. |

`keychain-on-critical-path`, `process-suspended` and `bookkeeping-stall` are
one mechanism seen three ways, and the check follows `tracing.md`'s own
procedure rather than guessing. Given a gap in the bookkeeping region it
reports the two discriminators and lets them decide:

- a `keychain.slow` **inside** the window → `keychain-on-critical-path`, a
  blocking securityd read. This is the best-attributed keychain finding there
  is: the read sits between two of one dictation's own stages, so it is not
  proximity, it is containment;
- a `hotkey.timer_stall` **covering** the window → `process-suspended`, the
  app was descheduled;
- neither → `bookkeeping-stall`, stated as evidence ("no stall covers it, no
  keychain.slow inside it, N other events logged during the window") rather
  than as a verdict, and saying so when the second of those was blind. Go and
  read the window.

One consequence for the CLI: because a single check emits findings under
three different ids, `--only <id>` filters the **findings**, not the registry.
Filtering the registry — which is what it used to do — silently skipped the
check that produces the best keychain evidence whenever you asked for
`--only keychain-on-critical-path`. Every check is read-only over the corpus,
so running them all and filtering afterwards costs nothing but time.


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

`KNOWN_STAGES` is **reconciled against the emitting source**, not extended one
name at a time when somebody notices a gap — which is how `whisper.attempt`
came to sit in the "not taught about" list for a whole wave. The reconciliation
is a scan of every stage-name literal passed to `trace::event`,
`Trace::stage`/`text_stage`/`timed`/`transform`/`decision`, and the panic
hook's hand-built record, across `src-tauri/src`. Two things that scan turns up
which are *not* stages: `trace::degraded`'s **sites** (`capture.reclaim`,
`keychain.secret`, `keychain.csprng`, `keychain.migration_flag`,
`settings.fsync`, `audio.size`, `backup.audio`, `history.save`, `input_mode`)
all land on the single stage `degraded` with the site in a `site` field —
listing them would invent nine stages the writer never emits — and the abort
**reasons** passed to `Trace::abort` belong to `DOCUMENTED_ABORT_REASONS`, not
here. One entry deliberately has no emitter left: `audio.rms`, the
pre-`audio.signal` schema generation, kept because 52 dictations of the corpus
are in it.

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

5. **Add a row to the table above**, filling in **both** columns. The evidence
   column is not decoration: writing down what the check actually knows, next
   to what it asserts, is the step that catches an over-claim before it ships.
   Every check regraded in this document was caught by asking exactly that
   question of a row that had only the first column. Add a row to
   `docs/tracing.md` too if the shape is new to it as well.

6. **Choose the severity from the evidence, not from how bad the failure
   would be.** ERROR means *a user was affected, or the code is wrong*. Before
   registering one, answer three questions in the check's own docstring:

   - **What attributes this record to a dictation the app served?** A trace
     id, containment in a dictation's stage sequence, or a timed call that
     fits inside one dictation's life. Time proximity is none of those.
   - **Could a co-tenanting dev or test process produce this shape?** Where
     the records carry `proc`, look: a process that never emitted an
     `app.launched` is a harness, and the finding belongs in the harness block
     rather than the app's. Where they do not, and nothing else
     distinguishes them, the finding is a WARN that says so.
   - **Are these two evidence standards, or one?** If the answer to the two
     questions above differs between the shapes your check fires on, it is two
     checks. `remote-call-failed` was one id over a dictation-scoped abort and
     an unattributable standalone record, and the merge cost a real
     invalid-key finding its visibility for weeks. An id is a claim about the
     kind of evidence behind a finding; do not make it carry two.
   - **What else explains this evidence?** A slept laptop, a process exiting,
     a schema generation without the field. If an innocent explanation fits
     the same evidence, the grade belongs to the ambiguity, not to the worst
     case.

### When a check turns out to be wrong

Fix the check, and leave a comment saying what it used to claim — the
`WHAT THIS USED TO CLAIM` paragraphs in `invariants.py` are load-bearing, not
apology. This has now happened three times in this programme:

1. A confident "reproduced bug" was a 32-second dictation misread as an orphan
   because the rule used a **time window**. `capture-handoff-missing` scopes by
   *event order* — the next `capture.start`, `app.launched`, or end of corpus —
   and never by elapsed time. `test_long_dictation_is_not_read_as_an_orphan`
   keeps it that way.
2. `keychain-on-critical-path` reported 14 ERRORs claiming blocks of up to 397
   seconds inside dictations that never ran past 3.5 seconds, because it read
   **nearness in the file** as causation. It now grades on attribution, and
   `test_a_keychain_read_outside_every_dictation_is_not_a_block` and
   `test_a_keychain_read_inside_a_dictation_still_errors` keep both halves
   honest — the second one matters as much as the first, because a check
   taught not to cry wolf that can no longer bark is worse than the one it
   replaced.
3. `remote-call-failed` and `polish-outage` reported the golden test suite
   exhausting the API tier as a live production incident, and it was reported
   to the maintainer and retracted. Records with **no dictation id are
   frequently not the app serving a user**.
4. `remote-call-failed` was not one check. It was two, sharing an id, a
   severity and a `--only` name: a `whisper_error` abort (dictation-scoped,
   exactly attributed, the user lost their words) and a non-200
   `polish.attempt` (no trace id, attributable only by window containment, a
   working fallback, frequently our own harness). Merging them is what let 46
   test-suite 429s sit in the same block as a real 403 invalid-key abort, at
   the same grade, sorted only by timestamp — a reader scanning that block had
   no way to see that one of the twelve rows was a user losing a dictation.
   They are now `whisper-call-failed` (ERROR) and `polish-call-failed` (WARN),
   and each fixture leaves the other silent. **An id is a claim about what
   kind of evidence produced the finding**; two evidence standards cannot
   share one.

The first three are the same error: concluding from what was *near* the evidence
rather than from the evidence. A false positive reported confidently is more
expensive than a missed finding, because it costs the tool the credibility
that makes it worth running. When something fires, go read the surrounding
events with `--context` before calling it a bug.

## Files

```
scripts/check_trace.py                          entry point
scripts/trace_analyser/model.py                 parsing, sessions, dictations
scripts/trace_analyser/invariants.py            the checks
scripts/trace_analyser/stats.py                 percentiles, distributions
scripts/trace_analyser/report.py                text and JSON rendering
scripts/trace_analyser/tests/make_fixtures.py   regenerates the fixtures
scripts/trace_analyser/tests/test_invariants.py the suite
scripts/trace_analyser/tests/fixtures/          clean.log, proc_identity.log,
                                                and one per invariant
scripts/trace_analyser/tests/fixtures/.gitignore  re-includes them (see below)
scripts/.gitignore                              __pycache__/, so it does not
                                                sit in git status next to the
                                                fixtures a reader is checking
```

**The fixtures were not actually committed.** The repository root `.gitignore`
has `*.log` on line 3, which swallowed every file in `fixtures/` — so the
sentence above about fixtures being committed rather than generated was true
of the intent and false of the tree, and on a fresh clone
`test_every_invariant_has_a_fixture_that_violates_it` would have failed for
every check. A nested `fixtures/.gitignore` containing `!*.log` re-includes
them; the directory itself was never excluded, so the negation works. They are
synthetic logs, not runtime output, and they belong in git.

To verify rather than assume — the mistake was believing the intent over the
tree, so the fix comes with a way to check it:

```sh
git ls-files scripts/trace_analyser/tests/fixtures | grep -c '\.log$'   # 44
find  scripts/trace_analyser/tests/fixtures -name '*.log' | wc -l       # 44
git check-ignore -v --no-index scripts/trace_analyser/tests/fixtures/clean.log
```

The first two numbers must match — 39 in `broken/`, one per invariant, plus
`clean.log`, `two_sessions.log`, `schema_drift.log`, `merged_lines.log` and
`proc_identity.log` — and the third must report
`fixtures/.gitignore:7:!*.log` rather than the root rule. (`--no-index` is
required: `check-ignore` skips tracked paths by default, and every one of
these is tracked, which is the thing being verified. The `grep` matters too —
a bare `git ls-files` on that directory also counts `.gitignore` itself.)
