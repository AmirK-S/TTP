# Marketing research: what actually converts for TTP

**Question being answered:** TTP is a macOS-first dictation utility, sold by one
person, fully free and uncapped, with an optional ~€17 purchase that unlocks
cosmetics and a manual. What does the *evidence* say about what drives installs
and what drives that purchase — as opposed to what the growth-marketing genre
says?

**How to read this:** same tagging as `fun-purchase-research.md`.

- **[STRONG]** — replicated experiments, field data, or primary numbers I
  fetched myself
- **[MODERATE]** — single good study, first-hand report with numbers, or
  industry data with a clear source
- **[WEAK]** — plausible theory, thin empirical support in *this* domain
- **[FOLKLORE]** — repeated everywhere, traceable to a single self-interested
  source, or to no source at all

**Method note.** Competitor copy was fetched as raw HTML on **2026-08-31** and
text-extracted locally, not summarised by a model and not recalled from
memory. This matters: a first pass through a summarising fetcher reported
Superwhisper Pro as "$849/month". The raw page's schema.org markup says
`"price":"8"` and the string `8.49` appears in the bundle. It is $8.49/month.
Every price and quote below was read out of the page source.

Where I generated a dataset (Hacker News, Product Hunt, GitHub) the query is
described so it can be re-run.

---

## 0. The one structural fact that shapes everything

`fun-purchase-research.md` opens with the no-audience problem. This document
has to open with something worse, because it invalidates the premise I was
given.

**TTP is not a local transcription app. It sends your audio to Groq.**

```
src-tauri/src/transcription/whisper.rs:28
const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

src-tauri/src/transcription/polish.rs:27
const CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

src-tauri/src/dictionary/classify.rs:11
const CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
```

There is no local model anywhere in the tree. There is no `whisper.cpp`, no
`whisper-rs`, no CoreML path, no fallback. Every dictation leaves the machine.
[STRONG — it is in the repository]

The marketing site currently says, under a heading reading **"Privacy First"**:

> `landing/src/i18n/ui.ts:63` — `'features.privacyDesc': 'API keys and history
> stored locally. Nothing leaves your machine.'`

The sentence is defensible if you read "Nothing" as scoped to the two nouns in
the previous sentence. Nobody reads it that way. Placed under "Privacy First",
on a page for a *dictation app*, "Nothing leaves your machine" is read as a
claim about the audio, and the claim about the audio is false.

The README, by contrast, is honest and precise:

> "Free forever. No account required. Bring your own Groq API key."
> "**Privacy first** — API keys in your OS keychain, history local-only,
> telemetry off by default."

So the repository already contains the correct sentence. The website contains
the wrong one. **This is the single highest-priority item in this document, and
it is a correctness fix, not a copy preference.**

Two reasons it is urgent rather than merely tidy:

1. **The audience TTP is courting is specifically primed to check.** See §2 —
   the market has been burned by exactly this claim, and now audits it.
2. **The honest version is a better story than the false one.** See §6.

Everything downstream in this document assumes the local claim is dropped.

---

## 1. How the comparables actually present themselves

All fetched 2026-08-31. Quotes are verbatim from page source.

### Superwhisper — the market leader, and it does not mention privacy

https://superwhisper.com/

Above the fold, in order:

> **"Just speak. Write faster. Turn your voice into polished text."**
> "Works in Slack, Gmail and any other site or app."
> `Download` · `Watch my demo`
> "Select an app, press ⌥ + space and start dictating to try it out yourself!"
> "Used by those who move fast"

Word counts on the homepage HTML:

| term | occurrences |
|---|---|
| offline | 18 |
| privacy | 2 (both the footer "Privacy Policy" link) |
| local | 1 |
| private | 0 |
| on-device | 0 |
| "your data" | 0 |
| "never leaves" | 0 |
| encrypt | 0 |

[STRONG — reproducible: `grep -oi <term> superwhisper-homepage.html | wc -l`]

This is the most important competitive fact in the document. **The category
leader has removed privacy from its positioning entirely.** Its local-execution
capability appears once, framed as *availability*, not as confidentiality:

> "Duh, yes online too" / "Works offline"
> "Superwhisper works offline, so you can transcribe anytime. No Wi-Fi, no
> problem."

What it leads with instead: speed, ubiquity, and social proof. The testimonial
wall is Andrej Karpathy, Pieter Levels, Andrew Wilkinson, Guillermo Rauch — all
quoted verbatim from their own tweets.

**How it handles "free but there's a paid thing":** the free tier is real and
generous.

> **Free** — "Must-have features for everyday use." — Voice to text that works
> in any app · Meeting recording and transcription · Support for 100+ Languages
> · **Unlimited use of Whisper models** · Custom prompt control · Email Support
>
> **Pro** — $8.49/month — "Everything in Free, plus:" — **Use your own AI API
> Keys** · Unlimited use of Cloud and Local AI models · Translate any language
> to English · Transcribe audio and video files · Priority Support

Note what is in the Pro column: **"Use your own AI API Keys."** For the market
leader, BYOK is a *power-user upsell*. For TTP it is mandatory before the first
word is transcribed. Hold that thought for §4.

And the FAQ, which describes a trial that does not expire in time:

> "Yes, you can try the Pro features for 3,000 words for free, after that the
> free tier features are available to you forever."

### MacWhisper — privacy is present, but attached to an artefact

https://www.macwhisper.com/ · https://goodsnooze.gumroad.com/l/macwhisper

Above the fold:

> **"The all-in-one Mac app to transcribe it all."**
> "Like lectures, videos, podcasts and **private files**. It also records all
> your online meetings. **No bots joining.** And dictation lets you use your
> voice in any app."

Privacy is a whole section — the fourth one down, not the hero:

> **"Built for privacy from the ground up"**
> "Private files — We use local models to transcribe your files, so it's
> perfect for your most **sensitive and private files**"
> "Offline — Process sensitive content locally without data **ever leaving your
> Mac**."
> "Local models — Transcribe sensitive files with complete privacy using local
> AI models."

Counts: privacy 12, private 8, local 24, offline 2. [STRONG]

**The pattern worth stealing:** MacWhisper never says "we respect your
privacy". Every privacy sentence names a *thing you own that you would not want
read* — your lecture, your interview, your sensitive file, your meeting. The
abstraction is always cashed out into a noun. Compare Obsidian below; it is the
same move.

Pricing:

> **"Use MacWhisper for free. Level up with Pro."**
> MacWhisper Free — **€0 / month** — "Free forever" — `Download Free`
> MacWhisper Pro — **Pay Once** — **€64 / license** — "Includes lifetime
> updates" — `Purchase license`

