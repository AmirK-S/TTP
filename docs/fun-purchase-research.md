# The Fun Purchase: research notes for TTP's optional support tier

**Question being answered:** TTP goes fully free, no gating. We add one optional ~€17–20 purchase that unlocks something creative and delightful. What does the evidence say about why anyone would buy that, what makes it feel like a gift rather than a shakedown, and what specifically should we build?

**How to read this:** every claim is tagged with how well-evidenced it is.

- **[STRONG]** — replicated experiments, field data, or primary numbers
- **[MODERATE]** — single good study, or industry data with a clear source
- **[WEAK]** — plausible theory, thin empirical support in *this* domain
- **[FOLKLORE]** — repeated everywhere, traceable to a single self-interested source

---

## 0. The one structural fact that shapes everything

Almost all the canonical "people pay for useless things" cases run on **social visibility**. Skins, NFT profile pictures, Discord badges, amiibo on a shelf — the buyer is showing something to somebody. Ariely, Bracha & Meier (2009) demonstrated experimentally that image motivation is *dependency on visibility*: prosocial behaviour rises when observed and falls when private, and monetary incentives crowd out image motivation specifically in the public condition. [STRONG] — https://www.aeaweb.org/articles?id=10.1257/aer.99.1.544 · PDF: https://www.bostonfed.org/-/media/Documents/Workingpapers/PDF/wp0709.pdf

**A macOS menu-bar dictation tool has essentially zero social surface.** Nobody sees your tray icon. Nobody sees your recording pill. This kills the single strongest mechanism behind skins, Nitro, and PFPs.

Two consequences, and they should drive the whole design:

1. **Lean on the mechanisms that work in private** — reciprocity, warm glow, *self*-signaling, and anthropomorphic attachment. These do not need an audience.
2. **Where you want image motivation, you must manufacture the audience.** Three cheap surfaces exist: a public supporters wall/credits, something the user can *screenshot and post*, and — uniquely for a dictation app — **sound, which the people in the room actually hear**.

Everything ranked in §5 is ranked partly on how well it copes with the no-audience problem.

---

## 1. The mechanisms that actually explain the behaviour

Ordered by how much weight I'd put on each *for this specific product*.

### M1. Reciprocity / gift exchange — give first, then ask [STRONG]

Falk's field experiment (Econometrica, 2007) mailed ~10,000 charity solicitation letters: no gift, small gift, or large gift enclosed. Donation *frequency* rose **17% with a small gift and 75% with a large gift**, randomly assigned. https://onlinelibrary.wiley.com/doi/abs/10.1111/j.1468-0262.2007.00800.x · https://www.iza.org/publications/dp/1148/charitable-giving-as-a-gift-exchange-evidence-from-a-field-experiment

This is the mechanism TTP is *already sitting on* and hasn't cashed. A free, fully-featured, genuinely good app that saves someone twenty minutes a day **is the large gift**. The ask is the reciprocation. This is why the "remove all gating" decision is not a revenue sacrifice — it is the precondition for the mechanism to fire at all. A crippled free tier is not a gift; it's a demo, and demos generate no reciprocal obligation.

**Design implication:** the ask must come *after* substantial delivered value, and must visibly reference it. Not on day one. Not in onboarding.

### M2. Warm glow / impure altruism [STRONG]

Andreoni (1989, 1990) formalised that people get private utility from *the act of giving itself*, separate from any utility from the outcome. https://academic.oup.com/ej/article-abstract/100/401/464/5190270 · overview: https://en.wikipedia.org/wiki/Warm-glow_giving · experimental test (Crumpler & Grossman): https://econweb.ucsd.edu/~jandreon/PhilanthropyAndFundraising/Volume%201/23%20Crumpler%20Grossman%202007.pdf

The practical consequence is counter-intuitive and important: **the buyer does not need the thing they receive to be worth €17.** They need the *act* to feel good. This is precisely why a deliberately useless object works better than a mediocre feature — a mediocre feature invites value comparison and loses; a googly-eyed companion invites no comparison at all.

### M3. Self-signaling — and the trap in it [STRONG]

Gneezy, Gneezy, Riener & Nelson, *Pay-what-you-want, identity, and self-signaling in markets*, PNAS 2012. https://www.pnas.org/doi/10.1073/pnas.1120893109

The headline finding is a warning: under pay-what-you-want, **purchase rates can go *down***, because paying a small amount forces the buyer to confront an unflattering self-image ("I'm the kind of person who took this for nothing"). Rather than pay a shameful price, people walk away entirely.

Paired with the same group's Science 2010 field experiment (113,047 amusement-park customers): PWYW alone was weakly profitable; **PWYW + "half goes to charity" was substantially more profitable**, because it let customers express a social self through the purchase. https://www.science.org/doi/10.1126/science.1186744 · https://pubmed.ncbi.nlm.nih.gov/20647467/

**Design implications, and they are specific:**
- **Do not build a slider.** A free-form "pay what you want" field is the exact structure that triggers the shame-avoidance walkaway.
- **Do build a named tier at a fixed price.** A named identity ("Supporter") is something you can *be*; an amount is something you can be judged on.
- The price being *above* trivial is a feature. €17 says "I actually backed this person," which is a self-image worth buying. €3 says "I threw a coin."

