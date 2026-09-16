# TTP — the overhaul

Standing brief for a multi-session, multi-agent programme. Written to be picked
up cold by a manager agent that has never seen this codebase.

The goal is not a feature. It is to take a dictation app that works and make it
*good* — sharp enough that someone shows it to a friend, honest enough that
nothing in it is hidden, and finished enough that its paid tier is a gift
rather than a toll.

---

## 0. Read these first, in this order

| File | What it settles |
|---|---|
| `docs/ttp-pro-design.md` | The product arbitration. The non-negotiable rule is in the first paragraph. |
| `docs/fun-purchase-research.md` | Why anyone pays for something useless. 67 sources, every claim tagged STRONG/MODERATE/WEAK/FOLKLORE. |
| `docs/companion-manual.md` | The voice of the product. 3,600 words, deadpan, never winks. |
| `docs/tracing.md` | How to read `ttp-trace.log`, and what it cannot tell you. |
| `docs/architecture.md` | The codebase as it stood before this programme. |
| `.claude/projects/*/memory/MEMORY.md` | User context and standing decisions. |

**The rule everything obeys:** TTP Pro unlocks nothing you need. Every feature
that makes a transcription happen is free and uncapped. The paywall was removed
on 2026-08-28 and does not come back in a smaller shape. If a workstream finds
itself proposing "just a small limit on X", it has gone wrong.

---

## 1. Where things stand

**The app.** Tauri 2 + React. Rust in `src-tauri/src/`, UI in `src/`, Astro
marketing site in `landing/`. macOS-first, Windows builds in CI.

**Reliability.** Five days of instrumented use, 339 traced dictations. Seven
real defects found and fixed — a permanently-dead event tap, a hallucination
filter that deleted real speech, a decommissioned Groq model, an exhausted
polish quota, AirPods delivering 21 seconds of digital silence, a start/stop
race that left the microphone live, and the keychain sitting on the dictation
critical path.

The original complaint — "I press, I dictate, nothing is written" — was **never
reproduced**. Its diagnosis (enigo inheriting the Globe modifier flag) comes
from reading enigo and core-graphics sources, not from observation. Three
independent links in that chain are now cut. Treat it as probably fixed and
definitely unproven.

**The Companion.** Six sound packs, a face for the pill, a name, and the
manual. `cosmetics.rs` is the seam; `TTP_COSMETICS=1` unlocks it for
development.

**Unshipped.** ~25 commits on `polaris/v3.0.0`, never pushed, version still
3.1.6 while `v3.1.6` is already tagged. Every test iteration costs an ad-hoc
rebuild, a TCC reset and a keychain password. Fixing this is cheap and unblocks
everything else — see workstream G.

---

## 2. Workstreams

Each is independently ownable. Files listed are the expected blast radius;
respect them so parallel agents do not collide.

### A — Appearance: the pill as a character

**The headline.** Amir's own idea, and probably a stronger draw than the
sounds: not one face but several, with distinct animation personalities.

The research says why it could work — anthropomorphism (Epley factor (b): the
user is already watching an opaque agent they are motivated to predict), plus
endowment through naming, plus collecting. It also says why it is the riskiest
thing here: a face in peripheral vision can go from charming to intolerable in
a week, and nobody has yet lived with the single face for a week.

So the first question is not "how many faces" but **"does one survive normal
use"**. Answer that before multiplying the bet.

This is a design problem before it is a coding problem. Getting a character to
feel alive rather than gimmicky is where it succeeds or fails.

Hard constraints: subtle by default, independently switchable, honours
`prefers-reduced-motion`, and **it never demands anything** — nothing decays,
nothing needs feeding, nothing is ever sad that you did not dictate today. We
take Tamagotchi's attachment and refuse its care burden.

*Files:* `src/windows/FloatingBar.tsx`, `src/components/ui/`, settings plumbing.

### B — Marketing research

Genuine research, not a listicle. What actually converts for a free, local,
privacy-first macOS utility sold by one person?

Worth investigating: how comparable tools present themselves (Raycast,
Superwhisper, MacWhisper, Obsidian, Bartender, CleanShot); what indie
developers report about what actually drove installs; whether "local and
private" is a selling point or table stakes in 2026; the honest role of Show
HN, Product Hunt and r/macapps; and what the download-to-activation funnel
looks like when there is no trial to expire.

