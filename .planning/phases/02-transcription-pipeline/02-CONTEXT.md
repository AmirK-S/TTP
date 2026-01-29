# Phase 2: Transcription Pipeline - Context

**Gathered:** 2026-01-29
**Status:** Ready for planning

<domain>
## Phase Boundary

User speaks and polished transcription appears in active text field. This phase covers Whisper API transcription, GPT-4o-mini polish, and auto-paste to active application. Dictionary learning and settings are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Polish behavior
- Remove all filler words (um, uh, like, you know) — no exceptions
- Light touch grammar fixes only — correct obvious mistakes, preserve casual speech patterns
- Preserve exact tone — don't elevate formality, keep original voice
- Full punctuation and capitalization — periods, commas, question marks, proper caps

### Self-correction handling
- Keep final choice only — "Tuesday no wait Wednesday" becomes "Wednesday"
- Claude identifies correction patterns contextually (no hardcoded phrase list)
- When uncertain about a correction, preserve original verbatim
- Cross-sentence corrections supported — "Send it Monday. Actually make that Tuesday." resolves correctly

### Paste insertion
- Replace selection if text is selected when recording starts
- Smart spacing — add space if cursor follows a word, no space after punctuation/newline
- Cursor stays at original position after paste (before the inserted text)

### Failure modes
- Whisper API failure: retry 2-3 times silently, notify only if all retries fail
- Success path: text goes to clipboard AND auto-pastes
- Empty transcription: show subtle "No speech detected" notification
- Auto-paste failure: copy to clipboard + notification "Text copied — paste manually"
- GPT polish failure: retry with "processing..." indicator, fall back to raw only if retries exhaust

### Claude's Discretion
- Multi-line handling: Claude decides whether to preserve line breaks based on pause length and context
- Exact correction phrase detection patterns
- Retry timing and backoff strategy

</decisions>

<specifics>
## Specific Ideas

- User expects clipboard to always have the transcription as backup, regardless of auto-paste success
- "Processing..." indicator during polish retries so user knows something is happening
- Light touch means preserving how the user actually speaks — not making it "proper"

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-transcription-pipeline*
*Context gathered: 2026-01-29*