### M4. Anthropomorphism and the Tamagotchi effect [STRONG for the mechanism, MODERATE for monetisation]

Epley, Waytz & Cacioppo, *On Seeing Human: A Three-Factor Theory of Anthropomorphism*, Psychological Review 2007. People attribute human qualities to non-human agents when (a) human-centric knowledge is accessible, (b) they're motivated to predict the agent's behaviour, and (c) they lack social connection. https://www.scholars.northwestern.edu/en/publications/on-seeing-human-a-three-factor-theory-of-anthropomorphism · https://philpapers.org/rec/EPLOSH

Factor (b) is doing enormous work for TTP. **A user staring at a recording pill, waiting to find out whether it heard them correctly, is in a maximally anthropomorphism-prone state**: watching an opaque agent, motivated to predict its behaviour. This is not a stretch — it's the textbook condition.

Tamagotchi (Bandai, 1996) is the proof-of-concept: an object with no utility whatsoever generated real grief responses on "death," to the point of school bans. The effect is named after it. https://en.wikipedia.org/wiki/Tamagotchi_effect · https://wellcomecollection.org/stories/WsT4Ex8AAHruGfWb

**Caveat I want to be honest about:** Tamagotchi worked partly through *care burden* — it demanded things from you. A productivity tool must never demand anything. So we can borrow the attachment mechanism (a character with moods, reactions, a name) but **not** the obligation mechanism. Any TTP creature must be pure upside: it reacts, it never needs.

### M5. Endowment + IKEA effect — ownership and naming inflate value [STRONG]

Kahneman, Knetsch & Thaler (1990): randomly endowed mug owners demanded ~$5.25 to sell what non-owners would only pay ~$2.25–2.75 for — roughly 2× — within minutes of arbitrary assignment. https://web.mit.edu/curhan/www/docs/Articles/15341_Readings/Behavioral_Decision_Theory/Kahneman_et_al_1990_Experimental_tests.pdf

Norton, Mochon & Ariely, *The IKEA Effect: When Labor Leads to Love* (J. Consumer Psychology, 2012): self-assembly raises valuation, and participants expected others to share their inflated view. Critically, **the effect only holds when the labour is successfully completed** — destroyed or abandoned builds show no effect. https://www.hbs.edu/faculty/Pages/item.aspx?num=41121 · https://dash.harvard.edu/bitstreams/7312037d-2473-6bd4-e053-0100007fdf3b/download

**Design implication:** whatever is bought should require a small, *guaranteed-to-succeed* act of personalisation — name it, pick its colours, choose its voice. Sixty seconds of customisation converts a purchase into a possession. The "guaranteed to succeed" part is not optional; a fiddly customiser that people abandon halfway actively destroys the effect.

### M6. Collecting and set completion [MODERATE]

Set completion is an independently modelled driver of collecting, separable from financial motive. https://www.sciencedirect.com/science/article/abs/pii/S0167487007000682 · overview https://en.wikipedia.org/wiki/Psychology_of_collecting

Relevant, but with a sharp caveat: **collecting mechanics are the fastest route from "delightful" to "tacky."** The moment a set is incomplete-by-design and completion costs money, you have built a compulsion loop, which is the opposite of the brief. Safe version: one purchase unlocks *the entire set at once*, and the delight is discovery, not acquisition. Unsafe version: drip-fed packs.

### M7. Conspicuous non-utility as a sincerity signal [WEAK in this domain, but theoretically solid]

Costly-signaling / handicap-principle logic: a signal is credible precisely because it is expensive and useless — inferior signallers can't afford the waste. https://en.wikipedia.org/wiki/Handicap_principle · https://www.sciencedirect.com/science/article/abs/pii/S1090513810001455

Applied here, the inference runs in an unusual direction. When a maker offers you a *useful* paid extra, you can't tell whether they care about you or about your money — the two hypotheses predict the same behaviour. When a maker offers you something with **zero instrumental value**, the money-motivated hypothesis gets much weaker. The uselessness is what makes the sincerity legible.

I'm flagging this **[WEAK]** because I found no direct experimental test of "deliberately useless paid extras signal seller sincerity." It's a coherent extension of well-established theory, not a measured result. But it's the best available account of why the Pet Rock and the Cards Against Humanity hole land as *charming* rather than *cynical*, and it argues strongly for: **do not make the fun purchase secretly useful.** Resist every temptation to sneak a real feature in "so it feels worth it." That instinct is wrong and it dissolves the signal.

### M8. Scarcity [MODERATE mechanism, HIGH ethical risk]

Worchel, Lee & Adewole (1975), the cookie-jar study: identical cookies rated more desirable when only two remained versus ten, and *most* desirable when supply visibly shrank due to demand. https://www.semanticscholar.org/paper/Effects-of-Supply-and-Demand-on-Ratings-of-Object-Worchel-Lee/e80a3b8c8b27fa69cc6f4fb4c4e497f705f07a89

The mechanism is real. **The only honest form is scarcity that is actually true** — e.g. a variant that genuinely only exists for people who bought before version 4.0, and genuinely never returns. Manufactured countdowns in a solo-dev tip mechanism will be read instantly as manipulation and will cost more trust than they earn revenue.

