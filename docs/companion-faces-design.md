# The pill as a character

Design document for workstream A. **No code was written for this.** Everything
below is intent for a later implementation phase, with numbers concrete enough
that the implementer does not have to re-decide anything.

Sources this rests on: `docs/overhaul-brief.md` §2A (the mandate),
`docs/ttp-pro-design.md` §2 and §4 (the arbitration and the deferral),
`docs/fun-purchase-research.md` M4/M5/M6 (the theory),
`docs/companion-manual.md` (the voice), `docs/tracing.md` (the instrument),
and the shipped implementation in `src/windows/FloatingBar.tsx`.

---

## 0. The recommendation, in one page

**Do not build a cast yet.** Build the answer to whether one survives.

1. **Start the survival test today, with the face exactly as it is.** It costs
   nothing, it needs no build, and it runs inside the log-harvest window that
   is already running. The current face is the *pessimal* case — its blink is
   perfectly metronomic and it shuts its eyes for the whole of transcription,
   which are the two worst things a peripheral character can do. If this
   version survives fourteen days, any corrected version survives.
2. **Pre-register the falsifier before the window opens** (§2.4). Write it
   down, dated, so the answer can come back "no" without anyone relitigating
   what "no" would have looked like.
3. **Put the instrumentation in the one owed build** (§2.2). Three trace
   events and one settings field. It makes the answer mechanical instead of
   remembered, which matters because the test subject is the person who wants
   the answer to be yes.
4. **The corrections in §3 are defect fixes, not new features**, and they ride
   the same owed build. The four that matter: irregular blinks, stop shutting
   the eyes for the duration of transcription, add a beat when the words
   land, delete the mouth.
5. **If it survives, build one more face, not a roster.** `shut` (§4.3) —
   the variety whose eyes are closed at idle. It is the strongest candidate to
   survive a month and it makes the point the whole feature rests on: the
   character is in the *transition*, not the presence. A roster of four is
   §4's proposal and it is a good one, but it is the second thing you build,
   not the first.

**On Amir's claim that this is a stronger draw than the sound packs.** Taking
it seriously: it is right about the ceiling and wrong about the floor. The
face has the higher emotional ceiling — it fires anthropomorphism (M4),
endowment (M5) and collecting (M6) at once, where sounds fire M7 and M6 only.
But §0 of the research is unmoved: sound is heard by other people in the room
and a 16px lozenge at the bottom of your own screen is not. The face cannot
beat the sounds on distribution, and it carries a failure mode the sounds do
not have — a bad sound is turned off once; a bad face is *irritating every day
until* it is turned off. So: plausibly the stronger draw for the person who
already bought, and definitely not the stronger draw for the person deciding
whether to. That reading changes nothing about the build order.

---

## 1. What exists today

Read from `src/windows/FloatingBar.tsx` at `polaris/v3.0.0` (3f3a6f8).

### 1.1 The face

`CompanionFace`, lines 48–101. A 16×16 SVG holding two rounded rects — no
paths, no colour, no third element except a mouth in one state.

| Property | Value |
|---|---|
| Box | 16×16, `viewBox="0 0 16 16"` |
| Eyes | two rects, x-centres 5 and 11, width 2.2, `rx` 1.1, vertically centred on y = 8 |
| Fill | `fill-white/90` |
| Eye height, idle | 3.5 |
| Eye height, listening | 4.5 |
| Eye height, thinking | 1 (shut) |
| Eye height, error | 2 |
| Eye height, blinking | 1 (shut) |
| Mouth | quadratic curve `M 5.6 11.6 Q 8 13.2 10.4 11.6`, stroke `white/70`, width 1.1 — **listening state only** |
| Transition | `all 120ms ease-out`, inline, dropped when reduced motion is set |
| Blink | `setInterval` at **5200 ms**, eyes shut for **140 ms**, idle state only |
| Accessibility | `aria-hidden`; the pill's `role="status" aria-live="polite"` already speaks the state |

### 1.2 Where it sits and when it appears

The face renders only when `companion_face_enabled` **and**
`cosmetics_unlocked()` are both true (lines 118–130; refreshed on the
`settings-changed` event, because the floating-bar window has no settings store
of its own). Default is `false` — `src-tauri/src/settings/store.rs:153`.

It is the first child of the pill's flex row, `gap-2` (8px) from whatever comes
next. The pill (`DarkPill`) is `rounded-full`, backdrop-blurred, bottom-centre
of the screen, `pb-1`, `pointer-events-none`, and it changes size between
states: `min-height` **16px idle → 28px** while recording / processing /
erroring, with padding `px-4` idle → `px-3.5` active.

**The number that matters most for this design:** at idle the pill is 16px
tall and the face's box is 16px. The face is not a detail on the idle pill; at
idle the face *is* the pill, plus some padding. And `hide_pill_when_inactive`
defaults to `false` (`store.rs:141`), so on a default install the idle pill —
and therefore the face — is on screen, at the bottom centre of the display,
permanently. That is the 99% case and it is the whole of the survival question.