Free includes "System wide dictation"; Pro includes "System wide **high
quality** dictation", batch, YouTube import, speaker recognition, meeting
recording, and — again — "Bring your own API keys to connect to AI providers".

Also worth noting for a solo dev's trust posture, verbatim from the footer:

> "Please note that www.macwhisper.com is the official website for MacWhisper.
> Unfortunately there are scam websites online that pretend to be the official
> website for MacWhisper, and they may contain malware downloads."

And the credit line, which is doing positioning work:

> "Built by Jordi Bruin and friends at Good Snooze in the Netherlands"

### Wispr Flow — $81M, and it leads with a before/after

https://wisprflow.ai/

> **"Don't type, just speak."**
> "The voice-to-text AI that turns speech into clear, polished writing in every
> app."
> `Download for free` — "Available on Mac, Windows, iPhone, and Android"
> "Wispr raises $81M to build the Voice OS."

The hero is not a feature list. It is a **side-by-side of raw speech and
finished text** — the messy version ("Umm, hope your week has started
well…I was talking to Cheyene earlier but reception was really bad and I think
their going to handle the first part…") next to the cleaned version ("Hope your
week is off to a good start. I was talking to Cheyene earlier, but the
reception was really bad. I think they're going to handle the first part…").
Then a speed comparison: "Keyboard 45 wpm" vs "Flow 220 wpm", "4x faster than
typing".

Counts: privacy 10, local 6, private 1, offline 0.

### Obsidian — the closest structural analogue, and the copy to study hardest

https://obsidian.md/ · https://obsidian.md/pricing

Above the fold:

> **"Sharpen your thinking."**
> **"The free and flexible app for your private thoughts."**

Then three parallel claims, each a second-person possessive sentence:

> **"Your thoughts are yours."** "Obsidian stores notes privately on your
> device, so you can access them quickly, even offline. No one else can read
> them, **not even us**."
>
> **"Your mind is unique."** "With thousands of plugins and themes, you can
> shape Obsidian to fit your way of thinking."
>
> **"Your knowledge should last."** "Obsidian uses open file formats, so you're
> never locked in. You own your data for the long term."
>
> **"Free without limits."** `Download now`

Three things to take from this and one to notice:

1. Every privacy sentence is **a sentence about the user's possession**, not
   about the vendor's virtue. "Your thoughts are yours", not "we take privacy
   seriously".
2. The privacy claim ships with **a second, selfish benefit stapled to it** —
   "so you can access them quickly, even offline". Privacy alone is not
   presented as sufficient reason.
3. The punchline is **"not even us"** — an admission of the vendor's own
   incapacity, which is the only form of the claim that is checkable.
4. Notice what is *absent*: the word "encrypted", any lock iconography, any
   compliance badge.

Pricing page:

> **"Free without limits. No sign-up required. No strings attached."**
> "Optional add-on services make it easy to sync and publish your notes."
>
> **"100% user-supported."**
> "Optional licenses help support the independent development of Obsidian."
>
> **Catalyst — $25 USD — One-time payment — `Support Obsidian`**
> Early access to beta versions · Community badges · Exclusive channels

**Update to `fun-purchase-research.md`:** that document records Catalyst as
three tiers, $25 / $50 / $100. As of 2026-08-31 the public pricing page shows
**one Catalyst tier at $25**, plus a separate Commercial licence at $50/user/
year. The three-tier structure is gone. This weakens — it does not kill — the
"let people self-select upward" recommendation in §6 of that document. Obsidian
had the tiers and removed them. That is evidence, and it points the same way as
TTP's own arbitration: one named tier, one price.

The reason-to-buy copy is worth quoting exactly, because it is the hardest
problem TTP has and Obsidian solved it in one sentence:

> "Catalyst plays a vital role in helping Obsidian remain 100% user-supported,
> **free from investor influence that could compromise our values**"

The purchase is not framed as buying a badge. It is framed as **buying the
absence of a worse business model.** That is a real thing to sell, it costs the
seller nothing, and it is true for TTP in a stronger form than it is for
Obsidian (which has taken no outside money either, but is a company; TTP is one
person).

### Raycast — the answer to "why is this free?"

https://www.raycast.com/ · https://www.raycast.com/pricing

> **"Your shortcut to everything."**
> "A collection of powerful productivity tools all within an extendable
> launcher. Fast, ergonomic and reliable."
> "It's not about saving time. It's about feeling like you're never wasting
> it."

And the FAQ entry TTP needs an equivalent of:

> **"Why is Raycast free for personal use?"** — "We think of Raycast as a
> productivity layer that everybody should use to get work done faster. To make
> it accessible, we don't charge for the individual plan."

Also notable: Raycast explicitly disarms the "is the free tier a trap?"
question — "Is the free Team plan a trial?" → "No, the free plan for Teams
doesn't expire, and isn't a trial."

**A free product provokes the question "what's the catch?" and every one of
these companies answers it explicitly in copy.** TTP currently does not.

### CleanShot X — no free tier at all

https://cleanshot.com/

> **"Capture your Mac's screen like a pro."**
> `Buy now` · "30-Day Money-Back Guarantee"
> "It feels like 7 apps in one." / "CleanShot X provides over **50 features**"

Privacy appears only as a negative-space reassurance — "Cloud account is not
required to use the app" — and on-device processing is sold as **speed**:
"Super fast on-device recognition". Not privacy. Speed.

Its social proof is named individuals with company affiliations (Daniel Zarick
of Arrows.to, Tyler Tringas of Earnest Capital, Chris Messina), plus "4.9 Based
on user reviews" and "350+ tweets".

### Bartender — the trial-based model, and a cautionary tale

https://www.macbartender.com/

> **"Raising the bar. Again."**
> "Bartender is an award-winning app for macOS that for **more than 10 years**
> has superpowered your menu bar…"
> `Download free trial` · "**4-week unlimited trial** - upgrade discount
> available"

No privacy positioning whatsoever. The lead is longevity and awards.

Bartender is in the brief presumably as a positioning comparable, but its most
useful lesson is about trust rather than copy: in 2024 Bartender changed
ownership without announcement, was flagged by MacUpdater, and spent
considerable goodwill. I could not re-verify the primary sources for that
episode within this pass, so I am tagging it **[WEAK — recalled, not
re-sourced]** and noting it only because the structural point stands on its
own: for a menu-bar utility with deep system permissions, *who is behind it*
is part of the product, and TTP currently under-uses this (see §6, "the person").

### Summary table

| | above the fold | privacy in hero? | free tier | paid |
|---|---|---|---|---|
| Superwhisper | speed + "works everywhere" + big-name social proof | **no** (0 uses of "private") | real, uncapped Whisper | $8.49/mo, lifetime, enterprise |
| MacWhisper | "transcribe it all" + sensitive-file use cases | **no** — section 4 | real, dictation included | €64 one-time |
| Wispr Flow | before/after transcript + 4x speed | no | yes | subscription |
| Obsidian | "free and flexible app for your **private thoughts**" | **yes**, in the subhead | everything | $25 one-time, buys nothing you need |
| Raycast | "your shortcut to everything" | no | everything personal | $/mo for AI |
| CleanShot X | "like a pro" + 50 features | no | none | one-time |
| Bartender | 10 years + awards | no | 4-week trial | one-time |

**Nobody in the dictation category leads with privacy. The only product in the
set that does is Obsidian — and Obsidian is also the only one whose privacy
claim is structurally unfalsifiable (plain files on your disk).**

---

## 2. Is "local and private" a selling point or table stakes in 2026?

This is the highest-value question in the brief and the assumption it tests is
wrong twice over: TTP cannot make the claim (§0), *and* the claim has stopped
working.

### 2a. Primary data: the claim has commoditised

I pulled every Hacker News story whose title begins "Show HN" and matches
`dictation`, `speech-to-text`, `transcription`, `voice typing`, or `whisper
mac` from the Algolia API (`hn.algolia.com/api/v1/search_by_date`,
`tags=story`, deduplicated). n = 1,062. I then flagged titles containing
`local|offline|on-device|private|privacy`.

**Share of Show HN titles in this category using a local/private word:**

| year | n | with the word | share |
|---|---|---|---|
| 2020 | 26 | 0 | 0% |
| 2021 | 17 | 1 | 6% |
| 2022 | 31 | 1 | 3% |
| 2023 | 96 | 5 | 5% |
| 2024 | 142 | 5 | 4% |
| **2025** | **299** | **64** | **21%** |
| **2026** (to Aug) | **377** | **115** | **31%** |

[STRONG — reproducible from a public API]

Two things happened at once. The category volume grew ~2.6× from 2024 to 2026,
and the differentiator went from 4% adoption to 31%. **A claim that a third of
your competitors make in the title is not a differentiator. It is an entry
requirement.**

### 2b. And it does not predict success

Same dataset, splitting on the keyword:

| | n | median pts | mean pts | p90 | share ≥50 pts |
|---|---|---|---|---|---|
| title says local/offline/private | 202 | 2.0 | 16.6 | 25 | 5.4% |
| title does not | 860 | 2.0 | 17.8 | 22 | 7.1% |

[STRONG for the correlation; **the causal reading is WEAK** — this is
observational, heavily confounded by project quality and submitter reputation,
and a null result on a noisy metric is not proof of no effect.]

The honest summary: putting "local" in your title neither helps nor hurts. It
is invisible.

### 2c. What *does* correlate with the top of the category

The top performers in this space all pair the local claim with a **second,
harder-to-copy** claim:

| pts | title |
|---|---|
| 591 | Show HN: Whispering – **Open-source**, local-first dictation you can trust |
| 467 | Show HN: Ghost Pepper – Local hold-to-talk speech-to-text **for macOS** |
| 399 | Show HN: **Port of OpenAI's Whisper model in C/C++** |
| 316 | Show HN: Moonshine **Open-Weights** STT models – **higher accuracy** than WhisperLargev3 |
| 289 | Show HN: OWhisper – **Ollama for** realtime speech-to-text |

Open-source, or a specific technical achievement. Never "local" alone.
[MODERATE — n is small at the top of a power-law distribution]

### 2d. The claim is now met with suspicion, not credit

This is the part that should actually change TTP's copy. From the Whispering
Show HN thread (591 pts, https://news.ycombinator.com/item?id=44942731), the
author's own opening:

> "For years, I relied on transcription tools that were almost good, but they
> were all closed-source. Even a lot of them that **claimed to be 'local' or
> 'on-device' were still black boxes that left me wondering where my audio
> really went**."

And in the same thread, a commenter auditing that author's own claim within
hours:

> **Aachen:** "Wait, I'm confused. The text here says all data remains on
> device and emphasises how much you can trust that… Clicking on the demo
> video, step one is… configuring access tokens for external services? Are the
> services shown at 0:21 (Groq, OpenAI, Antrophic, Google, ElevenLabs) doing
> the actual transcription, listening to everything I say…? Because that's not
> at all what I expected after reading this description"

[STRONG as evidence of audience behaviour — it is a public, timestamped audit
of a privacy claim by a reader, on the exact architecture TTP has.]

**Read that comment as a rehearsal of TTP's Show HN.** TTP's setup screen asks
for a Groq API key. A page claiming "nothing leaves your machine" and a setup
flow that begins "get an API key from console.groq.com" is the identical
contradiction, and it will be found in the first hour.

### 2e. The related preference that *is* live: one-time pricing

From the same thread, unprompted:

> **mrbig0:** "Honestly, I'm getting tired of subscription-based apps. **If
> it's truly offline, shouldn't it support a one-time purchase model?** The
> whole point of local-first is that you're not dependent on ongoing cloud
> services, so why structure pricing like you are?"

[MODERATE — a single articulate comment, but it names a coherence the audience
is checking for: *architecture and pricing model should agree*.]

TTP's structure is unusually coherent here and nobody has said so out loud:
there is no TTP server, there is no subscription, and there is nothing to
cancel. That coherence is a stronger asset than the word "private".

### 2f. Verdict

**Table stakes, trending toward liability.** In 2026 the local/private claim:

- is made by 31% of the category (§2a),
- does not predict attention (§2b),
- is audited on arrival by exactly the audience that would find TTP (§2d),
- and TTP cannot honestly make it (§0).

Recommendation in §8. The short version: replace the *capability* claim with a
**relationship** claim, which is true, checkable, and which none of the
competitors can match.

---

## 3. The honest role of Show HN, Product Hunt and r/macapps

### 3a. Show HN: a lottery with a floor of nothing

Same dataset as §2. Points distribution for Show HN posts in the
voice/transcription category:

| query | n | median | mean | ≥100 pts | ≤5 pts |
|---|---|---|---|---|---|
| "Show HN" + dictation | 366 | **3** | 15.4 | 3% | **79%** |
| "Show HN" + speech-to-text | 191 | **2** | 21.0 | 6% | **77%** |
| "Show HN" + transcription | 524 | **2** | 15.9 | 4% | **76%** |

[STRONG — public API, reproducible]

**Roughly four out of five Show HN posts in TTP's exact category score five
points or fewer.** The median is two. The mean is dragged up entirely by a
handful of outliers. Planning around the outlier is the standard error.

### 3b. What a *winning* Show HN is actually worth, in installs

Ghost Pepper (`matthartman/ghost-pepper`) is the cleanest natural experiment
available, because it is on GitHub and its release history is public. Its Show
HN was 2026-04-06 and scored 467 points. Per-release asset downloads from the
GitHub API:

| release | date | downloads |
|---|---|---|
| v1.7.0 | 2026-03-26 | 61 |
| v1.8.0 | 2026-03-29 | 40 |
| v1.9.0 | 2026-03-30 | 97 |
| **v2.0.1** | **2026-04-06 — Show HN day** | **1,996** |
| v2.1.0 | 2026-04-09 | 1,447 |
| v2.3.0 | 2026-04-22 | 3,169 |
| v2.4.0 | 2026-05-21 | 3,706 |
| v2.4.4 | 2026-07-27 | **4,296** |

Totals as of 2026-08-31: **3,145 stars, 19,110 asset downloads** across 21
releases, from a standing start in March 2026.

[STRONG for the numbers. **Important confound:** later releases include
auto-updates by the existing installed base, so the post-launch figures are not
all new users. The pre/post step at a constant metric — 40–97 before, ~2,000 on
the day — is the clean part.]

Two findings, and the second is the one nobody says:

1. **The step change is ~20–50×.** A front-page Show HN took a project from
   dozens of downloads per release to thousands.
2. **The line went up afterwards, not down.** Every release after the launch
   out-performed the launch release. A launch is not a spike that decays; for
   a tool people keep using, it is an ignition. This matters enormously for
   §4, because it means the thing to optimise is not launch-day conversion.

Scale context, all fetched 2026-08-31:

| project | stars | total asset downloads | created |
|---|---|---|---|
| `cjpais/Handy` | 30,750 | 4,341,727 | 2025-02-13 |
| `Beingpax/VoiceInk` | 6,213 | 336,528 | 2024-10-20 |
| `epicenter-so/epicenter` (Whispering) | 4,781 | 200,350 | 2023-03-16 |
| `matthartman/ghost-pepper` | 3,145 | 19,110 | 2026-03-20 |
| **`AmirK-S/TTP`** | **1** | **975** (61 releases) | 2026-02-15 |

TTP's honest baseline is 1 star and 975 lifetime asset downloads, most of which
are plausibly Amir's own rebuilds. Every number in this document should be read
against that. The realistic good outcome of a successful launch is *thousands*,
not tens of thousands, and Handy's 4.3M is a different phenomenon entirely
(free, open-source, Rust, and reusable as a library — see the HN comment from
`ghm2199` about embedding Handy's Rust infra in an Android app).

### 3c. Product Hunt: the folklore layer is thick, so here are the real numbers

**What I measured.** Product Hunt's own daily leaderboard pages, fetched
2026-08-31:

| day | #1 | #5 | #10 | last listed | median of listed | #1's comments |
|---|---|---|---|---|---|---|
| 2026-08-28 | 368 | 179 | 116 | 83 | 135 | 49 |
| 2026-08-21 | 328 | 162 | 126 | 84 | 133 | 34 |
| 2026-07-15 | 461 | 266 | 139 | 116 | 153 | 189 |
| 2026-05-12 | 537 | 183 | 111 | 100 | 123 | 84 |
| 2024-08-21 | 484 | 282 | 179 | 61 | 207 | 98 |

[STRONG for the upvote counts as displayed by Product Hunt on its own
leaderboard, 2026-08-31.]

**Winning Product Hunt outright, in 2026, means persuading about 330–540 people
to click a button, and getting 30–190 comments.** A top-ten finish is
110–180 upvotes. The 2024 sample is slightly higher through the middle of the
board than the 2026 samples, consistent with — but not proof of — the widely
claimed decline.

**What that converts to.** Here the folklore begins. The circulating figures
are "top 3 pulls 5,000–15,000 visitors", "top 10 sends 1,000–3,000 visitors and
30–100 signups". Every instance I could trace resolves to a site selling launch
services (shno.co, causo.ai, getlaunchlist.com, smollaunch.com, upvote.club,
poindeo.com). **[FOLKLORE]** — the numbers may be right; they are published by
parties whose revenue depends on you launching, and none of them shows an
analytics screenshot.

Against that, one **first-hand** report with itemised numbers, posted on
Product Hunt's own forum by the maker
(https://www.producthunt.com/p/rolyai/3-days-after-my-first-product-hunt-launch-traffic-learnings-next-steps):

> "I got place **#13 out of 307 submissions** this day."
> "Product Hunt upvotes: **110**"
> "Product Hunt comments: **13** --> without me/replies: **5**"
> "Total Site visitors: **323**"
> "Total Site signups: **26**"
> "Total paid customers: **0**"
> "Visitors after launch: Day 1: 229 (68 from PH) · Day 2: 64 (20 from PH) ·
> Day 3: 33 (**0** from PH)"

[MODERATE — n=1, self-reported, no analytics screenshot, and a web SaaS rather
than a Mac app. But it is first-hand, itemised, and it is the only
non-self-interested account with numbers I found.]

Note the denominators. **307 submissions in one day.** And the reason the tail
dies: "PH only lists the top 15 products from yesterday". Product Hunt's
memory is 48 hours long.

A #13 finish delivered **323 visitors and zero revenue.** Set against the
folklore's "1,000–3,000 visitors for a top-10 finish", the folklore is off by
roughly an order of magnitude at that rank.

**Also documented:** a paid-upvote industry exists and advertises openly ($199
for 20 upvotes, ~$1,000 for 200, per upvote-service marketing pages). Product
Hunt's countermeasure — weighting votes by account trust rather than counting
them — is described only by Product Hunt and by the services selling around it.
**[FOLKLORE on the mechanism; the existence of the market is verifiable]**

**Verdict for TTP:** Product Hunt is an audience of product-launch spectators,
not of Mac users looking for a dictation tool. Winning it means ~400 clicks.
It is cheap to do and should not be planned around. It is not where TTP's
people are.

### 3d. r/macapps — I could not measure it, and that is a finding

Reddit blocked every access route available to me on 2026-08-31: `.json`
endpoints returned 403 to curl regardless of user-agent, WebFetch is blocked
from reddit.com at the tool level, and the two public Reddit front-ends I tried
returned 403 and a proof-of-work challenge respectively. Only the Atom feed
responded, and it carries no scores.

So I have **no primary data on r/macapps** and I am not going to launder a
number from an SEO blog into this document. What I will record:

- The one analysis of r/macapps I found with a stated sample (496–500 posts) is
  published by upvote.net, a company that sells Reddit marketing. **[FOLKLORE
  by conflict of interest]** — not because it is necessarily wrong, but because
  nobody can check it and the publisher is not disinterested.
- The strong prior from the structure of the place — that r/macapps is a
  self-promotion-tolerant subreddit where the audience is *specifically* people
  who install Mac apps for fun, which makes it a far better audience match than
  Product Hunt — is **[WEAK]**, because I could not verify it this pass.

**What C should do with this:** treat r/macapps as untested, and treat testing
it as cheap. It costs one post.

### 3e. The channel nobody listed, and the one that matters most

The Ghost Pepper and Whispering threads are full of people naming the tools
they already use, unprompted: MacWhisper, Superwhisper, Handy, Wispr Flow,
VoiceInk, Whispernotes, Vibe, hyprwhspr, paseo. There is a
`primaprashant/awesome-voice-typing` list linked in-thread by its author.

**In this category, discovery happens through comparison threads and awesome-
lists, continuously, not through launches.** [MODERATE — pattern observed
across the two largest threads in the category; I did not quantify it.]

That is a durable, unglamorous channel: be *listed*, be *comparable*, and have
a page that answers "how is this different from MacWhisper" in one screen.
Ghost Pepper's rising post-launch curve (§3b) is what being in those lists
looks like.

### 3f. A warning about anti-competitor copy, from the same thread

Ghost Pepper's README included: "it's spicy to offer something for free that
other apps have raised $80M to build." The top-voted response:

> **nidnogg:** "this bothered me a bit… **I'd straight up drop the comparison
> to big AI labs. This isn't rebellious or subversive, it's downstream of a ton
> of already-funded work. Calling it 'spicy' is a bit misframed.**"

[MODERATE — one comment, but it is the *only* negative comment in an otherwise
warm thread, and it is aimed precisely at the underdog framing.]

TTP's positioning has an obvious temptation to punch at Wispr Flow's $81M.
The evidence says the audience punishes it.

---

## 4. The funnel with no trial to expire

### 4a. TTP's funnel has a cliff, and it is not where anyone is looking

From the README, the actual setup sequence:

> "1. Install the app
> 2. **Get a free API key from [Groq Console](https://console.groq.com)**
> 3. Paste the key in TTP's setup screen
> 4. Start talking"

Step 2 requires leaving the app, creating an account on a third-party developer
platform, navigating a console designed for engineers, generating a credential,
copying it, and coming back. On top of that, macOS will demand Microphone,
Accessibility and Input Monitoring permissions.

Compare, from §1: **Superwhisper free and MacWhisper free both work with zero
configuration**, because they bundle local Whisper models. Both treat BYOK as a
*paid* feature for power users.

**So TTP's free tier is more generous than its competitors' paid tiers and
harder to start than their free ones.** That is the funnel problem in one
sentence, and it is a bigger lever than any headline.

[STRONG on the facts — README and both competitors' pricing tables. **WEAK on
the magnitude**, see 4c.]

### 4b. TTP cannot currently see any of this

`src-tauri/src/telemetry/analytics.rs` is a deliberate no-op:

> "Analytics module — kept as a no-op shim after Aptabase was removed… The only
> thing we lose is product analytics nobody was looking at anyway."

Sentry is opt-in and defaults to off, correctly. The consequence is that **TTP
has no instrument that can measure download → key-entered → first-successful-
dictation.** The seven bugs found in the log-harvest window were found because
`ttp-trace.log` exists and Amir read his own; there is no equivalent for the
funnel, and there cannot be one without collecting something.

This is a genuine values conflict, not an oversight, and I am not going to
resolve it by recommending telemetry for a privacy-positioned app. Two options
that do not require it:

- **A GitHub Releases download count is already public and free** (975 today).
  It is the top of the funnel and it costs nothing to watch.
- **The bottom of the funnel is Lemon Squeezy**, which reports purchases.

Everything between the two is dark, and C should write copy knowing that no
A/B test will ever settle an argument here. **Every recommendation in §8 is
therefore a reasoned bet, not a measured result.**

### 4c. What I could not find: hard numbers on macOS permission drop-off

I searched for developer-reported activation-rate data on macOS permission
prompts (Microphone / Accessibility / Input Monitoring) and found only
troubleshooting articles. **No study, no aggregate, no first-hand numbers.**
[ABSENCE OF EVIDENCE — recorded as such.] The claim "permission prompts cost
you users" is universally believed and, as far as I can establish, never
measured in public.

### 4d. What replaces urgency? Nothing does. Plan for 66 days.

The growth-marketing answer is "manufacture urgency". `fun-purchase-research.md`
§4 already rules that out on ethical and mechanism grounds (fake scarcity,
countdowns, guilt). So the honest question is what the real timeline looks like.

**Lally, van Jaarsveld, Potts & Wardle (2010), "How are habits formed:
Modelling habit formation in the real world", *European Journal of Social
Psychology* 40(6), 998–1009.** 96 participants adopted a daily behaviour in a
stable context for 12 weeks. Median time to plateau automaticity: **66 days**,
range **18 to 254 days**, among the ~half of participants who reached
asymptote. https://onlinelibrary.wiley.com/doi/abs/10.1002/ejsp.674 [STRONG for
the study; **WEAK as applied to software adoption** — the study is about eating,
drinking and exercise behaviours, not app usage, and I found no equivalent for
software.]

The transferable structure, not the number: **habit formation is asymptotic and
slow, and it is driven by repetition in a stable context.** TTP's "stable
context" is a keypress bound to Fn. Nothing about a 7-day or 14-day trial
window corresponds to anything real in that curve. A trial deadline is a
*sales* instrument that happens to sit near the habit curve; removing it
removes an artificial deadline, not a real one.

This is also what Ghost Pepper's rising post-launch download curve (§3b) looks
like from the inside.

**Consequence for TTP:** the correct unit of the funnel is not the session or
the week. It is *did this person still have it installed in October*. The ask
(per M1 in `fun-purchase-research.md` — after delivered value, once) should sit
on the far side of that curve, not near it.

### 4e. The activation-metric folklore, named

The single most-cited claim in this space is Facebook's "7 friends in 10 days".
It is worth naming because C will encounter it in every guide.

It traces to **Chamath Palihapitiya's talk at the 2012 Growth Hackers
Conference** — an interested party, retrospectively, about his own success. It
has never been published with data. It is correlational: engaged users had more
friends, which does not establish that adding friends caused engagement. And
the number itself appears to have been chosen for memorability rather than
derived — Andrew Chen's own account of it says other numbers "work well too"
and Tanay Jaipuria's widely-shared thread says plainly that **7 was chosen
because it is memorable**.

**[FOLKLORE]** — and it is the load-bearing folklore under the entire
"find your aha moment" genre. https://andrewchen.com/my-quora-answer-to-how-do-you-find-insights-like-facebooks-7-friends-in-10-days-to-grow-your-product-faster/
· https://medium.com/geckoboard-under-the-hood/how-facebooks-7-friends-in-10-days-got-everyone-confused-about-correlation-and-causation-25da4bb8220e

TTP should not go looking for its number. It does not have the instrumentation
to find one (§4b) and the concept is weaker than advertised.

### 4f. The one activation fact that *is* solid

From the Product Hunt post-mortem in §3c, the maker's own conclusion after
323 visitors produced 26 signups:

> "323 went to the site and only 26 actually signed up. So 297 people just left
> after the signup page. My takeaway from this is that I should implement a
> little demo where you can use the platform **100% for free without signup
> (Ideally it should be directly accessible on the Landingpage)**"

And Superwhisper does exactly this, on the homepage, above the fold:

> "Select an app, press ⌥ + space and start dictating to try it out yourself!"

**Convergent evidence for one recommendation: put the experience before the
credential.** [MODERATE]

---

## 5. The manual: give it away, or keep it behind the purchase?

`docs/companion-manual.md` is 3,600 words, and per `ttp-pro-design.md` it is
"the actual documented Pet Rock mechanism — the manual was the product, the
rock was packaging". The brief calls it "the strongest asset nobody can see".

### 5a. The structural argument, which I think is decisive

Recall §0 of `fun-purchase-research.md`: TTP's central problem is that it has
**no audience surface**. Nobody sees your pill. That document found exactly two
routes to visibility: sound, and something screenshot-able.

There is a third and it has been sitting in `docs/` the whole time. **The
manual is the only component of the Companion that can be experienced by
someone who has not installed TTP, on any device, in a browser, in ten
seconds.** The sound packs require an install and headphones. The face requires
an install. The manual is a URL.

It is also the only component that survives being posted to Hacker News as a
*link*. Nobody upvotes a screenshot of a menu-bar icon. People do upvote a
well-written joke sustained over 3,600 words.

[WEAK as evidence — this is a structural argument, not a measurement. But it is
the same structural argument that §0 of the prior document used to rank the
sound packs first, and that ranking has held up.]

### 5b. The precedent that matches almost exactly

**Cards Against Humanity gives the entire game away, for free, as a PDF, and
sells the boxes anyway.** Verified from their own site, 2026-08-31:

> "…a free download on our website. You can download the PDFs and printing
> instructions right here—all you need is a printer, [scissors] and a
> prehensile appendage."
> "Please note: there's no legal way to use these PDFs to make money, so don't
> ask."

The game is CC BY-NC-SA 4.0. There is a "Steal" link in the site footer's
information nav. This is the company from §2 of `fun-purchase-research.md` that
got 11,000 people to pay $5 for nothing.

[STRONG that they do it; **MODERATE for the inference** — I have no
counterfactual showing the free PDF *increased* box sales, and CAH is a
physical product where the free version is genuinely inferior. That
asymmetry does not exist for a PDF manual.]

### 5c. The counter-argument, which is real

The Pet Rock manual *was* the product. Give away the manual and you have given
away the thing. If the Companion is sounds + face + manual and the manual is
public, the purchase is sounds + face, which is thinner.

And the free-sampling literature does document a **cannibalisation effect**
alongside the acceleration and expansion effects (Bawa & Shoemaker, "The
Effects of Free Sample Promotions on Incremental Brand Sales", *Marketing
Science* 23(3), 2004 — https://pubsonline.informs.org/doi/abs/10.1287/mksc.1030.0052).
Free samples measurably move some people from paying to not paying; the net
effect is positive when the *expansion* effect dominates, which happens when
awareness is low. [MODERATE for the study; **WEAK** transferring
consumer-packaged-goods sampling to a joke PDF.]

Doctorow's "the problem isn't piracy, the problem is obscurity" is the same
argument in a quotable form, and TTP is at 1 GitHub star. Obscurity is not the
risk; it is the current state. **[WEAK — Doctorow's is a well-argued position
with no controlled data behind it, restated for two decades.]**

### 5d. The resolution I would defend

**Publish a substantial, self-contained excerpt as a web page. Ship the
complete, typeset, illustrated object to buyers.**

This is not a fudge; the two artefacts genuinely differ:

- The **public excerpt** is the marketing asset. It costs nothing, it is the
  only shareable thing TTP owns, and its job is to make a stranger laugh in
  under a minute and then wonder what else is in there.
- The **purchased object** is a PDF with a cover, a Certificate of Adoption
  bearing the name the buyer chose for their pill, and the sections that only
  make sense once you own the thing. Per M5 in the prior document, the
  personalised certificate is the endowment mechanism and it *cannot* be
  pre-published, because it does not exist until someone names their pill.

Which sections to publish: the ones that are funny without context. The
letter-Q section is the obvious candidate — it is the most quotable thing in
the document and it is what someone would screenshot.

**Flag for C: this is a bet, not a finding.** The evidence supports "giving
away a strong artefact is survivable" (5b) and "obscurity is the bigger risk at
1 star" (5c). It does not establish the optimal split. My confidence is in the
*direction*, not the *proportion*.

---

## 6. Framing a purchase that buys nothing you need

`ttp-pro-design.md` has already arbitrated the product. This section only adds
what the competitive research changes.

### 6a. The Obsidian sentence, adapted

Obsidian's reason-to-buy is not "get a badge". It is:

> "Catalyst plays a vital role in helping Obsidian remain 100% user-supported,
> free from investor influence that could compromise our values"

**You are buying the absence of a worse business model.** For TTP this is true
in a stronger form, and it is checkable:

- there is no TTP account and no TTP server;
- product analytics were *removed* from the codebase, not merely disabled
  (`analytics.rs` is a no-op shim);
- crash reporting is opt-in and off by default;
- the transcription runs on **the user's own Groq key**, which means the person
  paying for the API calls is the user, and the developer has no relationship
  with the audio at all.

That last point is the reframe. TTP is not local. But it is **disintermediated**
— there is no middleman between the user and the transcription service, because
the middleman would be Amir and he opted out. Superwhisper, Wispr Flow and
MacWhisper's cloud modes all route through *their* infrastructure with *their*
keys and *their* accounts.

**"I never see your audio. I couldn't if I wanted to. There is no server to
ask." is true, unusual, and structurally verifiable — and it is not the same
claim as "local", so it does not collide with §2's commoditisation.**

### 6b. The person is part of the product

MacWhisper: "Built by Jordi Bruin and friends at Good Snooze in the
Netherlands." Bartender: "for more than 10 years". Obsidian: "100%
user-supported". CleanShot: named individuals with company affiliations.

Every successful comparable in the set makes the maker legible in some form.
For a menu-bar app holding Accessibility and Input Monitoring permissions, this
is not sentiment — it is the security posture. `landing/src/assets/amir-photo.png`
already exists in the repo and the last commit on this branch is literally "let
the maintainer see his own product".

[MODERATE — convergent across all six comparables; no controlled evidence that
it converts.]

### 6c. Price sanity check against the market

| | price | what it buys |
|---|---|---|
| MacWhisper Pro | €64 one-time | real features |
| Obsidian Catalyst | $25 one-time | nothing you need |
| Superwhisper Pro | $8.49/mo | real features |
| **TTP Companion** | **€17 one-time** | **nothing you need** |

€17 is below the only directly comparable "buys nothing" tier in the market
($25 Catalyst) and roughly a quarter of the direct competitor's paid tier
(€64). The prior document's pricing reasoning survives contact with 2026
prices. The one update: **Obsidian has collapsed Catalyst from three tiers to
one**, which weakens the "add a higher tier so people self-select upward"
suggestion in `fun-purchase-research.md` §6. [MODERATE]

---

## 7. What I could not find out

Written down because absence of evidence is a finding.

- **r/macapps: nothing.** Reddit blocked curl (403 on `.json` regardless of
  user-agent), WebFetch is blocked from reddit.com at the tool level, and two
  public front-ends returned 403 and an Anubis proof-of-work challenge. I have
  no subscriber count, no post-score distribution, no rules text. The only
  analysis with a stated sample size is published by a company that sells
  Reddit upvotes. **The brief's third channel is untested.**
- **No first-hand macOS-app Product Hunt post-mortem with analytics.** The one
  itemised report I found (§3c) is a web SaaS. Every macOS-specific launch
  guide I found was published by someone selling launch services.
- **No data on macOS permission-prompt drop-off** (§4c). Universally believed,
  never measured in public as far as I can establish.
- **No indie-macOS revenue reports with audited numbers.** The genre is
  self-reported by definition; I declined to build a section on numbers nobody
  can check. The one solid class of primary data available — GitHub release
  download counts (§3b) — I used instead, and it measures installs, not money.
- **Product Hunt upvote counts do not reconcile across sources.** My leaderboard
  scrape shows #1 of the day at 330–540 upvotes. A first-hand write-up of a
  "#1 Product of the Day / Week / Month" launch claims "1,200+ upvotes". These
  may be different quantities (leaderboard-day votes vs. lifetime accumulated
  votes) or different eras. I could not resolve it and am flagging rather than
  picking. Treat my table as *the number Product Hunt renders on its own daily
  leaderboard*, which is what it is.
- **The §2b null result is observational.** A keyword having no correlation
  with points does not prove the keyword has no effect. It proves it is not a
  strong, simple predictor.
- **The Bartender ownership episode is recalled, not re-sourced** (§1). Treat
  as unverified.
- **Nothing here is a measurement of TTP.** TTP has 975 lifetime downloads, 1
  star, and no product analytics (§4b). There is no TTP baseline against which
  any recommendation below can be validated. This is the largest single
  limitation of this document.

---

## 8. For workstream C: positioning recommendations

Ordered by confidence. Each says what evidence backs it and where the bet is.

### C1. Delete the "Nothing leaves your machine" claim. Today. [FINDING — do this first]

`landing/src/i18n/ui.ts:63` and its French counterpart at `:237`
("Rien ne quitte votre machine"). TTP sends audio to `api.groq.com` (§0). This
is not a positioning question; the site is currently untrue in the way most
likely to be caught by the exact audience TTP wants (§2d — a public HN audit of
this precise architecture happened to a competitor within hours).

**Replace it with the honest claim, which is better.** Suggested substance, not
final copy:

> **Your key. Your Groq account. No middleman.**
> TTP has no account, no server, and nothing to sign up for. Your audio goes
> straight from your Mac to Groq using an API key you own. I never see it —
> there is no server for me to see it on. History stays on disk. Crash
> reporting is off unless you turn it on.

Every clause is verifiable in the repository. Evidence: §0, §6a.

### C2. Do not lead with privacy. Lead with what the thing does. [FINDING]

Not one product in the dictation category leads with privacy (§1 table).
Superwhisper — the leader — uses the word "private" **zero times** on its
homepage. 31% of Show HN titles in the category now carry a local/private word
and it does not predict attention (§2a, §2b).

Privacy belongs where MacWhisper and Obsidian put it: **a section further down,
where the abstraction is cashed out into a noun the reader owns.** Obsidian's
move — "Your thoughts are yours… No one else can read them, not even us" —
is the model. TTP's version has to be about the *relationship*, not the
*location*, because the location claim is unavailable.

### C3. The hero should be the output, not the feature. [FINDING — convergent across three competitors]

Wispr Flow's hero is a messy transcript beside a polished one. Superwhisper's
is the same sentence rendered in Formal / Casual / Legal / Chat. Both lead with
**a demonstration of the result**, not a description of the capability (§1).

TTP has "AI polish" — filler removal and grammar repair via a Groq LLM pass —
and currently describes it in a feature grid. It is the single most
demonstrable thing TTP does. Show it. The raw-versus-polished pair is the best
hero available and the category has already proven the format.

### C4. Move the experience in front of the credential. [FINDING, MODERATE]

TTP's funnel has a cliff at "get an API key from console.groq.com" (§4a), and
both direct competitors' *free* tiers require zero configuration because they
bundle local models. TTP's free tier is more generous than their paid tiers and
harder to start than their free ones.

C cannot fix the app's onboarding, but the site can:

- Say the Groq step out loud, above the fold, with the friction named and
  defused — it is free, it takes two minutes, and it is *why* TTP costs
  nothing. Hiding it converts a download into a bounce at the setup screen,
  where nobody can see it happen (§4b).
- Put a playable/watchable demonstration on the page so the value lands before
  the credential is requested. Superwhisper does this above the fold; the one
  first-hand launch post-mortem I found reached the same conclusion after
  watching 297 of 323 visitors leave at a signup wall (§4f).

### C5. Answer "why is this free?" explicitly, in the maker's voice. [FINDING]

Raycast has an FAQ entry for it. Obsidian answers it with "100%
user-supported". Superwhisper answers "is the free tier a trap?" directly. A
free product provokes the question, and every successful comparable answers it
in copy (§1).

TTP's answer is unusually good and currently unstated: *it is free because it
costs me nothing to run — you pay Groq directly, I have no server, and there is
nothing for me to meter.* That sentence simultaneously explains the free tier,
justifies the BYOK friction, and establishes the privacy posture. It is the
highest-leverage paragraph on the site.

### C6. Frame the purchase as buying the absence of a worse business model. [FINDING, adapted]

Obsidian: "free from investor influence that could compromise our values"
(§1, §6a). Not "support me", not "if you enjoy TTP, consider…" — which
`fun-purchase-research.md` §6 explicitly identifies as failure.

For TTP the equivalent is stronger and it is already true: no subscription, no
account, no analytics, no server, no gate — and one optional object that does
nothing. Per M7 in the prior document, **the uselessness is the sincerity
signal**, so the copy must not apologise for it or hint that it is secretly
worth it.

The one-time-purchase coherence is worth stating explicitly: the audience
notices when architecture and pricing agree (§2e). "No server, so no
subscription" is a sentence that lands.

### C7. Make the maker legible. [FINDING, MODERATE]

Every comparable does it (§6b) and for an app holding Accessibility and Input
Monitoring permissions it is part of the security story, not decoration.
`amir-photo.png` is already in the repo.

### C8. Publish an excerpt of the manual as a web page; ship the object to buyers. [BET — direction confident, proportion not]

The manual is the only Companion component experienceable without an install,
on any device, in a browser — which makes it the only answer TTP has to the
no-audience problem from `fun-purchase-research.md` §0 (§5a). Cards Against
Humanity gives away the entire game as a PDF and sells the boxes (§5b).

The purchased object stays distinct because it contains something that cannot
be pre-published: the Certificate of Adoption bearing the name the buyer gave
their pill (M5, endowment).

**Flag for C: the split is a judgement call, not a measurement.** The free-
sampling literature documents genuine cannibalisation alongside expansion
(§5c). What is not in doubt is that TTP's current problem is obscurity — 1
GitHub star, 975 lifetime downloads — not leakage.

### C9. Write a "how is this different from MacWhisper / Superwhisper" page. [BET, MODERATE]

Discovery in this category happens through comparison threads and awesome-lists,
continuously, rather than through launches (§3e), and Ghost Pepper's *rising*
post-launch download curve is what that looks like from the inside (§3b).

Two constraints from the evidence:

- Be accurate about the trade-off. TTP is faster and free and uncapped; it also
  requires a Groq key and does not run offline. Say so. The audience checks
  (§2d).
- **Do not punch at the funded competitors.** The one negative comment in an
  otherwise warm 467-point thread was aimed exactly at underdog framing (§3f).

### C10. Calibrate the launch expectations in writing, so nobody is disappointed. [FINDING]

Give Amir the real numbers before he launches, because the folklore will
otherwise set the expectation:

- ~78% of Show HN posts in this exact category score **five points or fewer**;
  the median is **two** (§3a).
- Winning Product Hunt outright in 2026 means **330–540 upvotes** (§3c).
- A **#13-of-307** Product Hunt finish delivered **323 visitors and zero
  revenue** (§3c).
- A **front-page 467-point Show HN** took Ghost Pepper from ~60 downloads per
  release to ~2,000 on the day — and to **4,296 per release four months
  later** (§3b).

**The last number is the important one.** For a tool people keep, the launch is
ignition, not the event. The thing worth optimising is not launch-day
conversion; it is whether the product is still installed in October. That is
also the only funnel shape compatible with having no trial to expire (§4d).

---

## Source index

**Primary — competitor pages, fetched 2026-08-31 as raw HTML**
- Superwhisper — https://superwhisper.com/
- MacWhisper — https://www.macwhisper.com/ · https://goodsnooze.gumroad.com/l/macwhisper
- Wispr Flow — https://wisprflow.ai/
- Obsidian — https://obsidian.md/ · https://obsidian.md/pricing · https://obsidian.md/help/catalyst
- Raycast — https://www.raycast.com/ · https://www.raycast.com/pricing
- CleanShot X — https://cleanshot.com/
- Bartender — https://www.macbartender.com/
- Cards Against Humanity (free PDF / CC licence) — https://www.cardsagainsthumanity.com/

**Primary — datasets I generated**
- Hacker News Algolia API, `search_by_date`, `tags=story`, queries `"Show HN" +
  {dictation, speech-to-text, transcription, voice typing, whisper mac}`,
  deduplicated (n=1,062) — https://hn.algolia.com/api/
- Product Hunt daily leaderboards, 2026-08-28, 2026-08-21, 2026-07-15,
  2026-05-12, 2024-08-21 — https://www.producthunt.com/leaderboard/daily/
- GitHub REST API release asset download counts for `cjpais/Handy`,
  `Beingpax/VoiceInk`, `epicenter-so/epicenter`, `matthartman/ghost-pepper`,
  `AmirK-S/TTP` — https://api.github.com/

**Primary — first-hand accounts**
- Whispering Show HN, 591 pts, incl. the "black boxes" framing and the Aachen
  audit — https://news.ycombinator.com/item?id=44942731
- Ghost Pepper Show HN, 467 pts, incl. the nidnogg critique of underdog framing
  — https://news.ycombinator.com/item?id=47666024
- Product Hunt launch post-mortem with itemised traffic (#13/307, 110 upvotes,
  323 visitors, 26 signups, 0 customers) —
  https://www.producthunt.com/p/rolyai/3-days-after-my-first-product-hunt-launch-traffic-learnings-next-steps

**Academic**
- Lally, van Jaarsveld, Potts & Wardle, *How are habits formed: Modelling habit
  formation in the real world*, Eur. J. Soc. Psych. 40(6), 2010 —
  https://onlinelibrary.wiley.com/doi/abs/10.1002/ejsp.674
- Bawa & Shoemaker, *The Effects of Free Sample Promotions on Incremental Brand
  Sales*, Marketing Science 23(3), 2004 —
  https://pubsonline.informs.org/doi/abs/10.1287/mksc.1030.0052

**Folklore, named as such**
- "7 friends in 10 days" — Palihapitiya, Growth Hackers Conference 2012;
  correlational, unpublished, number chosen for memorability —
  https://andrewchen.com/my-quora-answer-to-how-do-you-find-insights-like-facebooks-7-friends-in-10-days-to-grow-your-product-faster/
  · https://medium.com/geckoboard-under-the-hood/how-facebooks-7-friends-in-10-days-got-everyone-confused-about-correlation-and-causation-25da4bb8220e
- Product Hunt "traffic by rank" figures — every traceable instance published by
  a vendor of launch services (shno.co, causo.ai, getlaunchlist.com,
  smollaunch.com, poindeo.com)
- r/macapps post analyses — published by upvote.net, which sells Reddit upvotes

**Repository evidence**
- `src-tauri/src/transcription/whisper.rs:28`, `polish.rs:27`,
  `dictionary/classify.rs:11` — Groq endpoints
- `src-tauri/src/telemetry/analytics.rs` — analytics removed, no-op shim
- `src-tauri/src/telemetry/consent.rs` — Sentry opt-in, default off
- `landing/src/i18n/ui.ts:63`, `:237` — the claim to remove
- `README.md` — the honest version of the same claim
