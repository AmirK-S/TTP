# Project Research Summary

**Project:** TTP (Talk To Paste)
**Domain:** Cross-platform desktop voice transcription utility
**Researched:** 2026-01-29
**Confidence:** HIGH

## Executive Summary

TTP is a menu bar/system tray voice transcription app built with Tauri that captures speech, transcribes via OpenAI Whisper API, polishes with GPT-4o-mini, and auto-pastes into any application. Research shows this is a mature, competitive domain where the bar has been raised from "basic transcription" to "AI-polished, context-aware output." Users now expect instant, clean text with filler words removed and grammar corrected. The key differentiators are the BYOK (Bring Your Own Key) model, learning dictionary, and true cross-platform parity.

The recommended approach is a Tauri 2.x app with Rust-side audio capture (cpal), async OpenAI API calls (async-openai or reqwest), and system clipboard integration with paste simulation. The architecture should be modular with clear separation between commands, services, and state. Critical risks include macOS permission handling (microphone, accessibility), global hotkey conflicts, and cross-platform build complexity. These can be mitigated by proper entitlements configuration from day one, user-configurable hotkeys, and CI/CD with native platform runners.

The research reveals that this project requires careful attention to platform-specific permissions and integration points, but the core technologies (Tauri, Whisper API, cpal) are mature and well-documented. The biggest risks are in the polish: ensuring permissions work on signed builds, handling edge cases in auto-paste across different applications, and managing the complexity of cross-platform builds.

## Key Findings

### Recommended Stack

The stack centers on Tauri 2.x for the desktop framework, providing a ~10MB binary (vs 150MB+ Electron) with excellent cross-platform support. Rust backend uses cpal for audio capture, async-openai for API integration, and arboard for clipboard management. React frontend with Zustand for state management and shadcn/ui for components keeps the UI lightweight and modern.

**Core technologies:**
- Tauri 2.9.x: Desktop framework with small binaries, native tray/menu bar APIs, official plugins for all needed features
- cpal + hound: Cross-platform audio capture and WAV encoding (alternative: tauri-plugin-mic-recorder for faster MVP)
- async-openai: Rust client for Whisper transcription and GPT-4o-mini polishing
- arboard: Clipboard management (used by 1Password, battle-tested)
- React 18 + TypeScript + Zustand: Lightweight frontend with type safety and minimal boilerplate
- shadcn/ui + Tailwind v4: Modern UI components with full customization ownership

