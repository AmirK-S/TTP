# Engineering standards for TTP

This codebase was largely written by AI. AI-written code has characteristic
failure modes, and the interesting question is not what they are in general but
which of them are in *this* repository, at which line, and what to do about
each. This document answers that, and then says what good looks like for a
small local-first desktop app, using the things this project already does right
as the model.

It is written to be executed against. §4 is the audit: an ordered list with
file paths, ranked by whether the change would have caught one of the seven
defects earlier.

---

## 0. How to read this

Every claim carries how well I know it. The bar is `fun-purchase-research.md`:
a confident sentence that is wrong is worse than a hedged one that is right.

- **[PROVEN]** — I read the cited lines, or I ran the command and have the
  output. If it says "four copies", I counted four.
- **[OBSERVED]** — the repository's own record attests it: a commit message, a
  trace line, a dated incident comment in the source.
- **[INFERRED]** — follows from code I read, but the failure has not been seen
  in the wild. Plausible mechanism, no sighting.
- **[UNVERIFIED]** — worth checking, not checked. Present so nobody mistakes it
  for a finding.

**What I did not do.** I did not run `cargo test` (the branch has 202 lib tests;
I took the count from the brief and the commit log rather than spending a build
on it). I did not read `pipeline.rs` end to end — 2,115 lines, sampled by grep
around every `let _ =`, every `const`, and the paste block. I did not read
`Settings.tsx` whole (1,575 lines), `fnkey.rs` whole (820), or the Astro site at
all. I did not audit the `landing/` tree, the CI workflows, or the Windows
paths. Frontend claims are from `npx vitest run` and `npx tsc --noEmit`, both of
which I ran and both of which pass.

**A caveat on tone.** The list below is a list of defects, so it reads harsher
than the codebase deserves. Several modules here — `fnkey_fsm.rs`,
`transcription/backup.rs`, `scripts/synth_sounds.py`, `trace.rs`, the two
capture wrappers — are better than most hand-written code I have read, and §3
is built almost entirely out of them. The pattern worth naming is not "this is
bad"; it is that **rigour in this repository is retrospective**. Wherever a bug
was felt, the fix arrived with a measurement, an incident date, and a test.
Everywhere else the same file reverts to plausible defaults. The seven defects
were found in the untouched half.

---

## 1. The seven, found or not found

### 1.1 Defensive code that hides failures — **found, and it is the big one**

The brief's framing is right: seven real defects stayed invisible for weeks
because every layer degraded politely. The repository's own commit log is a
record of undoing this — `950455e surface polish outages instead of swallowing
them`, `f9bc75c close the blind spots that could still eat a dictation
silently`, `799a929 tell the user the microphone is dead while they can still do
something about it`. [OBSERVED] The work is real and it is not finished.

Counted across `src-tauri/src/`, excluding tests: 107 `let _ =` bindings, 29
`.ok()` discards, 12 `unwrap_or_default()`, 48 `unwrap_or(`. [PROVEN] Most are
harmless — `let _ = std::fs::remove_file(&audio_path)` on a temp file nobody
will miss is correct, and there are twelve of those in `pipeline.rs` alone. The
problem is that a swallow that matters looks exactly like a swallow that does
not, so nobody can grep for the ones that matter.

Here are the ones that matter.

**The fsync in the atomic write.** `src-tauri/src/settings/store.rs:311`

```rust
f.write_all(json.as_bytes()).map_err(|e| {
    let _ = fs::remove_file(&tmp_path);
    format!("Failed to write temp settings file: {}", e)
})?;
let _ = f.sync_all();
```

[PROVEN] Two adjacent lines with opposite standards of care. `write_all`'s error
is caught, the temp file cleaned up, and a descriptive error returned. The
`sync_all` immediately below it — the single step that makes the whole
temp-write-then-rename dance crash-safe, and the thing `architecture.md`
advertises as "temp file → fsync → atomic rename … so a crash mid-write can
never corrupt the live settings file" — is discarded. If the fsync fails, the
rename installs a file whose bytes are not on disk, `set_settings` returns
`Ok(())`, the in-memory cache is refreshed with the new values, and the app is
now confidently serving settings that will not survive a power cut. The user is
told nothing and the trace records nothing.

*Do:* propagate it. `f.sync_all().map_err(|e| format!("Failed to fsync temp
settings file: {}", e))?`, with the same temp cleanup as the line above.

*Also, separately:* [INFERRED] there is no `fsync` on the parent directory after
the rename. On POSIX the rename itself is not durable until the directory entry
is flushed. This is a smaller hole than the one above and I have not seen it
bite; noting it so it is a decision rather than an omission.

**The event that tells the UI a permission is missing.**
`src-tauri/src/lib.rs:669` and `:681`

```rust
let _ = app.handle().emit("accessibility-missing", ());
```

[PROVEN] Both sites fire during Tauri `setup()`, having just discovered either
that the app is not trusted for Accessibility at all, or — the interesting case
— that macOS reports it as trusted while the AX probe fails, which is the stale
TCC state left behind by an in-place update. The second branch then runs
`tccutil reset Accessibility com.ttp.desktop`, destroying the user's granted
permission, sleeps 300 ms, and re-prompts.

Three things are wrong with the observability here, in ascending order of
seriousness. The emit is `let _ =` at both sites, so if no window is listening
yet — and during `setup()` the webview may well not have mounted, which is
precisely when this runs — the banner never appears and nothing records that it
did not. The TCC reset writes a `log_warn` and **no trace event**, so a user
whose permission was wiped at launch has nothing in `ttp-trace.log` explaining
it; `tracing.md` promises `paste.accessibility` with `tcc_trusted` vs
`ax_probe_ok`, but that line is emitted during a dictation, long after the reset
already happened. And the destructive action is taken on a heuristic: if
`probe_accessibility()` ever returns a false negative, the app resets a working
grant and demands the user re-grant it. [INFERRED — I have not seen the probe
misfire, and the two-level check exists for a documented real reason.]

*Do:* emit `permission.tcc_reset` on the trace before calling `tccutil`, with
`api_trusted`, `probe_ok`, and the app version. Handle the emit failure —
either retry once the main window emits its ready event, or record a
`permission.notify_failed` line. Make the launch-time permission story
greppable from the same file as the dictation-time one.

