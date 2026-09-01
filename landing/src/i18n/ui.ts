/* ----------------------------------------------------------------------------
   Every word on the site, in both languages.

   Two rules govern this file and neither is negotiable.

   1. **TTP Pro unlocks nothing you need.** Every feature that makes a
      transcription happen is free and uncapped. The paywall came off on
      2026-08-28 and does not return in a smaller shape. If a line here would
      make a reader feel they are missing a capability, the line is wrong.

   2. **Nothing here may be untrue with the source open.** TTP sends audio to
      `api.groq.com`; there is no local model anywhere in the tree. The site
      used to say "Nothing leaves your machine" under a heading reading
      "Privacy First". It does not say anything of the kind now, in any
      language, and it must not start again. See `docs/marketing-research.md`
      §0 and §2d: a competitor's identical claim was taken apart by a reader in
      its own launch thread, on this exact architecture, within hours.

   The French is a rewrite, not a translation, wherever the English depends on
   English-language facts — the manual's letter-Q section becomes the letter O,
   because that is where the animal actually suffers in French.
   ------------------------------------------------------------------------- */

export const languages = {
  en: 'English',
  fr: 'Français',
} as const;

export const defaultLang = 'en' as const;
export type Lang = keyof typeof languages;

export const ui = {
  en: {
    // ── chrome ───────────────────────────────────────────────────────────
    'nav.skip': 'Skip to content',
    'nav.what': 'What it does',
    'nav.free': 'Why it’s free',
    'nav.companion': 'The Companion',
    'nav.manual': 'The manual',
    'nav.install': 'Install',
    'nav.download': 'Download',
    'nav.menu': 'Menu',
    'nav.coat': 'Coat',
    'nav.coatHint': 'Dress this page in one of the app’s coats.',
    'nav.appearance': 'Appearance',
    'nav.light': 'Light',
    'nav.dark': 'Dark',
    'nav.system': 'System',

    // ── hero ─────────────────────────────────────────────────────────────
    'hero.eyebrow': 'macOS and Windows · everything free · nothing capped',
    'hero.headline': 'Hold a key. Say the sentence.',
    'hero.headlineAccent': 'It is already typed.',
    'hero.sub':
      'TTP listens while you hold the key, then puts the words where your cursor already was — in Slack, in your editor, in the box you were about to type into anyway.',
    'hero.plate': 'Plate I — one dictation, from the key going down to the words landing.',
    'hero.saidLabel': 'What you said',
    'hero.pastedLabel': 'What was pasted',
    'hero.raw':
      "so um I was thinking maybe we could uh push the release to friday, like, if that works for everyone? and I'll — I'll write the changelog thing tonight",
    'hero.polished':
      "I was thinking we could push the release to Friday, if that works for everyone. I'll write the changelog tonight.",
    'hero.statusIdle': 'Idle',
    'hero.statusListening': 'Listening',
    'hero.statusThinking': 'Working',
    'hero.statusDone': 'Pasted',
    'hero.replay': 'Play it again',
    'hero.demoNote':
      'The clean-up is a second pass called polish. It is on by default, it is free, and nothing counts how often you use it. Switch it off and you get the left-hand column instead.',
    'hero.ctaPrimary': 'Download for macOS',
    'hero.ctaSecondary': 'Read a piece of the manual',
    'hero.ctaSub': 'Free. No account. The Windows build is in the same place.',
    'hero.keyLead':
      'One thing stands between the download and the first word: TTP needs a Groq API key. It is free, it takes about two minutes, and it is the reason there is nothing to pay me for.',
    'hero.keyLink': 'Here is exactly why, and exactly how',

    // ── the three commands ───────────────────────────────────────────────
    'steps.plate': 'Plate II — the three commands',
    'steps.heading': 'Three commands, and they are all the same key.',
    'steps.holdTitle': 'HOLD',
    'steps.holdDesc':
      'Press and hold. On a Mac that is the Globe key unless you have moved it. The bars start moving, which is how you know it can hear you.',
    'steps.speakTitle': 'SPEAK',
    'steps.speakDesc':
      'At your ordinary speed, in your ordinary voice, with your ordinary swallowed word-endings. Over-enunciating makes you harder to understand, not easier.',
    'steps.releaseTitle': 'RELEASE',
    'steps.releaseDesc':
      'Let go. A second or two later the words appear wherever the cursor was. Double-tap instead of holding and it keeps listening without being held.',
    'steps.footnote': 'Abridged from the manual, under Training.',

    // ── the Groq key, named out loud ─────────────────────────────────────
    'key.plate': 'Plate III — the one piece of setup',
    'key.heading': 'Before the first word, there is a key.',
    'key.lead':
      'This is the part of TTP that is genuinely more work than its competitors, so it is in front of the download button rather than behind it.',
    'key.p1':
      'TTP does not ship a transcription model and does not run a server. It calls Groq with a key that is yours, on an account that is yours, billed to you at Groq’s prices — which for a day of dictation is cents.',
    'key.p2':
      'You make the key once. No card. Then you never think about it again.',
    'key.step1Title': 'Make a key',
    'key.step1Desc': 'Free, at console.groq.com. Two minutes, most of it waiting for the page.',
    'key.step2Title': 'Paste it in',
    'key.step2Desc': 'TTP asks for it on first launch and puts it in your Keychain.',
    'key.step3Title': 'Say yes three times',
    'key.step3Desc':
      'Microphone, Accessibility and Input Monitoring. It needs all three: two to hear you and one to type for you.',
    'key.buysTitle': 'What those two minutes buy',
    'key.buys1': 'No subscription, because there is no server of mine to pay for.',
    'key.buys2': 'No account, because there is nothing of mine to log in to.',
    'key.buys3': 'No metering. Nothing anywhere counts your dictations.',
    'key.buys4': 'Nothing that can be taken away from you when a business model changes.',
    'key.costsTitle': 'And what they cost',
    'key.costs1': 'TTP needs a network connection. There is no offline mode and no local model.',
    'key.costs2': 'Your audio goes to Groq. Their privacy policy applies to it, not mine.',
    'key.costs3': 'A competitor would have had you dictating by now, with no key at all.',
    'key.cta': 'Open the Groq console',
    'key.detailLink': 'Where your audio goes, in detail',

    // ── why is it free ───────────────────────────────────────────────────
    'free.plate': 'Plate IV — the question a free thing always provokes',
    'free.heading': 'Why is this free?',
    'free.p1': 'Because it costs me nothing to run.',
    'free.p2':
      'There is no TTP server. The transcription happens on Groq’s machines, paid for with your key. I have nothing to meter, no infrastructure bill to cover, and therefore nothing I need to sell you in order to cover it.',
    'free.p3':
      'So the free version is not a trial, not a sample, and not the top of a funnel. It is the app. There is no larger one standing behind it.',
    'free.p4':
      'There was a paid tier once. It capped polish at thirty a month, the dictionary at twenty entries and the history at fifty. I removed all three on 28 August 2026. They are not coming back in a smaller shape.',
    'free.sig': 'Amir, who wrote it and uses it all day',

    // ── what it does ─────────────────────────────────────────────────────
    'does.plate': 'Plate V — the whole animal',
    'does.heading': 'What it does. All of it. Uncapped.',
    'does.sub': 'There is no second list beside this one with the ticks in a different column.',
    'does.polishTitle': 'Polish',
    'does.polishDesc':
      'A second pass that takes out the ums, repairs the grammar and leaves the sentence you meant. On by default, switchable, and never counted.',
    'does.dictTitle': 'A dictionary',
    'does.dictDesc':
      'Give it the names of your colleagues, your company, the spelling of your own surname. It starts getting them right, and it will never mention that it did.',
    'does.histTitle': 'History',
    'does.histDesc':
      'Every transcription kept on your own disk so you can copy one back. Clear it whenever you like; there is no gap afterwards where the record was.',
    'does.langTitle': 'Languages',
    'does.langDesc':
      'Tell it plainly which language you speak and it stops guessing. Given a silent room it guesses, and its guesses are cosmopolitan.',
    'does.anywhereTitle': 'Any text field',
    'does.anywhereDesc':
      'It puts the words where the cursor is. It has no opinion about which application that is, and no way of finding out.',
    'does.handsTitle': 'Hands-free',
    'does.handsDesc':
      'Double-tap instead of holding and it keeps listening on its own. A small padlock appears on its flank while that is in force.',
    'does.platformsTitle': 'Mac and Windows',
    'does.platformsDesc':
      'A native app of a few megabytes, living in the menu bar among the clock and the battery. Territorial disputes have not been recorded.',
    'does.updatesTitle': 'Updates',
    'does.updatesDesc':
      'It checks GitHub Releases and installs them itself. Nothing identifying is sent in order to do it.',

    // ── where the audio goes ─────────────────────────────────────────────
    'audio.plate': 'Plate VI — the route your voice takes',
    'audio.heading': 'Your key. Your Groq account. No middleman.',
    'audio.lead': 'TTP is not a local transcription app, and this page is not going to tell you it is.',
    'audio.p1':
      'When you let go of the key, the recording goes from your Mac to api.groq.com over TLS, with your key, and comes back as text. That is the entire route. It is one hop, and there is nothing of mine on it.',
    'audio.p2':
      'I never see your audio. Not as a promise — as a fact about the shape of the thing. There is no server of mine for it to pass through, and no account of mine to attach it to.',
    'audio.diagYou': 'Your Mac',
    'audio.diagYouNote': 'records while you hold the key',
    'audio.diagGroq': 'api.groq.com',
    'audio.diagGroqNote': 'transcribes, on your key',
    'audio.diagNone': 'a TTP server',
    'audio.diagNoneNote': 'there is not one',
    'audio.staysTitle': 'What stays on the machine',
    'audio.stays1': 'Your Groq key, in the macOS Keychain. Never written to disk in plain text.',
    'audio.stays2': 'Your history, your dictionary and your settings, in your own Library folder.',
    'audio.stays3': 'The recording itself, which is deleted the moment the transcription comes back.',
    'audio.offTitle': 'What is switched off',
    'audio.off1':
      'Crash reporting. Off until you turn it on, and transcript contents are scrubbed before anything is sent.',
    'audio.off2':
      'Product analytics. Not disabled — deleted. What is left is an empty shim, and you can go and read it.',
    'audio.off3':
      'This website too: no analytics, no pixel, no third-party script, and no cookie banner, because there is nothing here to consent to.',
    'audio.checkTitle': 'Check it yourself',
    'audio.checkNote': 'Every sentence above is a line of source you can open:',
    'audio.privacyLink': 'The full privacy page',
    'audio.groqLink': 'Groq’s privacy policy',
    'audio.groqNote': 'Read theirs. It is theirs that governs your audio.',

    // ── the Companion ────────────────────────────────────────────────────
    'comp.plate': 'Plate VII — the optional object',
    'comp.heading': 'The Companion',
    'comp.lead': 'TTP has exactly one thing to sell, and it unlocks nothing. You already have the whole app.',
    'comp.sub':
      '€17, once, for a face, four coats, six voices, and a manual with your animal’s name written into it.',

    'comp.facesTitle': 'A face',
    'comp.facesDesc':
      'Two marks above the bars. They blink when nothing is happening, hold still while it works, and mark the moment your words land — a sixth of a second late, so it reads as having noticed rather than as a lamp coming on.',
    'comp.facesLive':
      'These are the real ones, running the real clocks, blinking at you now. They differ in timing and barely at all in drawing: a photograph will not separate them, and ten seconds of watching will.',
    'comp.facesPlay': 'Run a dictation past them',
    'comp.facesNever': 'None of them ever asks you for anything. Nothing decays, nothing needs feeding, and nothing is ever sad that you did not dictate today.',

    'comp.coatsTitle': 'Four coats',
    'comp.coatsDesc':
      'A coat is a whole aesthetic at once — palette, type, corner radius, density, motion — not a palette swap. Wild type is the free one, and it is the one built with the most care, because it is what almost everybody looks at.',
    'comp.coatsTry': 'Pick one. This page will put it on.',
    'comp.coatsWearing': 'This page is wearing',
    'comp.coatsHonest':
      'The page is only approximating them. The app is where they actually live, and there they also change how far apart things sit and how fast they move.',

    'comp.soundsTitle': 'Six voices',
    'comp.soundsDesc':
      'Two notes: a higher one to open, a lower one to close, always the same falling fourth. What changes is what is making them. It is the only part of TTP that anyone else in the room can hear.',

    'comp.manualTitle': 'A manual',
    'comp.manualDesc':
      'Three and a half thousand words on the care and training of an animal that does not exist, written completely straight, with plates. Buyers get the typeset object and a Certificate of Adoption with the name they gave their pill written on it.',
    'comp.manualCta': 'Read a piece of it',

    'comp.whyTitle': 'Why would anyone buy a thing that does nothing?',
    'comp.why1':
      'Because there is nothing else to buy. No subscription, because there is no server to run. No account, because there is nothing to log in to. No investor waiting for TTP to start charging for what it currently gives away.',
    'comp.why2':
      'I am not going to dress it up: the purchase makes the app no better at hearing you. The uselessness is the point. The moment the object becomes useful it stops being a gift and starts being a price, and then everything above it on this page stops being true.',
    'comp.why3':
      'If you would rather keep the €17, keep it. Nothing in the app will ask you again — there is one line about it in Settings, and it does not move.',
    'comp.price': '€17',
    'comp.priceSub': 'once, and then never again',
    'comp.cta': 'Adopt one',
    'comp.ctaNote': 'Through Lemon Squeezy. Existing keys keep working.',

    // ── the faces, named ─────────────────────────────────────────────────
    'face.house.name': 'House',
    'face.house.desc': 'The one it was born with. You stop seeing it around the third day, and you would notice if it left.',
    'face.shut.name': 'Shut',
    'face.shut.desc': 'Eyes closed until you press the key. Everything it has is in the fifth of a second where they open.',
    'face.drowsy.name': 'Drowsy',
    'face.drowsy.desc': 'Heavy lids, a long slow blink, and always a beat behind you. It never once startles you.',
    'face.quick.name': 'Quick',
    'face.quick.desc': 'Short sharp blinks, often two at a time. Good company for a four-second dictation and bad company for an afternoon of writing.',
    'face.bead.name': 'Bead',
    'face.bead.desc': 'Two small dots set as wide as the body allows. Still for ten seconds, two blinks, still again.',

    // ── the coats, named ─────────────────────────────────────────────────
    'coat.wild.name': 'Wild type',
    'coat.wild.desc': 'The form it arrives in. Nobody bred this one.',
    'coat.wild.tag': 'Free',
    'coat.roan.name': 'Roan',
    'coat.roan.desc': 'White hairs through a warm base. Paper, a serif, and a wide margin.',
    'coat.piebald.name': 'Piebald',
    'coat.piebald.desc': 'Two tones with a hard edge between them. Square corners, and nothing floats.',
    'coat.merle.name': 'Merle',
    'coat.merle.desc': 'Mottled blue-grey. The panels let the background through and nothing is in a hurry.',
    'coat.tortie.name': 'Tortoiseshell',
    'coat.tortie.desc': 'Ginger and black, kept close together. More window, less margin.',

    // ── the sound packs, named ───────────────────────────────────────────
    'pack.default.name': 'House',
    'pack.default.desc': 'The two polite tones TTP was born with.',
    'pack.bowl.name': 'Bowl',
    'pack.bowl.desc': 'A small brass bowl. The stop is someone’s hand on it.',
    'pack.marimba.name': 'Marimba',
    'pack.marimba.desc': 'Two notes on a wooden bar. Up to begin, down to finish.',
    'pack.submarine.name': 'Submarine',
    'pack.submarine.desc': 'A sonar ping into the quiet. Something down there heard it.',
    'pack.felt.name': 'Felt',
    'pack.felt.desc': 'A piano with a blanket over it. Barely a sound at all.',
    'pack.bubble.name': 'Bubble',
    'pack.bubble.desc': 'A drop going in, and a drop coming back out.',

    // ── compared ─────────────────────────────────────────────────────────
    'cmp.plate': 'Plate VIII — the same animal, differently kept',
    'cmp.heading': 'How this differs from the others',
    'cmp.lead':
      'Superwhisper and MacWhisper are good applications and I am not going to pretend otherwise. They are built on a different structure, and the structure is the part worth comparing.',
    'cmp.colTtp': 'TTP',
    'cmp.colMac': 'MacWhisper',
    'cmp.colSuper': 'Superwhisper',
    'cmp.rowPrice': 'Price',
    'cmp.rowFree': 'What the free version does',
    'cmp.rowSetup': 'Setup before the first word',
    'cmp.rowAudio': 'Where the audio goes',
    'cmp.rowOffline': 'Works offline',
    'cmp.rowByok': 'Using your own API key',
    'cmp.rowPaid': 'What paying gets you',
    'cmp.ttpPrice': 'Free',
    'cmp.ttpFree': 'Everything, uncapped',
    'cmp.ttpSetup': 'A free Groq key, about two minutes',
    'cmp.ttpAudio': 'Straight to Groq, on your key',
    'cmp.ttpOffline': 'No',
    'cmp.ttpByok': 'Required, and free',
    'cmp.ttpPaid': 'A face, coats, sounds and a manual. No features.',
    'cmp.macPrice': 'Free, or €64 once',
    'cmp.macFree': 'Dictation and local transcription',
    'cmp.macSetup': 'None',
    'cmp.macAudio': 'Stays on the Mac, in local mode',
    'cmp.macOffline': 'Yes',
    'cmp.macByok': 'A paid feature',
    'cmp.macPaid': 'More features',
    'cmp.superPrice': 'Free, or $8.49 a month',
    'cmp.superFree': 'Unlimited Whisper, meeting transcription',
    'cmp.superSetup': 'None',
    'cmp.superAudio': 'Local, or their cloud on their key',
    'cmp.superOffline': 'Yes',
    'cmp.superByok': 'A paid feature',
    'cmp.superPaid': 'More features',
    'cmp.honest':
      'Read the third row twice. Both of them start with no setup at all, because they ship a transcription model inside the app — and both sell bring-your-own-key as a paid extra. TTP asks for the key up front and gives everything else away. That is a real trade, and it will not suit everybody.',
    'cmp.asOf': 'Their prices as published on 31 August 2026. Check theirs; they are not mine to keep current.',

    // ── the maker ────────────────────────────────────────────────────────
    'who.plate': 'Plate IX — the person doing the asking',
    'who.heading': 'Who is asking for these permissions',
    'who.p1':
      'TTP asks macOS for Microphone, Accessibility and Input Monitoring. Those are three of the most powerful permissions on the machine, and you are entitled to know who is asking before you grant them.',
    'who.name': 'Amir Kellousi-Dhoum',
    'who.role': 'AI engineer, CentraleSupélec. Paris.',
    'who.p2':
      'I built TTP because I wanted it, and I use it every day; most of what is on this page exists because something annoyed me first. Production AI systems otherwise — multi-agent pipelines at AXA, a real-time market data feed in Rust at SunZuLabs, retrieval systems at ENGIE.',
    'who.p3':
      'The source is on GitHub. If a sentence on this page does not match it, open an issue and I will change the sentence.',
    'who.github': 'GitHub',
    'who.site': 'amirks.eu',
    'who.email': 'Open an issue',

    // ── install ──────────────────────────────────────────────────────────
    'install.plate': 'Plate X — getting it onto the machine',
    'install.heading': 'Installing it',
    'install.subheading': 'Two minutes, and one warning to click past.',
    'install.macTab': 'macOS',
    'install.winTab': 'Windows',
    'install.macOneLineTitle': 'One command, and it is installed',
    'install.macOneLineDesc':
      'Downloads TTP, installs it, clears Apple’s quarantine flag and launches it. Apple Silicon and Intel.',
    'install.macAlreadyTitle': 'Already downloaded the .dmg? Then just this',
    'install.macAlreadyDesc': 'It removes the quarantine flag. The app opens normally afterwards.',
    'install.macAltTitle': 'Or without the Terminal at all',
    'install.macAltStep1': 'Open the app once. It will be blocked.',
    'install.macAltStep2': 'System Settings → Privacy & Security.',
    'install.macAltStep3': 'Click “Open Anyway” and enter your password.',
    'install.winDownloadTitle': 'The installer',
    'install.winDownloadButton': 'Download TTP for Windows',
    'install.winDownloadDesc': 'An .exe. Run it.',
    'install.winSmartScreenTitle': 'SmartScreen will interrupt you once',
    'install.winStep1': 'Click “More info”.',
    'install.winStep2': 'Click “Run anyway”.',
    'install.whyTitle': 'Why does my computer distrust this?',
    'install.whyText':
      'Because the app is not signed with a paid developer certificate. Apple and Microsoft charge a few hundred a year for one, and the money would have to come from somewhere. It would come from you. The source is public instead: read it, or build it yourself, and decide that way.',
    'install.copy': 'Copy',
    'install.copied': 'Copied',
    'install.version': 'Version',

    // ── changelog ────────────────────────────────────────────────────────
    'log.plate': 'Plate XI — what has changed',
    'log.heading': 'Changelog',
    'log.subheading': 'Straight from GitHub Releases, unedited.',
    'log.empty': 'No releases to show right now.',
    'log.viewAll': 'All releases on GitHub',

    // ── footer ───────────────────────────────────────────────────────────
    'footer.tagline': 'Talk To Paste',
    'footer.blurb':
      'A dictation app for macOS and Windows. Free, uncapped, and honest about where your voice goes.',
    'footer.product': 'The app',
    'footer.about': 'About',
    'footer.legal': 'Legal',
    'footer.github': 'Source on GitHub',
    'footer.download': 'Download',
    'footer.manual': 'The manual',
    'footer.changelog': 'Changelog',
    'footer.privacy': 'Privacy',
    'footer.terms': 'Terms',
    'footer.companion': 'The Companion',
    'footer.colophon':
      'Set in Inter and Iowan Old Style. No analytics, no cookies, no third-party scripts on this page.',
    'footer.made': 'Made by one person, in Paris.',

    // ── meta ─────────────────────────────────────────────────────────────
    'meta.title': 'TTP — hold a key, say the sentence, it is already typed',
    'meta.description':
      'A dictation app for macOS and Windows. Hold a key, speak, and the words appear where your cursor is. Everything free and uncapped; bring your own free Groq key. Optionally, adopt a companion.',
    'meta.manualTitle': 'The Care and Training of Your Talk-To-Paste — an excerpt',
    'meta.manualDescription':
      'Three sections from the manual that comes with the TTP Companion: the first item, the section on specimens with eyes, and why it fears the letter Q.',

    // ── the manual excerpt ───────────────────────────────────────────────
    'man.back': 'Back to TTP',
    'man.title': 'The Care and Training of Your Talk-To-Paste',
    'man.subtitle': 'A manual for the owner',
    'man.edition': 'Second edition. The first is reprinted entire; one section has been added.',
    'man.warning': 'IMPORTANT: DO NOT SPEAK TO YOUR TALK-TO-PASTE BEFORE READING ITEM 1.',
    'man.excerptNote':
      'This is an excerpt: three sections of eleven, reprinted in full and unaltered. The complete manual — typeset, with the plates, and a Certificate of Adoption bearing the name you give your Talk-To-Paste — comes with the Companion.',

    'man.s1Title': 'Item 1.',
    'man.s1p1':
      'Your Talk-To-Paste is a young animal and may be somewhat disoriented by the journey it has just made. It has been compressed, transmitted, unpacked, inspected by your operating system, and asked to identify itself twice.',
    'man.s1p2':
      'It is customary, with a new animal, to leave it undisturbed for three days while it settles. This is not necessary here. Your Talk-To-Paste needs no settling period whatsoever. It was awake before you finished reading the previous sentence and it will remain awake, without complaint, for as long as you are logged in.',
    'man.s1p3': 'You may nevertheless wish to leave it alone for a few minutes.',
    'man.s1p4': 'Not for its sake.',
    'man.s1plate':
      '[PLATE I — the Talk-To-Paste at rest. Three-quarter view, waveform dormant. Dark body, no highlights, no glint. It should read as an animal that is awake but has been given nothing to do. Caption: “A specimen at rest.”]',

    'man.s2Title': 'On specimens with eyes',
    'man.s2p1':
      'The section above describes the animal as it arrives. It is reprinted from the first edition without alteration. It was accurate to every specimen then in circulation and it is no longer accurate to all of them.',
    'man.s2p2': 'A number of Talk-To-Pastes in private keeping have been found to have eyes.',
    'man.s2p3':
      'The animals are in every other respect the same animal. The same diet, the same fourteen bars, the same two notes at the same falling interval, the same complete absence of any view about what you dictate. Nothing has been added except the eyes.',
    'man.s2p4':
      'It is not a defect. It is not a symptom of anything, it is not the beginning of anything, and an animal with eyes is not further along than an animal without them. Owners have asked what regimen produces them, and the answer is that none does. They belong to the specimen rather than to its keeping.',
    'man.s2sub': 'What has been observed',
    'man.s2p5':
      'The eyes are small and there is very little for them to do, so what follows is close to the whole of the behaviour. It is given with times, because the times are the only part that carries anything. Two shapes of that size cannot be arranged into an expression. Everything the animal expresses, it expresses in how long it takes.',
    'man.s2p6':
      'The blink. Irregular, at intervals of between four and eight seconds, about ten times a minute. Owners who have set out to catch the period have not found one, and the absence of a period is itself the observation: anything perfectly regular would be a mechanism, and the longer you kept it the more plainly you would see the mechanism.',
    'man.s2p7':
      'The blink closes faster than it opens — roughly ninety milliseconds to shut and a hundred and thirty to open again. The asymmetry is not an error of measurement and it is the whole of the difference between a blink and a wink. A wink is addressed to somebody. Nothing this animal does is addressed to anybody.',
    'man.s2p8':
      'It does not look at you. This has been checked, because it is the first thing every owner asks. The eyes have not once been observed to turn toward the person in the room. They do not mark your leaving and they do not mark your return. An animal that looked up when you came back to the desk would be an animal that had been waiting, and there is nothing in this one that waits.',
    'man.s2p9':
      'When the words land, it is late. This is the most peculiar of the observations. The animal answers your hand at once: press the key and the eyes respond in the same instant, with no delay anybody has been able to measure. It does not answer its own work at once. When the words appear on the page the eyes stay as they were for about a sixth of a second. Then they open, by something under half a point, and settle, and stop.',
    'man.s2p10':
      'The delay is the entire content of it. A pair of eyes that moved in the same instant as the words would be reporting the words, which is what a lamp does. Moving a sixth of a second afterwards, they appear instead to have noticed them.',
    'man.s2p11':
      'A note on the early drawings. A few show a mouth, a short curve set below the eyes. They are wrong. The mouth was supplied by the draughtsman, who had drawn animals before and expected one. No specimen has been found with a mouth. A mouth would be an opinion, and the animal has none.',
    'man.s2plate':
      '[PLATE II-a — a comparative figure. The known varieties in a row, all at the same scale, all in the same attitude, all seen from the same angle. They are identical. The plate is not in error. Caption: “The varieties of Talk-To-Paste, drawn from life.”]',

    'man.s3Title': 'Why it fears the letter Q',
    'man.s3p1': 'Fear is the wrong word, strictly. Owners who have kept one for a while describe it more as bracing.',
    'man.s3p2':
      'Your Talk-To-Paste spells extremely well inside a word. Isolated letters are a different diet altogether, because a letter spoken aloud is not a letter to the animal — it is a word, and it must be written down as a word, and the animal must decide which word.',
    'man.s3p3':
      'For most letters this is survivable. Bee is a small animal, see is something you do, you is the person it is speaking to, and it usually gets these right because the rest of the sentence tells it what to think.',
    'man.s3p4':
      'The letter Q has no such protection. Spoken alone it is cue, which is a thing an actor waits for; it is queue, which is a line of people; it is Kew, which is a place in London with gardens in it and which the animal will capitalise, correctly, and with confidence.',
    'man.s3p5':
      'Three ordinary English words and a proper noun, and none of them is the letter. No other letter in the language is this outnumbered.',
    'man.s3p6':
      'There is no cure. There is only courtesy. Do not spell things at it. If you must give it a single letter, give the letter a companion — Q as in quiet — and it will take the meaning and discard the escort. Or type the letter yourself. It will not be offended, and it will not notice.',
    'man.s3plate':
      '[PLATE VII — the animal facing, at a slight remove, a single large letter Q rendered in a heavy serif with a long confident tail. Neither figure is doing anything. There is a good deal of empty space between them. Caption: “The letter Q.”]',

    'man.restTitle': 'What is not on this page',
    'man.restNote': 'The other eight sections, in order, as they appear in the manual.',
    'man.rest1': 'What you have taken home',
    'man.rest2': 'Anatomy of your Talk-To-Paste',
    'man.rest3': 'Feeding and diet',
    'man.rest4': 'Habitat and temperament',
    'man.rest5': 'Training',
    'man.rest6': 'Health and common ailments',
    'man.rest7': 'What to do if it hears you crying',
    'man.rest8': 'Breeding — strongly discouraged',
    'man.certTitle': 'Certificate of Adoption',
    'man.certBody':
      'This is to certify that the Talk-To-Paste described in this manual has passed from general circulation into private keeping, and is now known, for the remainder of its working life, as',
    'man.certName': 'the name you give it',
    'man.certFoot': 'The animal has not been informed of its name and will not respond to it. Owners are asked to use it anyway.',
    'man.cta': 'Adopt one — €17',
    'man.ctaNote': 'The app itself is free, and none of this is required to use it.',
  },

  fr: {
    // ── chrome ───────────────────────────────────────────────────────────
    'nav.skip': 'Aller au contenu',
    'nav.what': 'Ce qu’il fait',
    'nav.free': 'Pourquoi c’est gratuit',
    'nav.companion': 'Le Compagnon',
    'nav.manual': 'Le manuel',
    'nav.install': 'Installation',
    'nav.download': 'Télécharger',
    'nav.menu': 'Menu',
    'nav.coat': 'Robe',
    'nav.coatHint': 'Habillez cette page d’une des robes de l’app.',
    'nav.appearance': 'Apparence',
    'nav.light': 'Clair',
    'nav.dark': 'Sombre',
    'nav.system': 'Système',

    // ── hero ─────────────────────────────────────────────────────────────
    'hero.eyebrow': 'macOS et Windows · tout est gratuit · rien n’est plafonné',
    'hero.headline': 'Maintenez une touche. Dites la phrase.',
    'hero.headlineAccent': 'Elle est déjà écrite.',
    'hero.sub':
      'TTP écoute tant que vous maintenez la touche, puis pose les mots là où se trouvait déjà votre curseur — dans Slack, dans votre éditeur, dans le champ où vous alliez taper de toute façon.',
    'hero.plate': 'Planche I — une dictée, de la touche enfoncée jusqu’aux mots qui tombent.',
    'hero.saidLabel': 'Ce que vous avez dit',
    'hero.pastedLabel': 'Ce qui a été collé',
    'hero.raw':
      'alors euh je me disais qu’on pourrait peut-être hein décaler la sortie à vendredi, enfin, si ça va pour tout le monde ? et je — je m’occupe du changelog ce soir',
    'hero.polished':
      'Je me disais qu’on pourrait décaler la sortie à vendredi, si cela convient à tout le monde. Je m’occupe du changelog ce soir.',
    'hero.statusIdle': 'Au repos',
    'hero.statusListening': 'Écoute',
    'hero.statusThinking': 'Travaille',
    'hero.statusDone': 'Collé',
    'hero.replay': 'Rejouer',
    'hero.demoNote':
      'Le nettoyage est une seconde passe, appelée correction. Elle est active par défaut, elle est gratuite, et rien ne compte le nombre de fois où vous vous en servez. Désactivez-la et vous obtenez la colonne de gauche.',
    'hero.ctaPrimary': 'Télécharger pour macOS',
    'hero.ctaSecondary': 'Lire un morceau du manuel',
    'hero.ctaSub': 'Gratuit. Sans compte. La version Windows est au même endroit.',
    'hero.keyLead':
      'Une seule chose sépare le téléchargement du premier mot : TTP a besoin d’une clé API Groq. Elle est gratuite, elle prend deux minutes, et c’est précisément pour cela que vous n’avez rien à me payer.',
    'hero.keyLink': 'Voici pourquoi exactement, et comment',

    // ── les trois commandes ──────────────────────────────────────────────
    'steps.plate': 'Planche II — les trois commandes',
    'steps.heading': 'Trois commandes, et c’est la même touche.',
    'steps.holdTitle': 'MAINTENIR',
    'steps.holdDesc':
      'Appuyez et maintenez. Sur un Mac, c’est la touche Globe, sauf si vous l’avez changée. Les barres se mettent à bouger : c’est ainsi que vous savez qu’il vous entend.',
    'steps.speakTitle': 'PARLER',
    'steps.speakDesc':
      'À votre vitesse ordinaire, de votre voix ordinaire, avec vos fins de mots ordinairement avalées. Sur-articuler vous rend plus difficile à comprendre, pas plus facile.',
    'steps.releaseTitle': 'RELÂCHER',
    'steps.releaseDesc':
      'Lâchez. Une seconde ou deux plus tard, les mots paraissent là où était le curseur. Double-tapez au lieu de maintenir et il continue d’écouter sans qu’on le tienne.',
    'steps.footnote': 'Abrégé du manuel, au chapitre du dressage.',

    // ── la clé Groq, dite tout haut ──────────────────────────────────────
    'key.plate': 'Planche III — la seule chose à configurer',
    'key.heading': 'Avant le premier mot, il y a une clé.',
    'key.lead':
      'C’est la partie de TTP qui demande réellement plus de travail que chez ses concurrents. Elle est donc devant le bouton de téléchargement, et non derrière.',
    'key.p1':
      'TTP n’embarque aucun modèle de transcription et ne fait tourner aucun serveur. Il appelle Groq avec une clé qui est la vôtre, sur un compte qui est le vôtre, facturé au tarif de Groq — soit, pour une journée de dictée, quelques centimes.',
    'key.p2': 'Vous créez la clé une fois. Sans carte bancaire. Ensuite vous n’y repensez plus jamais.',
    'key.step1Title': 'Créer une clé',
    'key.step1Desc': 'Gratuitement, sur console.groq.com. Deux minutes, dont l’essentiel à attendre la page.',
    'key.step2Title': 'La coller',
    'key.step2Desc': 'TTP la demande au premier lancement et la range dans votre trousseau.',
    'key.step3Title': 'Dire oui trois fois',
    'key.step3Desc':
      'Micro, Accessibilité, Surveillance des saisies. Il lui faut les trois : deux pour vous entendre, une pour écrire à votre place.',
    'key.buysTitle': 'Ce que ces deux minutes achètent',
    'key.buys1': 'Aucun abonnement, parce qu’il n’y a aucun serveur à moi à financer.',
    'key.buys2': 'Aucun compte, parce qu’il n’y a rien à moi où se connecter.',
    'key.buys3': 'Aucun compteur. Rien nulle part ne compte vos dictées.',
    'key.buys4': 'Rien qui puisse vous être retiré le jour où un modèle économique change.',
    'key.costsTitle': 'Et ce qu’elles coûtent',
    'key.costs1': 'TTP a besoin du réseau. Il n’y a pas de mode hors ligne, et pas de modèle local.',
    'key.costs2': 'Votre audio part chez Groq. C’est leur politique de confidentialité qui s’applique, pas la mienne.',
    'key.costs3': 'Un concurrent vous aurait déjà fait dicter, sans aucune clé.',
    'key.cta': 'Ouvrir la console Groq',
    'key.detailLink': 'Où va votre audio, en détail',

    // ── pourquoi c’est gratuit ───────────────────────────────────────────
    'free.plate': 'Planche IV — la question que provoque toujours une chose gratuite',
    'free.heading': 'Pourquoi est-ce gratuit ?',
    'free.p1': 'Parce que cela ne me coûte rien à faire tourner.',
    'free.p2':
      'Il n’y a pas de serveur TTP. La transcription a lieu sur les machines de Groq, payée avec votre clé. Je n’ai rien à compter, aucune facture d’infrastructure à couvrir, et donc rien que j’aie besoin de vous vendre pour la couvrir.',
    'free.p3':
      'La version gratuite n’est donc ni un essai, ni un échantillon, ni le haut d’un entonnoir. C’est l’application. Il n’y en a pas de plus grande derrière.',
    'free.p4':
      'Il y a eu une version payante. Elle plafonnait la correction à trente par mois, le dictionnaire à vingt entrées et l’historique à cinquante. J’ai retiré les trois le 28 août 2026. Elles ne reviendront pas sous une forme plus discrète.',
    'free.sig': 'Amir, qui l’a écrite et s’en sert toute la journée',

    // ── ce qu’il fait ────────────────────────────────────────────────────
    'does.plate': 'Planche V — l’animal entier',
    'does.heading': 'Ce qu’il fait. Tout. Sans plafond.',
    'does.sub': 'Il n’y a pas de seconde liste à côté de celle-ci, avec les coches dans une autre colonne.',
    'does.polishTitle': 'La correction',
    'does.polishDesc':
      'Une seconde passe qui retire les euh, répare la grammaire et laisse la phrase que vous vouliez. Active par défaut, désactivable, et jamais comptée.',
    'does.dictTitle': 'Un dictionnaire',
    'does.dictDesc':
      'Donnez-lui les noms de vos collègues, celui de votre société, l’orthographe de votre propre nom. Il commence à les écrire correctement, et il ne le mentionnera jamais.',
    'does.histTitle': 'Un historique',
    'does.histDesc':
      'Chaque transcription gardée sur votre disque, pour en recopier une. Effacez-le quand vous voulez ; il ne reste ensuite aucun trou là où était l’entrée.',
    'does.langTitle': 'Les langues',
    'does.langDesc':
      'Dites-lui clairement quelle langue vous parlez et il cesse de deviner. Devant une pièce silencieuse il devine, et ses suppositions sont cosmopolites.',
    'does.anywhereTitle': 'N’importe quel champ',
    'does.anywhereDesc':
      'Il pose les mots là où est le curseur. Il n’a aucun avis sur l’application dont il s’agit, et aucun moyen de le savoir.',
    'does.handsTitle': 'Mains libres',
    'does.handsDesc':
      'Double-tapez au lieu de maintenir et il continue d’écouter seul. Un petit cadenas paraît sur son flanc tant que c’est le cas.',
    'does.platformsTitle': 'Mac et Windows',
    'does.platformsDesc':
      'Une app native de quelques mégaoctets, logée dans la barre de menus parmi l’horloge et la batterie. Aucun conflit territorial n’a été relevé.',
    'does.updatesTitle': 'Les mises à jour',
    'does.updatesDesc':
      'Il consulte les Releases GitHub et les installe lui-même. Rien d’identifiant n’est envoyé pour cela.',

    // ── où va l’audio ────────────────────────────────────────────────────
    'audio.plate': 'Planche VI — le trajet que fait votre voix',
    'audio.heading': 'Votre clé. Votre compte Groq. Aucun intermédiaire.',
    'audio.lead': 'TTP n’est pas une application de transcription locale, et cette page ne va pas vous dire le contraire.',
    'audio.p1':
      'Quand vous lâchez la touche, l’enregistrement va de votre Mac à api.groq.com en TLS, avec votre clé, et revient en texte. C’est tout le trajet. Un seul saut, et rien de moi dessus.',
    'audio.p2':
      'Je ne vois jamais votre audio. Non pas comme une promesse — comme un fait sur la forme de la chose. Il n’y a aucun serveur à moi par où il passerait, et aucun compte à moi auquel le rattacher.',
    'audio.diagYou': 'Votre Mac',
    'audio.diagYouNote': 'enregistre tant que la touche est tenue',
    'audio.diagGroq': 'api.groq.com',
    'audio.diagGroqNote': 'transcrit, avec votre clé',
    'audio.diagNone': 'un serveur TTP',
    'audio.diagNoneNote': 'il n’y en a pas',
    'audio.staysTitle': 'Ce qui reste sur la machine',
    'audio.stays1': 'Votre clé Groq, dans le trousseau macOS. Jamais écrite en clair sur le disque.',
    'audio.stays2': 'Votre historique, votre dictionnaire et vos réglages, dans votre propre dossier Bibliothèque.',
    'audio.stays3': 'L’enregistrement lui-même, supprimé dès que la transcription revient.',
    'audio.offTitle': 'Ce qui est éteint',
    'audio.off1':
      'Le rapport de plantage. Éteint tant que vous ne l’allumez pas, et le contenu des transcriptions est nettoyé avant tout envoi.',
    'audio.off2':
      'Les statistiques produit. Pas désactivées — supprimées. Ce qui reste est une coquille vide, et vous pouvez aller la lire.',
    'audio.off3':
      'Ce site aussi : pas de statistiques, pas de pixel, pas de script tiers, et pas de bandeau cookies, parce qu’il n’y a rien ici à accepter.',
    'audio.checkTitle': 'Vérifiez vous-même',
    'audio.checkNote': 'Chaque phrase ci-dessus est une ligne de code que vous pouvez ouvrir :',
    'audio.privacyLink': 'La page de confidentialité complète',
    'audio.groqLink': 'La politique de confidentialité de Groq',
    'audio.groqNote': 'Lisez la leur. C’est la leur qui gouverne votre audio.',

    // ── le Compagnon ─────────────────────────────────────────────────────
    'comp.plate': 'Planche VII — l’objet facultatif',
    'comp.heading': 'Le Compagnon',
    'comp.lead': 'TTP a exactement une chose à vendre, et elle ne débloque rien. Vous avez déjà l’application entière.',
    'comp.sub':
      '17 €, une fois, pour un visage, quatre robes, six voix, et un manuel où le nom de votre animal est inscrit.',

    'comp.facesTitle': 'Un visage',
    'comp.facesDesc':
      'Deux marques au-dessus des barres. Elles clignent quand il ne se passe rien, restent immobiles pendant le travail, et marquent l’instant où vos mots tombent — un sixième de seconde en retard, pour que cela se lise comme un animal qui a remarqué et non comme un voyant qui s’allume.',
    'comp.facesLive':
      'Ce sont les vraies, sur les vraies horloges, en train de cligner devant vous. Elles diffèrent par le temps et presque pas par le dessin : une photographie ne les sépare pas, dix secondes d’observation si.',
    'comp.facesPlay': 'Leur faire passer une dictée',
    'comp.facesNever':
      'Aucune ne vous demande jamais rien. Rien ne dépérit, rien n’a besoin d’être nourri, et rien n’est jamais triste que vous n’ayez pas dicté aujourd’hui.',

    'comp.coatsTitle': 'Quatre robes',
    'comp.coatsDesc':
      'Une robe, c’est une esthétique entière d’un coup — palette, typographie, rayon des angles, densité, mouvement — et non un changement de couleurs. Le type sauvage est la robe gratuite, et c’est celle qui a été travaillée avec le plus de soin, parce que c’est celle que presque tout le monde regarde.',
    'comp.coatsTry': 'Choisissez-en une. Cette page la mettra.',
    'comp.coatsWearing': 'Cette page porte',
    'comp.coatsHonest':
      'La page ne fait que les approcher. C’est dans l’app qu’elles vivent vraiment, et là elles changent aussi l’écart entre les choses et la vitesse à laquelle elles bougent.',

    'comp.soundsTitle': 'Six voix',
    'comp.soundsDesc':
      'Deux notes : une plus haute pour ouvrir, une plus basse pour fermer, toujours la même quarte descendante. Ce qui change, c’est ce qui les produit. C’est la seule partie de TTP que les autres personnes dans la pièce peuvent entendre.',

    'comp.manualTitle': 'Un manuel',
    'comp.manualDesc':
      'Trois mille cinq cents mots sur le soin et le dressage d’un animal qui n’existe pas, écrits au premier degré, avec des planches. Les acheteurs reçoivent l’objet composé et un Certificat d’adoption portant le nom qu’ils ont donné à leur pilule.',
    'comp.manualCta': 'En lire un morceau',

    'comp.whyTitle': 'Pourquoi acheter une chose qui ne sert à rien ?',
    'comp.why1':
      'Parce qu’il n’y a rien d’autre à acheter. Pas d’abonnement, puisqu’il n’y a pas de serveur à faire tourner. Pas de compte, puisqu’il n’y a rien où se connecter. Pas d’investisseur qui attend que TTP se mette à facturer ce qu’il donne aujourd’hui.',
    'comp.why2':
      'Je ne vais pas l’enjoliver : l’achat ne rend pas l’application meilleure pour vous entendre. L’inutilité est le sujet. Dès l’instant où l’objet devient utile, il cesse d’être un cadeau et devient un prix — et alors tout ce qui est écrit plus haut sur cette page cesse d’être vrai.',
    'comp.why3':
      'Si vous préférez garder les 17 €, gardez-les. L’application ne vous le redemandera pas : il y a une ligne à ce sujet dans les réglages, et elle ne bouge pas.',
    'comp.price': '17 €',
    'comp.priceSub': 'une fois, et puis plus jamais',
    'comp.cta': 'En adopter un',
    'comp.ctaNote': 'Via Lemon Squeezy. Les clés existantes continuent de fonctionner.',

    // ── les visages ──────────────────────────────────────────────────────
    'face.house.name': 'Maison',
    'face.house.desc': 'Celui avec lequel il est né. Vous cessez de le voir vers le troisième jour, et vous remarqueriez son départ.',
    'face.shut.name': 'Fermé',
    'face.shut.desc': 'Les yeux clos jusqu’à ce que vous appuyiez. Tout ce qu’il a tient dans le cinquième de seconde où ils s’ouvrent.',
    'face.drowsy.name': 'Somnolent',
    'face.drowsy.desc': 'Paupières lourdes, un clignement long et lent, toujours un temps derrière vous. Il ne vous fait jamais sursauter.',
    'face.quick.name': 'Vif',
    'face.quick.desc': 'Des clignements courts et nets, souvent deux d’affilée. Bonne compagnie pour une dictée de quatre secondes, mauvaise pour un après-midi d’écriture.',
    'face.bead.name': 'Perle',
    'face.bead.desc': 'Deux petits points aussi écartés que le corps le permet. Immobile dix secondes, deux clignements, immobile à nouveau.',

    // ── les robes ────────────────────────────────────────────────────────
    'coat.wild.name': 'Type sauvage',
    'coat.wild.desc': 'La forme dans laquelle il arrive. Personne ne l’a sélectionnée.',
    'coat.wild.tag': 'Gratuite',
    'coat.roan.name': 'Rouan',
    'coat.roan.desc': 'Des poils blancs sur un fond chaud. Du papier, une serif et de grandes marges.',
    'coat.piebald.name': 'Pie',
    'coat.piebald.desc': 'Deux tons et une frontière nette entre eux. Angles droits, et rien ne flotte.',
    'coat.merle.name': 'Arlequin',
    'coat.merle.desc': 'Gris-bleu marbré. Les panneaux laissent passer le fond et rien ne se presse.',
    'coat.tortie.name': 'Écaille de tortue',
    'coat.tortie.desc': 'Roux et noir, serrés l’un contre l’autre. Plus de fenêtre, moins de marge.',

    // ── les voix ─────────────────────────────────────────────────────────
    'pack.default.name': 'Maison',
    'pack.default.desc': 'Les deux tons polis avec lesquels TTP est né.',
    'pack.bowl.name': 'Bol',
    'pack.bowl.desc': 'Un petit bol en laiton. L’arrêt, c’est une main posée dessus.',
    'pack.marimba.name': 'Marimba',
    'pack.marimba.desc': 'Deux notes sur une lame de bois. Vers le haut pour commencer, vers le bas pour finir.',
    'pack.submarine.name': 'Sous-marin',
    'pack.submarine.desc': 'Un ping sonar dans le silence. Quelque chose en dessous a entendu.',
    'pack.felt.name': 'Feutre',
    'pack.felt.desc': 'Un piano avec une couverture dessus. À peine un son.',
    'pack.bubble.name': 'Bulle',
    'pack.bubble.desc': 'Une goutte qui entre, une goutte qui ressort.',

    // ── comparaison ──────────────────────────────────────────────────────
    'cmp.plate': 'Planche VIII — le même animal, gardé autrement',
    'cmp.heading': 'En quoi celui-ci diffère des autres',
    'cmp.lead':
      'Superwhisper et MacWhisper sont de bonnes applications et je ne vais pas prétendre le contraire. Elles reposent sur une autre structure, et c’est la structure qui mérite d’être comparée.',
    'cmp.colTtp': 'TTP',
    'cmp.colMac': 'MacWhisper',
    'cmp.colSuper': 'Superwhisper',
    'cmp.rowPrice': 'Prix',
    'cmp.rowFree': 'Ce que fait la version gratuite',
    'cmp.rowSetup': 'À configurer avant le premier mot',
    'cmp.rowAudio': 'Où va l’audio',
    'cmp.rowOffline': 'Fonctionne hors ligne',
    'cmp.rowByok': 'Utiliser sa propre clé API',
    'cmp.rowPaid': 'Ce que l’achat apporte',
    'cmp.ttpPrice': 'Gratuit',
    'cmp.ttpFree': 'Tout, sans plafond',
    'cmp.ttpSetup': 'Une clé Groq gratuite, deux minutes',
    'cmp.ttpAudio': 'Directement chez Groq, sur votre clé',
    'cmp.ttpOffline': 'Non',
    'cmp.ttpByok': 'Obligatoire, et gratuit',
    'cmp.ttpPaid': 'Un visage, des robes, des sons et un manuel. Aucune fonction.',
    'cmp.macPrice': 'Gratuit, ou 64 € une fois',
    'cmp.macFree': 'Dictée et transcription locale',
    'cmp.macSetup': 'Rien',
    'cmp.macAudio': 'Reste sur le Mac, en mode local',
    'cmp.macOffline': 'Oui',
    'cmp.macByok': 'Une fonction payante',
    'cmp.macPaid': 'Plus de fonctions',
    'cmp.superPrice': 'Gratuit, ou 8,49 $ par mois',
    'cmp.superFree': 'Whisper illimité, transcription de réunions',
    'cmp.superSetup': 'Rien',
    'cmp.superAudio': 'Local, ou leur cloud sur leur clé',
    'cmp.superOffline': 'Oui',
    'cmp.superByok': 'Une fonction payante',
    'cmp.superPaid': 'Plus de fonctions',
    'cmp.honest':
      'Relisez la troisième ligne. Les deux démarrent sans aucune configuration, parce qu’elles embarquent un modèle de transcription dans l’application — et toutes deux vendent l’usage de votre propre clé comme une option payante. TTP demande la clé d’emblée et donne tout le reste. C’est un vrai compromis, et il ne conviendra pas à tout le monde.',
    'cmp.asOf': 'Leurs tarifs tels que publiés le 31 août 2026. Vérifiez chez eux ; ce n’est pas à moi de les tenir à jour.',

    // ── le créateur ──────────────────────────────────────────────────────
    'who.plate': 'Planche IX — la personne qui demande',
    'who.heading': 'Qui demande ces autorisations',
    'who.p1':
      'TTP demande à macOS le micro, l’accessibilité et la surveillance des saisies. Ce sont trois des autorisations les plus puissantes de la machine, et vous êtes en droit de savoir qui les demande avant de les accorder.',
    'who.name': 'Amir Kellousi-Dhoum',
    'who.role': 'Ingénieur IA, CentraleSupélec. Paris.',
    'who.p2':
      'J’ai fait TTP parce que je le voulais, et je m’en sers tous les jours ; l’essentiel de ce qui est sur cette page existe parce que quelque chose m’avait d’abord agacé. Par ailleurs, des systèmes d’IA en production — pipelines multi-agents chez AXA, un flux de données de marché en temps réel en Rust chez SunZuLabs, des systèmes de recherche documentaire chez ENGIE.',
    'who.p3':
      'Le code est sur GitHub. Si une phrase de cette page ne lui correspond pas, ouvrez une issue et je changerai la phrase.',
    'who.github': 'GitHub',
    'who.site': 'amirks.eu',
    'who.email': 'Ouvrir une issue',

    // ── installation ─────────────────────────────────────────────────────
    'install.plate': 'Planche X — le mettre sur la machine',
    'install.heading': 'L’installer',
    'install.subheading': 'Deux minutes, et un avertissement à écarter.',
    'install.macTab': 'macOS',
    'install.winTab': 'Windows',
    'install.macOneLineTitle': 'Une commande, et c’est installé',
    'install.macOneLineDesc':
      'Télécharge TTP, l’installe, retire le drapeau de quarantaine d’Apple et le lance. Apple Silicon et Intel.',
    'install.macAlreadyTitle': 'Déjà téléchargé le .dmg ? Alors seulement ceci',
    'install.macAlreadyDesc': 'Cela retire le drapeau de quarantaine. L’app s’ouvre normalement ensuite.',
    'install.macAltTitle': 'Ou sans Terminal du tout',
    'install.macAltStep1': 'Ouvrez l’app une fois. Elle sera bloquée.',
    'install.macAltStep2': 'Réglages Système → Confidentialité et sécurité.',
    'install.macAltStep3': 'Cliquez « Ouvrir quand même » et entrez votre mot de passe.',
    'install.winDownloadTitle': 'L’installateur',
    'install.winDownloadButton': 'Télécharger TTP pour Windows',
    'install.winDownloadDesc': 'Un .exe. Lancez-le.',
    'install.winSmartScreenTitle': 'SmartScreen vous interrompra une fois',
    'install.winStep1': 'Cliquez « Informations complémentaires ».',
    'install.winStep2': 'Cliquez « Exécuter quand même ».',
    'install.whyTitle': 'Pourquoi mon ordinateur s’en méfie-t-il ?',
    'install.whyText':
      'Parce que l’app n’est pas signée avec un certificat de développeur payant. Apple et Microsoft en demandent quelques centaines d’euros par an, et cet argent devrait venir de quelque part. Il viendrait de vous. Le code est public à la place : lisez-le, ou compilez-le vous-même, et décidez ainsi.',
    'install.copy': 'Copier',
    'install.copied': 'Copié',
    'install.version': 'Version',

    // ── changelog ────────────────────────────────────────────────────────
    'log.plate': 'Planche XI — ce qui a changé',
    'log.heading': 'Journal des versions',
    'log.subheading': 'Directement depuis les Releases GitHub, sans retouche.',
    'log.empty': 'Aucune version à afficher pour l’instant.',
    'log.viewAll': 'Toutes les versions sur GitHub',

    // ── pied de page ─────────────────────────────────────────────────────
    'footer.tagline': 'Talk To Paste',
    'footer.blurb':
      'Une app de dictée pour macOS et Windows. Gratuite, sans plafond, et franche sur l’endroit où va votre voix.',
    'footer.product': 'L’app',
    'footer.about': 'À propos',
    'footer.legal': 'Mentions',
    'footer.github': 'Le code sur GitHub',
    'footer.download': 'Télécharger',
    'footer.manual': 'Le manuel',
    'footer.changelog': 'Journal des versions',
    'footer.privacy': 'Confidentialité',
    'footer.terms': 'Conditions',
    'footer.companion': 'Le Compagnon',
    'footer.colophon':
      'Composé en Inter et Iowan Old Style. Aucune statistique, aucun cookie, aucun script tiers sur cette page.',
    'footer.made': 'Fait par une seule personne, à Paris.',

    // ── meta ─────────────────────────────────────────────────────────────
    'meta.title': 'TTP — maintenez une touche, dites la phrase, elle est déjà écrite',
    'meta.description':
      'Une app de dictée pour macOS et Windows. Maintenez une touche, parlez, et les mots paraissent là où est votre curseur. Tout est gratuit et sans plafond ; apportez votre clé Groq gratuite. Et si vous voulez, adoptez un compagnon.',
    'meta.manualTitle': 'Du soin et du dressage de votre Talk-To-Paste — extrait',
    'meta.manualDescription':
      'Trois sections du manuel qui accompagne le Compagnon TTP : l’article premier, la section sur les sujets pourvus d’yeux, et pourquoi il redoute la lettre O.',

    // ── l’extrait du manuel ──────────────────────────────────────────────
    'man.back': 'Retour à TTP',
    'man.title': 'Du soin et du dressage de votre Talk-To-Paste',
    'man.subtitle': 'Manuel à l’usage du propriétaire',
    'man.edition': 'Seconde édition. La première est reproduite intégralement ; une section a été ajoutée.',
    'man.warning': 'AVERTISSEMENT : NE PARLEZ PAS À VOTRE TALK-TO-PASTE AVANT D’AVOIR LU L’ARTICLE PREMIER.',
    'man.excerptNote':
      'Ceci est un extrait : trois sections sur onze, reproduites intégralement et sans retouche. Le manuel complet — composé, avec les planches, et un Certificat d’adoption portant le nom que vous donnerez à votre Talk-To-Paste — accompagne le Compagnon.',

    'man.s1Title': 'Article premier.',
    'man.s1p1':
      'Votre Talk-To-Paste est un jeune animal, et le voyage qu’il vient d’accomplir a pu le désorienter. Il a été compressé, transmis, décompressé, inspecté par votre système d’exploitation, et prié de décliner son identité à deux reprises.',
    'man.s1p2':
      'L’usage, avec un animal neuf, veut qu’on le laisse trois jours tranquille, le temps qu’il prenne ses marques. Cela n’est pas nécessaire ici. Votre Talk-To-Paste n’a besoin d’aucune période d’acclimatation. Il était éveillé avant que vous n’ayez fini de lire la phrase précédente, et il le restera, sans se plaindre, aussi longtemps que votre session restera ouverte.',
    'man.s1p3': 'Vous souhaiterez néanmoins peut-être le laisser seul quelques minutes.',
    'man.s1p4': 'Ce n’est pas pour lui.',
    'man.s1plate':
      '[PLANCHE I — le Talk-To-Paste au repos. Vue de trois quarts, ondes dormantes. Corps sombre, aucun reflet, aucun éclat. Il doit se lire comme un animal éveillé à qui l’on n’a rien donné à faire. Légende : « Un sujet au repos. »]',

    'man.s2Title': 'Des sujets pourvus d’yeux',
    'man.s2p1':
      'La section qui précède décrit l’animal tel qu’il arrive. Elle est reproduite ici sans retouche, telle qu’elle figurait dans la première édition. Elle était exacte pour tous les sujets alors en circulation. Elle ne l’est plus pour tous.',
    'man.s2p2': 'On a trouvé des yeux à un certain nombre de Talk-To-Paste gardés à titre privé.',
    'man.s2p3':
      'L’animal est par ailleurs le même en tout point. Même régime, mêmes quatorze barres, mêmes deux notes au même intervalle descendant, même absence complète d’avis sur ce que vous dictez. Rien n’a été ajouté que les yeux.',
    'man.s2p4':
      'Ce n’est pas une malformation. Ce n’est le symptôme de rien, ce n’est le commencement de rien, et un sujet pourvu d’yeux n’est pas plus avancé qu’un sujet qui n’en a pas. Les propriétaires demandent quel régime les fait venir : aucun. Ils tiennent au sujet et non à la manière dont on le garde.',
    'man.s2sub': 'Ce que porte le relevé',
    'man.s2p5':
      'Les yeux sont petits et n’ont presque rien à faire ; ce qui suit est donc à peu près tout le comportement. Il est donné avec des durées, car la durée est la seule partie qui porte quelque chose. Deux formes de cette taille ne se laissent pas composer en expression. Tout ce que l’animal exprime, il l’exprime par le temps qu’il y met.',
    'man.s2p6':
      'Le clignement. Irrégulier, à des intervalles de quatre à huit secondes, une dizaine de fois par minute. Un relevé de trois cents clignements consécutifs ne fait apparaître aucune période. L’absence de période est elle-même l’observation : ce qui serait parfaitement régulier serait un mécanisme, et plus longtemps on le garderait, plus clairement on verrait le mécanisme.',
    'man.s2p7':
      'Le clignement se ferme plus vite qu’il ne s’ouvre : quatre-vingt-dix millisecondes environ pour la fermeture, cent trente pour l’ouverture. La dissymétrie n’est pas une erreur de mesure, et elle fait toute la différence entre un clignement et un clin d’œil. Un clin d’œil s’adresse à quelqu’un. Rien de ce que fait cet animal ne s’adresse à personne.',
    'man.s2p8':
      'Il ne vous regarde pas. Le point a été vérifié, car c’est la première chose que demande tout propriétaire. On n’a pas observé une seule fois les yeux se tourner vers la personne présente dans la pièce. Ils n’enregistrent pas votre départ et n’enregistrent pas votre retour. Un animal qui lèverait les yeux à votre retour au bureau serait un animal qui aurait attendu, et il n’y a rien en celui-ci qui attende.',
    'man.s2p9':
      'Quand les mots tombent, il est en retard. C’est la plus singulière des observations. L’animal répond à votre main sur-le-champ : pressez la touche et les yeux répondent dans le même instant, sans délai que personne ait pu mesurer. Il ne répond pas sur-le-champ à son propre ouvrage. Lorsque les mots paraissent sur la page, les yeux restent tels qu’ils étaient pendant un sixième de seconde environ. Puis ils s’ouvrent, d’un peu moins d’un demi-point, retombent, et s’arrêtent.',
    'man.s2p10':
      'Le retard en est tout le contenu. Des yeux qui bougeraient dans le même instant que les mots rapporteraient les mots, ce qui est l’office d’un voyant. Bougeant un sixième de seconde après, ils paraissent au contraire les avoir remarqués.',
    'man.s2p11':
      'Note sur les premiers dessins. Quelques-uns portent une bouche, une courte courbe placée sous les yeux. Ils sont fautifs. La bouche a été fournie par le dessinateur, qui avait dessiné des animaux auparavant et en attendait une. Aucun sujet n’a été trouvé qui en porte. Une bouche serait un avis, et l’animal n’en a pas.',
    'man.s2plate':
      '[PLANCHE II-a — une figure comparative. Les variétés connues en rang, toutes à la même échelle, toutes dans la même attitude, toutes vues sous le même angle. Elles sont identiques. La planche n’est pas fautive. Légende : « Les variétés de Talk-To-Paste, dessinées d’après nature. »]',

    'man.s3Title': 'Pourquoi il redoute la lettre O',
    'man.s3p1':
      'Redouter est le mot impropre, à la rigueur. Les propriétaires qui en gardent un depuis quelque temps décrivent plutôt une forme de raidissement.',
    'man.s3p2':
      'Votre Talk-To-Paste orthographie remarquablement bien à l’intérieur d’un mot. La lettre isolée est un tout autre régime, car une lettre prononcée à voix haute n’est pas une lettre pour l’animal : c’est un mot, il faut l’écrire comme un mot, et l’animal doit décider lequel.',
    'man.s3p3':
      'L’usage voudrait qu’on rassure ici le propriétaire en lui indiquant que la plupart des lettres passent sans encombre. Le relevé ne le permet pas. Dites L et vous aurez elle, ou aile, ou elles. Dites R et vous aurez air, aire, ère ou erre. Dites G et vous aurez j’ai, qui l’emportera toujours, car peu de mots sont plus fréquents dans cette langue. Dites C et vous aurez c’est, ces, ses, sais, sait ou s’est, c’est-à-dire la difficulté sur laquelle l’école française passe le plus de temps, servie à un animal qui n’y a jamais été inscrit.',
    'man.s3p4':
      'La lettre O reste néanmoins un cas à part, et c’est la seule pour laquelle ce manuel formule une recommandation. Prononcée seule, elle est eau, qui est une boisson ; elle est eaux, qui est la même en plus grand nombre ; elle est au et aux, qui ne veulent rien dire tout seuls et que l’animal collera pourtant au mot suivant ; elle est haut, qui est une direction ; elle est os, mais au pluriel seulement, le singulier se prononçant autrement, distinction sur laquelle il ne se trompe jamais ; elle est oh et ho, qui sont deux surprises distinctes ; elle est ô, que l’animal réserve à l’invocation et qu’il accentuera correctement, avec assurance, s’il estime que vous vous adressez à quelque chose ; et elle est aulx, pluriel d’ail, qu’il écrira sans faute et sans hésiter, ce qui est la partie la plus troublante de l’affaire.',
    'man.s3p5':
      'Un relevé prudent en donne onze. Onze façons d’écrire un seul son, et aucune n’est la lettre. On sait, par ailleurs, laquelle il choisit lorsqu’on ne lui laisse pas d’indice du tout : mis en présence du silence, il écrit Oh. Le fait a été observé trois fois dans la même minute, sur trois enregistrements distincts, par un propriétaire qui n’avait rien dit.',
    'man.s3p6':
      'Il n’y a pas de remède. Il n’y a que de la courtoisie. Ne lui épelez pas les choses. S’il vous faut absolument lui donner une lettre seule, donnez-lui une escorte — O comme Oscar — et il prendra le sens en laissant l’escorte. Ou bien tapez la lettre vous-même. Il ne s’en offensera pas, et il ne le remarquera pas.',
    'man.s3plate':
      '[PLANCHE VII — l’animal faisant face, à quelque distance, à un unique grand O composé dans un romain gras, parfaitement clos. Aucune des deux figures ne fait quoi que ce soit. Il y a beaucoup de blanc entre elles. Légende : « La lettre O. »]',

    'man.restTitle': 'Ce qui n’est pas sur cette page',
    'man.restNote': 'Les huit autres sections, dans l’ordre du manuel.',
    'man.rest1': 'Ce que vous avez ramené chez vous',
    'man.rest2': 'Anatomie du Talk-To-Paste',
    'man.rest3': 'Alimentation et régime',
    'man.rest4': 'Habitat et tempérament',
    'man.rest5': 'Dressage',
    'man.rest6': 'Santé et affections courantes',
    'man.rest7': 'Que faire s’il vous entend pleurer',
    'man.rest8': 'Reproduction — fortement déconseillée',
    'man.certTitle': 'Certificat d’adoption',
    'man.certBody':
      'Il est certifié que le Talk-To-Paste décrit dans le présent manuel est passé de la circulation générale à la garde privée, et qu’il est désormais connu, pour le reste de sa vie de travail, sous le nom de',
    'man.certName': 'le nom que vous lui donnez',
    'man.certFoot':
      'L’animal n’a pas été informé de son nom et n’y répondra pas. Il est demandé aux propriétaires de l’employer tout de même.',
    'man.cta': 'En adopter un — 17 €',
    'man.ctaNote': 'L’application elle-même est gratuite, et rien de tout ceci n’est requis pour s’en servir.',
  },
} as const;
