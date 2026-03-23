export const languages = {
  en: 'English',
  fr: 'Fran\u00e7ais',
} as const;

export const defaultLang = 'en' as const;
export type Lang = keyof typeof languages;

export const ui = {
  en: {
    // Navbar
    'nav.features': 'Features',
    'nav.howItWorks': 'How It Works',
    'nav.changelog': 'Changelog',
    'nav.download': 'Download',

    // Hero
    'hero.headline': 'Turn Speech into Text.',
    'hero.headlineAccent': 'Anywhere. Free.',
    'hero.subheadline':
      "Press a shortcut. Speak. Your words appear wherever you're typing. No switching apps. No copy-pasting. Just talk.",
    'hero.cta': 'Get TTP Free',
    'hero.socialProof': 'Free forever. No account. No strings attached.',
    'hero.whisperNote': 'Like Whisper... but FREE!',

    // How It Works
    'howItWorks.heading': 'How It Works',
    'howItWorks.subheading': 'Three steps. Zero friction.',
    'howItWorks.step1Title': 'Hold Your Shortcut',
    'howItWorks.step1Desc':
      'Press and hold your shortcut key. On Mac, just press Fn.',
    'howItWorks.step2Title': 'Speak',
    'howItWorks.step2Desc':
      'Talk naturally. TTP records your voice in the background.',
    'howItWorks.step3Title': 'Text Appears',
    'howItWorks.step3Desc':
      'Your words are transcribed and pasted instantly into any app.',

    // Features
    'features.heading': 'Everything You Need',
    'features.subheading': 'Powerful features, zero complexity.',
    'features.fastTitle': 'Lightning Fast',
    'features.fastDesc':
      'Powered by Groq Whisper. Your speech transcribed in under 2 seconds.',
    'features.everywhereTitle': 'Works Everywhere',
    'features.everywhereDesc':
      'Paste into any app \u2014 Slack, VS Code, Gmail, Notion, anywhere you type.',
    'features.aiPolishTitle': 'AI Polish',
    'features.aiPolishDesc':
      'Automatically removes filler words, fixes grammar, and cleans up your text.',
    'features.dictionaryTitle': 'Smart Dictionary',
    'features.dictionaryDesc':
      'Learns your names, jargon, and technical terms. Gets better with every use.',
    'features.platformsTitle': 'Mac + Windows',
    'features.platformsDesc':
      'Native app for macOS and Windows. Lightweight, lives in your menu bar.',
    'features.byokTitle': 'Bring Your Own Key',
    'features.byokDesc':
      'Use your Groq API key. No subscription. Pay only for what you use.',
    'features.privacyTitle': 'Privacy First',
    'features.privacyDesc':
      'API keys and history stored locally. Nothing leaves your machine.',
    'features.updatesTitle': 'Auto Updates',
    'features.updatesDesc':
      'Always up to date. Updates install automatically in the background.',

    // Footer
    'footer.tagline': 'Talk To Paste',
    'footer.github': 'GitHub',
    'footer.download': 'Download',
    'footer.changelog': 'Changelog',
    'footer.madeWith': 'Made with care.',

    // Download
    'download.heading': 'Get Started in Seconds',
    'download.subheading': 'Available for macOS and Windows. Free forever.',
    'download.downloads': 'downloads',
    'download.downloadFor': 'Download for',

    // Install Guide
    'install.heading': 'Installation',
    'install.subheading': 'Install TTP with a single command in your terminal.',
    'install.whyTitle': 'Why does this happen?',
    'install.whyText': "TTP is free and open source. Apple and Microsoft charge developers hundreds of dollars per year for a certificate that removes security warnings. We chose to keep TTP free instead. The app is fully open source \u2014 you can inspect every line of code on GitHub.",
    'install.macTab': 'macOS',
    'install.winTab': 'Windows',
    'install.macAlreadyTitle': 'Already downloaded? Open Terminal and run:',
    'install.macAlreadyDesc': "This removes Apple's quarantine flag. That's it \u2014 the app opens normally after this.",
    'install.macOneLineTitle': 'Install with one command:',
    'install.macOneLineDesc': 'Downloads TTP, installs it, removes the quarantine flag, and launches it. Works on Apple Silicon and Intel.',
    'install.macAltTitle': 'Alternative: no Terminal needed',
    'install.macAltStep1': 'Open the app once (it will be blocked)',
    'install.macAltStep2': 'Go to System Settings \u2192 Privacy & Security',
    'install.macAltStep3': 'Click "Open Anyway" and enter your password',
    'install.winDesc': 'Windows SmartScreen may show a warning on first launch.',
    'install.winStep1': 'Click "More info"',
    'install.winStep2': 'Click "Run anyway"',
    'install.copied': 'Copied!',
    'install.clickToCopy': 'Copy',

    // Changelog
    'changelog.heading': 'Changelog',
    'changelog.subheading': 'Every improvement, documented.',
    'changelog.empty': 'No releases yet. Check back soon!',
    'changelog.emptyLink': 'View project on GitHub',
    'changelog.error': "Couldn't load releases right now.",
    'changelog.errorLink': 'View releases on GitHub',
    'changelog.viewAll': 'View all releases on GitHub',

    // Meta
    'meta.title': 'TTP - Talk To Paste | Turn Speech into Text Anywhere',
    'meta.description':
      'TTP (Talk To Paste) - Turn speech into text anywhere. Press a shortcut, speak, and your words appear wherever you\'re typing. Free, open source, works on macOS and Windows.',

    // Portfolio
    'nav.about': 'About',
    'portfolio.heading': 'About the Builder',
    'portfolio.subheading': 'AI Engineer. Builder. Consultant.',
    'portfolio.title': 'AI Engineer / Builder / Consultant',
    'portfolio.bio':
      'CentraleSupelec-trained engineer building production AI systems. From multi-agent RAG pipelines at AXA to real-time Rust data systems at SunZuLabs.',
    'portfolio.credentials': 'CentraleSupelec',
    'portfolio.exp.heading': 'Experience',
    'portfolio.exp.axa.title': 'AXA AI Factory',
    'portfolio.exp.axa.desc':
      'Multi-agent RAG pipelines, vocal AI assistants, and LangGraph orchestration for insurance automation.',
    'portfolio.exp.engie.title': 'ENGIE Research & Innovation',
    'portfolio.exp.engie.desc':
      'Azure OpenAI RAG system processing 1,000+ documents, reducing research time by 68%.',
    'portfolio.exp.sunzu.title': 'SunZuLabs',
    'portfolio.exp.sunzu.desc':
      'Real-time market data feed handler in Rust with sub-10ms latency for fintech analytics.',
    'portfolio.exp.denem.title': 'DENEM Labs',
    'portfolio.exp.denem.desc':
      'AI consulting and system architecture for emerging tech startups.',
    'portfolio.exp.total.title': 'TotalEnergies Hackathon',
    'portfolio.exp.total.desc':
      'LLM agent for commodity trading analysis, built in 48 hours. Award-winning solution.',
    'portfolio.exp.audits.title': '15+ AI Audits',
    'portfolio.exp.audits.desc':
      'Comprehensive AI system evaluations and strategic recommendations for SMEs.',
    'portfolio.proj.heading': 'Projects',
    'portfolio.proj.ttp.title': 'TTP \u2014 Talk To Paste',
    'portfolio.proj.ttp.desc':
      'Open-source desktop app that turns speech into text anywhere. One shortcut, instant transcription.',
    'portfolio.proj.02viral.title': '02Viral.com',
    'portfolio.proj.02viral.desc':
      'AI-powered viral content platform with intelligent agents for content creation and optimization.',
    'portfolio.tech.heading': 'Tech Stack',
    'portfolio.viewProject': 'View Project',
    'portfolio.viewGithub': 'GitHub',
  },
  fr: {
    // Navbar
    'nav.features': 'Fonctionnalit\u00e9s',
    'nav.howItWorks': 'Comment \u00e7a marche',
    'nav.changelog': 'Changelog',
    'nav.download': 'T\u00e9l\u00e9charger',

    // Hero
    'hero.headline': 'Transformez la parole en texte.',
    'hero.headlineAccent': 'Partout. Gratuit.',
    'hero.subheadline':
      "Appuyez sur un raccourci. Parlez. Vos mots apparaissent l\u00e0 o\u00f9 vous tapez. Sans changer d'app. Sans copier-coller. Parlez, c'est tout.",
    'hero.cta': 'Obtenir TTP Gratuitement',
    'hero.socialProof': 'Gratuit pour toujours. Sans compte. Sans conditions.',
    'hero.whisperNote': 'Comme Whisper... mais GRATUIT !',

    // How It Works
    'howItWorks.heading': 'Comment \u00e7a marche',
    'howItWorks.subheading': 'Trois \u00e9tapes. Z\u00e9ro friction.',
    'howItWorks.step1Title': 'Maintenez votre raccourci',
    'howItWorks.step1Desc':
      'Appuyez et maintenez votre raccourci. Sur Mac, appuyez simplement sur Fn.',
    'howItWorks.step2Title': 'Parlez',
    'howItWorks.step2Desc':
      "Parlez naturellement. TTP enregistre votre voix en arri\u00e8re-plan.",
    'howItWorks.step3Title': 'Le texte appara\u00eet',
    'howItWorks.step3Desc':
      "Vos mots sont transcrits et coll\u00e9s instantan\u00e9ment dans n'importe quelle app.",

    // Features
    'features.heading': "Tout ce qu'il vous faut",
    'features.subheading':
      'Des fonctionnalit\u00e9s puissantes, z\u00e9ro complexit\u00e9.',
    'features.fastTitle': 'Ultra Rapide',
    'features.fastDesc':
      'Propuls\u00e9 par Groq Whisper. Votre parole transcrite en moins de 2 secondes.',
    'features.everywhereTitle': 'Fonctionne Partout',
    'features.everywhereDesc':
      "Collez dans n'importe quelle app \u2014 Slack, VS Code, Gmail, Notion, partout o\u00f9 vous tapez.",
    'features.aiPolishTitle': 'Correction IA',
    'features.aiPolishDesc':
      'Supprime automatiquement les mots superflus, corrige la grammaire et nettoie votre texte.',
    'features.dictionaryTitle': 'Dictionnaire Intelligent',
    'features.dictionaryDesc':
      "Apprend vos noms, jargon et termes techniques. S'am\u00e9liore \u00e0 chaque utilisation.",
    'features.platformsTitle': 'Mac + Windows',
    'features.platformsDesc':
      'App native pour macOS et Windows. L\u00e9g\u00e8re, vit dans votre barre de menu.',
    'features.byokTitle': 'Apportez Votre Cl\u00e9',
    'features.byokDesc':
      "Utilisez votre cl\u00e9 API Groq. Pas d'abonnement. Payez uniquement ce que vous utilisez.",
    'features.privacyTitle': "Vie Priv\u00e9e d'Abord",
    'features.privacyDesc':
      'Cl\u00e9s API et historique stock\u00e9s localement. Rien ne quitte votre machine.',
    'features.updatesTitle': 'Mises \u00e0 Jour Auto',
    'features.updatesDesc':
      "Toujours \u00e0 jour. Les mises \u00e0 jour s'installent automatiquement en arri\u00e8re-plan.",

    // Footer
    'footer.tagline': 'Talk To Paste',
    'footer.github': 'GitHub',
    'footer.download': 'T\u00e9l\u00e9charger',
    'footer.changelog': 'Changelog',
    'footer.madeWith': 'Fait avec soin.',

    // Download
    'download.heading': 'Commencez en quelques secondes',
    'download.subheading':
      'Disponible sur macOS et Windows. Gratuit pour toujours.',
    'download.downloads': 't\u00e9l\u00e9chargements',
    'download.downloadFor': 'T\u00e9l\u00e9charger pour',

    // Install Guide
    'install.heading': 'Installation',
    'install.subheading': 'Installez TTP avec une seule commande dans votre terminal.',
    'install.whyTitle': 'Pourquoi cette \u00e9tape ?',
    'install.whyText': "TTP est gratuit et open source. Apple et Microsoft facturent des centaines d'euros par an pour un certificat qui supprime les alertes de s\u00e9curit\u00e9. On a pr\u00e9f\u00e9r\u00e9 garder TTP gratuit. L'app est enti\u00e8rement open source \u2014 vous pouvez inspecter chaque ligne de code sur GitHub.",
    'install.macTab': 'macOS',
    'install.winTab': 'Windows',
    'install.macAlreadyTitle': 'D\u00e9j\u00e0 t\u00e9l\u00e9charg\u00e9 ? Ouvrez le Terminal et lancez :',
    'install.macAlreadyDesc': "Cela supprime le blocage d'Apple. C'est tout \u2014 l'app s'ouvre normalement apr\u00e8s \u00e7a.",
    'install.macOneLineTitle': 'Installez en une commande :',
    'install.macOneLineDesc': 'T\u00e9l\u00e9charge TTP, l\'installe, supprime le blocage et le lance. Fonctionne sur Apple Silicon et Intel.',
    'install.macAltTitle': 'Alternative : sans Terminal',
    'install.macAltStep1': "Ouvrez l'app une premi\u00e8re fois (elle sera bloqu\u00e9e)",
    'install.macAltStep2': 'Allez dans R\u00e9glages Syst\u00e8me \u2192 Confidentialit\u00e9 et s\u00e9curit\u00e9',
    'install.macAltStep3': 'Cliquez sur \u00ab Ouvrir quand m\u00eame \u00bb et entrez votre mot de passe',
    'install.winDesc': 'Windows SmartScreen peut afficher un avertissement au premier lancement.',
    'install.winStep1': 'Cliquez sur \u00ab Informations compl\u00e9mentaires \u00bb',
    'install.winStep2': 'Cliquez sur \u00ab Ex\u00e9cuter quand m\u00eame \u00bb',
    'install.copied': 'Copi\u00e9 !',
    'install.clickToCopy': 'Copier',

    // Changelog
    'changelog.heading': 'Journal des mises \u00e0 jour',
    'changelog.subheading': 'Chaque am\u00e9lioration, document\u00e9e.',
    'changelog.empty':
      'Pas encore de versions. Revenez bient\u00f4t\u00a0!',
    'changelog.emptyLink': 'Voir le projet sur GitHub',
    'changelog.error':
      'Impossible de charger les versions pour le moment.',
    'changelog.errorLink': 'Voir les versions sur GitHub',
    'changelog.viewAll': 'Voir toutes les versions sur GitHub',

    // Meta
    'meta.title':
      'TTP - Talk To Paste | Transformez la parole en texte, partout',
    'meta.description':
      "TTP (Talk To Paste) - Transformez la parole en texte, partout. Appuyez sur un raccourci, parlez, et vos mots apparaissent l\u00e0 o\u00f9 vous tapez. Gratuit, open source, disponible sur macOS et Windows.",

    // Portfolio
    'nav.about': 'A propos',
    'portfolio.heading': 'Le Cr\u00e9ateur',
    'portfolio.subheading': 'Ing\u00e9nieur IA. Cr\u00e9ateur. Consultant.',
    'portfolio.title': 'Ing\u00e9nieur IA / Cr\u00e9ateur / Consultant',
    'portfolio.bio':
      'Ingénieur CentraleSupelec spécialisé dans les systèmes IA en production. Des pipelines RAG multi-agents chez AXA aux systèmes temps réel en Rust chez SunZuLabs.',
    'portfolio.credentials': 'CentraleSupelec',
    'portfolio.exp.heading': 'Exp\u00e9rience',
    'portfolio.exp.axa.title': 'AXA AI Factory',
    'portfolio.exp.axa.desc':
      'Pipelines RAG multi-agents, assistants vocaux IA et orchestration LangGraph pour l\'automatisation en assurance.',
    'portfolio.exp.engie.title': 'ENGIE Recherche & Innovation',
    'portfolio.exp.engie.desc':
      'Syst\u00e8me RAG Azure OpenAI traitant 1 000+ documents, r\u00e9duisant le temps de recherche de 68%.',
    'portfolio.exp.sunzu.title': 'SunZuLabs',
    'portfolio.exp.sunzu.desc':
      'Gestionnaire de flux de donn\u00e9es march\u00e9 en temps r\u00e9el en Rust avec latence sub-10ms pour l\'analyse fintech.',
    'portfolio.exp.denem.title': 'DENEM Labs',
    'portfolio.exp.denem.desc':
      'Conseil en IA et architecture syst\u00e8mes pour startups tech \u00e9mergentes.',
    'portfolio.exp.total.title': 'Hackathon TotalEnergies',
    'portfolio.exp.total.desc':
      'Agent LLM pour l\'analyse du trading de mati\u00e8res premi\u00e8res, construit en 48h. Solution prim\u00e9e.',
    'portfolio.exp.audits.title': '15+ Audits IA',
    'portfolio.exp.audits.desc':
      '\u00c9valuations compl\u00e8tes de syst\u00e8mes IA et recommandations strat\u00e9giques pour PMEs.',
    'portfolio.proj.heading': 'Projets',
    'portfolio.proj.ttp.title': 'TTP \u2014 Talk To Paste',
    'portfolio.proj.ttp.desc':
      'Application desktop open-source qui transforme la parole en texte partout. Un raccourci, transcription instantan\u00e9e.',
    'portfolio.proj.02viral.title': '02Viral.com',
    'portfolio.proj.02viral.desc':
      'Plateforme de contenu viral propuls\u00e9e par l\'IA avec des agents intelligents pour la cr\u00e9ation et l\'optimisation de contenu.',
    'portfolio.tech.heading': 'Stack Technique',
    'portfolio.viewProject': 'Voir le Projet',
    'portfolio.viewGithub': 'GitHub',
  },
} as const;
