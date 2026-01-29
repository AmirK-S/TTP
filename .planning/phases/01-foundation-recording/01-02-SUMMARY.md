---
phase: 01-foundation-recording
plan: 02
subsystem: core
tags: [tauri, rust, tray, global-shortcut, recording-state, audio, react, zustand]

# Dependency graph
requires: [01-01]
provides:
  - System tray app with context menu (Settings, Quit)
  - Global shortcut Ctrl+Shift+Space for recording control
  - Push-to-talk mode (hold to record, release to stop)
  - Double-tap toggle mode (tap to start, tap to stop)
  - Recording state machine (Idle/Recording/Processing)
  - Audio feedback via start/stop sounds
  - Floating bar window with recording indicator
  - Frontend state hook for recording events
affects: [01-03, transcription, paste, settings]

# Tech tracking
tech-stack:
  added: [rodio@0.19]
  patterns: [event-driven state machine, global shortcut handler, double-tap detection, embedded audio assets]

key-files:
  created:
    - src-tauri/src/state.rs
    - src-tauri/src/tray.rs
    - src-tauri/src/shortcuts.rs
    - src-tauri/src/sounds.rs
    - src-tauri/sounds/start.wav
    - src-tauri/sounds/stop.wav
    - src/hooks/useRecordingState.ts
    - src/stores/app-store.ts
    - src/windows/FloatingBar.tsx
  modified:
    - src-tauri/src/lib.rs
    - src-tauri/tauri.conf.json
    - src-tauri/Cargo.toml
    - src-tauri/capabilities/default.json
    - src/main.tsx
    - src/index.css

key-decisions:
  - "Ctrl+Shift+Space as default shortcut (fn key cannot be captured on macOS)"
  - "300ms double-tap threshold for hands-free mode toggle"
  - "Embedded sound files via include_bytes! (compile-time, no runtime loading)"
  - "Separate handler function to avoid lifetime issues in shortcut closure"

patterns-established:
  - "AppState managed via Mutex<AppState> with try_lock for non-blocking access"
  - "Event emission from Rust to frontend via app.emit()"
  - "Window routing in main.tsx based on webview label"
  - "Transparent window styling for overlay UI"

# Metrics
duration: 5min
completed: 2026-01-29
---

# Phase 1 Plan 2: System Tray and Global Shortcuts Summary

**Tray-resident app with Ctrl+Shift+Space shortcut, push-to-talk and double-tap toggle modes, visual floating bar, and audio feedback**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-29T13:36:11Z
- **Completed:** 2026-01-29T13:41:29Z
- **Tasks:** 3
- **Files created:** 9
- **Files modified:** 6

## Accomplishments

- Created AppState struct with RecordingState enum (Idle/Recording/Processing)
- Implemented system tray with context menu (Settings, Quit)
- Registered Ctrl+Shift+Space as global shortcut with press/release detection
- Implemented push-to-talk mode (hold shortcut to record, release to stop)
- Implemented double-tap detection (300ms threshold) for hands-free toggle mode
- Added rodio-based sound playback with embedded WAV files
- Created floating-bar window with transparent background and always-on-top
- Built FloatingBar React component with pulsing recording indicator
- Added useRecordingState hook for frontend event subscription
- Added app-store with Zustand for global state management

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement recording state machine and tray icon** - `eacb970` (feat)
2. **Task 2: Implement global shortcuts with push-to-talk and double-tap** - `dfb31b7` (feat)
3. **Task 3: Create floating bar UI and frontend state hook** - `1e55841` (feat)

## Files Created/Modified

**Rust Backend:**
- `src-tauri/src/state.rs` - AppState struct with RecordingState enum and event emission
- `src-tauri/src/tray.rs` - TrayIconBuilder setup with context menu
- `src-tauri/src/shortcuts.rs` - Global shortcut handler with double-tap detection
- `src-tauri/src/sounds.rs` - Rodio-based audio playback with embedded WAV files
- `src-tauri/sounds/start.wav` - Recording start sound (880Hz beep)
- `src-tauri/sounds/stop.wav` - Recording stop sound (440Hz beep)
- `src-tauri/src/lib.rs` - Added module declarations and setup hooks
- `src-tauri/Cargo.toml` - Added rodio dependency
- `src-tauri/tauri.conf.json` - Added floating-bar window configuration
- `src-tauri/capabilities/default.json` - Added event and window permissions

**React Frontend:**
- `src/hooks/useRecordingState.ts` - Hook for recording state events
- `src/stores/app-store.ts` - Zustand store for global app state
- `src/windows/FloatingBar.tsx` - Transparent recording indicator component
- `src/main.tsx` - Window routing based on webview label
- `src/index.css` - Transparent background styles for floating bar

## Decisions Made

1. **Ctrl+Shift+Space as shortcut**: fn key cannot be captured on macOS (intercepted by system), so using modifier combination
2. **300ms double-tap threshold**: Standard UX threshold, may need tuning based on user feedback
3. **Embedded sounds via include_bytes!**: Compile-time embedding avoids runtime file loading issues
4. **Separate handler function**: Extracting shortcut logic to separate function avoids Rust lifetime issues with closures

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

1. **Tauri 2.x Image API change**: `Image::from_path()` doesn't exist in Tauri 2.x. Fixed by using `app.default_window_icon()` for now. Custom icon switching can be added later with proper resource embedding.
2. **Lifetime issue in closure**: Initial shortcut handler had lifetime issues with state reference. Fixed by extracting to separate `handle_shortcut_event` function.

## User Setup Required

None - no external service configuration required for this plan.

## Next Phase Readiness

- Recording state machine ready for actual audio capture
- Floating bar ready to display transcription progress
- Event system ready for transcription status updates
- Ready for Plan 01-03: Audio capture and transcription integration

---
*Phase: 01-foundation-recording*
*Completed: 2026-01-29*