### M9. Legitimizing paltry contributions — a real effect that I'd *not* use here [STRONG effect, poor fit]

Cialdini & Schroeder (1976): adding "even a penny will help" to a door-to-door ask nearly doubled compliance **without reducing average donation size**. https://www.communicationcache.com/uploads/1/0/8/8/10887248/increasing_compliance_by_legitimizing_paltry_contributions-_when_even_a_penny_helps.pdf · meta-analysis: https://www.communicationcache.com/uploads/1/0/8/8/10887248/the_legitimization_of_paltry_favors_effect-_a_review_and_meta-analysis...pdf

Noted for completeness because it's the standard tip-jar playbook — and because it **directly collides with M3**. LPF maximises the *number* of givers; the self-signaling result says a low anchor can make people not buy at all. Given the goal here is one €17–20 object rather than maximum donor count, I'd resolve the tension in favour of M3: one confident price, no "even €1 helps."

### M10. Motivation crowding — the reason not to over-engineer the ask [STRONG]

Gneezy & Rustichini's Haifa day-care study: introducing a fine for late pickup **increased** lateness from ~7.6 to ~12.3 per week, and removing the fine did not restore the old norm. Pricing a behaviour reframed a moral obligation as a purchasable permission. https://www.sciencedirect.com/science/article/abs/pii/S0144818820301447 · https://rady.ucsd.edu/_files/faculty-research/uri-gneezy/pay-enough.pdf

The transferable lesson: **once you attach an explicit price to "supporting the maker," you may destroy the diffuse goodwill that would otherwise have produced support.** And the damage persists after you remove the price. Practically: the fun purchase should be framed as *joining something*, not as *settling a debt*. Never "you've used TTP 500 times, consider paying." That's an invoice, and it converts a gift relationship into a transactional one permanently.

---

## 2. Case studies: what's documented vs. what's repeated

### The Pet Rock — mostly folklore, but the surviving kernel is the important part

**Documented [MODERATE→STRONG]:**
- Gary Dahl, an advertising copywriter, debuted the product at a **San Francisco gift show in August 1975**. Retail **$3.95**. Neiman Marcus ordered; Bloomingdale's followed; *Newsweek* covered it. Prices were being cut by **February 1976**. Total fad duration: roughly six months. https://en.wikipedia.org/wiki/Pet_Rock · https://www.mentalfloss.com/article/595180/pet-rock-history
- Rocks were ordinary stones (sourced via a sand-and-gravel supplier, said to be from Rosarito, Mexico), packed in a **cardboard carrier with air holes and straw bedding**.
- The manual — *The Care and Training of Your Pet Rock*, ~32 pages — is fully readable today. This is the best-preserved primary artefact: https://archive.org/details/pet_rock_manual_original · plain text: https://archive.org/stream/pet_rock_manual_original/pet_rock_manual_original_djvu.txt

**Actual quotes from the manual** (worth reading in full before writing any copy for TTP):

> "Your new rock is a very sensitive pet and may be slightly traumatized from all the handling and shipping required in bringing the two of you together."

> "Few pets are more anxious to please their masters than are PET ROCKS. It is surprisingly easy to teach your rock cute tricks that will entertain you and your friends for hours."

> **Shake hands:** "Don't be ridiculous. You can't teach a rock to shake hands."

> "Remember; if you take care of your PET ROCK, your PET ROCK will take care of you."

**Folklore [FOLKLORE]:**
- Sales figures are mutually contradictory across sources: "1.3 million," "nearly 1.5 million," and "5 million within six months" all circulate. "100,000 a day at peak" and "$6 million gross" trace to press coverage and to Dahl himself, not to audited records.
- "Cost less than a penny, 95 cents profit each" is the line every marketing blog repeats. It's a nice round story from an advertising man about his own advertising triumph. Treat it as an anecdote, not accounting.
- **The bar-conversation origin story is Dahl's own, unverifiable, and is exactly the origin story a professional copywriter would construct.** It may well be true. It is not evidence.

**The part that actually matters, and that everyone gets backwards:** the product was **not** the rock. The product was **the manual**. The rock was free; the joke was what cost $3.95, and the joke was executed with total commitment — deadpan register, real page count, real production values, no winking. The transferable lesson for TTP is not "sell something worthless." It is **"the writing is the product, and it has to be genuinely, laboriously good."**

**The strongest evidence against over-learning from the Pet Rock:** Dahl could not repeat it. The Bicentennial rock, Pet Rock shampoo, the Sand Breeding Kit and Canned Earthquake all failed. Whatever fired in late 1975 was substantially a cultural moment, not a reproducible formula. Anyone selling you "the Pet Rock playbook" is selling survivorship bias.

### Cards Against Humanity — the purest test of "pay for nothing" [STRONG, it's public record]

A four-year sequence of Black Friday stunts, escalating:
- **2013:** raised the price $5 for the day (an "anti-sale").
- **2014:** replaced the game with **boxes of actual bull faeces — ~30,000 sold**.
- **2015:** "Give Cards Against Humanity $5." Nothing offered in return. **11,000+ people paid.**
- **2016:** the Holiday Hole — a livestreamed excavation that continued as long as donations came in. **$100,573 over 48+ hours**, for a hole, which was then filled in.

