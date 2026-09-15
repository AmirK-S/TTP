/* ----------------------------------------------------------------------------
   Every word on the site, in both languages.

   Rules, each learned the hard way:

   1. Nothing here may be untrue with the source open. TTP sends audio to
      `api.groq.com`; there is no local model. Never write "local", "offline"
      or "nothing leaves your Mac".
   2. Everything is free and uncapped. The €17 licence is a thank-you that
      unlocks sound packs and nothing functional.
   3. macOS only until Windows has been tested on a real machine.
   4. Screen context is off until the person turns it on. Say so wherever it
      is shown.

   The French is the primary text (the launch audience is French) and speaks
   like the app does: « tu », short sentences.
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
    'nav.how': 'How it works',
    'nav.features': 'Features',
    'nav.privacy': 'Privacy',
    'nav.faq': 'FAQ',
    'nav.download': 'Download',
    'nav.menu': 'Menu',

    // ── hero ─────────────────────────────────────────────────────────────
    'hero.badge': 'Free · no account · for Mac',
    'hero.title1': 'You talk.',
    'hero.title2': 'It’s typed.',
    'hero.sub':
      'Hold a key, say your sentence, let go. Clean text appears where your cursor is, in any Mac app, in about a second.',
    'hero.cta': 'Download for Mac',
    'hero.intel': 'Intel Mac',
    'hero.signed': 'Signed and notarised by Apple',
    'hero.copy': 'Copy',
    'hero.copied': 'Copied',
    'hero.commandLabel': 'Or in Terminal',

    // ── the stage ────────────────────────────────────────────────────────
    'stage.label': 'Demonstration: a dictation into three apps',
    'stage.pause': 'Pause',
    'stage.play': 'Play',
    'stage.transcribing': 'Transcribing…',
    'stage.sceneChat': 'A message',
    'stage.sceneMail': 'An email',
    'stage.sceneScreen': 'With your screen',
    'stage.chatApp': 'Messages',
    'stage.chatWith': 'Julien',
    'stage.chatIncoming': 'When are you free for the review?',
    'stage.chatPlaceholder': 'Message',
    'stage.chatSaid': 'uh hi Julien, uh, let’s do the review Monday at 2… no, Tuesday, sorry',
    'stage.chatClean': 'Hi Julien, let’s do the review Tuesday at 2 pm.',
    'stage.mailApp': 'Mail',
    'stage.mailTo': 'To:',
    'stage.mailToValue': 'team',
    'stage.mailSubject': 'Subject:',
    'stage.mailSubjectValue': 'Release date',
    'stage.mailHello': 'Hi all,',
    'stage.mailSaid': 'so um I was thinking we could, like, push the release to Friday, if that works for everyone',
    'stage.mailClean': 'I was thinking we could push the release to Friday, if that works for everyone.',
    'stage.screenApp': 'Notes',
    'stage.screenDocTitle': 'Tonight',
    'stage.screenDocLine': 'Install Claude Code on the TTP repo',
    'stage.screenSaid': 'I’m trying cloud code tonight on the repo',
    'stage.screenClean': 'I’m trying Claude Code tonight on the repo.',
    'stage.screenBadge': 'Name read on screen',

    // ── apps strip ───────────────────────────────────────────────────────
    'apps.lead': 'Works wherever you can type.',

    // ── how ──────────────────────────────────────────────────────────────
    'how.eyebrow': 'How it works',
    'how.title': 'One key. Three moves.',
    'how.sub': 'No window to open, no button to click. TTP lives in the menu bar and waits for your key.',
    'how.holdTitle': 'Hold',
    'how.holdDesc': 'The fn key by default. Or any key you like, even a mouse button.',
    'how.speakTitle': 'Speak',
    'how.speakDesc': 'At your own pace. The ums and the “no, I mean” are TTP’s problem.',
    'how.releaseTitle': 'Let go',
    'how.releaseDesc': 'About a second later, the text is pasted where your cursor was.',
    'how.handsFree': 'A long one? Double-tap the key and TTP keeps listening on its own.',

    // ── features ─────────────────────────────────────────────────────────
    'feat.eyebrow': 'Features',
    'feat.title': 'Everything included. Nothing capped.',
    'feat.sub': 'No paid version with the good bits. This is the whole app.',
    'feat.polishTitle': 'It tidies, it doesn’t rewrite',
    'feat.polishDesc':
      'AI correction removes hesitations, fixes punctuation and follows your changes of mind. Your words stay yours. You can switch it off.',
    'feat.polishSaidLabel': 'You say',
    'feat.polishCleanLabel': 'TTP writes',
    'feat.speedValue': '≈ 1 s',
    'feat.speedTitle': 'From letting go to the text',
    'feat.speedDesc': 'For a short sentence, measured on my own dictations. It depends on your connection.',
    'feat.screenTitle': 'Names spelled right, thanks to your screen',
    'feat.screenDesc':
      'Turn it on and TTP sends Groq the names on screen that sound like what you said, and the sentence you are continuing. Never passwords.',
    'feat.screenOff': 'Off until you choose',
    'feat.screenSaid': 'you say “cloud code”',
    'feat.dictTitle': 'Your dictionary',
    'feat.dictDesc': 'Teach it a name once. It spells it right from then on.',
    'feat.histTitle': 'Nothing gets lost',
    'feat.histDesc': 'If a paste didn’t land, your dictation is waiting in the history.',
    'feat.histItem1': 'Hi Julien, let’s do the review Tuesday…',
    'feat.histItem2': 'I was thinking we could push the release…',
    'feat.longTitle': 'Long dictations',
    'feat.longDesc': 'One or two minutes in one go, for a full email or a detailed prompt.',
    'feat.langTitle': 'French, English',
    'feat.langDesc': 'It detects the language, or you set it once.',
    'feat.updTitle': 'Updates itself',
    'feat.updDesc': 'New versions install from the app. Nothing to redownload.',

    // ── setup ────────────────────────────────────────────────────────────
    'setup.eyebrow': 'Setup',
    'setup.title': 'Ready in two minutes. Once.',
    'setup.sub':
      'TTP has no server. It uses a Groq key in your name, and that is exactly why it costs nothing.',
    'setup.s1Title': 'Install TTP',
    'setup.s1Desc': 'Drag the app into Applications, or paste the command into Terminal.',
    'setup.s2Title': 'Create your Groq key',
    'setup.s2Desc': 'Free, no card, on console.groq.com. TTP keeps it in your Keychain.',
    'setup.s3Title': 'Allow three permissions',
    'setup.s3Desc':
      'Microphone to hear you, Accessibility to type for you, Input Monitoring to see your key. TTP opens the right panel.',
    'setup.s4Title': 'Choose for the screen',
    'setup.s4Desc': 'Yes or no. You can change your mind in Settings → Dictation.',
    'setup.cta': 'Open the Groq console',
    'setup.winTitle': 'Connect to Groq',
    'setup.winSub': 'Your voice is sent to Groq to be transcribed. TTP has no server.',
    'setup.winKey': 'Groq API key',
    'setup.winSaved': 'Key saved',
    'setup.winFree': 'Groq’s free tier covers normal daily use.',

    // ── privacy ──────────────────────────────────────────────────────────
    'priv.eyebrow': 'Privacy',
    'priv.title': 'Your voice goes to Groq. Never to me.',
    'priv.sub':
      'When you let go of the key, the recording goes straight from your Mac to Groq, with your key, and comes back as text. There is nothing of mine on the way.',
    'priv.mac': 'Your Mac',
    'priv.macNote': 'records while you hold',
    'priv.groq': 'Groq',
    'priv.groqNote': 'transcribes with your key',
    'priv.none': 'TTP server',
    'priv.noneNote': 'there isn’t one',
    'priv.staysTitle': 'Stays on your Mac',
    'priv.stays1': 'Your Groq key, in the Keychain',
    'priv.stays2': 'Your history, dictionary and settings',
    'priv.stays3': 'The recording, deleted once transcribed',
    'priv.consentTitle': 'Only if you say yes',
    'priv.consent1': 'Names read on screen, to spell them right',
    'priv.consent2': 'Crash reports, never your voice or your text',
    'priv.honestTitle': 'To be clear',
    'priv.honest1': 'You need a connection: there is no offline mode',
    'priv.honest2': 'Groq’s privacy policy applies to your audio',
    'priv.honest3': 'The source code is public on GitHub',
    'priv.policy': 'Full privacy page',
    'priv.groqPolicy': 'Groq’s policy',

    // ── maker ────────────────────────────────────────────────────────────
    'maker.eyebrow': 'Who makes it',
    'maker.title': 'Hi, I’m Amir.',
    'maker.p1':
      'I’m an AI engineer. I built TTP because I wanted to dictate everywhere on my Mac, fast and cleanly. I use it all day.',
    'maker.freeTitle': 'Why is it free?',
    'maker.p2':
      'Because it costs me nothing to run. No server, everyone uses their own Groq key. So no subscription, no account and no limits.',
    'maker.p3':
      'If you want to support me, a €17 licence, once, unlocks five sound packs. It’s a thank-you, nothing more: the app is complete without it.',
    'maker.role': 'AI engineer · CentraleSupélec · Paris',
    'maker.support': 'Support TTP · €17',
    'maker.site': 'amirks.eu',
    'maker.github': 'Source on GitHub',

    // ── faq ──────────────────────────────────────────────────────────────
    'faq.eyebrow': 'Questions',
    'faq.title': 'Before you install',
    'faq.q1': 'Is it really free?',
    'faq.a1':
      'Yes. Every feature, no limit, no trial. The only thing to buy is a €17 thank-you that unlocks sound packs. Groq’s free tier covers normal daily use.',
    'faq.q2': 'Why do I need a Groq key?',
    'faq.a2':
      'Because TTP has no server. The transcription runs at Groq, on your account. That’s why there is no TTP account and no subscription.',
    'faq.q3': 'Does it work offline?',
    'faq.a3':
      'No. Transcription happens at Groq, so you need a connection. If you need fully local, MacWhisper or Superwhisper in local mode will suit you better.',
    'faq.q4': 'Where do my dictations go?',
    'faq.a4':
      'Only to Groq, to be transcribed and corrected. TTP has no usage analytics. Crash reports are sent only if you turn them on, and never contain your text.',
    'faq.q5': 'What about Windows?',
    'faq.a5': 'Not yet. TTP is designed and tested on Mac first.',
    'faq.q6': 'How is it different from Superwhisper or Wispr Flow?',
    'faq.a6':
      'They are very good apps, with a subscription or a local model. TTP is free, needs no account, and your voice goes through no middleman. In exchange you need a Groq key and a connection.',
    'faq.q7': 'Something isn’t working?',
    'faq.a7':
      'In TTP: Settings → Advanced → Report a problem. It prepares an email with a log that never contains what you dictated. Or open an issue on GitHub.',
    'faq.q8': 'How do I uninstall it?',
    'faq.a8': 'Settings → Advanced → Uninstall. It removes the app and all of its data.',

    // ── final cta ────────────────────────────────────────────────────────
    'final.title': 'Try it on your next sentence.',
    'final.sub': 'Free, no account. Apple Silicon and Intel.',

    // ── footer ───────────────────────────────────────────────────────────
    'footer.tagline': 'Talk To Paste. Dictation for Mac.',
    'footer.made': 'Made in Paris by Amir Kellou-Sidhoum.',
    'footer.download': 'Download',
    'footer.releases': 'What’s new',
    'footer.github': 'GitHub',
    'footer.privacy': 'Privacy',
    'footer.terms': 'Terms',
    'footer.clean': 'No cookies and no analytics on this site.',

    // ── meta ─────────────────────────────────────────────────────────────
    'meta.title': 'TTP — you talk, it’s typed',
    'meta.description':
      'Dictation for Mac. Hold a key, speak, and clean text appears where your cursor is, in any app. Free, no account, bring your own free Groq key.',
  },

  fr: {
    // ── chrome ───────────────────────────────────────────────────────────
    'nav.skip': 'Aller au contenu',
    'nav.how': 'Fonctionnement',
    'nav.features': 'Fonctions',
    'nav.privacy': 'Confidentialité',
    'nav.faq': 'Questions',
    'nav.download': 'Télécharger',
    'nav.menu': 'Menu',

    // ── hero ─────────────────────────────────────────────────────────────
    'hero.badge': 'Gratuit · sans compte · pour Mac',
    'hero.title1': 'Tu parles.',
    'hero.title2': 'C’est écrit.',
    'hero.sub':
      'Maintiens une touche, dis ta phrase, relâche. Le texte propre apparaît là où est ton curseur, dans n’importe quelle app Mac, en une seconde environ.',
    'hero.cta': 'Télécharger pour Mac',
    'hero.intel': 'Mac Intel',
    'hero.signed': 'Signé et notarisé par Apple',
    'hero.copy': 'Copier',
    'hero.copied': 'Copié',
    'hero.commandLabel': 'Ou dans le Terminal',

    // ── the stage ────────────────────────────────────────────────────────
    'stage.label': 'Démonstration : une dictée dans trois apps',
    'stage.pause': 'Pause',
    'stage.play': 'Lecture',
    'stage.transcribing': 'Transcription…',
    'stage.sceneChat': 'Un message',
    'stage.sceneMail': 'Un e-mail',
    'stage.sceneScreen': 'Avec ton écran',
    'stage.chatApp': 'Messages',
    'stage.chatWith': 'Julien',
    'stage.chatIncoming': 'Tu es dispo quand pour le point ?',
    'stage.chatPlaceholder': 'Message',
    'stage.chatSaid': 'euh salut Julien, euh, on se voit lundi à 14h pour le point… non, mardi, pardon',
    'stage.chatClean': 'Salut Julien, on se voit mardi à 14 h pour le point.',
    'stage.mailApp': 'Mail',
    'stage.mailTo': 'À :',
    'stage.mailToValue': 'équipe',
    'stage.mailSubject': 'Objet :',
    'stage.mailSubjectValue': 'Date de sortie',
    'stage.mailHello': 'Bonjour à tous,',
    'stage.mailSaid':
      'alors euh je me disais qu’on pourrait hein décaler la sortie à vendredi, enfin si ça va pour tout le monde',
    'stage.mailClean': 'Je me disais qu’on pourrait décaler la sortie à vendredi, si ça convient à tout le monde.',
    'stage.screenApp': 'Notes',
    'stage.screenDocTitle': 'Ce soir',
    'stage.screenDocLine': 'Installer Claude Code sur le repo TTP',
    'stage.screenSaid': 'je teste cloud code ce soir sur le repo',
    'stage.screenClean': 'Je teste Claude Code ce soir sur le repo.',
    'stage.screenBadge': 'Nom lu à l’écran',

    // ── apps strip ───────────────────────────────────────────────────────
    'apps.lead': 'Marche partout où tu peux écrire.',

    // ── how ──────────────────────────────────────────────────────────────
    'how.eyebrow': 'Fonctionnement',
    'how.title': 'Une touche. Trois gestes.',
    'how.sub':
      'Pas de fenêtre à ouvrir, pas de bouton à cliquer. TTP vit dans la barre de menus et attend ta touche.',
    'how.holdTitle': 'Maintiens',
    'how.holdDesc': 'La touche fn par défaut. Ou celle que tu veux, même un bouton de souris.',
    'how.speakTitle': 'Parle',
    'how.speakDesc': 'À ton rythme. Les « euh » et les « non, pardon », c’est TTP qui s’en occupe.',
    'how.releaseTitle': 'Relâche',
    'how.releaseDesc': 'Une seconde plus tard environ, le texte est collé là où était ton curseur.',
    'how.handsFree': 'C’est long ? Tape deux fois la touche : TTP continue d’écouter tout seul.',

    // ── features ─────────────────────────────────────────────────────────
    'feat.eyebrow': 'Fonctions',
    'feat.title': 'Tout est inclus. Rien n’est limité.',
    'feat.sub': 'Pas de version payante avec les bonnes fonctions. C’est l’app entière.',
    'feat.polishTitle': 'Il corrige, il ne réécrit pas',
    'feat.polishDesc':
      'La correction IA retire les hésitations, remet la ponctuation et suit tes changements d’avis. Tes mots restent les tiens. Tu peux la couper.',
    'feat.polishSaidLabel': 'Tu dis',
    'feat.polishCleanLabel': 'TTP écrit',
    'feat.speedValue': '≈ 1 s',
    'feat.speedTitle': 'Entre la touche relâchée et le texte',
    'feat.speedDesc': 'Pour une phrase courte, mesuré sur mes propres dictées. Ça dépend de ta connexion.',
    'feat.screenTitle': 'Les bons noms, grâce à ton écran',
    'feat.screenDesc':
      'Si tu l’actives, TTP envoie à Groq les noms affichés qui ressemblent à ce que tu dis, et la phrase que tu continues. Jamais les mots de passe.',
    'feat.screenOff': 'Désactivé tant que tu ne choisis pas',
    'feat.screenSaid': 'tu dis « cloud code »',
    'feat.dictTitle': 'Ton dictionnaire',
    'feat.dictDesc': 'Apprends-lui un nom une fois. Il l’écrit bien ensuite.',
    'feat.histTitle': 'Rien ne se perd',
    'feat.histDesc': 'Si un collage n’a pas marché, ta dictée t’attend dans l’historique.',
    'feat.histItem1': 'Salut Julien, on se voit mardi à 14 h…',
    'feat.histItem2': 'Je me disais qu’on pourrait décaler la sortie…',
    'feat.longTitle': 'Longues dictées',
    'feat.longDesc': 'Une à deux minutes d’affilée, pour un e-mail entier ou un prompt détaillé.',
    'feat.langTitle': 'Français, anglais',
    'feat.langDesc': 'Il détecte la langue, ou tu la fixes une fois.',
    'feat.updTitle': 'Se met à jour seul',
    'feat.updDesc': 'Les nouvelles versions s’installent depuis l’app. Rien à retélécharger.',

    // ── setup ────────────────────────────────────────────────────────────
    'setup.eyebrow': 'Installation',
    'setup.title': 'Prêt en deux minutes. Une seule fois.',
    'setup.sub':
      'TTP n’a pas de serveur. Il utilise une clé Groq à ton nom, et c’est justement pour ça qu’il ne coûte rien.',
    'setup.s1Title': 'Installe TTP',
    'setup.s1Desc': 'Glisse l’app dans Applications, ou colle la commande dans le Terminal.',
    'setup.s2Title': 'Crée ta clé Groq',
    'setup.s2Desc': 'Gratuite, sans carte bancaire, sur console.groq.com. TTP la range dans ton Trousseau.',
    'setup.s3Title': 'Autorise trois permissions',
    'setup.s3Desc':
      'Le micro pour t’entendre, l’accessibilité pour écrire à ta place, la surveillance des entrées pour voir ta touche. TTP t’ouvre le bon panneau.',
    'setup.s4Title': 'Choisis pour l’écran',
    'setup.s4Desc': 'Oui ou non. Tu peux changer d’avis dans Réglages → Dictée.',
    'setup.cta': 'Ouvrir la console Groq',
    'setup.winTitle': 'Connecter à Groq',
    'setup.winSub': 'Ta voix est envoyée à Groq pour être transcrite. TTP n’a pas de serveur.',
    'setup.winKey': 'Clé API Groq',
    'setup.winSaved': 'Clé enregistrée',
    'setup.winFree': 'Le forfait gratuit de Groq suffit pour un usage quotidien normal.',

    // ── privacy ──────────────────────────────────────────────────────────
    'priv.eyebrow': 'Confidentialité',
    'priv.title': 'Ta voix va chez Groq. Jamais chez moi.',
    'priv.sub':
      'Quand tu relâches la touche, l’enregistrement part directement de ton Mac vers Groq, avec ta clé, et revient en texte. Rien à moi sur le trajet.',
    'priv.mac': 'Ton Mac',
    'priv.macNote': 'enregistre tant que tu tiens',
    'priv.groq': 'Groq',
    'priv.groqNote': 'transcrit avec ta clé',
    'priv.none': 'Serveur TTP',
    'priv.noneNote': 'il n’y en a pas',
    'priv.staysTitle': 'Reste sur ton Mac',
    'priv.stays1': 'Ta clé Groq, dans le Trousseau',
    'priv.stays2': 'Ton historique, ton dictionnaire, tes réglages',
    'priv.stays3': 'L’enregistrement, supprimé une fois transcrit',
    'priv.consentTitle': 'Seulement si tu dis oui',
    'priv.consent1': 'Les noms lus à l’écran, pour bien les écrire',
    'priv.consent2': 'Les rapports de plantage, jamais ta voix ni ton texte',
    'priv.honestTitle': 'Pour être clair',
    'priv.honest1': 'Il faut une connexion : pas de mode hors ligne',
    'priv.honest2': 'La politique de Groq s’applique à ton audio',
    'priv.honest3': 'Le code source est public sur GitHub',
    'priv.policy': 'Page confidentialité complète',
    'priv.groqPolicy': 'Politique de Groq',

    // ── maker ────────────────────────────────────────────────────────────
    'maker.eyebrow': 'Qui le fait',
    'maker.title': 'Salut, moi c’est Amir.',
    'maker.p1':
      'Je suis ingénieur IA. J’ai fait TTP parce que je voulais dicter partout sur mon Mac, vite et proprement. Je m’en sers toute la journée.',
    'maker.freeTitle': 'Pourquoi c’est gratuit ?',
    'maker.p2':
      'Parce que ça ne me coûte rien. Pas de serveur, chacun utilise sa clé Groq. Donc pas d’abonnement, pas de compte, pas de limite.',
    'maker.p3':
      'Si tu veux me soutenir, une licence à 17 €, une fois, débloque cinq packs de sons. C’est un merci, rien de plus : l’app est complète sans.',
    'maker.role': 'Ingénieur IA · CentraleSupélec · Paris',
    'maker.support': 'Soutenir TTP · 17 €',
    'maker.site': 'amirks.eu',
    'maker.github': 'Code source sur GitHub',

    // ── faq ──────────────────────────────────────────────────────────────
    'faq.eyebrow': 'Questions',
    'faq.title': 'Avant d’installer',
    'faq.q1': 'C’est vraiment gratuit ?',
    'faq.a1':
      'Oui. Toutes les fonctions, sans limite, sans essai. La seule chose à acheter est un merci à 17 € qui débloque des packs de sons. Le forfait gratuit de Groq suffit pour un usage quotidien normal.',
    'faq.q2': 'Pourquoi une clé Groq ?',
    'faq.a2':
      'Parce que TTP n’a pas de serveur. La transcription se fait chez Groq, sur ton compte. C’est pour ça qu’il n’y a ni compte TTP ni abonnement.',
    'faq.q3': 'Ça marche hors ligne ?',
    'faq.a3':
      'Non. La transcription se fait chez Groq, il faut donc une connexion. Si tu veux du 100 % local, MacWhisper ou Superwhisper en mode local te conviendront mieux.',
    'faq.q4': 'Où vont mes dictées ?',
    'faq.a4':
      'Seulement chez Groq, pour être transcrites et corrigées. TTP n’a aucune statistique d’usage. Les rapports de plantage partent seulement si tu les actives, et ne contiennent jamais ton texte.',
    'faq.q5': 'Et sur Windows ?',
    'faq.a5': 'Pas pour l’instant. TTP est pensé et testé d’abord sur Mac.',
    'faq.q6': 'Quelle différence avec Superwhisper ou Wispr Flow ?',
    'faq.a6':
      'Ce sont de très bonnes apps, avec un abonnement ou un modèle local. TTP est gratuit, sans compte, et ta voix ne passe par aucun intermédiaire. En échange, il faut une clé Groq et une connexion.',
    'faq.q7': 'Quelque chose ne marche pas ?',
    'faq.a7':
      'Dans TTP : Réglages → Avancé → Signaler un problème. Ça prépare un e-mail avec un relevé qui ne contient jamais ce que tu as dicté. Ou ouvre un ticket sur GitHub.',
    'faq.q8': 'Comment le désinstaller ?',
    'faq.a8': 'Réglages → Avancé → Désinstaller. Ça supprime l’app et toutes ses données.',

    // ── final cta ────────────────────────────────────────────────────────
    'final.title': 'Essaie-le sur ta prochaine phrase.',
    'final.sub': 'Gratuit, sans compte. Apple Silicon et Intel.',

    // ── footer ───────────────────────────────────────────────────────────
    'footer.tagline': 'Talk To Paste. La dictée pour Mac.',
    'footer.made': 'Fait à Paris par Amir Kellou-Sidhoum.',
    'footer.download': 'Télécharger',
    'footer.releases': 'Nouveautés',
    'footer.github': 'GitHub',
    'footer.privacy': 'Confidentialité',
    'footer.terms': 'Conditions',
    'footer.clean': 'Ni cookies ni statistiques sur ce site.',

    // ── meta ─────────────────────────────────────────────────────────────
    'meta.title': 'TTP — tu parles, c’est écrit',
    'meta.description':
      'La dictée pour Mac. Maintiens une touche, parle, et le texte propre apparaît là où est ton curseur, dans n’importe quelle app. Gratuit, sans compte, avec ta clé Groq gratuite.',
  },
} as const;