**The two Companion IPC calls that fail closed.**
`src-tauri/src/../src/windows/Settings.tsx:591-592`

```tsx
invoke<SoundPack[]>('list_sound_packs').then(setSoundPacks).catch(() => setSoundPacks([]));
invoke<boolean>('cosmetics_unlocked').then(setCosmeticsUnlocked).catch(() => setCosmeticsUnlocked(false));
```

[PROVEN] An IPC failure renders as "you own no sound packs" and "your Companion
is locked". For a paying customer those two lines are the difference between a
gift and a broken promise, and they are indistinguishable from the honest
states. This is the exact shape of the seven defects: the failure has a
plausible-looking non-failure rendering, so nobody reports it as a bug.

*Do:* a third state. `null` means "not loaded yet"; render a "couldn't reach the
app — try reopening Settings" row rather than an empty list. Never let a
transport failure and a legitimate empty result produce the same pixels.

**Four copies of a swallow that shows the user `...` as their version number.**
`src/windows/Settings.tsx:159, 217, 322, 1429`

```tsx
useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);
```

[PROVEN] Four byte-identical lines in one 1,575-line file, each in a different
component. `appVersion` initialises to the string `'...'`, so a failure leaves
the literal `...` sitting in the About panel forever. This is both anti-patterns
at once — a silent swallow, and consistency-by-copy — and it is also a hint that
four components each want the version and none of them share a hook.

*Do:* one `useAppVersion()` hook, one call, an error state that renders
something a bug report can quote.

**Everything else.** 35 `.catch(` sites in live frontend files [PROVEN, count
excludes the ` 2.ts` orphans], most of them `catch(() => {})`. The ones in
`useRecordingControl.ts` (lines 68, 71, 115, 131, 161, 162, 166) are the hot
path. `tray.rs:499 let _ = window.show()` is the pill failing to appear, which
is the literal user complaint the whole trace was built to explain.
`lib.rs:771 let _ = onboarding::show_onboarding(...)` is a first-time user
silently getting no onboarding.

**The rule to adopt.** A discarded error is a decision and it should read like
one. Three allowed forms, and nothing else:

| Form | When |
|---|---|
| `?` / propagate | The caller can do something about it. |
| `if let Err(e) = … { log_warn(…) }` plus a `trace::event` | Best-effort work whose failure changes what the user sees. |
| `let _ = …` **with a trailing comment saying why it cannot matter** | Cleanup of something already unreachable. |

`let _ = std::fs::remove_file(&tmp_path)` after an error return qualifies for the
third. `let _ = f.sync_all()` does not.

### 1.2 Plausible-looking constants nobody measured — **found, and the contrast is inside one function**

The brief names `scripts/synth_sounds.py` as the counter-example and it earns
it. Every threshold in that file carries the measurement that produced it, and
— rarer and more valuable — the measurement of the thing that was *rejected*:

```python
CENTROID_CEILING_HZ = 2000.0     # above this a sound reads as bright, not warm
FLATNESS_CEILING = 0.05          # above this it reads as noise, not as a note
```

with, twenty lines above, why: three packs built on transients and band-limited
noise "measured, sat at 2.7–4.0 kHz centroid and up to 0.27 flatness, and they
were the three that got rejected on listening". [PROVEN — `scripts/synth_sounds.py:31-56, 90-102`]
`REFERENCE_RMS_DBFS = -14.8` is the measured RMS of the built-in beeps, not a
round number. The `--verify` pass re-checks every constant against every shipped
file, so the constants cannot silently stop being true. That last property is
the one worth copying: a measured constant with no re-measurement decays into an
unmeasured one.