Deliverable: `docs/marketing-research.md`, same standard as
`fun-purchase-research.md` — primary sources, evidence tagged by strength,
folklore separated from fact. That file is the quality bar; match it.

### C — The website

`landing/` is Astro with bilingual pages. It currently describes an app with a
paid tier that no longer gates anything, so it is at best out of date and at
worst untrue.

Rewrite around what is actually true now: everything free, everything
uncapped, and an optional purchase that buys a companion and a manual. The
manual is the strongest asset nobody can see — figure out how a visitor
encounters it.

Wait for B before writing copy. Design and build can start immediately.

*Files:* `landing/` only.

### D — Anti-patterns of AI-built software

Amir asked for this specifically, and it is the most interesting brief here.

This codebase was largely written by AI, and AI-written code has
characteristic failure modes. Some are visible in this repository right now —
find them, name them, and say what to do about each:

- Defensive code that hides failures instead of surfacing them. Seven bugs in
  this app were invisible for weeks because every layer degraded politely.
- Comments that restate the code instead of explaining the decision.
- Abstractions built for a second caller that never arrived.
- Tests asserting the implementation rather than the behaviour, which pass
  through a rewrite that breaks the feature.
- Plausible-looking constants nobody measured. (Counter-example worth
  studying: the sound synthesis harness, where every threshold has a
  measurement behind it.)
- Consistency-by-copy: a pattern propagated to places it does not fit.
  `RadioOption` was misused three times in three days here.
- Work that looks complete because it compiles and the tests pass.

Then: what does *good* look like for a small local-first desktop app?
Observability, error handling, permission handling, update strategy, what to
test and what not to bother testing.

Deliverable: `docs/engineering-standards.md` — evidence-based, with concrete
examples pulled from this repository, and an audit of what should change.

### E — Codebase audit

Run against D's standards once they exist. Correctness first, then the honest
cleanups. `/code-review` and `/security-review` are available.

Specific known debt: `licensing::storage` tests reach the real macOS keychain
and block on an authorization dialog on every recompile, so nine tests are
skipped locally. Several `*  2.ts` duplicate files are lying around. The
`panic = "abort"` release profile makes `catch_unwind` in the pipeline
decorative.

### F — The manual, delivered

`docs/companion-manual.md` exists and no buyer can receive it. It needs to
become an object — a bundled PDF, a page, something. The research is explicit
that this is what turns "I bought some cosmetics" into "I bought a thing".

A French edition is a **rewrite, not a translation**: the letter-Q section and
the *cue/queue/Kew* argument are structurally English. French needs its own
letter and its own hallucination examples — the codebase supplies real ones.

### G — Ship it

Push the branch, bump to 3.1.7, let CI produce a Developer ID build. This ends
the reinstall-and-password cycle that taxes every other workstream. Amir's
call on timing, but propose it early.

---

## 3. How to work

**Trust the evidence over the story.** Twice in this programme a confident,
well-argued fix was wrong and a failing test caught it: a peak-based silence
gate that would have let keyboard clicks through, and a "reproduced bug" that
was a 32-second dictation misread as an orphan. Write the test first when the
claim matters.

**Delegate wide, keep context narrow.** Subagents for research and for
parallel, non-overlapping file sets. Persist findings to `docs/` rather than
carrying them in a context window. A manager session should hold the plan and
almost none of the detail.

**Say what is not done.** Every report distinguishes what was proven from what
was inferred, and names what was left out. "Probably fixed and definitely
unproven" is a better sentence than a confident one that is wrong.

**Ask when the answer changes the work.** Do not ask for permission to do
ordinary work; do ask when two readings lead to materially different products.

---

## 4. Verification

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cd src-tauri && cargo test --lib -- --skip licensing::storage   # 202 tests
cd .. && npx tsc --noEmit && npx vitest run && npm run i18n:check
```

`licensing::storage` is skipped for the keychain reason above — fixing that
(stub the secret, as `fresh_record()` already does) is a small win for every
future session.

**No user-facing prose in Rust.** The app is bilingual; anything displayed goes
through `src/i18n/locales/`. `npm run i18n:check` enforces it.

**`RadioOption` is for a short trailing word.** If the content is a sentence or
contains a control, write the row.

---

## 5. What success looks like

Not a longer feature list. A product where the free tier is complete enough
that paying is obviously a gift, the paid tier is charming enough that people
want to, the website tells the truth, the code has standards written down and
met, and the next failure explains itself in the trace without anyone having to
guess.
