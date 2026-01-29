# Roadmap: TTP (Talk To Paste)

## Overview

TTP delivers voice-to-text transcription in four phases: establishing the foundation with a menu bar app and audio recording, building the core transcription pipeline with AI polish and auto-paste, adding the learning system with dictionary and settings, and finalizing with cross-platform polish. Each phase delivers a complete, testable capability that builds on the previous one.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3, 4): Planned milestone work
- Decimal phases (e.g., 2.1): Urgent insertions (marked with INSERTED)

- [x] **Phase 1: Foundation + Recording** - Menu bar app with audio capture and global shortcuts
- [x] **Phase 2: Transcription Pipeline** - Whisper API, AI polish, and auto-paste to active app
- [ ] **Phase 3: Learning + Settings** - Dictionary learning, history, and user configuration
- [ ] **Phase 4: Platform Parity** - Cross-platform consistency and final polish

## Phase Details

### Phase 1: Foundation + Recording
**Goal**: User can trigger voice recording from menu bar app via global shortcut on macOS and Windows
**Depends on**: Nothing (first phase)
**Requirements**: REC-01, REC-02, REC-03, REC-04, REC-05, REC-06, CFG-01, CFG-02, PLT-01, PLT-02
**Success Criteria** (what must be TRUE):
  1. User sees app icon in menu bar (macOS) or system tray (Windows)
  2. User can hold keyboard shortcut to record and release to stop (push-to-talk)
  3. User can double-tap shortcut to toggle persistent recording
  4. User sees visual indicator when recording is active
  5. User is prompted for API key on first run and key is stored securely
**Plans**: 3 plans in 3 waves (sequential)
**Research flag**: Skip - standard Tauri patterns

Plans:
- [x] 01-01-PLAN.md — Project scaffolding with Tauri 2.x, React, and CI/CD
- [x] 01-02-PLAN.md — Tray app with global shortcuts and recording state machine
- [x] 01-03-PLAN.md — Audio capture and API key setup

### Phase 2: Transcription Pipeline
**Goal**: User speaks and polished transcription appears in active text field
**Depends on**: Phase 1
**Requirements**: TRX-01, TRX-02, TRX-03, POL-01, POL-02, POL-03, POL-04, OUT-01, OUT-02, OUT-03
**Success Criteria** (what must be TRUE):
  1. User's speech is transcribed via Whisper API with proper punctuation
  2. Transcription is cleaned by GPT-4o-mini (filler words removed, grammar corrected)
  3. Self-corrections in speech are handled ("Tuesday no wait Wednesday" becomes "Wednesday")
  4. Polished text is auto-pasted into active application
  5. When auto-paste fails, text goes to clipboard with notification
**Plans**: 3 plans in 2 waves (2 parallel, 1 sequential)
**Research flag**: Complete - see 02-RESEARCH.md

Plans:
- [x] 02-01-PLAN.md — Transcription and polish backend (Whisper + GPT-4o-mini API clients)
- [x] 02-02-PLAN.md — Paste simulation and clipboard management (AppleScript + clipboard plugin)
- [x] 02-03-PLAN.md — Pipeline integration and UI (wire everything, frontend progress)

### Phase 3: Learning + Settings
**Goal**: App learns from user corrections and provides full configurability
**Depends on**: Phase 2
**Requirements**: POL-05, LRN-01, LRN-02, LRN-03, LRN-04, CFG-03, CFG-05, HST-01, HST-02, HST-03
**Success Criteria** (what must be TRUE):
  1. User corrections after paste are detected and stored as dictionary entries
  2. Future transcriptions use learned dictionary for improved AI polish
  3. User can view and edit learned corrections in settings
  4. User can toggle AI polish on/off (shortcut customization deferred to Phase 4)
  5. User can view recent transcription history and copy past transcriptions
**Plans**: 3 plans in 2 waves
**Research flag**: Skip - standard CRUD and settings patterns

Plans:
- [ ] 03-01-PLAN.md — Dictionary system backend and correction detection
- [ ] 03-02-PLAN.md — Settings UI with AI polish toggle and dictionary management
- [ ] 03-03-PLAN.md — Transcription history storage and UI

### Phase 4: Platform Parity
**Goal**: Consistent, polished experience across macOS and Windows
**Depends on**: Phase 3
**Requirements**: PLT-03
**Success Criteria** (what must be TRUE):
  1. All features work identically on macOS and Windows
  2. Platform-specific edge cases are handled (permission flows, paste behavior)
  3. App is ready for distribution (signed, notarized where required)
**Plans**: TBD (estimated 1-2 plans)
**Research flag**: Needs research - notarization workflow and Windows installer options

Plans:
- [ ] 04-01: Cross-platform testing and fixes
- [ ] 04-02: Distribution preparation

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation + Recording | 3/3 | ✓ Complete | 2026-01-29 |
| 2. Transcription Pipeline | 3/3 | ✓ Complete | 2026-01-29 |
| 3. Learning + Settings | 0/3 | Not started | - |
| 4. Platform Parity | 0/2 | Not started | - |

---
*Roadmap created: 2026-01-29*
*Depth: quick (4 phases)*
*Coverage: 30/30 requirements mapped*