### 1.3 The rest of the pill, for context

- **Waveform.** 14 bars, 2px wide, min height 2 / max 16, driven by a RAF loop
  that reads `audio-level` events emitted at ~30 Hz from `audio_monitor.rs`
  (RMS × 18, capped at 1.0). Bell-curve envelope so the centre bars stand
  taller, two phased sines for organic motion, animated via `scaleY` transform
  only, `transformOrigin: bottom`, `willChange: transform`.
- **Timer.** `m:ss`, 11px tabular-nums, updated every 100 ms.
- **Clasp.** `Lock` icon when hands-free.
- **Dead input.** Replaces the timer with a warning string when
  `audio-dead-input` fires.
- **Error.** `anim-shake` — 320 ms, max 2px amplitude, `--ease-app-out`
  (`cubic-bezier(0.32, 0.72, 0, 1)`), plus an `AlertCircle` icon and a
  sentence, on a red pill.
- **Reduced motion.** `window.matchMedia('(prefers-reduced-motion: reduce)')`
  read once at render; the waveform mounts as a static silhouette and skips
  the RAF loop, the face holds still with eyes open, and `src/index.css:417`
  additionally clamps every animation and transition in the app to 0.01 ms.

### 1.4 Five honest observations about the current face

Not a bug list to fix in this phase — the character work in §3 is a response
to these, and they are also what makes the current face the right *pessimal*
subject for the survival test.

1. **The blink is metronomic.** A constant 5200 ms `setInterval` means blinks
   land on a perfect grid. Perfect periodicity is the single strongest tell
   that something is a mechanism rather than an animal — and it is the tell
   that gets *stronger* with exposure, because the viewer learns the period.
   Worse, the interval is torn down and recreated on every state change
   (line 60), so the phase resets: the first blink after every dictation lands
   exactly 5200 ms after the pill returns to idle. That is a regularity a
   person will find, and finding it is the moment the illusion dies.
2. **The eyes are shut for the entire transcription.** `eyesShut` is true
   whenever `state === 'thinking'` (line 72). Transcription is typically
   1–3 s and can be far longer on a long dictation. Eyes closed for nine
   seconds does not read as concentration; it reads as asleep, or off, or
   crashed — and it happens at precisely the moment the research says the user
   is most engaged with the thing (Epley factor (b): watching an opaque agent
   they are motivated to predict). The face goes blank exactly when it is
   being looked at hardest. This is the worst-placed behaviour in the current
   implementation.
3. **There is no beat when the words land.** `stage === 'complete'` produces
   nothing from the face. The one event that is unambiguously good news, that
   the user actually cares about, and that would earn a reaction, passes
   without one.
4. **The face is loudest in the states that are already loud.** Eyes widen to
   4.5 while listening — when the pill has already grown, the tone has already
   played, fourteen bars are already moving and a timer is already running. A
   fifth simultaneous signal is not information. And in the error state the
   face narrows *while* the pill is shaking, which puts two animations inside
   one 100px lozenge.
5. **The reduced-motion query is read once, not subscribed.** Line 204 is a
   plain `matchMedia(...).matches` at render time with no `change` listener.
   A user who turns on Reduce Motion while TTP is running keeps the old
   behaviour until that window reloads. Small, real, and worth fixing in the
   implementation phase — reported, not fixed here.

### 1.5 A cross-workstream conflict, flagged for F

`docs/companion-manual.md` — the object the buyer receives — describes the
anatomy of an animal that **has no eyes**:

> You will look for eyes. Owners always do. Where you expect to find them you
> will find the bars instead, and on the whole the bars tell you more.

The manual is the thing that is supposed to be *true*. Shipping a face as the
headline of the Companion while the manual denies its existence is the kind of
seam that makes the whole package read as assembled rather than made.

Two resolutions, and this is workstream F's call, not mine:

- **Preferred.** The Anatomy section describes the specimen as it arrives.
  The second edition adds one short section — *On specimens with eyes* — in
  the same register: that a small number of animals in private keeping have
  been observed to develop them, that it is not a defect, that owners report
  it is a matter of temperament rather than health, and that the bars remain
  the more honest instrument. Two paragraphs. It turns the conflict into the
  best joke in the book, and it makes the *unlock* diegetic.
- **Cheap.** Add a clause to the existing paragraph acknowledging the varieties
  that have them. Weaker, because it spends the discovery in a subclause.

Either way, the French edition needs its own version — it is a rewrite, per the
brief, and this section is not structurally English so it should translate
straightforwardly.

---

## 2. The survival question

**The question.** Does one face, on by default for the person who enabled it,
survive fourteen days of ordinary work without becoming something they want
gone?

**The design constraint on the answer.** It has to cost almost nothing, it has
to produce a *finding* rather than an impression, and it has to be able to come
back "no". A face that is disabled after four days is a result, and the app
should be able to notice it without anyone remembering to write it down.