**Critical version notes:**
- Tauri requires Rust 1.77.2+ (MSRV for plugins)
- Tauri 2.x stable as of 2.9.5 (don't use 1.x, no upgrade path)
- Use official Tauri plugins over community alternatives where available

### Expected Features

Research shows that the voice transcription app market has evolved significantly in 2025-2026. Basic transcription accuracy is now a given; users expect AI-enhanced output by default.

**Must have (table stakes):**
- Global keyboard shortcut activation (configurable)
- Push-to-talk recording mode (hold key, release to transcribe)
- Menu bar/system tray presence with recording state indicator
- Auto-paste to active field (core value proposition)
- AI polish with filler word removal and grammar correction
- Multi-language support (Whisper handles 100+ languages natively)
- BYOK API key configuration (users enter their own OpenAI key)

**Should have (competitive advantages):**
- Toggle recording mode (press to start, press to stop)
- Custom dictionary with learning from user corrections (rare feature, major differentiator)
- Course correction handling ("Tuesday, no wait, Wednesday" outputs "Wednesday")
- Shortcut customization to avoid conflicts
- True cross-platform parity (many competitors are Mac-only)

**Defer (v2+):**
- Offline fallback mode with local Whisper models (~500MB-1.6GB download, complex)
- Context-aware formatting (different output for code comments vs emails)
- Voice commands for editing ("delete that", "make it formal")
- Cross-platform settings sync

**Anti-features (deliberately NOT build):**
- Always-on voice activation (privacy nightmare, battery drain, accidental triggers)
- Subscription pricing (users prefer BYOK or one-time purchase)
- Meeting recording/transcription (different product category, feature creep)
- Real-time streaming transcription (latency issues, Whisper doesn't support it)
- Browser extensions (platform fragmentation, system-level integration works everywhere)

### Architecture Approach

The architecture follows standard Tauri patterns: React frontend communicates with Rust backend via IPC (invoke commands and events), backend manages global state with Mutex<AppState>, and integrates with system APIs (microphone, clipboard, tray) directly. The data flow is linear: hotkey trigger → audio capture → Whisper API → dictionary application → GPT polish → clipboard → paste simulation.

**Major components:**
1. Global Hotkey Handler (Rust) — Captures shortcuts even when app is unfocused, guards recording state
2. Audio Capture Manager (Rust) — Records microphone input to buffer using cpal or tauri-plugin-mic-recorder
3. API Client (Rust) — Async HTTP client for Whisper transcription and GPT-4o-mini polishing
4. Local Storage (Rust) — Persists custom dictionary, user settings, corrections using tauri-plugin-store
5. Clipboard Manager (Rust) — Writes text and simulates paste keystroke via arboard + platform APIs
6. Recording UI (React) — Minimal visual indicator, settings panel, dictionary editor
7. State Manager (Rust) — Mutex-protected AppState with RecordingState enum (Idle/Recording/Processing)

**Key architectural patterns:**
- Command-based IPC for all frontend-to-backend operations (type-safe via TypeScript bindings)
- Event-based state sync for real-time UI updates (backend emits events, frontend subscribes)
- State machine for recording lifecycle (prevents race conditions in hotkey handler)
- Modular Rust structure (commands/, services/, state/, storage/, tray/) to avoid monolithic lib.rs

**Build order implications:**
The architecture suggests starting with foundation (tray + state machine), then adding audio capture, followed by the transcription pipeline, and finally enhancement layers (polish, dictionary, paste). Each phase depends on the previous one working correctly.

### Critical Pitfalls

Research uncovered several high-impact pitfalls that have bitten real projects in this domain.

1. **macOS Microphone Permission Not Prompting** — Works in dev mode but fails on signed builds. Must add NSMicrophoneUsageDescription to Info.plist AND proper entitlements.plist from day one. Test signed builds early, not just before release.

2. **Global Hotkey Conflicts** — Chosen shortcut conflicts with popular apps (Figma, VS Code, browser dev tools). Make hotkeys user-configurable from launch, use uncommon modifier combinations by default, provide clear conflict resolution UI.

3. **Accessibility Permission Corruption on macOS** — Auto-paste breaks randomly after OS updates even when permissions appear granted. Implement permission status checking, provide "Repair Permissions" flow, detect OS version changes proactively.

4. **Cross-Platform Build Failures** — Cannot cross-compile MSI installers on macOS or vice versa. Use GitHub Actions with native platform runners from day one, never promise features based on untested cross-compiled builds.

5. **Tray Icon Disappears or Crashes App** — Known Tauri bugs on macOS; app crashes when all windows closed and menu clicked. Keep invisible window alive, intercept CloseRequested events, test extensively on both platforms, pin Tauri versions.

6. **Different Paste Behavior Across Apps** — Works in most apps but fails in Microsoft Office, terminals, password managers. Add configurable delay before paste (50-100ms default), offer "Type Text" fallback mode, build app compatibility list.

7. **Notarization Stuck or Failing** — Build hangs for hours or Apple rejects without clear errors. Use paid Apple Developer account ($99/year), set APPLE_TEAM_ID, implement timeout/retry logic, sign all sidecars explicitly, test notarization early with minimal builds.

**Common thread:** Platform-specific integration points (permissions, tray, paste) require more testing than the core business logic. The "looks done but isn't" checklist from research should be followed religiously.

## Implications for Roadmap

Based on combined research, here's the recommended phase structure with rationale:

### Phase 1: Foundation & Tray
**Rationale:** Must establish correct architecture patterns before building features. Tray behavior is fragile and needs to work from the start (Pitfall 8). Project scaffolding and CI/CD setup are prerequisites for everything else (Pitfall 5).

**Delivers:** Working Tauri app with system tray, basic Rust state structure, invisible window pattern (macOS), CI/CD with native platform runners.

**Addresses:** Foundation for all features; prevents cross-platform build failures and tray crashes.

**Avoids:** Pitfall 5 (cross-platform build failures), Pitfall 8 (tray icon issues).

**Research flag:** Standard Tauri patterns, skip phase research.

### Phase 2: Audio Capture & Permissions
**Rationale:** Cannot test transcription without working audio input. Permission handling must be correct from the start or it becomes architectural rework later (Pitfall 1, 2).

**Delivers:** Global hotkey registration, push-to-talk recording, microphone permission flow with proper entitlements, recording state machine.

**Uses:** tauri-plugin-global-shortcut, cpal (or tauri-plugin-mic-recorder for faster MVP), hound for WAV encoding.

**Implements:** Audio Capture Manager, Global Hotkey Handler components.

**Avoids:** Pitfall 1 (permission not prompting on signed builds), Pitfall 2 (WebView permission persistence by using Rust-side audio).

**Research flag:** Standard patterns, skip phase research.

### Phase 3: Transcription Pipeline
**Rationale:** Core value proposition. Once audio works, prove the concept end-to-end before adding enhancement layers. Validates API integration patterns.

**Delivers:** Whisper API transcription, basic clipboard write, end-to-end flow (record → transcribe → copy).

**Uses:** async-openai or reqwest for Whisper API, arboard for clipboard.

**Implements:** API Client component.

**Avoids:** Early validation of API patterns before complexity increases.

**Research flag:** Standard OpenAI API integration, skip phase research.

### Phase 4: AI Polish & Auto-Paste
**Rationale:** Builds on working transcription. AI polish is table stakes (FEATURES.md), auto-paste is the main UX differentiator. Auto-paste has many edge cases (Pitfall 6, 7) so needs dedicated focus.

**Delivers:** GPT-4o-mini polish with filler removal and grammar correction, simulated paste keystroke, accessibility permission handling, configurable paste delay, type-text fallback mode.

**Uses:** async-openai for GPT-4o-mini, platform-specific keystroke APIs.

**Implements:** Clipboard Manager enhancements, paste simulation.

**Avoids:** Pitfall 4 (accessibility permission corruption via status checking), Pitfall 7 (paste behavior differences via fallback modes).

**Research flag:** Needs research. Platform-specific paste simulation APIs, accessibility permission handling varies by OS, app compatibility quirks need investigation.

### Phase 5: Dictionary & Learning
**Rationale:** Major differentiator but depends on working transcription pipeline. Custom dictionary enhances both Whisper (via prompt context) and GPT polish.

**Delivers:** Custom word dictionary, dictionary persistence, pronunciation hints, learning from user corrections.

**Uses:** tauri-plugin-store for persistence, dictionary application in transcription pipeline.

**Implements:** Local Storage component, dictionary editor UI.

**Avoids:** Building enhancement layers before core works.

**Research flag:** Standard CRUD patterns, skip phase research.

### Phase 6: Settings & Configurability
**Rationale:** By this point, users are testing and will hit conflicts/preferences. Configurable hotkeys prevent Pitfall 3, settings UI enables user customization for edge cases discovered during Phase 4.

**Delivers:** Settings panel, hotkey customization, API key entry UI, recording mode toggle (push-to-talk vs toggle), paste behavior preferences.

**Uses:** React forms, Zustand state management, tauri-plugin-store.

**Implements:** Settings Panel component.

**Avoids:** Pitfall 3 (hotkey conflicts via user configuration).

**Research flag:** Standard settings UI patterns, skip phase research.

### Phase 7: Distribution & Polish
**Rationale:** Final phase addresses release blockers: notarization, installers, platform-specific fixes. Notarization should be tested early but full flow happens here (Pitfall 6).

**Delivers:** Notarized macOS builds, Windows installers (MSI/NSIS), app signing, release automation, platform-specific icon/metadata, error recovery flows.

**Uses:** CI/CD pipelines, Apple Developer account, code signing certificates.

**Addresses:** All remaining platform-specific polishing.

**Avoids:** Pitfall 6 (notarization issues via early testing and proper setup).

**Research flag:** Needs research. Notarization workflow details, installer configuration options, platform-specific distribution requirements.

### Phase Ordering Rationale

- **Foundation first:** Tray and build system issues are architectural; fixing them later is expensive.
- **Audio before transcription:** Can't test API without audio input; logical dependency.
- **Transcription before polish:** Proves concept with minimal complexity; validates approach.
- **Polish before dictionary:** Dictionary enhances polish, not the reverse; logical dependency.
- **Settings after features:** Need working features to know what settings are needed.
- **Distribution last:** Can't test distribution until features work; natural endpoint.

**Dependency chain:** Foundation → Audio → Transcription → Polish → Dictionary → Settings → Distribution

**Pitfall prevention integrated:** Each phase explicitly addresses pitfalls discovered in research. Phase 2 handles permission pitfalls, Phase 4 handles paste pitfalls, Phase 7 handles distribution pitfalls.

### Research Flags

**Phases needing deeper research during planning:**
- **Phase 4 (Auto-Paste):** Platform-specific paste simulation APIs, accessibility permission workflows, app compatibility workarounds. Sparse documentation for edge cases.
- **Phase 7 (Distribution):** Notarization workflow details, Windows installer options (MSI vs NSIS), code signing certificate management. Lots of tribal knowledge, not well-documented.

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Foundation):** Well-documented Tauri setup, standard CI/CD patterns.
- **Phase 2 (Audio):** cpal is mature, standard Rust audio capture patterns.
- **Phase 3 (Transcription):** OpenAI API is well-documented, standard HTTP client usage.
- **Phase 5 (Dictionary):** Standard CRUD operations, basic key-value storage.
- **Phase 6 (Settings):** Standard React forms and state management.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All core technologies verified via official docs and crates.io. Tauri 2.x is stable and production-ready. |
| Features | HIGH | Analyzed 4 major competitors (WisprFlow, MacWhisper, Handy, Superwhisper) and market research from multiple sources. Clear patterns emerged. |
| Architecture | HIGH | Official Tauri architecture docs and verified reference implementations (Handy open source). Patterns are well-established. |
| Pitfalls | HIGH | Verified via GitHub issues, official Tauri documentation, and community resources. All pitfalls have real-world precedent. |

**Overall confidence:** HIGH

Research drew heavily from official documentation (Tauri v2, OpenAI API), verified open source implementations, and documented GitHub issues. The domain is mature enough that standard patterns exist, but niche enough that platform-specific edge cases require careful attention.

### Gaps to Address

Research was comprehensive, but a few areas need validation during implementation:

- **async-openai stability:** The crate is well-maintained but unofficial. May need fallback to raw reqwest if issues arise. Validate during Phase 3.
- **Paste simulation on Windows:** Less research available on Windows keystroke simulation compared to macOS accessibility APIs. Validate during Phase 4.
- **Dictionary learning effectiveness:** No competitors have publicly documented their learning algorithms. Will need experimentation to find effective patterns. Iterate during Phase 5.
- **Radix UI maintenance:** shadcn/ui uses Radix primitives; some maintenance concerns in community. Monitor during development; React Aria is fallback. Not urgent.

**Handling strategy:** These gaps are known unknowns. Each has a clear validation point (phase) and fallback option. None are blockers.

## Sources

### Primary (HIGH confidence)
- Tauri 2.0 Official Documentation (v2.tauri.app) — architecture, IPC, plugins, system tray, permissions
- Tauri GitHub (github.com/tauri-apps/tauri) — releases, issues, plugin workspace
- OpenAI API Reference (platform.openai.com) — Whisper transcription, GPT-4o-mini
- cpal (github.com/RustAudio/cpal), hound (github.com/ruuda/hound), arboard (github.com/1Password/arboard) — audio capture and clipboard
- async-openai (github.com/64bit/async-openai) — Rust OpenAI client
- Zustand (npmjs.com/package/zustand), shadcn/ui (ui.shadcn.com) — React state and UI

### Secondary (MEDIUM confidence)
- WisprFlow, MacWhisper, Handy, Superwhisper reviews and documentation — feature expectations
- Zapier, TechCrunch, Voicy market analysis — competitive landscape
- tauri-plugin-mic-recorder (github.com/ayangweb/tauri-plugin-mic-recorder) — alternative audio approach
- GitHub issues for specific Tauri bugs — permission dialogs, tray icons, notarization
- Community posts on React UI libraries and state management — shadcn/ui vs alternatives

### Tertiary (LOW confidence)
- Exact version numbers for some crates (validated against crates.io at project start)
- App-specific paste behavior quirks (will need real-world testing)
- Radix UI long-term maintenance trajectory (monitor during development)

---
*Research completed: 2026-01-29*
*Ready for roadmap: yes*
