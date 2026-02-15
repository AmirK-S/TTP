<p align="center">
  <img src="src-tauri/icons/icon.png" width="120" alt="TTP Logo" />
</p>

<h1 align="center">TTP — Talk To Paste</h1>

<p align="center">
  <strong>Press a shortcut. Speak. Your words appear wherever you're typing.</strong>
</p>

<p align="center">
  <a href="https://github.com/AmirK-S/TTP/actions/workflows/build.yml"><img src="https://github.com/AmirK-S/TTP/actions/workflows/build.yml/badge.svg" alt="Build" /></a>
  <a href="https://github.com/AmirK-S/TTP/releases"><img src="https://img.shields.io/github/v/release/AmirK-S/TTP?label=latest" alt="Latest Release" /></a>
  <a href="https://github.com/AmirK-S/TTP/releases"><img src="https://img.shields.io/github/downloads/AmirK-S/TTP/total" alt="Downloads" /></a>
</p>

<p align="center">
  <a href="https://ttp.amirk.eu">Website</a> &middot;
  <a href="https://github.com/AmirK-S/TTP/releases">Download</a> &middot;
  <a href="https://github.com/AmirK-S/TTP/releases">Changelog</a>
</p>

---

TTP is a lightweight desktop app that turns speech into text — instantly, in any app. Hold a shortcut, speak, and your words are transcribed and pasted wherever your cursor is. No switching apps, no copy-pasting, just talk.

Free forever. No account required. Bring your own Groq API key.

## Features

- **Lightning fast** — Powered by Groq Whisper. Transcription in under 2 seconds.
- **Works everywhere** — Paste into Slack, VS Code, Gmail, Notion, any app where you type.
- **AI polish** — Automatically removes filler words, fixes grammar, cleans up your text.
- **Smart dictionary** — Learns your names, jargon, and technical terms.
- **Mac + Windows** — Native app, lives in your menu bar / system tray.
- **Privacy first** — API keys and history stored locally. Nothing leaves your machine.
- **Auto updates** — Updates install automatically in the background.

## How It Works

1. **Hold your shortcut** — On Mac, just press `Fn`. On Windows, configure your preferred key.
2. **Speak** — Talk naturally. TTP records your voice in the background.
3. **Text appears** — Your words are transcribed and pasted instantly into the active app.

## Installation

### Download

Grab the latest release for your platform:

- **macOS** — `.dmg` from [Releases](https://github.com/AmirK-S/TTP/releases)
- **Windows** — `.msi` from [Releases](https://github.com/AmirK-S/TTP/releases)

### Setup

1. Install the app
2. Get a free API key from [Groq Console](https://console.groq.com)
3. Paste the key in TTP's setup screen
4. Start talking

## Build from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)

### Steps

```bash
# Clone the repo
git clone https://github.com/AmirK-S/TTP.git
cd TTP

# Install dependencies
npm install

# Run in development
npm run tauri dev

# Build for production
npm run tauri build
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri 2](https://v2.tauri.app/) |
| Frontend | React + TypeScript |
| Backend | Rust |
| Styling | Tailwind CSS |
| Transcription | [Groq Whisper](https://groq.com/) (whisper-large-v3) |
| AI Polish | Groq LLM (llama-3.3-70b-versatile) |
| Landing Page | [Astro](https://astro.build/) + GSAP |

## Project Structure

```
TTP/
├── src/                  # React frontend
│   ├── components/       # UI components (Pill, Setup, History)
│   └── hooks/            # Recording control, settings, dictionary
├── src-tauri/            # Rust backend
│   └── src/
│       ├── transcription/  # Whisper API + LLM polish pipeline
│       ├── dictionary/     # Smart dictionary with auto-detection
│       ├── paste/          # Clipboard + accessibility paste
│       └── fnkey.rs        # macOS Fn key listener
└── landing/              # Astro landing page (ttp.amirk.eu)
```

## Author

**Amir Kellou-Sidhoum** — AI Engineer / Builder / Consultant

- [LinkedIn](https://www.linkedin.com/in/amirks/)
- [Website](https://ttp.amirk.eu)

---

<p align="center">
  Made with care.
</p>
