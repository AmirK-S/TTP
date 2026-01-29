# Phase 1: Foundation + Recording - Context

**Gathered:** 2026-01-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Menu bar/tray app with global shortcuts for voice recording control. User can trigger recording via keyboard shortcut (push-to-talk or toggle mode), see visual feedback when recording, and set up API key on first run. Transcription and pasting are Phase 2.

</domain>

<decisions>
## Implementation Decisions

### Reference Product
- Follow Wispr Flow's UX patterns closely
- Same interaction model: push-to-talk + hands-free toggle mode
- Similar visual feedback approach

### Keyboard Shortcuts
- Default: `Ctrl+Shift+Space` (cross-platform, works on both macOS and Windows)
- Note: `fn` key (Wispr Flow's default) cannot be captured on macOS — Apple intercepts it at system level for emoji picker/dictation. Using modifier combo instead.
- Double-tap shortcut to enter hands-free mode (stays listening until pressed again)
- Shortcuts will be customizable in Phase 3 settings

### Visual Feedback
- Floating bar visible during recording (like Wispr's Flow bar)
- Shows "Recording..." or similar status text with animation
- Toggle to hide floating bar available in settings
- Tray/menu bar icon also changes state when recording

### Sound Effects
- Audio cues for recording start/stop — enabled by default
- Subtle sounds, not intrusive
- Toggle to disable available in settings

### Recording Modes
- Push-to-talk: Hold shortcut key to record, release to stop
- Hands-free/toggle: Double-tap shortcut to start, tap again to stop
- Same behavior as Wispr Flow

### Claude's Discretion
- Exact floating bar design and positioning
- Specific sound effect choice
- Tray icon design (recording vs idle states)
- Auto-start on login implementation

</decisions>

<specifics>
## Specific Ideas

- "Match Wispr Flow closely" — user wants familiar UX for anyone coming from Wispr
- Floating bar should be dismissable but present by default
- Sound effects help users know recording state without looking at screen

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-recording*
*Context gathered: 2026-01-29*