https://www.npr.org/sections/thetwo-way/2016/11/27/503502142/people-donated-nearly-100-000-to-dig-a-big-pointless-hole-in-the-ground · https://www.huffpost.com/entry/cards-against-humanity-holiday-hole_n_583bcee9e4b000af95eeb28d

The 2015 event is the cleanest data point in this entire document: **11,000 people paid $5 for explicitly nothing.** Not a cosmetic, not a badge. Nothing.

Two things made it work, and both are transferable:
1. **Total commitment to the bit.** Asked "Why aren't you giving this to charity?", the FAQ answer was: *"A deeper hole."* No hedging, no apology, no "we know this is silly, but…". The refusal to break character *is* the sincerity signal (M7).
2. **It was counter-programming.** It worked *because* it was staged against Black Friday, i.e. against the most cynical commercial moment of the year. Buying in was a way of siding with a set of values.

**Honest caveat:** CAH had a large pre-existing audience with a strong shared identity. TTP does not, yet. The mechanism is real; the magnitude does not transfer.

### itch.io pay-what-you-want — the best public numbers on voluntary overpayment [MODERATE, first-party]

From itch.io's own published store data: **~30% of all money spent on the platform is paid above the minimum**, averaging about **$1.50 extra** per purchase. For content whose minimum is **zero**, the average voluntary payment is **$3.68**, with most payers between $1 and $5. The largest single voluntary payment recorded for a free game was **$500**. https://itch.io/blog/2/running-an-indie-game-store-2015 · secondary summary: https://itch.io/t/1081038/money-for-the-honey-a-case-study-of-pay-what-you-want-pricing-on-itchio

**Read this as a warning about price level.** The natural, undirected voluntary payment for a free digital thing clusters at **$1–5**. €17–20 is roughly 5× that. It is not reachable by simply putting up a "pay what you want" box — it is reachable only by giving the purchase an *identity and an object* (M3, M5), which is exactly why the Obsidian model works and a slider doesn't.

### Radiohead, *In Rainbows* [MODERATE, comScore]

comScore's 2007 data: **62% of downloaders paid $0**; the 38% who paid averaged about **$6**; blended average across all downloads was **$2.26**. https://www.comscore.com/Insights/Press-Releases/2007/11/Radiohead-Downloads · https://www.billboard.com/music/music-news/study-most-paid-nothing-for-radiohead-album-1047408/

Same lesson, different domain: **unstructured voluntary payment converges on a few dollars**, even with one of the most beloved brands on earth and enormous press coverage.

### Steam skins, Discord Nitro, NFT PFPs — powerful, and mostly not available to you

- **Cosmetics as identity:** research consistently finds skins are bought for self-expression, social visibility, and in-group/out-group navigation. https://bristoluniversitypressdigital.com/view/journals/consoc/5/2/article-p285.xml · https://www.sciencedirect.com/science/article/pii/S2451958825001150 · https://guof.people.clemson.edu/papers/skin.pdf
- **Discord Nitro:** the badge and animated avatar are widely cited as primary purchase drivers, over the practical upload-limit benefits. https://discord.com/nitro
- **NFT PFPs:** studied explicitly as conspicuous consumption plus tribal affiliation. https://arxiv.org/pdf/2206.06443 · https://arxiv.org/pdf/2503.17457
- **amiibo:** ~$1.7bn in a single six-month reporting period, largely bought as objects to look at rather than for their thin in-game function. https://gamerant.com/nintendo-amiibo-sales-splatoon/ · https://www.techradar.com/features/amiibo-figures-are-a-waste-of-money-but-i-cant-stop-collecting-them

**All four run on an audience** (see §0). The NFT case adds a specific warning: value that rests *entirely* on other people's regard evaporates when the crowd leaves. Anything TTP sells should be enjoyable **alone, on your own machine, forever** — the buyer's private pleasure has to be the whole justification, with any social layer as pure bonus.

### Bongo Cat (Steam) — the closest living relative to what TTP should build [MODERATE]

A free desktop companion that reacts to your keyboard and mouse. Cosmetic DLC packs are sold with the framing **"Support Bongo Cat with [themed] item sets."** Free cosmetics are also earnable through use; paying is explicitly optional and explicitly framed as support. https://store.steampowered.com/app/3419430/Bongo_Cat/ · https://store.steampowered.com/dlc/3419430/Bongo_Cat/

Note the exact structure: **a reactive on-screen creature driven by input you were already producing, with optional cosmetic packs framed as support rather than as content.** TTP's recording pill is already a reactive on-screen creature driven by input you were already producing. This is a very short walk.

### Desktop Goose, and desktop pets generally [WEAK on numbers]

Desktop Goose (samperson) is pay-what-you-want on itch, wildly popular, thousands of comments. https://samperson.itch.io/desktop-goose — I could not find published revenue figures, so I'm not going to invent any. Directionally it confirms that a deliberately unhelpful desktop creature is something people pay for voluntarily.

Also of note: **Pixel Pals** ships a menu-bar/widget pet roster with individually *named* characters — Rupert the dog, Hugo the cat, Mochi the axolotl, and, delightfully, **Dwayne the Pet Rock**. https://apps.apple.com/us/app/pixel-pals-widget-pet-game/id6443919232 — Names matter (M5). Named characters are possessions; unnamed ones are graphics.