`src-tauri/src/fnkey_fsm.rs` is the same standard in Rust. `FN_DEBOUNCE_MS = 150`
carries the hardware it was tuned on and the value that failed ("at 100ms we
still mis-triggered on fast users"); `HANDS_FREE_STOP_GRACE_MS = 400` carries the
invariant that constrains it ("MUST comfortably exceed DOUBLE_TAP_THRESHOLD_MS
so the SECOND tap of the starting double-tap cannot self-cancel the session it
just began"). [PROVEN — `fnkey_fsm.rs:28-75`]

**Now the contrast, and it lives inside a single function.**
`src-tauri/src/transcription/pipeline.rs:451-458`

```rust
const MIN_UNIQUE_WORD_RATIO: f32 = 0.5;
…
const TIGHT_GAP: usize = 8;
const MIN_CHAIN: usize = 3;
```

[PROVEN] `MIN_UNIQUE_WORD_RATIO` is exemplary. Twenty lines of comment give the
incident date (2026-08-27), the exact French sentence that was destroyed, the
299 characters lost, the score the real dictation gets (~0.8), the score a real
loop gets (~0.1–0.2), and the reason the threshold sits far from both
populations rather than between them.

`TIGHT_GAP` and `MIN_CHAIN`, on the next two lines, have no justification of any
kind. And they are **the two constants that caused the incident**: the comment
above them says the lost dictation had "three anaphoric repeats of 'tu l'as bien',
five words apart, which is exactly the shape the chain detector looks for" — that
is `MIN_CHAIN = 3` and a gap of five inside `TIGHT_GAP = 8`. The fix added a new
measured gate in front of the unmeasured one and left the unmeasured one exactly
as it was. It works, because the new gate is strong. But the hallucination filter
still contains two bare numbers whose job is to decide whether to delete a user's
speech, and nobody knows what they should be.

*Do:* run the two constants against the corpus that already exists. There is a
test module in `pipeline.rs` with curated French/English hallucination samples,
and — better — there are 339 traced dictations from the instrumented run. Sweep
`TIGHT_GAP` ∈ [4,16] and `MIN_CHAIN` ∈ [2,5] over both populations, print the
separation, and write the result into the comment the way `synth_sounds.py`
does. If the two populations turn out not to separate, that is a finding too and
the gate should be deleted rather than tuned.

**Other unmeasured numbers, by whether I would spend time on them:**

| Constant | File:line | Verdict |
|---|---|---|
| `TIGHT_GAP`, `MIN_CHAIN` | `pipeline.rs:457-458` | **Measure.** Decides whether speech is deleted. |
| `SILENCE_RMS_THRESHOLD = 0.005` | `vad.rs:32` | **Deduplicate first** — see §1.3. The number itself is defended in `backup.rs:431` and `pipeline.rs:1005`, just not here. |
| `FOCUS_SETTLE_MS = 100` | `paste/simulate.rs:48` | **Measure.** On the critical path of every dictation, and its comment restates the code without defending the value. Too short and the first characters are swallowed — the exact failure class the trace exists for. |
| `PASTE_VERIFY_POLL_MS = 25` | `pipeline.rs:78` | Leave. Resolution/cost tradeoff, correctly described, low stakes either way. |
| `CACHE_TTL_SECS = 5` ×3 | `settings/store.rs`, `dictionary/store.rs:15`, `history/store.rs:13` | Leave the value, note the triplication. |
| `DETECTION_WINDOW_SECS = 15`, `POLL_INTERVAL_MS = 500` | `dictionary/detection.rs:15-18` | Leave. Bounded, user-invisible. |
| `OFFLINE_GRACE_DAYS = 14`, `TRIAL_DAYS = 4` | `licensing/mod.rs:17,35` | Product decisions, not measurements. Fine as they are. |

To be fair to the file: `PASTE_VERIFY_TIMEOUT_MS = 600` and
`CLIPBOARD_PASTE_RESTORE_DELAY_MS = 1500` in the same header *are* defended,
with named applications and observed ranges. [PROVEN — `pipeline.rs:64-88`] The
`HALLUCINATIONS` list cites its sources and a refresh date. The pattern holds:
touched by an incident, measured; never touched, bare.

### 1.3 Consistency-by-copy — **found, three distinct shapes**

**Shape one: the same tuned number defined twice.** `SILENCE_RMS_THRESHOLD =
0.005` at `vad.rs:32` and `SILENCE_RMS_FLOOR = 0.005` at `pipeline.rs:1009` are
two independent definitions of the same physical quantity — the RMS below which
this app declares the microphone silent. [PROVEN] The same literal appears again,
hard-coded rather than imported, in four test assertions at
`transcription/backup.rs:242, 274, 279, 332`. Six places, one number, no link
between any of them. Move the floor and the VAD keeps the old one, and two of
the tests that exist to protect the floor keep passing against a value the
product no longer uses.

`DOUBLE_TAP_THRESHOLD_MS` is the same story: `fnkey_fsm.rs:42` with a paragraph
of tuning history, and `shortcuts.rs:24` with the comment "Double-tap detection
threshold in milliseconds" and the same `300`. [PROVEN] Two code paths that must
agree, no compiler relationship, and only one of them knows why.

*Do:* one `pub const` per quantity, imported everywhere including the tests.
This is a ten-minute change and it removes a whole class of future divergence.

**Shape two: a shared component with one caller, propagated into rows it does
not fit.** This is the `RadioOption` case, and it is worth reading closely
because the repository already diagnosed it in `818a9c8`:

> That is the third time I have misused this component in as many days — first
> nesting a button inside it, then suppressing the description by passing
> `trailing`, now overflowing it. The component is fine; my content does not fit
> its contract.

[OBSERVED] Half right. The component is not fine — its **type signature is wider
than its contract**, and all three misuses are the type system failing to say no.

`src/components/ui/RadioOption.tsx:52-56`:

```tsx
{trailing && <span className="text-[12px] text-app-faint shrink-0">{trailing}</span>}
{!trailing && description && (
  <span className="text-[12px] text-app-faint shrink-0">{description}</span>
)}
```

[PROVEN] `trailing` and `description` are mutually exclusive at runtime and
independently optional in the props interface. Pass both and `description`
vanishes with no error, no warning, and no visual hint — which is misuse #2,
"every pack blurb was invisible", verbatim. The root element is a `<button>`,
which is misuse #1: `trailing` invites a control, and a control inside a button
is invalid HTML that browsers resolve by swallowing the inner click "a good
share of the time" [OBSERVED — `4aa576e`, which is also the maintainer's own
report of the symptom, "doesn't work top"]. And `shrink-0` on the description
span is misuse #3, the sentence colliding with the name.

The docstring is the tell:

```
/**
 * Used for shortcut chooser, language picker, etc. Replaces the inline
 * border-2 transition-all radio buttons that jitter their neighbors on
 * selection. Uses outline-offset trickery to avoid the 2px width shift —
 * a senior-designer detail.
 */
```

[PROVEN] "etc." is a promise of callers that do not exist: `RadioOption` is
imported by exactly one file and used at exactly three sites, all in
`Settings.tsx` (lines 666, 686, 752). "a senior-designer detail" is a comment
that praises the code instead of explaining it — see §1.4. Living in
`components/ui/` alongside `Button`, `Card`, `Modal` signals design-system
primitive, which is what creates the pull to reach for it in every list-shaped
row, which is what produced three misuses in three days.

*Do, in order of value:*

1. Make the contract a discriminated union so the third misuse would have been a
   compile error, not a screenshot:
   `type RadioOptionProps = Base & ({ description?: ReactNode; trailing?: never } | { trailing?: ReactNode; description?: never })`.
2. Rename or relocate to reflect the actual scope. `components/ui/` currently
   holds thirteen components of which `Modal`, `RadioOption` and
   `SettingsSection` have exactly one importing file each [PROVEN]. A folder
   called `ui/` is an invitation; `settings/` is not.
3. Keep the escape hatch the commit already established: when the content is a
   sentence or contains a control, write the row. This is already in
   `overhaul-brief.md` §4 — it belongs next to the component too, as a doc
   comment the next reader will actually see.

**Shape three: four copies of one `useEffect`.** Covered in §1.1 —
`Settings.tsx:159, 217, 322, 1429`.

### 1.4 Comments restating the code — **found, mild, and not the interesting variant**

Present but not epidemic. A representative run in `src-tauri/src/lib.rs:654-691`:
`// Set up system tray` above `tray::setup_tray(…)`, `// Set up settings change
listener…` above `tray::setup_settings_listener(…)`, `// Set up global keyboard
shortcuts` above `shortcuts::setup_shortcuts(…)`. [PROVEN] Also
`recording.rs:18` `/// Get the directory where recordings are stored` above
`get_recording_dir`, `permissions.rs:17` `/// Get the app config directory path`,
`onboarding.rs:11` `// Check if onboarding window already exists` above
`if let Some(window) = app.get_webview_window("onboarding")`.

These are noise, not danger. They cost a line and they age harmlessly.

Two variants are worth acting on:

**The comment that praises rather than explains.** `RadioOption.tsx:17`, "a
senior-designer detail". [PROVEN] This tells a reader the author was pleased and
nothing else; the actual decision — why `outline-offset` instead of a
transparent border — is left implicit. Replace with the mechanism.

**The comment that documents an intent the code contradicts.**
`paste/simulate.rs:46-48`:

```rust
/// Focus-settle delay before the first synthetic event, so the keystroke
/// isn't swallowed by an app that has just regained foreground.
const FOCUS_SETTLE_MS: u64 = 100;
```

[PROVEN] This looks like a justification and is not one. It restates what the
constant is for without defending `100`, on a value that sits in the injection
path of every dictation. That is more dangerous than `// Set up system tray`,
because it reads as though someone thought about it.

*The standard:* a comment earns its place by recording something the code cannot
— why this and not the obvious alternative, what broke when it was otherwise,
what invariant the next person will violate. The best comments in this repo all
do exactly that, and there are many: `state.rs:141-148` ("CAREFUL: we are holding
`&mut self` … If anyone refactors `hide_pill` to take the AppState lock … this
becomes a deadlock. Verified safe 2026-05-07, re-verified 2026-06-10") is a
model. It names the invariant, names the refactor that would break it, and dates
the verification.

### 1.5 Abstractions built for a second caller that never arrived — **found, small**

Rust first: **there are no traits and no generic abstractions in
`src-tauri/src/`** [PROVEN — zero matches for `trait ` outside dependencies].
Whatever else is true of this codebase, it has not been over-abstracted. That is
worth saying plainly, because it is the anti-pattern people expect first and it
is not here.

What is here is dead code with an interface: five `pub fn`s defined once and
called nowhere [PROVEN — each has exactly one occurrence in the whole tree]:

| Function | File:line |
|---|---|
| `get_permission_message` | `permissions.rs:183` |
| `get_permission_instructions` | `permissions.rs:192` |
| `log_debug` | `logging.rs:215` |
| `vad::is_active` | `vad.rs:144` |
| `generate_recording_path` | `recording.rs:27` |

`get_permission_instructions` is the sharpest of these: it returns English
user-facing prose from Rust — `"1. Open System Settings\n2. Go to Privacy &
Security\n…"` — in an app whose stated contract is that no user-facing prose
lives on the Rust side. It survives only because it is dead, so
`check-i18n-keys-used.mjs` never sees it. Its sibling twelve lines above,
`get_permission_message`, does it correctly (returns translation keys) and has a
doc comment explaining why. The wrong one and the right one, adjacent, both
unused.

The purest instance is `paste/permissions.rs:146-150`:

```rust
/// Get the app's bundle identifier
#[cfg(target_os = "macos")]
fn get_bundle_id() -> Option<String> {
    Some("com.ttp.desktop".to_string())
}
```

called at line 130 as `get_bundle_id().ok_or("Could not determine bundle
identifier")?`. [PROVEN] A function that is infallible by construction, wrapped
in `Option` as though it might not be, producing an error branch that cannot
execute, with a doc comment that restates its name. Three anti-patterns in five
lines. Either read the real bundle id from the running bundle — which would make
the `Option` honest and would matter the first time someone ships under a
different identifier — or make it a `const`.

*Do:* delete the five dead functions (`cargo` will not warn: they are `pub` in a
lib crate, which is exactly why they survived). Turn `get_bundle_id` into a
const or a real lookup. Add `#![warn(unreachable_pub)]` or run
`cargo +nightly udeps`/`cargo-machete` in CI so the next one is caught. [The CI
suggestion is [UNVERIFIED] — I did not check what the workflows currently run.]

### 1.6 Tests asserting the implementation rather than the behaviour — **largely not found; the real problem is a different one**

I sampled `useTauriEvent.test.ts` (5 tests, read whole), `settings-store.test.ts`
(10 tests, read half), `transcription/backup.rs`'s test module, and
`cosmetics.rs`'s. [PROVEN] They are behavioural, and several are better than
behavioural — they name the incident they exist for.
`useTauriEvent.test.ts` asserts "does not re-subscribe when only the callback
identity changes" with the comment "Re-subscribing on every render was the TTP-5
listener-churn root cause". `backup.rs:263-266` explicitly declines to
over-assert: "Asserting an exact count would be asserting a property of the test
signal rather than of the code." That sentence is the standard, written by the
codebase about itself.

Two real weaknesses, neither of which is the named anti-pattern:

**Tests that hard-code a production constant instead of importing it.**
`backup.rs:242, 274, 279, 332` compare against the literal `0.005` rather than
against `SILENCE_RMS_FLOOR`. [PROVEN] Covered in §1.3; the test consequence is
that the guard rail and the thing it guards can drift apart while everything
stays green.

**A test suite whose shape maps to what is easy, not to what breaks.** 211
`#[test]` attributes across 19 Rust files [PROVEN — per-file counts]. The
distribution:

- `pipeline.rs` 33 — every one against a pure helper (`classify_transcription_error`,
  `is_hallucination`, `apply_dictionary_to_text`, `has_repetition_loop`).
  `process_recording`, the 2,115-line orchestrator those helpers hang off, has
  **zero**. [PROVEN]
- `fnkey_fsm.rs` 22 — the pure FSM, thoroughly.
- Zero tests in `lib.rs` (871 lines), `shortcuts.rs` (244), `tray.rs` (598),
  `permissions.rs` (307), `keychain.rs` (199), `audio_monitor.rs` (123).

The pure-helper extraction is genuinely good practice and §3 recommends more of
it. But look at where the seven defects actually were: a permanently-dead event
tap, a start/stop race that left the microphone live, the keychain on the
dictation critical path, an exhausted polish quota, a decommissioned model. Not
one of those lives in a pure function. Every one lives in the orchestration and
the OS boundary, which is the part with no tests — and, not coincidentally, the
part that needed a bespoke trace log to be debuggable at all.

That is not an argument for testing `process_recording` end to end; it cannot be
done cheaply and the attempt would produce a slow, flaky suite that people learn
to ignore. It is an argument that **the trace is this project's integration test
suite**, and should be resourced as one. See §3.5.

**One genuine implementation assertion, and it is defensible:**
`settings-store.test.ts:66`, `expect(mockInvoke).toHaveBeenCalledWith('get_settings', undefined)`.
[PROVEN] The command name *is* the contract with Rust, so asserting it catches a
real class of bug. The `undefined` second argument is incidental and would break
on a harmless refactor. Minor.

### 1.7 Work that looks complete because it compiles and the tests pass — **found, and this is the second big one**

The clearest statement of it is the repository's own, in `4aa576e`: *"Two bugs
from one misuse of a component, neither of which I checked after writing."*
[OBSERVED] Both bugs were visible in a single screenshot. Neither was visible to
`tsc`, `vitest`, or `cargo test`, all of which were green. The feature shipped
"complete" and the maintainer found it by looking at his own product — which is
what the preceding commit, `ce09c4a let the maintainer see his own product`, was
about.

Then it happened again three days later, same component, and `818a9c8` opens
"Two bugs visible in one screenshot of the Voice list." [OBSERVED]

The structural version of the same failure is the duplicate-file situation, and
it is worse than the brief suggests.

`git ls-files` tracks seven ` 2.` files [PROVEN]:

```
scripts/check-i18n-parity 2.mjs
src/hooks/useRecordingControl 2.ts
src/hooks/useUpdater 2.ts
src/i18n/locales/fr 2.json
src/lib/sentry 2.ts
src/lib/sentry.test 2.ts
src/stores/settings-store 2.ts
```

They are **not** stale identical copies. I diffed them [PROVEN]:
`useRecordingControl 2.ts` is identical, but `useUpdater 2.ts` differs by 48 diff
lines, `sentry 2.ts` by 58, `settings-store 2.ts` by 81, `fr 2.json` by 97, and
`check-i18n-parity 2.mjs` by 5. These are divergent forks of live files, checked
into the branch.

Now trace them through every quality gate:

- **`tsc --noEmit`**: `tsconfig.json` has `"include": ["src"]` and excludes only
  `src/**/*.test.ts(x)` and `src/**/*.spec.ts(x)`. [PROVEN] So all four ` 2.ts`
  source files are type-checked — and so is `src/lib/sentry.test 2.ts`, because
  its name ends in `" 2.ts"`, not `".test.ts"`, so the exclude pattern misses it.
  I ran it: exit 0.
- **`vitest run`**: 12 files, 109 tests, all passing. [PROVEN — I ran it]
  `sentry.test 2.ts` is *not* among them, for the same naming reason in reverse:
  vitest's default include wants `*.test.ts` and " 2.ts" does not match. So that
  file is type-checked and never executed — the worst of both.
- **`check-i18n-keys-used.mjs`**: walks every `.ts`/`.tsx` under `src` excluding
  only `.test.`/`.spec.` suffixes [PROVEN — `scripts/check-i18n-keys-used.mjs:42-56`],
  so the orphan forks *are* scanned. A translation key referenced only by a dead
  fork counts as live and cannot be garbage-collected. I ran `npm run i18n:check`:
  370 keys, parity OK — which is exactly the point. It is green, and it is
  green partly because of files nobody ships.
- **The bundle**: nothing imports them [PROVEN — grep for the ` 2` module
  specifiers returns nothing], so Vite drops them.

So: seven divergent files pass every gate, one of them is a test file that is
compiled but never run, and one of them props up the i18n key census. Every
signal this project has says the branch is clean. That is the anti-pattern
exactly — green is not the same as done, and here green is partly *produced by*
the mess.

*Do:* `git rm` all seven, then tighten the gates so the next one cannot hide.
Change the tsconfig exclude to a pattern that catches `*test*.ts`, and add a
one-line CI check: `git ls-files | grep -E ' [0-9]+\.' && exit 1`. Ten minutes,
and it closes the hole permanently.

### 1.8 Not on the list, found here: error branches that cannot execute

Two of these, and the first is the one the brief flags.

**`catch_unwind` under `panic = "abort"`.** `src-tauri/Cargo.toml:77` sets
`panic = "abort"` in `[profile.release]`. [PROVEN] `pipeline.rs:1751` and `:1756`
wrap the paste injection in `std::panic::catch_unwind`. [PROVEN] With
`panic = "abort"` there is no unwinding, so a panic in `simulate_typing` or
`simulate_paste` terminates the process; `catch_unwind` never returns `Err`. The
handler at `pipeline.rs:1809-1815` — which logs "Paste simulation panicked" and
writes `paste.result {"ok":false,"kind":"panic"}` to the trace — is unreachable
in every shipped binary.

It is worse than merely dead. `[profile.dev]` uses the default `panic = "unwind"`,
so the arm *is* live under `cargo test`. Any test written against it would pass
while proving nothing about the product. [INFERRED — no test currently references
`catch_unwind`; I grepped.] And the trace's promise is undermined: `tracing.md`
presents `paste.result` as covering the injection outcome, but the panic case
that would produce the most confusing user experience — the app vanishing
mid-dictation — produces no trace line at all, because the process is gone.

*Do — decide, do not patch:*
- If a paste panic should be survivable, remove `panic = "abort"` from the
  release profile. Cost: a larger binary and some unwinding overhead, both
  irrelevant at this size. Benefit: the handler works, and `sentry` gets a
  chance to report before the process dies.
- If aborting is right — and it is defensible; a panic inside CGEvent injection
  means the process state is suspect — then **delete the `catch_unwind` and the
  `Ok(Err(_))` arm** and say so in a comment. Then make the abort observable
  instead: the trace already writes `dictation.start` and `paste.decision`, so a
  dictation with a `paste.decision` and no `paste.result` in the next session's
  log *is* the signature of a mid-paste abort. Document that shape in
  `tracing.md` under "Aborted dictations".

Either is fine. Leaving decorative recovery code in place is not, because it
makes the next reader believe the failure is handled.

**`ok_or` on an infallible function.** `paste/permissions.rs:130` — covered in
§1.5.

---

## 2. Summary of the seven

| # | Anti-pattern | Verdict here |
|---|---|---|
| 1 | Defensive code that hides failures | **Found, severe.** `settings/store.rs:311`, `lib.rs:669/681`, `Settings.tsx:591-592`. The dominant defect class and the direct cause of the seven bugs' invisibility. |
| 2 | Comments restating code | **Found, mild.** Noise in `lib.rs`, `recording.rs`, `permissions.rs`. Two dangerous variants: `RadioOption.tsx:17`, `paste/simulate.rs:46`. |
| 3 | Abstractions for a second caller that never arrived | **Found, small.** Five dead `pub fn`s; `get_bundle_id`. No trait over-abstraction anywhere in the Rust. |
| 4 | Tests asserting implementation | **Largely not found.** The sampled tests are behavioural and often incident-anchored. The real gap is *what is untested* (§1.6) and constants hard-coded into assertions (§1.3). |
| 5 | Plausible constants nobody measured | **Found, and localised.** `pipeline.rs:457-458` and `paste/simulate.rs:48` are the ones that matter. Counter-examples in the same repo set the standard. |
| 6 | Consistency-by-copy | **Found, three shapes.** Duplicated constants; `RadioOption`; four copies of one `useEffect`. |
| 7 | Complete because it compiles | **Found, severe.** Two documented screenshot-visible regressions; seven divergent tracked forks passing every gate. |
| 8 | *(added)* Error branches that cannot execute | **Found.** `catch_unwind` under `panic = "abort"`; `ok_or` on an infallible function. |

---

## 3. What good looks like for a small local-first desktop app

Everything below is grounded in something this repository already did, well or
badly. Where the good version exists here, it is cited rather than described.

### 3.1 Observability: trace the boundary, not the branch

`docs/tracing.md` is the strongest artefact in this project and the reason the
seven defects were findable at all. Three properties make it work, and they are
the ones to preserve:

**It is not gated by log level.** "Release builds default to `warn`, which is
precisely why every silent-drop path used to be invisible; gating the trace
behind that threshold would reproduce the blind spot it was written to remove."
Evidence beats tidiness.

**It has a correlation id.** `grep 0007-3f2a ttp-trace.log` returns one
dictation. Everything else follows from that.

**It documents what it cannot see.** The "What is not covered" section names the
frontend handoff, VAD decisions, settings changes, anything before
`app.launched`, and rotation. A tool that admits its blind spots is a tool you
can reason with; one that does not, you cannot distinguish silence-means-healthy
from silence-means-broken.

The implementation pattern to copy is `audio_capture.rs:398-412`:

```rust
/// A thin wrapper around [`start_recording_inner`] whose only job is to make
/// failure observable. The inner function has nine early exits — a poisoned
/// lock, two microphone-permission states, device config, WAV writer, stream
/// build — and every one of them means the user pressed the hotkey, spoke,
/// and got nothing. Tracing them individually would leave the next one added
/// untraced by default; tracing the boundary cannot miss any.
```

[PROVEN] This is the correct answer to "how do I not miss a failure path". Trace
at the seam, where the type system enumerates the failures for you, so that a
future early return is instrumented by default rather than by discipline. Apply
it to the three sites in §1.1 that are missing it.

**The standard:** every state change a user could notice gets a trace line, and
the line is written at the boundary the compiler can see. If you can add a new
early return without adding a trace call, the instrumentation is in the wrong
place.

### 3.2 Error handling: three states, never two

The rule that would have prevented the largest number of these defects:
**a transport failure and a legitimate empty result must never render the same.**

`Settings.tsx:591-592` violates it (IPC failure → "no packs"). `credentials.rs:40`
violates it more quietly (`serde_json::from_str(&content).unwrap_or_default()`
turns a corrupt legacy key file into "no legacy key"). The paste path gets it
right and shows what right costs: `paste.result {"ok":true}` was not good enough,
because `CGEventPost` returns void, so `paste.verify` was added to read the
target back — and even then the doc is careful that `ax_readable:false` "is
**not** evidence of failure". [PROVEN — `tracing.md`, `pipeline.rs:1730-1745`]
Three states: it worked, it failed, we cannot tell. That third state is what
makes the other two trustworthy.

The same idea applies to `keychain.rs:97`, `let _ = entry.set_password(&encoded);`.
[PROVEN] If the write fails, `get_or_create_hmac_secret` returns a freshly
generated secret that was never persisted, so the next launch generates a
different one and the HMAC-signed licence cache fails to verify. [INFERRED — I
read the mechanism at `keychain.rs:75-99` and `licensing/storage.rs`, but did not
reproduce it.] A Pro user's Companion would quietly re-lock, once, for no visible
reason, and nothing would record why. Either propagate the failure or trace a
`keychain.secret_write_failed` event.

### 3.3 Permission handling (macOS TCC): assume it lies, and record when you act

TCC's failure mode is not denial — it is **stale trust**, where the database says
yes and the API says no, which is what an in-place app update produces. This
codebase already knows that: `check_accessibility()` (TCC says) and
`probe_accessibility()` (an actual AX call) are separate, and the pipeline traces
both as `paste.accessibility {"tcc_trusted":…,"ax_probe_ok":…}`. That two-level
check is the right design and should be the template for microphone permission
too. [PROVEN — `permissions.rs:207-228`, `tracing.md`]

Three rules:

1. **Never trust the permission database alone.** Probe. The probe is the
   ground truth; the database is a claim.
2. **Any destructive remediation is a traced event.** `lib.rs:675` runs
   `tccutil reset Accessibility` — it revokes a grant the user gave — and writes
   only a `log_warn`, which release builds keep but which carries no dictation
   context and no correlation. It must emit on the trace, with `api_trusted`,
   `probe_ok`, and version, *before* it acts.
3. **Ask at the moment of need, not at launch.** The current flow prompts during
   `setup()`. A user who launches TTP for the first time gets a permission
   dialog before understanding what the app does. [UNVERIFIED — I did not run
   the onboarding flow; this is a read of the code path in `lib.rs:660-690` plus
   `onboarding.rs`, and it may be that onboarding already sequences this. Worth
   checking before changing anything.]

### 3.4 Update strategy: the running process is not the installed one

`src/lib/updater-decisions.ts` is the model, and specifically because of what its
comments preserve. [PROVEN — read whole]

```
 * The audit flagged `autoInstalledThisSessionRef` as load-bearing: Tauri's
 * update plugin compares the manifest with the RUNNING process, not the
 * on-disk bundle. After a silent install, the running process is still
 * the old version — so subsequent 4h checks keep reporting "available"
 * and we'd re-download + re-install the same update every cycle.
```

and

```
 * The v2.1.6 60s idle timer was naive: it would arm on idle and a user
 * who opened TTP, did something else for ~60s, then pressed Fn would hit
 * the restart firing right as they began recording. v2.1.9 added the
 * "user must not have recorded this session" gate.
```

Both record a bug that was actually shipped, the version it shipped in, and the
guard that fixed it. Every guard in `shouldAutoInstall` and `shouldAutoRestart`
is one of these. That is what makes them safe to refactor: the next person
cannot delete a guard without reading why it is there.

The generalisable rules: **never interrupt work** (every decision function takes
`recordingState` and returns false unless `Idle`); **track what you did this
session**, because the OS-level state you are reasoning about is not the state
your process reports; and **extract the decisions from the lifecycle** so they
can be unit-tested without a Tauri harness — which is exactly why
`updater-decisions.ts` exists as a separate file from `useUpdater.ts`'s 366
lines of refs and timers.

An in-place update also invalidates the TCC grant (§3.3). Those two subsystems
are coupled and neither file mentions the other. [PROVEN — grepped; no
cross-reference.] Add one, in both directions.

### 3.5 What is worth testing, and what is not

The distribution to aim for, derived from where this app's defects actually
were:

**Worth testing, and this repo mostly does it:**

- **Pure decision functions, extracted deliberately.** `fnkey_fsm::fn_decide`,
  `is_pro_at`, `vad_step`, `classify_transcription_error`,
  `apply_dictionary_to_text`, `shouldAutoRestart`. Fast, deterministic, and each
  one encodes a rule that would otherwise be spread across a side-effecting
  function nobody can test.
- **The incident, as a test, at the moment of the fix.** `backup.rs`'s
  `leading_silence_does_not_drag_speech_below_the_floor` reproduces the AirPods
  case with the observed ratio and says so. `useTauriEvent.test.ts`'s
  no-churn test names TTP-5. A test that names its incident survives the
  refactor that would otherwise delete it as redundant.
- **Contract boundaries the compiler cannot see.** `check-i18n-parity.mjs` and
  `check-i18n-keys-used.mjs` are tests, and good ones — they enforce a rule
  ("no user-facing prose in Rust") that no type system here can express.
- **Invariants over data you control.** `cosmetics.rs`'s `pack_ids_are_unique`
  with the comment "A duplicate id would make `effective_sound_pack` resolve to
  whichever came first, silently" — a test whose whole value is preventing a
  silent resolution.

**Not worth testing:**

- Getters, `Default` impls, serde round-trips of structs with no custom logic.
- Anything requiring a real OS grant. The `licensing::storage` tests currently
  reach the actual macOS keychain and block on an authorization dialog on every
  recompile, so nine tests are skipped locally. That is a test suite taxing every
  session for nothing; `fresh_record()` already shows how to stub the secret.
- `process_recording` end to end. 2,115 lines across the network, the filesystem,
  CoreAudio and the window server. A test would be slow, flaky, and ignored.

**And the third category, which is where this project should invest:** the
integration surface that a test cannot reach but the trace can. The seven defects
were all here. Concretely, and this is the highest-leverage single idea in this
document:

> **Assert on the trace log.** Write a small analyser over `ttp-trace.log` that
> checks structural invariants across a session: every `dictation.start` has a
> matching `dictation.finish`; every `capture.stop` is followed by a
> `dictation.start`; every `paste.decision` is followed by a `paste.result`;
> `hotkey.tap_rearmed` streaks never climb; no `dictation.finish` outcome is
> `aborted` with a reason not in the documented set.

Every one of those is a property that no unit test can check and that a real
session either satisfies or does not. Three of the seven defects — the dead
event tap, the start/stop race, the 21 seconds of AirPods silence — are
invariant violations of exactly this kind, and `tracing.md` already documents the
shapes: "A dictation that leaves `capture.stop` with no `dictation.start` after
it is the one shape the trace can currently only bound, not explain." Turning
those documented shapes into an executable check converts a document into a test
suite, at a cost of maybe 150 lines of Python.

The log-harvest window running from 2026-08-28 makes this cheap and timely: the
traces are being collected anyway, and an analyser gives the harvest a verdict
instead of requiring someone to read them.

### 3.6 The standing rules

Short enough to be remembered, each earned by something above.

1. **A discarded error is a decision.** `let _ =` requires a comment saying why
   it cannot matter. (§1.1)
2. **Transport failure and empty result must never render the same.** Three
   states, always. (§3.2)
3. **Trace at the boundary, not the branch**, so the next early return is
   instrumented by default. (§3.1)
4. **A constant that decides whether a user's words survive carries its
   measurement**, and the measurement is re-run. (§1.2)
5. **One definition per quantity**, imported by production and tests alike.
   (§1.3)
6. **If a contract cannot be violated by mistake, encode it in the type.** If it
   can only be documented, document it where the mistake would be made. (§1.3)
7. **A comment records what the code cannot**: the alternative rejected, the
   incident, the invariant. Never the author's opinion of the code. (§1.4)
8. **Green is not done.** Look at the screen. Both `RadioOption` regressions were
   visible in a screenshot and invisible to every gate. (§1.7)
9. **Recovery code that cannot execute is worse than no recovery code**, because
   it tells the next reader the case is handled. (§1.8)
10. **Say what you did not check.** Every report names what was proven, what was
    inferred, and what was skipped. (This document's §0.)

---

## 4. Audit — what should change, in order

Ranked by "would this have caught one of the seven defects earlier". Workstream E
should be able to execute these without re-deriving anything above.

### Tier 1 — would have caught a defect, or prevents the next one being invisible

**A1. Trace-log invariant analyser.** *New file, e.g. `scripts/check_trace.py`.*
Assert the structural properties in §3.5 over `ttp-trace.log`. Run it against the
traces from the harvest window. Highest leverage item here: it converts
`docs/tracing.md`'s documented failure shapes into an executable check, and three
of the seven defects are violations it would have flagged. No source changes
required, so it does not compete with any other workstream.

**A2. Stop swallowing the settings fsync.** `src-tauri/src/settings/store.rs:311`.
Propagate `sync_all`'s error with the same temp-file cleanup as line 305.
Decide separately on the parent-directory fsync after `fs::rename` (line 313).
Two lines; makes the durability guarantee in `architecture.md` true.

**A3. Make the launch-time permission path observable.** `src-tauri/src/lib.rs:660-690`.
Emit a trace event before `paste::reset_accessibility_tcc()` carrying
`api_trusted`, `probe_ok`, version. Handle the two `let _ = emit("accessibility-missing")`
failures at :669 and :681 — at minimum trace them; better, re-emit once the main
window signals ready. A user whose grant was silently reset at launch currently
has no record of it.

**A4. Resolve `catch_unwind` under `panic = "abort"`.**
`src-tauri/Cargo.toml:77`, `src-tauri/src/transcription/pipeline.rs:1751, 1756, 1809-1815`.
Pick one of the two options in §1.8 — remove `panic = "abort"`, or delete the
dead handler and document the "`paste.decision` with no `paste.result`" signature
in `docs/tracing.md`. Do not leave it as is.

**A5. Three states for the Companion IPC.** `src/windows/Settings.tsx:591-592`.
`null` for "not loaded", empty array for "genuinely none", an error row for
"could not reach the backend". Directly protects the paid tier from rendering as
broken.

**A6. Delete the seven tracked ` 2.` forks and close the hole.**
`scripts/check-i18n-parity 2.mjs`, `src/hooks/useRecordingControl 2.ts`,
`src/hooks/useUpdater 2.ts`, `src/i18n/locales/fr 2.json`, `src/lib/sentry 2.ts`,
`src/lib/sentry.test 2.ts`, `src/stores/settings-store 2.ts`. They are divergent,
not identical — diff before deleting in case a fork holds work that never made it
back. Then: widen the `tsconfig.json` test-exclude pattern so `sentry.test 2.ts`
cannot be type-checked-but-never-run, and add
`git ls-files | grep -E ' [0-9]+\.' && exit 1` to CI. Note that
`check-i18n-keys-used.mjs` currently scans these files, so the 370-key parity
result may change once they are gone — that is the check working, not breaking.

### Tier 2 — removes a class of future defect

**B1. One definition per quantity.** Make `vad.rs:32` import
`pipeline.rs:1009`'s `SILENCE_RMS_FLOOR` (or lift both into
`transcription/backup.rs` next to the doc comment that defends the value), and
replace the four hard-coded `0.005` literals at `backup.rs:242, 274, 279, 332`
with the constant. Same for `shortcuts.rs:24`'s `DOUBLE_TAP_THRESHOLD_MS`, which
should import `fnkey_fsm.rs:42`.

**B2. Measure `TIGHT_GAP` and `MIN_CHAIN`.**
`src-tauri/src/transcription/pipeline.rs:457-458`. Sweep both against the
existing hallucination test corpus and the harvest traces; write the separation
into the comment the way `MIN_UNIQUE_WORD_RATIO` seven lines above does. If the
populations do not separate, delete the chain detector — the diversity gate is
doing the work.

**B3. Make `RadioOption`'s contract a type.**
`src/components/ui/RadioOption.tsx`. Discriminated union so `description` and
`trailing` cannot both be passed; document the "short trailing word only" rule in
the docstring, replacing "Used for … etc." and "a senior-designer detail". Then
consider moving it out of `components/ui/` — it has one importing file. This is
the third-misuse-in-three-days component; a compile error is cheaper than a
fourth screenshot.

**B4. `keychain.rs:97`.** Propagate or trace the `set_password` failure. See
§3.2 for the mechanism by which a silent failure re-locks a paying user's
Companion.

**B5. Stub the keychain in `licensing::storage` tests.** Nine tests are skipped
locally because they hit the real macOS keychain and block on an authorization
dialog every recompile; `fresh_record()` already shows the shape. This taxes
every session in every workstream. Also called out in the brief under
workstream E. **In flight at time of writing:** a parallel session is already
adding a `verify_license_signature_with(record, machine_secret, legacy_closed)`
split to `licensing/storage.rs` for exactly this reason — check the working tree
before starting.

### Tier 3 — hygiene, do while nearby

**C1. Delete the five dead `pub fn`s.** `permissions.rs:183`, `permissions.rs:192`
(this one also emits English prose from Rust, violating the i18n contract —
delete it, do not translate it), `logging.rs:215`, `vad.rs:144`,
`recording.rs:27`. Consider a lint or `cargo-machete` in CI.

**C2. `get_bundle_id`.** `src-tauri/src/paste/permissions.rs:146-150` — make it a
`const`, or read the real bundle identifier. Remove the `ok_or` at line 130 that
guards a branch which cannot be taken.

**C3. One `useAppVersion()` hook.** Replaces the four identical `useEffect`s at
`src/windows/Settings.tsx:159, 217, 322, 1429`, with an error state that does not
leave the literal `...` on screen.

**C4. `FOCUS_SETTLE_MS`.** `src-tauri/src/paste/simulate.rs:48` — measure it or
say honestly in the comment that it is unmeasured. It is on the critical path of
every dictation and its current comment reads like a justification without being
one.

**C5. Cross-reference updates and TCC.** `src/hooks/useUpdater.ts` /
`src/lib/updater-decisions.ts` and `src-tauri/src/paste/permissions.rs`. An
in-place update invalidates the Accessibility grant; neither file mentions the
other.

---

## 5. What this document does not cover

- **The `landing/` Astro site.** Not read. Workstream C owns it.
- **CI workflows.** `.github/workflows/release.yml` is described in
  `architecture.md` and I did not open it. Every CI recommendation above
  (`cargo-machete`, the ` 2.` grep, the trace analyser) is [UNVERIFIED] as to
  where it should be wired in.
- **Windows paths.** Everything above is macOS-first. The `#[cfg(windows)]`
  branch in `write_settings_atomic` (`settings/store.rs:314-320`) does a
  remove-then-rename, which is not atomic; I did not analyse whether that
  matters. [UNVERIFIED]
- **Security.** `THREAT_MODEL.md` and `/security-review` exist and are workstream
  E's. The `LEGACY_HMAC_SECRET` fallback logic in `licensing/storage.rs` looked
  carefully reasoned in the parts I read, and I did not audit it.
- **Whether the seven defects are actually fixed.** The brief is explicit that
  the original complaint was never reproduced, and nothing here changes that
  assessment. This document is about why they were invisible, not about whether
  they are gone.
