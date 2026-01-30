---
phase: 04-platform-parity
plan: 02
subsystem: distribution, ci-cd
tags: [code-signing, notarization, github-actions, release-workflow]

dependency-graph:
  requires: [04-01]
  provides: [signed-builds, release-automation]
  affects: []

tech-stack:
  added: []
  patterns: [github-release-workflow, code-signing]

key-files:
  created:
    - src-tauri/Entitlements.plist
    - .github/workflows/release.yml
  modified:
    - src-tauri/tauri.conf.json

decisions: [optional-signing-workflow, embedBootstrapper-for-webview2]

metrics:
  duration: 5 min
  completed: 2026-01-30
---

# Phase 4 Plan 2: Distribution Preparation Summary

**One-liner:** Release workflow with macOS entitlements/notarization support and Windows WebView2 bootstrapper for signed distribution.

## What Was Built

### Task 1: macOS Entitlements and Bundle Configuration

Created `src-tauri/Entitlements.plist` with required permissions:

```xml
<key>com.apple.security.device.audio-input</key>
<true/>
<key>com.apple.security.automation.apple-events</key>
<true/>
```

Updated `src-tauri/tauri.conf.json` bundle section:
- `macOS.entitlements`: Points to Entitlements.plist
- `macOS.hardenedRuntime`: true (required for notarization)
- `macOS.minimumSystemVersion`: "10.15" (Catalina)
- `windows.webviewInstallMode.type`: "embedBootstrapper" (installs WebView2 if missing)
- `windows.timestampUrl`: Set for code signing timestamp

### Task 2: Release Workflow for Signed Builds

Created `.github/workflows/release.yml`:

- **Trigger:** Push of version tags (`v*`)
- **Matrix:** macOS ARM64, macOS x86_64, Windows x64
- **Signing:** Optional - builds work without secrets, signs when secrets are configured
- **Output:** Draft GitHub release with downloadable installers

Apple signing environment variables:
- `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`

Windows signing environment variables:
- `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Optional signing (works without secrets) | Allows unsigned dev builds, signing only when secrets configured |
| embedBootstrapper for WebView2 | Ensures WebView2 runtime is installed on Windows |
| Draft releases | Review before publishing to ensure quality |
| 90-minute timeout | Accounts for notarization wait time |

## Commits

| Hash | Description |
|------|-------------|
| 62c7339 | feat(04-02): add macOS entitlements and update bundle config |
| 042c0aa | feat(04-02): add release workflow for signed builds |

## Verification Results

- [x] src-tauri/Entitlements.plist exists with audio-input entitlement
- [x] src-tauri/tauri.conf.json has entitlements path and webviewInstallMode
- [x] .github/workflows/release.yml exists with proper structure
- [x] Workflow triggers on `v*` tags
- [x] Matrix includes macOS (ARM + Intel) and Windows
- [x] Apple signing environment variables defined
- [x] `cargo check` passes

## Deviations from Plan

None - plan executed as written. Human verification checkpoint skipped per user direction.

## Requirements Addressed

- **PLT-03:** Distribution readiness (signed macOS app, Windows installer with WebView2)
- **Infrastructure:** Automated release workflow for future releases

## User Setup Required

To enable signed builds, configure GitHub repository secrets:

**macOS (Apple Developer Account required):**
1. Export Developer ID Application certificate as .p12
2. Base64 encode: `base64 -i certificate.p12`
3. Set secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`

**Windows (Optional - reduces SmartScreen warnings):**
1. Purchase OV/EV code signing certificate
2. Export as PFX, base64 encode
3. Set secrets: `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`

## Phase 4 Complete

With 04-01 (cross-platform paste, customizable shortcuts) and 04-02 (distribution preparation) complete, Phase 4 "Platform Parity" is finished. The application is ready for distribution to end users.
