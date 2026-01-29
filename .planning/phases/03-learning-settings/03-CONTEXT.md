# Phase 3: Learning + Settings - Context

**Gathered:** 2026-01-29
**Status:** Ready for planning

<domain>
## Phase Boundary

App learns from user corrections and provides full configurability. Dictionary system detects and stores proper noun corrections. Settings UI provides AI polish toggle and reset. Transcription history shows all past transcriptions.

Shortcut customization (including fn key) is deferred to Phase 4.

</domain>

<decisions>
## Implementation Decisions

### Correction Detection
- 10-second window after paste to detect corrections
- Research Whisperflow's approach for detection mechanism
- Focus on proper nouns (names, places, specialized terms) — ignore general grammar corrections
- Dictionary is for specialized vocabulary, not human error fixes

### Dictionary UI
- Simple table: Original → Correction columns with delete button per row
- Auto-learned only — no manual add button
- Local storage only — no export/import
- "Clear all" button with confirmation dialog

### Settings Organization
- AI polish toggle in settings only (persists across sessions)
- Global "Reset to defaults" button
- Skip shortcut customization for Phase 3 — keep Option+Space fixed

### History Behavior
- Keep all history (no limit)
- Display: text + timestamp per entry
- Explicit copy button per row

### Claude's Discretion
- Settings layout (single page vs tabs)
- Clear history option design
- Exact detection mechanism (after researching Whisperflow)

</decisions>

<specifics>
## Specific Ideas

- "Look at what Whisperflow does and copy" — research their correction detection approach
- fn key shortcut is the ultimate goal (like Whisperflow) — Phase 4

</specifics>

<deferred>
## Deferred Ideas

- Shortcut customization including fn key — Phase 4
- Dictionary export/import — not needed for now

</deferred>

---

*Phase: 03-learning-settings*
*Context gathered: 2026-01-29*
