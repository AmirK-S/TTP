# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-20)

**Core value:** One shortcut to turn speech into text, anywhere.
**Current focus:** v1.3.0 User Experience Polish

## Current Position

Phase: 26-onboarding-settings-ux
Plan: 02 completed
Status: In progress
Last activity: 2026-02-20 — Plan 26-02 completed

Progress (v1.3.0): [████████░░░░░░░░░░░░░░░░░░░░░] 40% (2/5 plans)

## Performance Metrics

**Velocity:**
- v0.2.3: 6 phases, 16 plans
- v0.3.0: 1 phase, 2 plans
- v0.4.0: 3 phases, 6 plans
- v0.5.0: 4 phases, 7 plans
- v0.6.0: 5 phases, 12 plans
- v1.2.0: 4 phases, 10 plans
- v1.3.0: 2 phases (planned)
- Total completed: 24 phases, 53 plans

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full log.

v1.2.0 decisions (from research):
- Sentry must init BEFORE tauri::Builder in run() -- ClientInitGuard must outlive the app
- Aptabase is the only analytics platform with an official Tauri 2 plugin
- Consent is opt-in with default OFF (PRIV-01) -- consent must gate Sentry/Aptabase init
- Stack: sentry 0.42, tauri-plugin-sentry 0.5, tauri-plugin-aptabase 1.0
- PII scrubbing via before_send hooks on both Rust and JS sides
- Auto-updater signing config already exists in tauri.conf.json -- needs keypair + CI secrets + frontend UI

v1.2.0 decisions (from Phase 21 execution):
- Hardcoded real Sentry DSN in consent.rs (EU region: de.sentry.io)
- OnceLock for regex compilation (Rust 1.93, no external crate needed)
- Removed stale OpenAI/Gladia URLs from capabilities during Sentry URL addition
- Used relaunch() from @tauri-apps/plugin-process for restart button
- Minidump init gated on telemetry_active to prevent dual-process duplicate app

v1.2.0 decisions (from Phase 22-01 execution):
- Aptabase plugin consent-gated via conditional registration (same pattern as Sentry)
- Switched from Builder.run() to Builder.build().run() for lifecycle event access
- Scoped analytics blocks with local use statements for clean no-op when plugin absent
- Placeholder app key A-EU-0000000000 -- user replaces with real key
- Error categories: too_short, too_long, no_speech, api_error, network

v1.2.0 decisions (from Phase 22-02 execution):
- Safe JS analytics wrapper with try/catch silently ignores errors when Aptabase plugin not registered
- Props type Record<string, string | number> matches Aptabase SDK (no boolean allowed)
- Non-blocking getVersion().then() for update event version props
- API key changes explicitly excluded from analytics tracking (privacy)
- Replaced hardcoded v0.2.3 in Settings with dynamic getVersion() from Tauri API

v1.2.0 decisions (from Phase 23-01 execution):
- Sync std::thread::sleep retry loop for Windows remove_backup (not tokio, function is sync)
- Sentry breadcrumb on backup creation for crash report traceability
- let Ok(...) = ... else pattern for cleanup_stale_backups (never panics or fails startup)

v1.2.0 decisions (from Phase 23-02 execution):
- Removed cleanup closure, inlined cleanup at each call site to include backup removal
- backup_audio failure is non-blocking (logs warning, continues with None backup_path)
- Transcription error handler preserves audio on disk (no remove_file on API failure)

v1.2.0 decisions (from Phase 24-01 execution):
- autoCheck option (default false) prevents duplicate intervals from multiple hook instances
- pendingUpdateRef stores Update object to eliminate double check() in downloadAndInstall
- Dynamic import for WebviewWindow/emit in App.tsx to avoid loading until update found
- Dismissed state resets when new version discovered (tracked via lastFoundVersionRef)

v1.2.0 decisions (from Phase 24-02 execution):
- Dual event listeners in Settings.tsx: Settings listens for scroll, UpdateSection listens for auto-check trigger
- Later button calls dismiss() from useUpdater hook (dismissed flag resets on new version)
- Read-only verification for signing infrastructure (no files modified, all 4 checks PASS)

v1.3.0 decisions (from research):
- FN key default on macOS requires checking `fn_key_enabled` before calling `start_fn_key_monitor()`
- Toggle mode requires state machine changes in shortcuts.rs
- Pill visibility requires `should_show_pill()` method that respects setting
- Settings/Rust state desync: Emit events after settings changes to sync all windows
- Permission onboarding: Implement polling/re-check mechanism for denied permissions
- Window focus: Call `show()` before `set_focus()` for auto-opened settings

v1.3.0 decisions (from Phase 25-01 execution):
- Settings persist across app restarts via JSON file (~/.config/ttp/settings.json)
- Settings-changed event emission for cross-window sync

v1.3.0 decisions (from Phase 25-02 execution):
- Hands-free mode loads from settings on app startup
- Mode preference persists via settings when toggled via double-tap
- FN key is default shortcut on macOS (cfg-specific defaults)

v1.3.0 decisions (from Phase 26-01 execution):
- Show onboarding before setup window on first launch
- Use system_profiler and TCC database for macOS permission detection

### Pending Todos

- [x] **[URGENT] Fix long audio transcription failure on Windows** -- RESOLVED in Phase 23 (AUDI-01..05)
- [x] Phase 25-01: Settings for hands_free_mode and hide_pill_when_inactive -- COMPLETED
- [x] Phase 25-02: Load persisted mode, FN default -- COMPLETED
- [x] Phase 25-03: Pill visibility based on settings -- COMPLETED
- [x] Phase 25-04: Settings event sync -- COMPLETED
- [x] Phase 26-01: Onboarding flow with permission check -- COMPLETED
- [x] Phase 26-02: Settings UX improvements -- COMPLETED

### Blockers/Concerns

- Windows long audio data loss bug at pipeline.rs line 195 -- RESOLVED in Phase 23
- Aptabase exact version compatibility needs verification during Phase 22 -- RESOLVED in 22-01
- Sentry before_send Rust API lifetime annotations -- RESOLVED in 21-01
- Settings/Rust state desync (UX-3) -- RESOLVED in Phase 25-04
- Permission re-check flow (UX-2) -- RESOLVED in Phase 26-01

## Session Continuity

Last session: 2026-02-20
Stopped at: Completed 26-02-PLAN.md (settings UX)
Next action: Continue with remaining Phase 26 plans

---

*State updated: 2026-02-20*
