# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-29)

**Core value:** One shortcut to turn speech into text, anywhere.
**Current focus:** Phase 5 - Ensemble Transcription

## Current Position

Phase: 5 of 5 (Ensemble Transcription)
Plan: 2 of 3 in current phase
Status: In progress
Last activity: 2026-01-30 - Completed 05-02-PLAN.md

Progress: [█████████░] 13/14 plans

## Performance Metrics

**Velocity:**
- Total plans completed: 13
- Average duration: 8.0 min
- Total execution time: 104 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3/3 | 30 min | 10 min |
| 2 | 3/3 | 30 min | 10 min |
| 3 | 3/3 | 12 min | 4 min |
| 4 | 2/2 | 13 min | 6.5 min |
| 5 | 2/3 | 11 min | 5.5 min |

**Recent Trend:**
- Last 5 plans: 03-03 (4min), 04-01 (8min), 04-02 (5min), 05-01 (8min), 05-02 (3min)
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
| 05-01 | 30-second provider timeout | Gladia polling can take 5-30s |
| 05-01 | tokio::join! over join_all | Fixed provider set makes static join! cleaner |
| 05-01 | Refactor whisper.rs | transcribe_with_provider for code reuse |
| 05-02 | 20-second fusion timeout | Slightly longer than polish for multi-transcription analysis |
| 05-02 | Require 2+ providers for ensemble | Ensemble only makes sense with multiple transcriptions |
| 05-02 | Fallback to polish on 1 result | When only 1 provider succeeds, use normal polish |

### Pending Todos

- [ ] fn key capture (requires native macOS code) — Future enhancement
- [ ] Volume-reactive floating bar animation — Future enhancement

### Blockers/Concerns

**Research flags from roadmap:**
- Phase 5: Optimal LLM fusion prompt needs empirical tuning

### Roadmap Evolution

- Phase 5 added: Ensemble Transcription — multi-provider parallel transcription with LLM fusion

## Session Continuity

Last session: 2026-01-30
Stopped at: Completed 05-02-PLAN.md (LLM fusion + pipeline integration)
Resume file: None

### Next Steps
1. Execute 05-03-PLAN.md (frontend ensemble toggle)

### Recent Work
- Added ensemble_enabled setting field (05-01)
- Created ensemble.rs with parallel provider execution (05-01)
- Created fusion.rs with LLM fusion logic (05-02)
- Integrated ensemble mode into transcription pipeline (05-02)