### 2.1 The test, in full

- **Subject:** one (Amir). n = 1, and §2.5 is honest about what that buys.
- **Duration:** 14 days. Seven is not enough — the research and the brief both
  say a week is roughly when charming turns intolerable, so a seven-day window
  ends exactly where the interesting part starts.
- **Setup:** face on, name it once on day one (the naming is part of the thing
  being tested — see §2.3), `hide_pill_when_inactive` left at `false` so the
  idle pill is actually on screen. Then use TTP normally and change nothing.
- **Start:** today, on the build already running. The corrections in §3 land
  in the owed build and do not need to precede the test; the current face is
  the harshest version and is the right thing to expose first.
- **Practical wrinkle:** the maintainer's trial expired 2026-08-18, so the
  face needs `TTP_COSMETICS=1` in the app's environment. If the running
  harvest build does not have it, enabling the face means a restart with the
  variable set — one extra `app.launched` boundary in the trace, which is a
  cheap price and should be noted in the harvest log so nobody later reads it
  as a crash.

### 2.2 What the app should record (the ~30 lines of instrumentation)

All of it is slugs and numbers in `ttp-trace.log`. **None of it is
user-facing prose, so none of it touches the i18n boundary** — trace event
names and JSON keys are not displayed anywhere.

The natural hook is `set_settings` in `src-tauri/src/settings/store.rs:331`,
which already receives the whole new `Settings` and can diff it against the
value in the cache before overwriting it.

| Event | Fields | Why |
|---|---|---|
| `companion.face` | `{"enabled":bool,"days_on":f32,"dictations_on":u32}` | The primary measurement. Emitted only when the value actually changed. `days_on` and `dictations_on` are zero on enable and carry the elapsed span on disable. |
| `companion.named` | `{"len":u16,"cleared":bool}` | Whether endowment fired. **Length only — never the name.** It is local and private and belongs to the owner. |
| `companion.pill_hidden` | `{"hidden":bool,"face_on":bool}` | The tell described in §2.3. |
| `companion.state` | `{"face":bool,"pack":"bowl","days_on":f32}` | One line at `app.launched`, alongside the existing session-boundary event. Log rotation means a disable event can age out; a state line at every session boundary means the log always shows where things stood. |

Two supporting changes, both mechanical:

- **`companion_face_enabled_at: Option<i64>`** in `Settings` (epoch seconds).
  It is the denominator for `days_on` and it survives a restart, which a
  process-lifetime timer would not. Not user-facing; no i18n implication.
- **`dictations_on`** counts `dictation.finish` events since the face was
  enabled. The `usage` store already keeps daily counts
  (`src-tauri/src/usage/store.rs`); a subtraction against the enable date is
  enough and no new counter is needed.

Reading it afterwards:

```sh
grep 'companion\.' ttp-trace.log
```

### 2.3 What to look for, ranked by how much it means

1. **A `companion.face {"enabled":false}` line.** The direct answer. `days_on`
   and `dictations_on` on that line are the finding.
2. **`companion.pill_hidden {"hidden":true,"face_on":true}`.** This is the
   subtler and, I think, the more reliable tell. Turning off the idle pill
   while keeping the face on means "I want this off my screen" without
   admitting the face is the reason. If this fires, treat it as a disable.
3. **Enable → disable → enable.** Ambiguous, and interesting. It means a
   *specific behaviour* annoyed rather than the face itself. Read the trace
   backwards from the disable: what was the last dictation, was it long (long
   transcription = long blank stare, observation 1.4.2), did it error?
4. **`companion.named` never firing.** Endowment (M5) did not fire. Not fatal
   to the face, but it undercuts the argument for a *cast* — if one is not
   worth naming, four are not worth collecting.
5. **The face still on at day 14 with no `pill_hidden`.** The pass condition.

### 2.4 The falsifier, pre-registered

Write this into the harvest log, dated, before the window opens:

> **Prediction.** The face is still enabled on 2026-09-14, `hide_pill_when_inactive`
> is still `false`, and Amir cannot name a specific moment in the fourteen days
> when it irritated him.
>
> **If any of the three is false, the answer is no, and the cast is not built.**
> A disable inside seven days means the face itself is wrong and needs the §3
> corrections before it is retested. A disable between seven and fourteen days
> means the *idle* behaviour is wrong specifically, because by then every other
> beat has been seen dozens of times and only the idle state is still
> accumulating.

The mechanical part of that prediction is the part that cannot be
re-remembered later. That is why it is instrumented.

### 2.5 The subjective instrument, which costs ten seconds a day

The trace records the decision. It cannot record the *reason*, and the reason
is what the design phase needs. So: three questions, once a day, in a plain
text file. Ten seconds.

1. Did you notice it today? — *yes / no / only when I went looking*
2. Did it do anything you did not expect?
3. Did it irritate you — and at what exact moment?

