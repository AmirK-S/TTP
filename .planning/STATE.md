# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-29)

**Core value:** One shortcut to turn speech into text, anywhere.
**Current focus:** Phase 4 - Platform Parity

## Current Position

Phase: 4 of 4 (Platform Parity)
Plan: 2 of 2 in current phase
Status: COMPLETE
Last activity: 2026-01-30 — Completed 04-02-PLAN.md

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**
- Total plans completed: 11
- Average duration: 8.5 min
- Total execution time: 93 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3/3 | 30 min | 10 min |
| 2 | 3/3 | 30 min | 10 min |
| 3 | 3/3 | 12 min | 4 min |
| 4 | 2/2 | 13 min | 6.5 min |

**Recent Trend:**
- Last 5 plans: 03-01 (~5min), 03-02 (4min), 03-03 (4min), 04-01 (8min), 04-02 (5min)
- Trend: consistent

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

| Plan | Decision | Rationale |
|------|----------|-----------|
| 01-02 | 300ms double-tap threshold | Standard UX threshold for toggle mode |
| 01-03 | Option+Space as shortcut | fn key requires native code (Phase 4) |
| 02-01 | gpt-4o-transcribe model | Better accuracy than whisper-1 |
| 02-02 | Clipboard guard save-write-restore | Ensures clipboard always has transcription |
| 02-03 | AppleScript for paste | enigo FFI was crashing, osascript is stable |
| 02-03 | OPENAI_API_KEY env var fallback | Avoids keychain prompts in development |
| 02-03 | Whisper prompt for multi-language | Preserves French/English mixed speech |
| 03-01 | 10-second detection window | Enough time for correction, not too long for clipboard changes |
| 03-01 | Single-word corrections only | Multiple changes likely unrelated clipboard content |
| 03-01 | Dictionary at ~/.config/ttp/dictionary.json | Cross-platform via dirs crate |
| 03-02 | Settings in ~/.config/ttp/settings.json | Cross-platform via dirs crate |
| 03-02 | Single-page settings layout | Simpler than tabs (CONTEXT.md discretion) |
| 03-02 | AI polish enabled by default | Users can disable in settings |
| 03-03 | Store raw_text only when polish enabled | Avoids duplication when they're the same |
| 03-03 | Unlimited history | Per CONTEXT.md guidance |
| 04-01 | enigo for cross-platform paste | Already in deps, unified macOS/Windows API |
| 04-01 | Unregister-all before new shortcut | Simpler than tracking old; only one shortcut used |

### Pending Todos

- [ ] fn key capture (requires native macOS code) — Future enhancement
- [ ] Volume-reactive floating bar animation — Future enhancement

### Blockers/Concerns

**Research flags from roadmap:**
- Phase 4: Notarization and Windows installer workflows need research (04-02)

## Session Continuity

Last session: 2026-01-30
Stopped at: Phase 4 complete - Milestone ready for completion
Resume file: None
