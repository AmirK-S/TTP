# TTP overhaul — programme status

Running ledger for the manager session. The standing brief is
`docs/overhaul-brief.md`; this file records only *where things are*.
Updated 2026-08-31.

## Dependency graph

    D (standards) ──> E (audit)
    B (research)  ──> C (website) ──> F2 (manual as object)
    A0 (design)   ──> A1 (implementation)
    G (ship 3.1.7) ──> unblocks the test cycle for everything

## Wave 1 — launched 2026-08-31, running in parallel

| Id | Workstream | Deliverable | Blast radius |
|---|---|---|---|
| D  | Anti-patterns of AI-built software | `docs/engineering-standards.md` | that file only |
| B  | Marketing research | `docs/marketing-research.md` | that file only |
| A0 | Pill-as-character, design only, no code | `docs/companion-faces-design.md` | that file only |
| F1 | French manual (rewrite, not translation) | `docs/companion-manual.fr.md` | that file only |
| G0 | Keychain test debt | `src-tauri/src/licensing/` | that dir only |

Added 2026-08-31 after Amir's ruling on G (see below):

| Id | Workstream | Deliverable | Blast radius |
|---|---|---|---|
| G1 | Signed installable build, no public release | `.github/workflows/`, version files | CI + version only |
| H1 | Deep trace instrumentation ("tracker comme un zinzin") | `src-tauri/src/`, `docs/trace-api.md` | src-tauri minus licensing |

Blast radii are disjoint by construction. Any future parallel wave must
preserve that property.

## Wave 2 — queued, each blocked on its predecessor

- **E — codebase audit.** Blocked on D's standards existing. Runs
  `/code-review` and `/security-review` against them. Known debt to fold in:
  `panic = "abort"` makes the pipeline's `catch_unwind` decorative; stray
  Finder-copy duplicate files (G0 is inventorying them).
- **C — website rewrite.** Copy blocked on B. Design and build are not, but
  are held anyway to avoid building a structure the research contradicts.
  `landing/` only.
- **A1 — face implementation.** Blocked on A0's arbitration, and in substance
  on the survival question: nobody has lived with the existing single face for
  a week. Multiplying faces before that is answered is the programme's
  riskiest move.
- **F2 — manual as an object.** PDF / page / bundle. Blocked on C deciding
  where a visitor encounters it.

## DONE

**G0 — keychain test debt: closed 2026-08-31.** Root cause was
`verify_license_signature()` in `src-tauri/src/licensing/storage.rs` inlining
two keychain reads; one test reached the real keychain and the whole module
was skipped because the harness shares a binary. The fix extended the seam
that already existed for signing (`..._with(record, secret)`) to verification,
passing `legacy_closed` as a closure rather than a bool so the keychain read
stays lazy — an eager read would have put a documented 9.5s cost back on the
ValidMachine path mid-dictation.

`cargo test --lib` now runs clean with **no `--skip` and no authorization
dialog**: 215 passed, 1 ignored. Was 202 passed / 9 filtered. The dialog's
absence was proven, not assumed — the agent stubbed `machine_hmac_secret()`
to `panic!` under `cfg(test)` and all 215 still passed, so no running test
touches the keychain at all.

One test is deliberately `#[ignore]`d: `keychain_backed_wrapper_round_trips`,
which asserts the production wrapper is really wired to the keychain and so
cannot be stubbed by definition. It guards the one risk the refactor
introduces. It was NOT run — its passing is unproven. Run it with
`cargo test --lib licensing::storage -- --ignored`.

Still untested, stated plainly: `load_license` / `save_license` /
`clear_license` do real filesystem and keychain I/O and have no coverage.

**F1 — French manual: delivered 2026-08-31.**
`docs/companion-manual.fr.md`, 4,427 words against the English 3,639. A rewrite,
as the brief demanded, not a translation.