The target answer for a healthy day is **no / no / no**. The valuable answer is
any day where question 3 has a *moment* attached, because that moment is a beat
to redesign rather than evidence to kill the feature. "It annoyed me while I
was reading" is a fact about the idle loop. "It annoyed me after the long
dictation" is observation 1.4.2 and has a fix in §3.

"Only when I went looking" on question 1 is the best possible answer. A
peripheral character that has become invisible has not failed; it has passed.
The illusion is meant to work at the edge of attention, and being noticed is
the failure mode, not the goal.

### 2.6 What this test cannot tell you

Stated plainly so nobody reads a pass as more than it is.

- **n = 1, and the subject wrote the app.** He is motivated for the answer to
  be yes. The pre-registered falsifier and the mechanical timestamp are the
  only two defences, and they are partial.
- **A pass generalises weakly.** It licenses building a second face. It does
  not license building four, and it certainly does not predict that a stranger
  who bought the Companion will keep it on.
- **The trace cannot see attention.** No line in `ttp-trace.log` says whether
  anyone looked at the pill, liked it, or stopped seeing it. Everything about
  perception comes from §2.5, which is self-report.
- **Rotation.** The trace rotates at 2 MB keeping three back-files, roughly
  three weeks of heavy use (`docs/tracing.md`). A 14-day window fits, with
  little margin if the harvest is also running. The `companion.state` line at
  every session boundary is the hedge.
- **The idle pill may not have been on screen.** If `hide_pill_when_inactive`
  is ever true, the test measured a face the user mostly did not see. Check
  the flag at the start and read `companion.state` to confirm.

---

## 3. The character work

This is the part the brief is right to call the crux. Everything here is
craft judgement, not measurement — I say where I am guessing.

### 3.1 The thesis

**Aliveness is timing. It is not drawing.**

The face is two rounded rectangles. It will stay two rounded rectangles.
Adding detail to a 16px glyph at the bottom of a screen buys nothing —
16 device pixels cannot hold an expression, and every attempt produces clip
art. What 16 pixels *can* hold is the difference between 90 ms and 260 ms, and
that difference is the entire feature.

Three laws follow, and every specific number below is derived from one of them.

**Law 1 — irregularity, but bounded.** Anything perfectly periodic reads as a
mechanism, and the viewer learns the period, so periodicity gets *worse* with
exposure. Anything unbounded-random reads as broken. Live things vary inside a
narrow band.

**Law 2 — the peripheral event threshold.** Peripheral vision is a motion
detector tuned to onsets. Roughly: a change of **less than ~1px over more than
~2s is texture**, and a change of **more than ~2px in under ~400ms is an
event** that steals a saccade. Idle behaviour must live entirely below the
event threshold, with one exception per minute at most (the blink, which is
short enough to be over before the eye arrives). *I am guessing at those two
constants* — they are calibrated to the 16px box from experience with
peripheral UI, not measured. The ratio between them is what matters and the
ratio is robust; verify the absolute numbers with a screen recording at 1× at
the real bottom-of-screen position before committing.

**Law 3 — latency separates response from indication.**

> **A reaction to the user's own action is immediate. A reaction to the app's
> own result is delayed 120–200 ms.**

This is the most useful single sentence in this document. A pill that changes
in the same frame as your keypress reads as *your keypress*, correctly. A pill
that changes in the same frame as the words appearing reads as a status light
firing. Delay that same change by 140 ms and it reads as the thing *noticing*
the words appear. The delay is the whole difference between an indicator and a
character, and it costs one `setTimeout`.

### 3.2 Breathing versus fidgeting

They are not distinguished by amount of motion. They are distinguished by
whether the motion produces *events*.

- **Breathing** is slow, low-amplitude, continuous, and has a stable period.
  It never crosses the onset threshold, so the visual system integrates it as
  a property of the object rather than as something happening. You cannot
  point at any moment where it moved. This is why it survives being watched
  for a month.
- **Fidgeting** is a sequence of discrete onsets. Each one is small, but each
  one is an *event*, and events accumulate into "that thing keeps moving".
  A character that shifts, glances, tilts or bobs is fidgeting no matter how
  gently it does it.

The operational rule for the idle state: **at most one perceptible event per
minute, and no continuous motion with amplitude above ~1px.**

**Idle breath, specified.** Both eyes together (independent eyes read as
broken, not as alive), `scaleY` on the eye group, amplitude ±6% of eye height
— on a 3.0px eye that is ±0.18px, deliberately below the texture threshold —
sinusoidal, period 4600 ms, phase randomised once at window creation so it is
never in lockstep with the clock, the caret, or anything else on screen.

**It may be imperceptible, and that is an acceptable outcome.** The test for
it is not "can you see it" but "does turning it off feel very slightly worse
without your being able to say why". If a side-by-side screen recording shows
nothing and feels the same, cut it. Cutting it costs nothing; keeping an
invisible animation costs battery.

**Which is the hard constraint on the breath, and it is not aesthetic.** The
idle pill is on screen permanently by default. Anything that animates at idle
animates forever, on a laptop. So:

