# TTP (Talk To Paste)

## What This Is

A cross-platform (macOS + Windows) voice transcription app that lives in the menu bar/system tray. Press a keyboard shortcut, speak, and your words get transcribed via OpenAI Whisper API then auto-pasted into whatever app you're using. Users bring their own API key — no subscription, just pay for what you use.

Inspired by WisprFlow's UX but with a "bring your own API key" model.

## Core Value

**One shortcut to turn speech into text, anywhere.** The transcription must be fast, accurate, and paste seamlessly into the active application.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Menu bar/system tray app with configurable keyboard shortcut
- [ ] Push-to-talk AND toggle recording modes (user configurable)
- [ ] Audio recording from system microphone
- [ ] Transcription via OpenAI Whisper API
- [ ] Custom dictionary for correction (replace known errors, e.g., "John" → "Jean")
- [ ] AI polish with GPT-4o-mini (optional, uses dictionary context)
- [ ] Auto-paste into active application
- [ ] Fallback to clipboard + notification when auto-paste impossible
- [ ] Secure local storage of API key
- [ ] Full transcription history with corrections
- [ ] Learning from user corrections post-paste to improve dictionary

### Out of Scope

- Offline transcription (local Whisper models) — user wants cloud API simplicity
- Multiple transcription providers — v1 focuses on OpenAI only
- Mobile apps — desktop first (Mac + Windows)
- Real-time streaming transcription — record then transcribe is fine
- Team features / sync — single user, local storage

## Context

**Inspiration:**
- WisprFlow: Polished UX, AI editing, but subscription model
- Handy: Open-source, offline-first, forkable — good reference for Tauri implementation

**User's motivation:** Wants WisprFlow's simplicity but without recurring subscription. Pay only for API usage.

**Key insight:** Auto-paste creates implicit feedback loop. When users correct transcriptions, the app can learn those corrections to improve over time.

## Constraints

- **Tech stack**: Tauri (Rust + React/TypeScript) — for cross-platform support
- **Platforms**: macOS and Windows (not Linux for v1)
- **API dependency**: Requires OpenAI API key — no offline fallback
- **Privacy**: API key and transcription history stored locally only

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Tauri over Electron | Lightweight (~10MB vs ~150MB), modern, good Rust ecosystem for audio | — Pending |
| OpenAI only for v1 | Simplifies implementation, user already has OpenAI in mind | — Pending |
| GPT-4o-mini for polish | Same API key as Whisper, fast and cheap | — Pending |
| Learn from corrections | Differentiator from simple transcription apps, improves with use | — Pending |

---
*Last updated: 2025-01-29 after initialization*