---

## 3. What actually works in indie software specifically

### Obsidian's Catalyst licence — the single closest analogue, and the model to copy [STRONG, it's their published pricing]

Obsidian is free and fully featured. Catalyst is a **one-time** purchase in three tiers: **Insider $25 / Supporter $50 / VIP $100**. It unlocks **zero app functionality**. What you get: early access to insider builds, a **badge on the Discord and forum**, and (at higher tiers) an exclusive lounge channel. Obsidian's own framing: *"a one-time purchase that gives you early access to beta versions and helps support continued development of Obsidian."* https://obsidian.md/help/catalyst

Why this matters enormously for TTP:
- It proves **$25 one-time works** for a free tool — 5× the itch.io voluntary-payment norm — because the purchase has an **identity** and a **name**, not a slider.
- Every deliverable is **near-zero marginal cost** to a solo dev: a beta channel and a Discord role.
- The framing is honest about being support, while still handing over something real.
- Three tiers let people self-select *upward*. RevenueCat's tip-jar guidance echoes this: developers report users choose higher amounts more often once given several options. https://www.revenuecat.com/blog/engineering/building-a-tip-jar-feature-with-revenuecat

### Vim as charityware — the long game [MODERATE]

Vim shipped `:help uganda` from v4.0 (1995): free software, with a request to donate to children in Uganda. Reported cumulative giving is on the order of **€500,000+ by 2016**, from roughly $2,000/year in the late 1990s. https://vimhelp.org/uganda.txt.html · https://www.moolenaar.net/Charityware.html · https://en.wikipedia.org/wiki/ICCF_Holland

Two lessons: (1) redirecting the ask toward a cause outside yourself removes the awkwardness entirely and matches the Gneezy 2010 "shared social responsibility" result; (2) the early numbers were *tiny*. This compounds over a decade; it does not pay rent in year one.

### Apollo's tip jar — the naming lesson [MODERATE]

Christian Selig's Apollo used **named tiers rather than raw amounts**: Nice Tip $1, Kind Tip $3, Generous Tip $5, Amazing Tip $10, **Godzilla Tip $20**. https://en.wikipedia.org/wiki/Apollo_(app) — Note that the top tier lands almost exactly where TTP wants to be (~€17–20), and note that it is **named after a monster**. The name is doing the self-signaling work (M3): "Godzilla Tip" is an identity you can enjoy occupying; "€20.00" is not.

### Shottr — the instructive reversal [MODERATE]

Shottr was free for a long time with just a "buy me a coffee" option, then moved to paid: **$12 basic**, and **$30 for a tier described in terms of bragging rights, experimental features, and better support**. https://shottr.cc/purchase.html · HN discussion: https://news.ycombinator.com/item?id=31773863

The $30 tier is the interesting bit — a *higher-priced supporter tier whose headline benefit is bragging rights.* Independent convergence with Obsidian on the same structure.

### Sindre Sorhus — the anti-cheap argument [MODERATE, first-hand]

> "I have a high price tier on my macOS apps, not because I'm greedy or earn a lot from them, but rather because life is too short to deal with idiots. For example, free & cheap apps require much more support. I'd rather sell fewer copies in favor of getting higher quality customers"

https://x.com/sindresorhus/status/1119690296936189952 · his apps: https://sindresorhus.com/apps

A practical solo-dev argument for €17–20 over €3: **support burden does not scale with price, so a higher price yields better hours-per-euro.** For an optional tier where nothing is gated, this cost is even lower — non-buyers still get full support, so you're not selecting your userbase, only your supporters.

### Practical constraint check for TTP specifically

- TTP distributes via **GitHub Releases + install.sh**, not the Mac App Store. **No 30% cut**, no Apple rules about what an IAP may be. Apple's policy treats voluntary tipping as an IAP subject to commission (https://techcrunch.com/2017/06/09/in-app-tips/), which is a headache TTP simply does not have.
- **Lemon Squeezy is already wired in** (`src-tauri/src/licensing/`, and the Buy link in the README). Merchant of record handles EU VAT. The existing licensing module can be repurposed from a gate into a cosmetic unlock with modest work.
- Everything below is **local-only** and requires no server, which matters for a privacy-positioned dictation tool.

---

## 4. Beloved vs. tacky: the dividing line

Across every case above, delightful asks share four properties and tacky ones violate at least one.

**The four properties of an ask that lands:**

1. **The gift is complete and unconditional first.** Nothing withheld. Nothing degraded. No "free version" asterisk. (M1 — a partial gift generates no reciprocity.)
2. **The ask is made exactly once, cheerfully, and then never again.** One mention, dismissible forever, and it *means* forever.
3. **What you get back is real, but transparently non-essential.** You receive an object. The object does nothing. Both parties know this and both find it funny. (M7)
4. **The tone is confident, not needy.** Cards Against Humanity never once apologised for the hole. Obsidian doesn't apologise for Catalyst. Neediness reads as manipulation *even when it's sincere*, because the user cannot distinguish the two from the inside.

**The anti-patterns, with evidence:**