- The breath **must** be a CSS keyframe animation on `transform` only,
  composited, with `transform-box: fill-box; transform-origin: center`. Never
  a JS timer, never `requestAnimationFrame`. A permanent RAF loop in an
  always-on-top window is a real battery cost and would be an unforced error
  in an app whose whole pitch is that it is polite.
- Measure it with `powermetrics` before it ships. If the breath costs anything
  measurable, cut it and keep only blinks — which are discrete, and idle
  99.9% of the time. Evidence over story: this is a claim about power, and the
  machine can answer it.

### 3.3 Blinking, specified

The blink is the only idle event the character gets, so it has to carry the
whole illusion.

| Parameter | Value | Reason |
|---|---|---|
| Interval | `4200 + random() * 3600` ms → 4.2–7.8 s, mean ~6.0 s | ≈10 blinks/min. Recalled human spontaneous rates are ~15–20/min in conversation and ~4.5/min during focused screen work; **these numbers are from memory, treat as approximate.** The exact rate matters far less than the irregularity. |
| Scheduling | `setTimeout`, re-armed with a fresh random interval at the end of each blink | Naturally irregular, and it removes the phase-reset artefact of the current `setInterval`. |
| Close | 90 ms, ease-in (accelerating shut) | |
| Hold shut | 40 ms | |
| Open | 130 ms, ease-out (decelerating open) | **Asymmetry is the point.** Real eyelids close faster than they open. A symmetric transition — which is what `all 120ms ease-out` currently produces — reads as a slow deliberate wink, which is a *communicative* gesture and therefore a demand. |
| Double blink | ~12% of blinks; second blink starts 180–260 ms after the first finishes | The single highest-value detail in this document. A double blink is the thing that makes two dots read as an animal instead of a status light, and it costs four lines. |
| Phase | Never reset on state change; the blink scheduler runs continuously and is simply suppressed (not restarted) while not idle | Kills observation 1.4.1's "always exactly 5.2 s after a dictation" tell. |

### 3.4 Acknowledging without demanding

The grammar of a reaction that does not ask for anything:

