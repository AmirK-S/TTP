---
phase: 04-platform-parity
plan: 01
subsystem: shortcuts, paste, settings
tags: [enigo, shortcuts, cross-platform, keyboard-simulation]

dependency-graph:
  requires: [02-02, 03-02]
  provides: [cross-platform-paste, dynamic-shortcuts, shortcut-settings-ui]
  affects: []

tech-stack:
  added: []
  patterns: [conditional-compilation, dynamic-shortcut-registration]

key-files:
  created: []
  modified:
    - src-tauri/src/paste/simulate.rs
    - src-tauri/src/shortcuts.rs
    - src-tauri/src/settings/store.rs
    - src-tauri/src/lib.rs
    - src/stores/settings-store.ts
    - src/windows/Settings.tsx

decisions: [enigo-for-cross-platform-paste, dynamic-shortcut-unregister-reregister]

metrics:
  duration: 8 min
  completed: 2026-01-29
---

# Phase 4 Plan 1: Cross-platform Paste and Customizable Shortcuts Summary

**One-liner:** Cross-platform paste using enigo (Meta+V macOS, Ctrl+V Windows) with runtime-changeable keyboard shortcuts persisted in settings.

## What Was Built

### Task 1: Cross-platform Paste Simulation
Replaced AppleScript-based paste with enigo library for cross-platform support:

```rust
// src-tauri/src/paste/simulate.rs
#[cfg(target_os = "macos")]
{
    enigo.key(Key::Meta, Press)?;
    enigo.key(Key::Unicode('v'), Click)?;
    enigo.key(Key::Meta, Release)?;
}

#[cfg(target_os = "windows")]
{
    enigo.key(Key::Control, Press)?;
    enigo.key(Key::Unicode('v'), Click)?;
    enigo.key(Key::Control, Release)?;
}
```

Note: On macOS, first use triggers Accessibility permission dialog (standard OS behavior).

### Task 2: Dynamic Shortcut Registration
Added shortcut field to Settings and implemented runtime shortcut changes:

**Settings struct:**
```rust
pub struct Settings {
    pub ai_polish_enabled: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,  // Default: "Alt+Space"
}
```

**Dynamic registration:**
- `setup_shortcuts()` now loads shortcut from settings on startup
- `update_shortcut()` function unregisters old shortcut and registers new one
- Exported as `update_shortcut_cmd` Tauri command

### Task 3: Shortcut Customization UI
Added Keyboard Shortcut section to Settings window:
- Text input for shortcut string
- Apply button that calls `update_shortcut_cmd`
- Success/error feedback messages
- Format hint: "Alt+Space, Ctrl+Shift+R, CmdOrCtrl+Space"

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| enigo for cross-platform paste | Already in dependencies, provides unified API for macOS/Windows |
| Unregister-all before new registration | Simpler than tracking old shortcut; only one shortcut used |
| Text input for shortcut | Simple approach per plan; no fancy keyboard capture needed |

## Commits

| Hash | Description |
|------|-------------|
| 6f774fe | feat(04-01): replace AppleScript paste with cross-platform enigo |
| 8f21b3a | feat(04-01): add customizable keyboard shortcut with dynamic registration |
| 183e307 | feat(04-01): add shortcut customization UI to Settings window |

## Verification Results

- [x] `cargo check` passes (only pre-existing warnings)
- [x] `npm run build` succeeds
- [x] Settings struct has shortcut field with default "Alt+Space"
- [x] setup_shortcuts() loads from settings via get_settings()
- [x] update_shortcut() enables runtime changes
- [x] lib.rs exports update_shortcut_cmd
- [x] Settings UI shows shortcut customization section

## Deviations from Plan

None - plan executed exactly as written.

## Requirements Addressed

- **CFG-04:** Customizable keyboard shortcut (user can change global shortcut in settings)
- **PLT-03 partial:** Cross-platform paste simulation (Windows now uses Ctrl+V via enigo)

## Next Phase Readiness

Ready for 04-02 (CI/CD and Distribution):
- Core platform parity for paste is complete
- Settings infrastructure handles cross-platform config storage
- No blockers for distribution work