| Anti-pattern | Why it fails | Evidence |
|---|---|---|
| **Nagging / repeat prompts** | Nagware ("guiltware") drives short-term clicks then durable resentment; it's classified as a dark pattern. | https://www.computerhope.com/jargon/n/nagware.htm · https://www.eleken.co/blog-posts/dark-patterns-examples |
| **Confirmshaming** ("No thanks, I don't support indie devs") | Guilt-framed decline options generate resentment and brand damage. | https://learningloop.io/glossary/dark-patterns |
| **Guilt appeals generally** | Strong guilt appeals trigger psychological reactance — anger, irritation, and negative attitudes toward the asker. Effect is worst when the product is high-hedonic (which "fun purchase" is by definition) and when the ask is large. | https://www.sciencedirect.com/science/article/pii/S0001691826001277 · https://www.researchgate.net/publication/269553739 |
| **Fake scarcity / countdown timers** | Scarcity works only when believed; a solo dev's fake deadline is transparent and costs trust permanently. | Mechanism: https://www.semanticscholar.org/paper/e80a3b8c8b27fa69cc6f4fb4c4e497f705f07a89 |
| **A payment slider** | Forces an unflattering self-assessment; some users walk rather than pay a shameful amount. | Gneezy et al. PNAS 2012, https://www.pnas.org/doi/10.1073/pnas.1120893109 |
| **Usage-count invoicing** ("you've dictated 10,000 words — consider supporting") | Converts a gift relationship into a debt relationship, and the conversion is *not reversible*. | Gneezy & Rustichini day-care fine, https://rady.ucsd.edu/_files/faculty-research/uri-gneezy/pay-enough.pdf |
| **Drip-fed collectible packs** | Turns set-completion motivation into a compulsion loop. Delight → dark pattern. | https://en.wikipedia.org/wiki/Psychology_of_collecting |
| **Sneaking real utility into the "fun" purchase** | Destroys the sincerity signal; reintroduces value comparison, which €17 of cosmetics loses. | M7, https://en.wikipedia.org/wiki/Handicap_principle |

**One more, specific to TTP:** anything that adds friction, latency, or unpredictability to the dictation path is disqualified regardless of charm. The pill is on screen during a task the user is trying to finish. A creature that delays paste by 200ms is not fun, it's a bug.

---

## 5. Ranked ideas for TTP

Ranking criteria: (a) how many strong mechanisms it fires, (b) whether it survives the no-audience problem from §0, (c) build cost for a solo dev, (d) risk of being annoying in a productivity tool.

TTP's existing surfaces, for reference: the dark pill with a 14-bar voice-reactive waveform and idle/recording/transcribing/error states plus a shake-on-error animation (`src/windows/FloatingBar.tsx`); the tray icon with composited status dots (`src-tauri/src/tray.rs`); embedded `start.wav`/`stop.wav` (`src-tauri/src/sounds.rs`); and HMAC-signed daily stats of transcriptions/words/chars (`src-tauri/src/usage/store.rs`).

---

### #1 — The Sound Packs ("Voices for TTP")

**What:** the €17–20 purchase unlocks a set of lovingly over-produced start/stop sound sets. A 1970s walkie-talkie squelch. A tiny polite cough. An 8-bit coin. A submarine ping. A librarian's *shhh*. A single dignified typewriter ding. Each with a name and a one-line description written in Pet Rock register.

**Mechanisms:** M7 conspicuous non-utility (nothing is more useless than a nicer beep) · M6 set completion, safely — one purchase unlocks all of them · M2 warm glow · **and it is the one and only TTP cosmetic that solves the no-audience problem**: sounds are heard by everyone in the room. Your colleague asks what that was. That is free distribution and free image motivation.

**Why #1:** highest ratio of delight to engineering. `sounds.rs` already loads embedded WAVs and is trivially extensible to a set. No new UI surface, no new render path, zero risk to the dictation hot path, zero risk of visual annoyance. And it's the only idea here where the *social* mechanism actually works.

**Risk:** low. Sounds are already toggleable, so anyone who finds them irritating already has the off switch.

---

### #2 — The pill gets a face (and a name)

**What:** the recording pill becomes a character. It blinks when idle. Its waveform bars become an expression. It squints when audio is too quiet, looks pleased on a clean transcription, and the existing error-shake becomes a full-body flinch. The purchase unlocks the full cast plus **the right to name yours**, stored locally.

**Mechanisms:** M4 anthropomorphism — and the user's state while watching the pill (opaque agent, high motivation to predict its behaviour) is textbook Epley factor (b) · M5 endowment + IKEA, via naming and choosing, which is a sixty-second guaranteed-success customisation · M6 collecting, done safely · M2 warm glow.

**Why #2 not #1:** highest emotional ceiling of anything in this list, but the highest annoyance risk too. A face in your peripheral vision during focused work can go from charming to intolerable in a week. Mitigations are mandatory: default subtle, fully switchable, honour the existing `prefers-reduced-motion` handling, and **never** let it demand anything (the Tamagotchi *care burden* is the part we're deliberately not copying).

**Build cost:** moderate. It's SVG/CSS in an existing component, but getting the character to feel alive rather than gimmicky is a real design problem, not a coding one.

---

### #3 — *The Care and Training of Your Talk-To-Paste*