1. **It is late** (120–200 ms after the app's own result — Law 3), and
   **immediate** when it responds to the user's key.
2. **It is shorter than the event that caused it.** The reaction must be over
   before the user has finished dealing with the outcome. A reaction still
   running when you have moved on is a reaction addressed to you.
3. **It returns fully to rest.** No residue, no "it's pleased for a while", no
   state that persists past the beat.
4. **It never moves toward the viewer.** No scale-up, no bounce, no growth,
   no motion toward the cursor. Growth is a bid for attention. Settling is
   not. The one permitted overshoot (§3.5, resolve) overshoots *within* the
   glyph, in eye height, and is 0.6px.
5. **It is the same every time.** This is the counter-intuitive one and it
   matters. Epley factor (b) says the user is engaged because they are
   *motivated to predict* an opaque agent. A character that surprises you is a
   slot machine and defeats the mechanism it is trading on. Aliveness here
   comes from being predictable in an animal way — same beat, same timing,
   every time — not from variety. **Variation belongs in the idle loop
   (§3.3) and nowhere else.**

### 3.5 The states, and every transition

Five states. Geometry unchanged from §1.1 except where noted.

| State | Eye height | Fill opacity | Notes |
|---|---|---|---|
| `idle` | **3.0** (was 3.5) | **0.85** (was 0.90) | Breath and blink run. Reduced from 3.5 because at idle the pill is 16px tall and the face is the whole of it — "subtle by default" is not currently met by *size*. |
| `listening` | **3.0** (was 4.5) | **0.55** | Deliberately quieter than idle. |
| `thinking` | **2.2** (was 1 / shut) | 0.85 | Narrowed and **held still**. |
| `resolving` | 3.0, peaking 3.6 | 0.85 | New. |
| `error` | 2.2 (was 2) | 0.85 | Frozen through the shake. |

**Blink closed height stays at 1.0.** Not 0 — a fully-collapsed rect
disappears, and a 1px line reads as a closed eye.

#### idle → listening (the user pressed the key)

Immediate, no delay — this is a response to the user's own action (Law 3), and
a delay here reads as lag. 140 ms, ease-out. The eyes do **not** widen. The
user already knows it is listening: the pill grew, the tone played, fourteen
bars are moving, a timer is running. A fifth signal is noise. The face's job
during recording is to get out of the way of the bars, which are, per the
manual, the only honest instrument in the animal. Dropping opacity to 0.55
does that without a layout change.

#### listening → thinking

Starts on the same frame the bars collapse. 180 ms, ease-in-out. Eyes narrow
to 2.2 and **hold, with nothing moving at all**.

This replaces the current shut-for-the-duration behaviour, and the reasoning
generalises into a rule worth stating on its own:

> **Any state that can last longer than ~4 seconds must have a stable
> appearance, not an animation.** An animation you watch for nine seconds
> becomes a loop you can count, and a loop you can count is a progress bar
> that is lying to you.

Holding perfectly still while the rest of the pill pulses is what reads as
concentration. It is also honest: TTP genuinely does not know how long
transcription will take, and a still face claims nothing.

#### thinking → resolving (the words landed) — new, and the most valuable addition

Fires on `stage === 'complete'`, **beginning 140 ms after it** (Law 3).

- 90 ms: eye height 2.2 → 3.6, ease-out.
- 170 ms: 3.6 → 3.0, ease-in-out.
- Done. Total 260 ms, then idle.

No mouth. No colour. No bounce. No sparkle. It is the visual equivalent of the
falling-fourth stop tone the manual describes: it says *done*, and then it
stops talking. The 0.6px overshoot is the entire expressive content and it is
enough, because it is the only overshoot in the whole vocabulary.

#### any → error

The pill already shakes for 320 ms, shows an `AlertCircle`, turns red and
prints a sentence. The face's contribution should be to do **less**:

- Face holds **completely still** for the 320 ms of the shake — no transition,
  no height change. A still face inside a shaking body is what actually reads
  as a flinch; a face that animates during a shake reads as two animations.
- 200 ms after the shake ends, eyes ease to 2.2 over 160 ms and hold while the
  message is up.
- Return to idle with the pill.

**Alternative worth prototyping:** hide the face entirely in the error state.
Four signals already carry it and the face has nothing to add. I prefer the
still-face version because vanishing is itself an event, but if the error pill
reads as cluttered on screen, this is the cheap fix.

#### Reduced motion

The current handling is nearly right and the principle deserves stating
sharply, because it is a distinction implementers usually get wrong:

> **Reduced motion removes transitions and loops. It keeps states.**

So, when `prefers-reduced-motion: reduce`:

- No breath, no blink.
- Every state change is an **instant cut** — no easing, no duration.
- The face still *narrows while thinking* and *narrows on error*, because
  those are states, not animations, and removing them would remove
  information from the user who asked for less motion, not less meaning.
- The resolve beat does not run at all. It is pure animation; there is no
  state underneath it.
- Subscribe to the media query's `change` event rather than reading `.matches`
  once (observation 1.4.5).

### 3.6 The mouth: delete it

Currently a quadratic curve that appears while listening. It should go, for
three separate reasons any one of which would be enough:

1. **The manual denies it.** "It has no legs, no head, and no discernible
   front, and appears to want none of these things." The manual is the object
   the buyer holds and it is supposed to be *true*. A smile contradicts the
   anatomy the product is sold with.
2. **A smile is an emotional claim,** and this character's entire register —
   the register that makes the manual work — is that it *has no opinion*. "It
   does not sulk and it cannot be offended. It has no view on what you dictate."
   A face that smiles at you has a view.
3. **It does not survive the pixel grid.** A 4.8px-wide curve at 1.1 stroke is
   two or three device pixels on a non-Retina display and renders as a smudge.

This is a deliberate deletion from shipped code and it makes the feature
smaller. That is the correct direction: two dots plus correct timing beats any
amount of drawing, and the whole design thesis in §3.1 depends on believing it.

### 3.7 What it does when nothing is happening — the rules for 99% of the time

Idle is the feature. Everything else is 1% of the running time. The complete
idle vocabulary is: **blink, breathe.** Nothing else. And these bans are not
polish, they are the load-bearing part:

- **No wandering, drifting, looking around, tilting or bobbing.** Looking
  around implies attention, attention implies interest, interest implies it
  wants something.
- **The idle behaviour at minute 1 and minute 400 must be byte-identical.** No
  escalation when unused, no settling-in, no "it got comfortable". Any drift
  over time is the care burden wearing a disguise.
- **It reacts to nothing outside the dictation lifecycle.** Not the mouse, not
  the keyboard, not the time of day, not the user leaving and coming back, not
  the calendar. This is a deliberate and important divergence from Bongo Cat,
  which the research names as the closest living relative: Bongo Cat reacts to
  your keystrokes, and TTP must not, because TTP's pill sits on screen during
  work someone is trying to finish, and a thing that reacts to your typing is
  a thing that is *watching you type*. (It would also put a render loop on the
  input path, which §4 of the research disqualifies outright.)
- **Nothing calendrical.** No birthday, no anniversary, no seasonal variant.
  Anything on a calendar becomes a thing to notice, then a thing to wait for,
  which is obligation running backwards.
- **No positive reinforcement for frequency either.** A face that gets happier
  the more you dictate is the same obligation with the sign flipped, and it is
  the failure mode that will feel most like a good idea during
  implementation. Banned.

Restated as the check an implementer can run against any proposal: **if the
face's behaviour depends on anything other than the current dictation state,
it is out.**

---

## 4. The cast, if the answer comes back yes

Everything in this section is conditional on §2 passing. It is written now so
that a pass does not turn into a scramble.

### 4.1 The rules of the set

- **Four, not twelve.** M6 says the whole set unlocks at once — no drip-feed,
  no compulsion loop — so the set does not need to be large to fire
  collecting; it needs to be *complete*. Four is also the number one person
  can animate well, and the number where each can be genuinely different in
  timing rather than a palette swap. **Ship three if any one of them is not
  carrying its weight.**
- **Difference must be in timing and behaviour, not shape.** All four are the
  same two rounded rects. The concrete falsifier for the whole cast:

  > Take a still screenshot of two varieties — you should **not** be able to
  > tell them apart. Take a 10-second silent screen recording of two
  > varieties at idle — you **must** be able to tell them apart. If either
  > test fails, they are the same character.

  That the set is four ways to draw the same two dots is also the joke, and it
  is a good one: it is M7 exactly — conspicuous non-utility as a sincerity
  signal. Four ways to draw nothing.
- **They are varieties, not characters.** No shipped names like "Rupert" or
  "Mochi". The name slot belongs to the owner (M5, endowment through naming),
  and shipping a named character occupies it. So the ids are plain nouns in
  the same register as the sound packs — House, Bowl, Marimba, Felt — and the
  manual's frame is that these are varieties of the same animal.
- **Settings shape.** `companion_face_enabled: bool` becomes
  `companion_face: Option<String>` — `None` is off, otherwise a variety id.
  Migration: existing `companion_face_enabled: true` maps to `"house"`. Ids
  follow the sound-pack rule exactly (lowercase ASCII, stable forever,
  i18n-key-safe) and `cosmetics.rs` grows a `FACES` table beside
  `SOUND_PACKS`, with the same silent fallback to *off* when locked. Names and
  descriptions stay in `src/i18n/locales/` — the Rust table carries `id` and
  `free` and nothing else, exactly as the sound packs do after 818a9c8.

### 4.2 `house` — the survivor

The §3 face, corrected. The baseline and the default.

Blink 4.2–7.8 s irregular, 12% doubles, 90 close / 40 hold / 130 open. Breath
±6%, 4600 ms. Wake 140 ms immediate. Think: narrow to 2.2 and hold. Resolve:
140 ms late, 260 ms, 0.6px overshoot.

**A month next to it:** it is the clock. You stop seeing it around day three
and you would notice if it left. Nothing it does is memorable, which is the
entire specification. This is the one that has to survive; the other three are
variations on a survivor and none of them should be built until it has.

### 4.3 `shut` — the one to build second

Eyes **closed at idle.** Two 2.2×1 dashes. Zero motion when nothing is
happening — no breath, no blink, because there is nothing open to blink.

Its whole personality lives in the ~200 ms where it opens on the key press,
and in a single slow close 400 ms after it returns to idle. Wake: 200 ms,
ease-out, opening to 3.0. Everything else is `house`. Sleep: 260 ms,
ease-in-out, beginning 400 ms after the resolve beat completes — late enough
that it reads as settling rather than as part of the same animation.

**Why this is the strongest candidate to survive a month:** it solves the
peripheral-vision problem completely rather than mitigating it. There is
literally nothing to habituate to, because at idle it does not move at all.
And it makes the design thesis explicit — the character is in the
*transition*, not in the presence.

**A month next to it:** it is the least present thing here and the most
satisfying to use, because every dictation begins with something waking up.
The pleasure is entirely in the wake, and the wake happens only when you asked
for it.

**The risk, named:** closed eyes at idle can read as *off*, or *asleep*, or
*broken* — especially to a user who has just paid for a face and cannot see
it. Mitigations: the settings preview must show the wake (§4.6), and the
description string has to do real work. The manual's own frame helps —
PLATE I is "the Talk-To-Paste at rest… an animal that is awake but has been
given nothing to do."

### 4.4 `drowsy` — the slow one

Heavy. Blink interval 7–13 s, no doubles, close 140 ms and open **260 ms** —
opening slower than closing, which is the inversion of `house` and is exactly
what reads as heavy-lidded. Breath period 6500 ms, amplitude ±10%. Wake takes
260 ms and **starts 80 ms late** — it is behind you, always, by the same
amount. Resolve is a single slow open over 380 ms with no overshoot.

**A month next to it:** it lowers the temperature of that corner of the
screen. It never once startles you. It is the right variety for someone
writing long-form, and it will be the one people keep on during deep work.

**The risk, named:** a late wake and a slow blink are also what a lagging UI
looks like. The mitigation is not subtlety, it is *consistency* — the 80 ms
delay must be constant to the millisecond, because a consistent delay reads as
character and a variable one reads as jank. Do not jitter this one.

### 4.5 `quick` — the alert one

Short blinks (70 close / 20 hold / 90 open), interval 3.2–5.5 s, ~25% doubles
and the occasional triple. Its distinguishing feature is not more motion but
**anticipation**: on wake, the eyes narrow by 0.6px for 50 ms *before* opening
to 3.0 over 90 ms. That is the classic animation anticipation beat, and it is
what makes fast motion read as intentional rather than abrupt. Resolve is
180 ms with a crisper 0.8px overshoot.

**A month next to it:** it is good company for short bursts — messages,
commit descriptions, quick replies — where every dictation is over in four
seconds and the acknowledgement lands like a nod. It is also the variety most
likely to be the reason someone turns the face off, and the description string
should say so honestly rather than selling it. It is bad company for long-form
writing, and admitting that is exactly the register the product uses
everywhere else.

### 4.6 What the settings UI needs

A static swatch cannot show the only thing that differs between these. So:

- Each variety row shows a **live 16px preview running its own idle loop** —
  its actual blink distribution, its actual breath.
- One button per row, mirroring the sound packs' **Listen**, that runs the
  full sequence — wake → listen → think → resolve — in about 2 seconds.
- Reduced motion: previews render as static states with no loop, and the
  sequence button steps through the states as cuts.
- Follow the pattern the sound packs settled on after 4aa576e and 818a9c8: a
  row with a radio-ish control and a separate preview button, **not**
  `RadioOption` (its description slot is for a short trailing word) and **not**
  a button inside a button.

---

## 5. Constraint check

| Constraint | How this design meets it |
|---|---|
| **Subtle by default** | Default is off. When on, the default variety is `house`. Idle eye height drops 3.5 → 3.0 and opacity 0.90 → 0.85, because at 16px the face currently *is* the idle pill. Idle motion is bounded below the peripheral event threshold (§3.2) — one blink per ~6 s and a sub-pixel breath. The face gets *quieter* during recording, not louder. The mouth is deleted. |
| **Independently switchable** | `companion_face` is its own setting, orthogonal to `sound_pack`, `None` = off. Nothing bundles them. The existing `companion_face_enabled` migrates to `"house"`. |
| **Honours `prefers-reduced-motion`** | Breath and blink off; every transition becomes an instant cut; the resolve beat does not run; states (`thinking` narrow, `error` narrow) are kept because they are meaning, not motion. Plus: subscribe to the media query instead of reading it once. |
| **It never demands anything** | Nothing depends on anything but the current dictation state (§3.7). No decay, no streak, no counter, no growth, no calendar, no reaction to absence — **and no reward for frequency**, which is the same obligation with the sign flipped. Explicit ban list in §3.7. |
| **No user-facing prose in Rust** | The Rust `FACES` table carries `id` and `free`, exactly as `SOUND_PACKS` does. Every displayed string is an i18n key (§6). Trace event names and JSON keys are not displayed and are not affected. |

---

## 6. Strings this design implies

**Named, not added.** All under `settings.companion.*` in
`src/i18n/locales/en.json` and `fr.json`, alongside the existing
`settings.companion.packs.*`.

```
settings.companion.faceLabel            (exists — reword: it selects a variety now, not a boolean)
settings.companion.faceDesc             (exists — may survive unchanged)
settings.companion.faceOff              off / none
settings.companion.previewFace          the per-row sequence button, sibling of `preview`
settings.companion.faces.house.name     + .desc
settings.companion.faces.shut.name      + .desc
settings.companion.faces.drowsy.name    + .desc
settings.companion.faces.quick.name     + .desc
```

Register for the `.desc` strings, from the sound packs: one sentence, plain,
observational, no winking. "A drop going in, and a drop coming back out." is
the bar. `quick.desc` in particular should tell the truth about who it is bad
for.

`settings.companion.nameLabel` / `namePlaceholder` already exist and are
unaffected — the name belongs to the animal, not to the variety, and survives
switching between them.

---

## 7. Where I am guessing

Listed separately because the programme's culture is that this gets said out
loud.

1. **The peripheral event thresholds** (~1px / ~2s, ~2px / ~400ms) are
   calibrated judgement, not measurement. The ratio is robust; the absolute
   numbers need a screen recording at 1× at the real position.
2. **Human blink statistics** (~15–20/min conversational, ~4.5/min focused
   screen work, 100–150 ms duration) are recalled, not looked up. They set the
   ballpark for §3.3 and nothing else depends on them; the irregularity
   matters far more than the rate.
3. **That the sub-pixel breath is perceptible at all** at 16px. It may be
   invisible, in which case cut it. §3.2 gives the test.
4. **That `shut` reads as dormant rather than broken.** This is the biggest
   single uncertainty in §4 and the reason `shut` is second in build order
   rather than first, despite my thinking it is the best of the four.
5. **That a 140 ms delay is the right value for Law 3.** The principle is
   solid; the number is a starting point. Anything in 120–200 ms should work,
   and the tell that it is too long is that the beat feels detached from the
   paste rather than caused by it.
6. **That four is the right size for the set.** It is a judgement about what
   one person can animate well and about M6 firing on completeness rather than
   count, not a measured optimum.
7. **The whole of §4** is conditional on §2. If the survival test comes back
   no, §4 is not a plan that needs adjusting — it is a plan that does not get
   built.
