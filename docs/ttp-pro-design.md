# TTP Pro — design decision

Source of truth for the optional purchase. Evidence and mechanism references
point at `fun-purchase-research.md`; this document is the arbitration, not the
research.

## The rule everything else follows from

**TTP Pro unlocks nothing you need.** Every feature that makes a transcription
happen — polish, dictionary, history, languages, everything — is free and
uncapped, and stays that way. The paywall was removed on 2026-08-28 and is not
coming back in a smaller shape.

Pro is an *object you adopt*, not a tier you upgrade to. The research is blunt
about why this matters: warm glow (M2) means the utility comes from the act of
giving, so the thing received does not need to be worth €17 — and a
conspicuously useless thing (M7) sidesteps the value comparison a
half-useful feature would invite and lose. The moment we smuggle in real
utility "so it feels worth it", the sincerity signal that makes the whole
thing work is gone.

Corollaries, all non-negotiable:

- No payment slider, no pay-what-you-want. PNAS 2012: naming your own small
  number forces an unflattering self-image and people walk. One named tier,
  one fixed price.
- No subscription. Recurring payment for a joke is not a joke, it's a bill.
- Nothing that adds latency or unpredictability to the dictation path.
- No nagging, no confirmshaming, no fake scarcity, no usage-count invoicing.
  Exactly one discoverable mention, in Settings, that does not move.

## What we are building

**The Companion.** One purchase, four parts that reinforce each other.

### 1. A voice — sound packs

The core. Replaces the start/stop beeps with a set of lovingly over-produced
alternatives, all unlocked together.

Ranked #1 in the research for a reason that survives scrutiny: it is the only
TTP cosmetic that **solves the no-audience problem**. Almost every canonical
"useless purchase" runs on social visibility, and a menu-bar app has no
audience — except a sound, which everyone in the room hears. Your colleague
asks what that was.

`sounds.rs` already plays embedded WAVs and extends to a set with no new
render path, no new UI surface, and zero risk to the hot path. Sounds are
already toggleable, so the escape hatch exists.

### 2. A face and a name — the pill as a character

Highest emotional ceiling, highest annoyance risk. The pill is the surface the
user already stares at while wondering whether they were heard — textbook
Epley factor (b), an opaque agent you are motivated to predict. Giving it a
face and letting the user *name* it fires anthropomorphism (M4), endowment via
naming, and the IKEA effect through a sixty-second customisation that cannot
fail (M5).

Mandatory mitigations, because a face in peripheral vision can go from
charming to intolerable in a week:

- default subtle
- fully switchable, independently of the sound packs
- honour `prefers-reduced-motion`
- **it never demands anything.** We borrow Tamagotchi's attachment and
  explicitly refuse its care burden. Nothing decays. Nothing needs feeding.
  Nothing is ever sad that you did not dictate today.

### 3. A manual — *The Care and Training of Your Talk-To-Paste*

The actual documented Pet Rock mechanism. The manual was the product; the rock
was packaging. This is what turns "I bought some cosmetics" into "I bought a
thing" (M1: it is an object, not a licence key).

Weak standalone, so it ships bundled. Zero engineering, substantial writing —
and per the research, the writing has to be genuinely, laboriously good or it
reads as a cheap gag and undermines everything above it.

### 4. Deferred: supporters wall

Opt-in name on a public page. Cheap, and the only other route to genuine image
motivation. Needs a website and a backend, so it is out of this pass. Recorded
here so it is not re-litigated later.

Explicitly not doing: physical objects (postage and addresses destroy the
margin and create a privacy liability for a privacy-branded app — worth
exactly one capped launch stunt, never a standing offer), tray icon sets (16
points of grayscale carrying no emotional payload), drip-fed packs.

## Price

€17, once. Between Apollo's $20 Godzilla Tip and Obsidian Insider's $25, both
proven on free tools. Roughly 5× the natural undirected voluntary payment —
that gap is closed by giving the purchase an identity and an object, never by
lowering the number. If a second tier is ever added it goes *above*, never
below.

## Naming

The product is **TTP Pro** in the store because that is what the existing
Lemon Squeezy checkout sells and existing keys must keep working. Inside the
app it is never called Pro, because "Pro" implies withheld capability. It is
**the Companion**, and the Settings section is **Support TTP**.

## Implementation shape

The licence layer already exists and is dormant — it gated the three caps and
now gates nothing (`licensing::is_pro_or_trial_disk`, kept deliberately). It
gets repurposed from a gate on *features* to a gate on *cosmetics*.

```
licensing (existing, dormant)
    └── cosmetics::unlocked() -> bool
            ├── sounds::active_pack()      → which WAV set plays
            └── settings.companion_*       → face, name, enabled
```

Hard constraint on the seam: **a locked TTP and an unlocked TTP must take the
same code path through a dictation.** The only difference is which bytes get
handed to rodio and which SVG the pill renders. Nothing in the transcription
pipeline learns that cosmetics exist.

Degradation is silent and total: if the licence check fails, is offline, or
the file is corrupt, the user gets the default sounds and the plain pill and
no message. A cosmetic that nags about its own licence is worse than no
cosmetic.

## Workstreams

| # | Work | Owner | Files |
|---|---|---|---|
| A | The manual | agent | `docs/companion-manual.md` |
| B | Sound pack synthesis + assets | agent | `scripts/synth_sounds.py`, `src-tauri/sounds/packs/` |
| C | Cosmetics plumbing, pack selection, settings | main | `src-tauri/src/cosmetics.rs`, `sounds.rs`, settings |
| D | The pill's face and name | main | `src/windows/FloatingBar.tsx` |

A and B touch no shared files and run in parallel. C lands before D.
