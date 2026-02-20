# Roadmap: TTP (Talk To Paste)

## Milestones

- ✅ **v0.2.3 MVP** — Phases 1-6 (shipped 2026-02-08)
- ✅ **v0.3.0 Code Cleanup** — Phase 7 (shipped 2026-02-08)
- ✅ **v0.4.0 Landing Page** — Phases 8-10 (shipped 2026-02-09)
- ✅ **v0.5.0 World-Class Redesign** — Phases 11-14 (shipped 2026-02-09)
- ✅ **v0.6.0 Premium Visual Redesign** — Phases 15-20 (shipped 2026-02-10)
- ✅ **v1.2.0 Production Hardening** — Phases 21-24 (shipped 2026-02-20)
- [ ] **v1.3.0 User Experience Polish** — Phases 25-26

## Phases

<details>
<summary>v0.2.3 MVP (Phases 1-6) — SHIPPED 2026-02-08</summary>

- [x] Phase 1: Foundation + Recording (3/3 plans) — completed 2026-01-29
- [x] Phase 2: Transcription Pipeline (3/3 plans) — completed 2026-01-29
- [x] Phase 3: Learning + Settings (3/3 plans) — completed 2026-01-29
- [x] Phase 4: Platform Parity (2/2 plans) — completed 2026-01-30
- [x] Phase 5: Ensemble Transcription (3/3 plans) — completed 2026-01-30
- [x] Phase 6: Auto-Update (2/2 plans) — completed 2026-02-08

</details>

<details>
<summary>v0.3.0 Code Cleanup (Phase 7) — SHIPPED 2026-02-08</summary>

- [x] Phase 7: Code Cleanup (2/2 plans) — completed 2026-02-08

</details>

<details>
<summary>v0.4.0 Landing Page (Phases 8-10) — SHIPPED 2026-02-09</summary>

- [x] Phase 8: Project Scaffolding (2/2 plans) — completed 2026-02-09
- [x] Phase 9: Page Content (2/2 plans) — completed 2026-02-09
- [x] Phase 10: Deployment (2/2 plans) — completed 2026-02-09

</details>

<details>
<summary>v0.5.0 World-Class Redesign (Phases 11-14) — SHIPPED 2026-02-09</summary>

- [x] Phase 11: Internationalization (2/2 plans) — completed 2026-02-09
- [x] Phase 12: Build-Time Data (1/1 plan) — completed 2026-02-09
- [x] Phase 13: Portfolio (2/2 plans) — completed 2026-02-09
- [x] Phase 14: UX & Design Polish (2/2 plans) — completed 2026-02-09

</details>

<details>
<summary>v0.6.0 Premium Visual Redesign (Phases 15-20) — SHIPPED 2026-02-10</summary>

- [x] Phase 15: Typography & Layout Foundation (3/3 plans) — completed 2026-02-09
- [x] Phase 16: Animation Infrastructure (2/2 plans) — completed 2026-02-09
- [x] Phase 17: Hero & Portfolio Redesign (2/2 plans) — completed 2026-02-09
- [x] Phase 18: Section Scroll Animations (2/2 plans) — completed 2026-02-10
- [x] Phase 19: Visual Polish & Performance (2/2 plans) — completed 2026-02-10
- [x] Phase 20: Full-Page Scroll Snapping (1/1 plan) — completed 2026-02-10

</details>

<details>
<summary>v1.2.0 Production Hardening (Phases 21-24) — SHIPPED 2026-02-20</summary>

- [x] Phase 21: Error Telemetry & Consent (3/3 plans) — completed 2026-02-14
- [x] Phase 22: Usage Analytics (2/2 plans) — completed 2026-02-15
- [x] Phase 23: Audio Reliability (2/2 plans) — completed 2026-02-15
- [x] Phase 24: Auto-Updater Signing (2/2 plans) — completed 2026-02-15

</details>

<details>
<summary>v1.3.0 User Experience Polish (Phases 25-26)</summary>

- [x] **Phase 25-01: Settings for hands_free_mode and hide_pill_when_inactive** — completed 2026-02-20
- [x] **Phase 25: Shortcuts, Modes & Pill Visibility** — In progress (1/2 plans) (completed 2026-02-20)
- [ ] **Phase 26: Onboarding & Settings UX** — Not started

</details>

---

## Phase Details

### Phase 25: Shortcuts, Modes & Pill Visibility

**Goal**: Users can configure keyboard shortcuts, recording modes, and pill visibility settings

**Depends on**: Nothing (first phase of v1.3.0)

**Requirements**: SHRT-01, SHRT-02, SHRT-03, SHRT-04, PILL-01, PILL-02, PILL-03, PILL-04

**Success Criteria** (what must be TRUE):

1. User sees FN key as default shortcut on first macOS launch (not Option+Space)
2. User can toggle between push-to-talk and toggle mode in settings
3. Double-tap FN key switches between push-to-talk and toggle mode
4. Recording mode preference persists across app restarts
5. User can enable "hide pill when inactive" toggle in settings
6. Pill hides when recording is idle and setting is enabled
7. Pill shows during active recording regardless of hide setting
8. Pill shows when error occurs regardless of hide setting

**Plans**: 4 plans

- [x] 25-01-PLAN.md — Settings fields + UI (hands_free_mode, hide_pill_when_inactive)
- [x] 25-02-PLAN.md — Mode persistence + FN default
- [x] 25-03-PLAN.md — Double-tap mode switching + pill visibility
- [x] 25-04-PLAN.md — Settings change events for sync

---

### Phase 26: Onboarding & Settings UX

**Goal**: Users see guided onboarding on first launch, settings opens automatically after API key entry

**Depends on**: Phase 25

**Requirements**: ONBR-01, ONBR-02, ONBR-03, ONBR-04, SETX-01, SETX-02, SETX-03

**Success Criteria** (what must be TRUE):

1. On first launch, app displays onboarding flow for permissions
2. Onboarding checks and displays microphone permission status
3. Onboarding shows guidance for enabling permissions in System Settings
4. User can re-check permissions from settings after initial denial
5. Settings window opens automatically after API key is saved
6. Settings window appears in foreground (not behind other windows)
7. Tutorial pill shows correct shortcut (FN) on first launch

**Plans**: TBD

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 25. Shortcuts, Modes & Pill Visibility | 0/4 | Complete    | 2026-02-20 |
| 26. Onboarding & Settings UX | 0/0 | Not started | - |

---

*Roadmap created: 2026-02-20*
*Last updated: 2026-02-20 — v1.3.0 roadmap defined*