**What:** a genuinely funny, genuinely well-made illustrated manual — PDF and printable — bundled with #1 and #2. Deadpan throughout. A Certificate of Adoption with the user's chosen name for their pill. A troubleshooting section that is entirely jokes. Sections on what your pill eats, why it fears the letter Q, and what to do if it hears you crying.

**Mechanisms:** this is **the actual documented Pet Rock mechanism** — the manual was the product, the rock was the packaging · M1 gift exchange (it's an object, not a licence key) · M7 · M2.

**Why it ranks here:** it is the highest-leverage *component* in this document but a weak standalone purchase. It transforms #1 or #2 from "I bought some cosmetics" into "I bought a thing." Bundle it; don't sell it alone.

**Build cost:** zero engineering, substantial writing. That's the point — per §2, **the writing has to be genuinely, laboriously good**, or it reads as a cheap gag and undermines everything.

---

### #4 — Supporters wall + in-app credits

**What:** buyers may optionally add a name/handle to a public supporters page on ttp.amirks.eu and a scrolling credits panel in Settings.

**Mechanisms:** M3 self-signaling · the only genuine **image motivation** (§0) available besides sound · directly copies the mechanism Obsidian charges $25 for.

**Why not higher:** it's a component, not a purchase. Nobody pays €17 for a name on a list; but bundled with #1–#3 it adds the visibility layer that the private cosmetics can't provide. Near-zero build cost. **Must be strictly opt-in** given TTP's privacy positioning.

---

### #5 — "Your Year in Words"

**What:** an annual (or on-demand) beautifully typeset card generated from the stats already in `usage/store.rs`: words spoken, hours of typing avoided, longest single dictation, most productive hour of the day. With absurd comparisons — *"412,000 words. That's War and Peace, twice, and you did it in your pyjamas."* Locally generated, screenshot-able.

**Mechanisms:** M3 self-signaling, strongly — it's a statement about who you are · M5 IKEA effect, because the content is literally the user's own accumulated labour · **shareable**, which restores audience.

**Why not higher:** the data is already there (a real advantage), but it's seasonal — it delights once a year, whereas #1 and #2 delight every single day. Also needs care: aggregate counts only, never content, never leaving the machine, opt-in. A privacy-first dictation app must not appear to be reading your transcripts, even to count them.

---

### #6 — Tray icon and app icon sets

**What:** a set of alternate tray/app icons, unlocked together.

**Mechanisms:** M6 collecting · M3 weakly. Custom app icons are consistently the best-selling cosmetic IAP in indie Apple software, so the demand is real.

**Why low:** the tray icon is 16 points of grayscale that the user consciously looks at maybe twice a day, and `tray.rs` already composites status dots onto it, so variants add real complexity for very little emotional payload. Fine as a bundle sweetener; weak as a headline.

---

### #7 — A physical object

**What:** a small numbered card, sticker, or — the obvious joke — an actual rock in a box with air holes, mailed to the first N supporters.

**Mechanisms:** maximal M5 endowment (physical possession is where the mug study lives), maximal M1, genuine and *honest* M8 scarcity if genuinely capped at N.

**Why last:** the emotional case is the strongest in this document and the operational case is the worst. International postage, addresses (a privacy liability for a privacy-branded app), and per-unit fulfilment time destroy the margin at €17–20. **Worth doing exactly once as a launch stunt with a hard cap** ("the first 50 supporters get a real rock"), never as a standing offer.

---

### Explicitly rejected

- **A payment slider / pay-what-you-want field.** — M3, PNAS 2012. Depresses purchase entirely.
- **Any tier that touches functionality**, however trivially — including "priority support," "faster model," "unlimited history." Breaks the §7 sincerity signal and reopens value comparison.
- **A subscription.** Recurring payment for a joke is not a joke; it's a bill.
- **Drip-fed packs / seasonal FOMO.** Compulsion loop.
- **Anything that adds latency or unpredictability to the dictation path.**

---

## 6. Framing and pricing specifics

**Price.** One tier, one price, one-time. €17–20 is well-justified: it sits between Apollo's "Godzilla Tip" ($20) and Obsidian Insider ($25), both of which work. It is deliberately ~5× the natural undirected voluntary payment for free digital goods (itch: $3.68 average; Radiohead: $6 among payers) — and that gap is closed by giving the purchase an *identity and an object* rather than by lowering the number.

**Optionally add one higher tier**, unnamed-price-wise but named-identity-wise (Obsidian's $50/$100; Shottr's $30). The downside is nil and self-selection upward is documented. Do **not** add a *lower* tier — that reintroduces the M3 shame problem and the M9/M3 conflict.

**Naming.** Name the tier after a character or a joke, not an amount and not a function. "Supporter" is fine and proven; something in TTP's own voice is better. Whatever it is, the user should enjoy *being* it.

**Timing of the ask.** After delivered value, once (M1). A candidate trigger: after the first genuinely successful week of use, a single non-modal pill-adjacent notice, dismissible permanently. Not onboarding. Not a modal. Not repeated. And critically — **do not phrase it in terms of how much they've used it.** That's an invoice (M10). Phrase it as an invitation.

**Copy register.** Read the Pet Rock manual first (link in §2). Deadpan, committed, never winking, never apologetic, never explaining the joke. If the copy contains the phrase "if you enjoy TTP, please consider…", it has already failed.

**What to never say:** anything implying obligation, anything implying the app's survival depends on it, anything with a deadline, anything with a guilt-framed decline button.

---

## 7. What I'd be honest about

- **Expected conversion is genuinely unknown and probably low.** The tip-jar and donationware literature is anecdote-heavy and metric-poor; RevenueCat's own guide is instructional rather than data-driven and publishes no benchmark conversion rates. Comparable public numbers (itch's 30%-of-revenue-above-minimum, Radiohead's 38% paying) come from *purchase* contexts, not from free-app tip jars, and are upper bounds that do not transfer. Plan for this as a goodwill instrument that might pay for coffee, not as a revenue line.
- **The Pet Rock is a bad model to plan around and a great model to write like.** Six months, then nothing, and its creator never repeated it. Take the manual, leave the business case.
- **The strongest single argument for this whole plan is M1 (reciprocity), and it is strong** — Falk's +75% is a large, clean, randomised field effect. It is also the mechanism most easily destroyed by getting the ask's tone wrong.
- **M7 (uselessness as sincerity) is the load-bearing idea for the *creative* brief and it is the least directly evidenced thing here.** I believe it's right; I couldn't find an experiment that tests it. Flagging that honestly because it's the claim doing the most work in §5's rankings.
- **The no-audience problem (§0) is the most under-appreciated constraint** and it is the reason the sound packs beat the visual cosmetics, which is not the intuitive ordering.

---

## Source index

**Academic**
- Andreoni, *Impure Altruism and Donations to Public Goods* (1990) — https://academic.oup.com/ej/article-abstract/100/401/464/5190270
- Falk, *Gift Exchange in the Field*, Econometrica (2007) — https://onlinelibrary.wiley.com/doi/abs/10.1111/j.1468-0262.2007.00800.x
- Gneezy, Gneezy, Nelson & Brown, *Shared Social Responsibility*, Science (2010) — https://www.science.org/doi/10.1126/science.1186744
- Gneezy, Gneezy, Riener & Nelson, *Pay-what-you-want, identity, and self-signaling*, PNAS (2012) — https://www.pnas.org/doi/10.1073/pnas.1120893109
- Ariely, Bracha & Meier, *Doing Good or Doing Well?*, AER (2009) — https://www.bostonfed.org/-/media/Documents/Workingpapers/PDF/wp0709.pdf
- Gneezy & Rustichini, *Pay Enough or Don't Pay at All* — https://rady.ucsd.edu/_files/faculty-research/uri-gneezy/pay-enough.pdf
- Epley, Waytz & Cacioppo, *On Seeing Human*, Psych Review (2007) — https://philpapers.org/rec/EPLOSH
- Kahneman, Knetsch & Thaler, *Experimental Tests of the Endowment Effect*, JPE (1990) — https://web.mit.edu/curhan/www/docs/Articles/15341_Readings/Behavioral_Decision_Theory/Kahneman_et_al_1990_Experimental_tests.pdf
- Norton, Mochon & Ariely, *The IKEA Effect* (2012) — https://dash.harvard.edu/bitstreams/7312037d-2473-6bd4-e053-0100007fdf3b/download
- Cialdini & Schroeder, *Legitimizing Paltry Contributions* (1976) — https://www.communicationcache.com/uploads/1/0/8/8/10887248/increasing_compliance_by_legitimizing_paltry_contributions-_when_even_a_penny_helps.pdf
- Worchel, Lee & Adewole, *Effects of Supply and Demand on Ratings of Object Value* (1975) — https://www.semanticscholar.org/paper/e80a3b8c8b27fa69cc6f4fb4c4e497f705f07a89
- Set completion in collecting — https://www.sciencedirect.com/science/article/abs/pii/S0167487007000682
- Cosmetics as social currency — https://bristoluniversitypressdigital.com/view/journals/consoc/5/2/article-p285.xml
- Guilt appeals & reactance — https://www.sciencedirect.com/science/article/pii/S0001691826001277

**Primary artefacts & first-hand**
- *The Care and Training of Your Pet Rock*, full scan — https://archive.org/details/pet_rock_manual_original
- Obsidian Catalyst pricing — https://obsidian.md/help/catalyst
- Vim `:help uganda` — https://vimhelp.org/uganda.txt.html
- itch.io store data on PWYW — https://itch.io/blog/2/running-an-indie-game-store-2015
- Shottr pricing — https://shottr.cc/purchase.html · HN — https://news.ycombinator.com/item?id=31773863
- Sindre Sorhus on pricing — https://x.com/sindresorhus/status/1119690296936189952
- Bongo Cat DLC — https://store.steampowered.com/dlc/3419430/Bongo_Cat/
- Desktop Goose — https://samperson.itch.io/desktop-goose
- Apollo (tip tiers) — https://en.wikipedia.org/wiki/Apollo_(app)
- CAH Holiday Hole (NPR) — https://www.npr.org/sections/thetwo-way/2016/11/27/503502142/people-donated-nearly-100-000-to-dig-a-big-pointless-hole-in-the-ground
- comScore on *In Rainbows* — https://www.comscore.com/Insights/Press-Releases/2007/11/Radiohead-Downloads
- Pet Rock history (Mental Floss) — https://www.mentalfloss.com/article/595180/pet-rock-history