The letter section is **O, not Q**, and the reasoning is the interesting part:
French letter names are almost all homophones, so the English premise — most
letters are survivable, one is not — is simply false in French. The agent
inverted it. The manual admits the usual reassurance cannot be given, runs a
litany (C → c'est/ces/ses/sais/sait/s'est, "la difficulté sur laquelle l'école
française passe le plus de temps, servie à un animal qui n'y a jamais été
inscrit"), then singles out O: spoken alone it is /o/, and eleven spellings
answer to it — eau, eaux, au, aux, haut, hauts, os, oh, ho, ô, aulx — none of
which is the letter. That replaces *cue/queue/Kew* with entirely French
material and beats it on count.

One section exists that the English does not have: *Et pourquoi il efface
parfois des choses qui ont été dites*, built on the real hallucination-filter
bug, trace `0004-ad78`, 27 August 22:02 — 15.99s of genuine French speech, 299
characters transcribed, recognised as a closing courtesy and destroyed
silently.

MANAGER-VERIFIED, not taken on trust: the French hallucination signatures are
genuinely in `src-tauri/src/transcription/pipeline.rs:743-887`; trace
`0004-ad78` genuinely exists in `ttp-trace.log.1`; `"Oh"` appears exactly 3
times in the logs as claimed; the file contains zero exclamation marks.

Corpus is now **402 traced dictations**, up from the 339 the brief records.

**A0 — face design: delivered 2026-08-31.** `docs/companion-faces-design.md`.
Central claim: **aliveness is timing, not drawing.** The rule the doc rates
highest — reactions to the USER's action are immediate; reactions to the APP's
own result are delayed 120-200 ms. That delay is the difference between a
status light and a thing that noticed.

It judged the shipped face the *pessimal* case: a metronomic 5200 ms blink
whose phase resets on every state change, and eyes shut for the whole
transcription — blank exactly when the user stares hardest.
MANAGER-VERIFIED: `BLINK_INTERVAL_MS = 5200` on a `setInterval` at
`src/windows/FloatingBar.tsx:23` and `:62`, as claimed.

**Cross-workstream conflict it surfaced, and the most valuable thing in the
report.** The shipped manual says the animal has NO EYES —
`docs/companion-manual.md:79`, verified: "You will look for eyes. Owners always
do. Where you expect to find them you will find the bars instead." Shipping a
face as the Companion's headline while the manual denies it is a visible seam.
A0's preferred fix is a second-edition section, *On specimens with eyes*, in
the same register — which turns the conflict into a joke and makes the unlock
diegetic. **This is now a requirement on F2, and it applies to the French
edition too.**

It also checked Amir's own claim that the face beats the sound packs and
declined to simply agree: right about the ceiling, wrong about the floor. A bad
sound is turned off once; a bad face irritates daily until it is turned off.
Plausibly the stronger draw for someone who already bought, not for someone
deciding.

**MANAGER ARBITRATION, and it goes against A0.** A0 recommended running 14 days
on the face exactly as-is, on the grounds that if the pessimal case survives,
anything survives. I overruled it and launched A1 to implement the corrections
now. Reason: Amir's explicit direction is to build a lot and to see the
software before anything ships, and a 14-day hold on a knowingly-defective
artefact serves an experiment he did not ask for. **The honest cost: the
survival test now runs on the CORRECTED face, so a pass proves that this face
survives, not that any face would.** That is a weaker result than A0 designed
for, and it was my call, not a finding.

Scope held: A1 builds the corrected SINGLE face. The four-variety cast
(`house`, `shut`, `drowsy`, `quick`) is NOT being built. Multiplying the bet
before one face survives normal use is the riskiest move on the board, and the
design doc says so itself.

**H1 scope extended** to carry A0's Rust-side instrumentation — the settings
diff hooks, `companion_face_enabled_at`, and the `companion.*` trace events.
The signal A0 rates sharpest is not the user disabling the face; it is
`hide_pill_when_inactive` flipping true *while the face is still on* — "get
this off my screen" without admitting why.

**D — engineering standards: delivered 2026-08-31.**
`docs/engineering-standards.md`, ~975 lines, every claim tagged PROVEN /
OBSERVED / INFERRED / UNVERIFIED, with §0 and §5 naming what was skipped
(landing/, CI, Windows paths, security, most of pipeline.rs and Settings.tsx).
It ran vitest (12 files, 109 tests, pass), tsc (clean) and i18n:check (370
keys, pass); it did NOT run cargo test.

Severe findings:
- `settings/store.rs:311` discards `sync_all`'s error inside
  `write_settings_atomic`, one line below a carefully-handled `write_all` —
  voiding the crash-safety guarantee `architecture.md` advertises.
- `lib.rs:675` runs `tccutil reset Accessibility`, destroying a user's granted
  permission, with a `log_warn` and **no trace event**.
- `Settings.tsx:591` renders IPC failure as "no sound packs / Companion
  locked" — the paid tier failing indistinguishably from an honest empty state.
- The duplicate forks (below).

Best find, and the one worth reading the doc for: inside a single function at
`pipeline.rs:451-458`, `MIN_UNIQUE_WORD_RATIO = 0.5` carries the incident date,
the exact French sentence it destroyed, and both population scores — while
`TIGHT_GAP = 8` and `MIN_CHAIN = 3` on the next two lines are bare. **Those two
are what caused the incident.** The fix added a measured gate in front of an
unmeasured one. The general shape: rigour here is *retrospective* — measured
where a bug was felt, plausible everywhere else, and the seven defects lived in
the untouched half.

It also declined to confirm one of the brief's own accusations: the
"tests assert the implementation" anti-pattern is largely NOT present here —
sampled tests are behavioural and incident-anchored, and `backup.rs:263`
explicitly refuses to over-assert. The real gap is distribution, not quality:
`pipeline.rs` has 33 tests, all on pure helpers; `process_recording` (2,115
lines) has zero; `lib.rs`, `shortcuts.rs`, `tray.rs`, `keychain.rs` have zero.
**None of the seven defects lived in a pure function.**

Confirmed: `panic = "abort"` at `Cargo.toml:77` makes `catch_unwind` at
`pipeline.rs:1751` decorative and the `Ok(Err(_))` arm at `:1809` unreachable
in release — while remaining live under `cargo test`, so a test there would
pass while proving nothing.

## MANAGER CORRECTION — the duplicate files

My earlier read was WRONG and it changes the work. I called `fr 2.json` stale
and dead. It is not litter:

- All seven duplicates are **tracked in git**, not untracked Finder droppings.
- They are **divergent forks**: `sentry 2.ts` 58 lines from its original,
  `settings-store 2.ts` 81, `useUpdater 2.ts` 48. (Manager-verified.)
- **Every quality gate is green partly because of them.** They pass `tsc`
  (tsconfig sweeps `src`); `sentry.test 2.ts` does NOT match the vitest
  `*.test.ts` glob, so it is type-checked and never run; and
  `check-i18n-keys-used.mjs` walks the tree with `readdirSync`, so the forks
  keep i18n keys alive.

Consequence for E: deleting them may turn `i18n:check` RED, and that would not
be a regression — it would be a pre-existing truth the duplicates concealed.
The direction of divergence must be established per file before deletion; a
fork 81 lines out might be the NEWER edit with the live file stale.

## Wave 3 — launched 2026-08-31

| Id | Workstream | Deliverable | Blast radius |
|---|---|---|---|
| A1 | Corrected single face (not the cast) | `src/windows/`, `src/components/ui/`, `src/i18n/locales/` | frontend face only |
| E1 | Resolve the seven divergent forks | those files + `scripts/` | narrow |
| H3 | Trace-log invariant analyser | `scripts/`, `docs/` | new files only |

H3 is D's highest-leverage recommendation: `docs/tracing.md` already documents
the shapes a failure makes in the log; H3 makes them executable and runs them
over the 402 real dictations. Three of the seven defects would have been
flagged by it. No source changes required.

## Inventory for workstream E — stray Finder-copy duplicates

Found, nothing deleted:

    scripts/check-i18n-parity 2.mjs
    src/stores/settings-store 2.ts
    src/hooks/useUpdater 2.ts
    src/hooks/useRecordingControl 2.ts
    src/lib/sentry 2.ts
    src/lib/sentry.test 2.ts
    src-tauri/gen/schemas/desktop-schema 2.json
    src/i18n/locales/fr 2.json

The last two deserve a close look rather than a blind delete: a stale
generated Tauri schema, and a duplicate French locale. Manager checked the
locale — `fr 2.json` is 23K dated 10 June against a 25K `fr.json` touched
today, 119 lines apart on a sorted diff. It is stale, not a live second
source. Deleting is E's call, with `npm run i18n:check` as the proof.

## G — ship it: RULED 2026-08-31

57 commits on `polaris/v3.0.0`, unpushed. Version still 3.1.6 while `v3.1.6`
is already tagged, so the branch cannot ship without a bump.

**The conflict:** a log-harvest window began 2026-08-28 and runs one to two
weeks on build `14ed403a12cc`. Shipping 3.1.7 replaces the binary under
observation. The harvest exists to find out whether the original
"I press, I dictate, nothing is written" failure recurs — a failure that has
never been reproduced and whose diagnosis was read out of enigo and
core-graphics sources, not observed. Three days of clean data exist so far.

**Amir's ruling: ship the BUILD, not the PROD.** Bump, push the branch, and
have CI produce a Developer-ID-signed installable he can put on his machine
and examine thoroughly. NO tag, NO GitHub Release, nothing in front of end
users until he has looked at the software himself. He is explicit that
permission prompts do not bother him because what he wants to judge is how it
looks — so the build must show the Companion cosmetics.

The harvest window loses its clean binary as a consequence. That was his call,
made knowingly. H1 exists partly to compensate: a much deeper trace means the
next window yields more per day than the one it replaces.

Obstacle found before launching G1: there was NO path from "branch pushed" to
"signed build I can install". `build.yml` fires only on `main`/`master`;
`release.yml` fires on `v*` tags and publishes a Release, which is the prod
path that is ruled out. G1's first job is to build that third path.

## Standing constraints (from the brief, restated so they are not re-litigated)

- TTP Pro unlocks nothing you need. No workstream proposes "just a small limit
  on X". The paywall came off 2026-08-28 and does not return in a smaller shape.
- No user-facing prose in Rust. Everything displayed goes through
  `src/i18n/locales/`; `npm run i18n:check` enforces it.
- `RadioOption` is for a short trailing word. Sentence or control -> write the row.
- Evidence over story. Write the test first when the claim matters.
- Every report separates what was proven from what was inferred, and names
  what was left out.
