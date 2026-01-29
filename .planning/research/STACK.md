# Stack Research: Tauri Voice Transcription App

**Domain:** Cross-platform desktop voice transcription utility (macOS + Windows)
**Researched:** 2025-01-29
**Confidence:** MEDIUM-HIGH

---

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| Tauri | 2.9.x | Desktop app framework | Stable 2.0 release with excellent cross-platform support, small binaries (~10MB vs 150MB+ Electron), native system tray/menu bar APIs. Active development with 2.9.5 latest. | HIGH |
| React | 18.x | Frontend UI | Mature ecosystem, excellent TypeScript support, huge component library availability. Works perfectly with Tauri's webview. | HIGH |
| TypeScript | 5.x | Type safety | Industry standard, catches errors at compile time, better DX with IDE support. | HIGH |
| Rust | 1.77.2+ | Backend logic | Required by Tauri. Memory-safe, excellent async support via Tokio. Tauri plugins require 1.77.2 minimum. | HIGH |

### Rust Backend Crates

| Crate | Version | Purpose | Why Recommended | Confidence |
|-------|---------|---------|-----------------|------------|
| `tauri` | 2.9.x | Core framework | Official Tauri framework. | HIGH |
| `tauri-plugin-global-shortcut` | 2.x | Keyboard hotkeys | Official Tauri plugin for global keyboard shortcuts. Supports push-to-talk via `CommandOrControl+Shift+Key` patterns. | HIGH |
| `tauri-plugin-positioner` | 2.3.0 | Window positioning | Official plugin for tray-relative window positioning. Essential for menu bar apps. | HIGH |
| `tauri-plugin-store` | 2.x | Persistent settings | Official plugin for key-value storage. Stores custom dictionary, user preferences, API keys. | HIGH |
| `cpal` | 0.16.0 | Audio capture | De-facto standard for cross-platform audio I/O in Rust. 8.7M+ downloads. Low-level access to microphone input streams. | HIGH |
| `hound` | 3.5.1 | WAV encoding | Standard WAV encoder/decoder. 7.5M+ downloads. Needed to encode audio for Whisper API (supports WAV input). | HIGH |
| `async-openai` | latest | OpenAI API client | Unofficial but well-maintained Rust client for OpenAI APIs. Supports Whisper transcription, GPT-4o-mini for polishing. Async-first design. | MEDIUM |
| `reqwest` | 0.13.x | HTTP client | Ergonomic HTTP client with async support. Fallback if async-openai doesn't meet needs. | HIGH |
| `tokio` | 1.x | Async runtime | Standard async runtime for Rust. Required by async-openai and reqwest. | HIGH |
| `arboard` | 3.x | Clipboard | Cross-platform clipboard library by 1Password. Supports text copy/paste on macOS, Windows, Linux. Handles system clipboard synchronization. | HIGH |
| `serde` | 1.x | Serialization | Standard serialization framework for config files, API payloads. | HIGH |
| `serde_json` | 1.x | JSON support | JSON serialization for OpenAI API, settings storage. | HIGH |

### Alternative: Tauri Mic Recorder Plugin

| Crate | Version | Purpose | When to Use | Confidence |
|-------|---------|---------|-------------|------------|
| `tauri-plugin-mic-recorder` | 2.0.0 | Audio recording | Higher-level alternative to cpal+hound. Uses cpal+hound internally. Simpler API but less control. Only supports desktop (no iOS/Android). | MEDIUM |

**Recommendation:** Start with `tauri-plugin-mic-recorder` for faster MVP, migrate to raw `cpal+hound` if you need streaming audio or fine-grained control.

### React Frontend Libraries

| Library | Version | Purpose | Why Recommended | Confidence |
|---------|---------|---------|-----------------|------------|
| `@tauri-apps/api` | 2.x | Tauri bindings | Official JS bindings for Tauri APIs. | HIGH |
| `@tauri-apps/plugin-global-shortcut` | 2.x | Hotkey bindings | JS bindings for global shortcut plugin. | HIGH |
| `@tauri-apps/plugin-positioner` | 2.x | Window positioning | JS bindings for positioner plugin. | HIGH |
| `@tauri-apps/plugin-store` | 2.x | Settings storage | JS bindings for persistent store. | HIGH |
| Zustand | 5.0.x | State management | Lightweight, hook-first state management. Excellent for Tauri apps (works well with multi-window sync). Simpler than Redux, more capable than Context. | HIGH |
| shadcn/ui | latest | UI components | Copy-paste component library built on Radix UI + Tailwind CSS. Full customization ownership. TypeScript-first. | MEDIUM |
| Tailwind CSS | 4.x | Styling | Utility-first CSS framework. shadcn/ui now supports v4 with OKLCH colors. | HIGH |
| Lucide React | latest | Icons | Modern icon library, works seamlessly with shadcn/ui. | HIGH |
| Sonner | latest | Toast notifications | Recommended toast library (shadcn/ui deprecated their own toast in favor of Sonner). | HIGH |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| Vite | Build tool | Fast dev server, works great with Tauri. Use `create-tauri-app` with Vite template. |
| ESLint | Linting | TypeScript-aware linting. |
| Prettier | Formatting | Code formatting. |
| `cargo-watch` | Rust dev | Auto-rebuild on Rust file changes. |
| `tauri-cli` | Tauri CLI | Build, dev, and bundle commands. Latest: 2.9.6. |

---

## Installation

### Rust Dependencies (Cargo.toml)

```toml
[dependencies]
tauri = { version = "2.9", features = ["tray-icon"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }
tauri-plugin-store = "2"
cpal = "0.16"
hound = "3.5"
async-openai = "0.27"  # Check crates.io for latest
reqwest = { version = "0.13", features = ["json"] }
tokio = { version = "1", features = ["full"] }
arboard = "3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Alternative higher-level mic recording
# tauri-plugin-mic-recorder = "2"
```

### JavaScript Dependencies

```bash
# Core Tauri
npm install @tauri-apps/api @tauri-apps/plugin-global-shortcut @tauri-apps/plugin-positioner @tauri-apps/plugin-store

# State Management
npm install zustand

# UI
npx shadcn@latest init  # Follow prompts for Tailwind v4
npm install lucide-react sonner

# Dev
npm install -D typescript @types/react @types/react-dom
npm install -D tailwindcss postcss autoprefixer
npm install -D eslint prettier
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Tauri 2.x | Electron | Never for this project. Electron bundles are 150MB+, Tauri ~10MB. Electron uses more memory. |
| Tauri 2.x | Tauri 1.x | Never. Tauri 2.x is stable, has better plugin system, mobile support path. |
| cpal + hound | tauri-plugin-mic-recorder | For faster MVP. Less control over audio streaming. |
| async-openai | reqwest (manual) | If async-openai has issues or you need custom API handling. |
| Zustand | Jotai | If you have highly atomic, interdependent state (unlikely for this app). |
| Zustand | Redux Toolkit | If you have a large team or need Redux DevTools ecosystem. Overkill for this app. |
| shadcn/ui | Mantine | If you want pre-built components without copy-paste. Less customizable. |
| shadcn/ui | React Aria | If Radix UI maintenance concerns become critical. More complex API. |
| arboard | copypasta | If you're building a terminal app (not applicable). |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Electron | Massive bundle size (150MB+), high memory usage, unnecessary complexity for a utility app. | Tauri 2.x |
| Tauri 1.x | Deprecated. No mobile path, older plugin system, fewer features. | Tauri 2.x |
| rodio | Audio playback library, not for capture. Doesn't provide microphone input. | cpal (for capture) |
| rust-clipboard | Unmaintained, no Wayland support. | arboard |
| Redux | Overkill for a small utility app. Boilerplate heavy. | Zustand |
| React Context (alone) | Not designed for complex state. No persistence, no devtools. | Zustand |
| MUI (Material UI) | Heavy bundle, opinionated Material design not suitable for native-feeling utility. | shadcn/ui + Tailwind |
| Chakra UI | Similar concerns to MUI, plus maintenance has slowed. | shadcn/ui + Tailwind |
| whisper-rs / whisper-burn | Local Whisper inference. Huge model downloads, slower than API, complex setup. Use only if offline support is required. | OpenAI Whisper API via async-openai |

---

## Stack Patterns by Use Case

### Push-to-Talk Recording

```
User presses hotkey → tauri-plugin-global-shortcut triggers Rust command
→ cpal opens input stream → Audio samples collected
→ User releases hotkey → hound encodes WAV
→ async-openai sends to Whisper API → Transcription returned
→ async-openai sends to GPT-4o-mini → Polished text
→ arboard copies to clipboard → (optional) paste via platform API
```

### Toggle Recording

```
Same flow, but hotkey toggles recording state stored in Zustand/Rust state.
Visual indicator in tray icon shows recording status.
```

### Custom Dictionary Learning

```
User corrects transcription → Correction stored via tauri-plugin-store
→ Corrections become part of Whisper API prompt context
→ GPT-4o-mini uses corrections for polishing
```

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| Tauri 2.9.x | Rust 1.77.2+ | MSRV for Tauri plugins is 1.77.2 |
| cpal 0.16.x | Rust stable | WASM requires Rust 1.82+ (not needed for desktop) |
| async-openai | Tokio 1.x | Requires Tokio runtime |
| shadcn/ui (2025) | Tailwind v4, React 18+ | tw-animate-css replaces tailwindcss-animate |
| Zustand 5.x | React 18+ | Uses useSyncExternalStore |
| @tanstack/react-query 5.x | React 18+ | Optional, for API caching |

---

## Platform-Specific Notes

### macOS

- **Code signing required for microphone access.** Unsigned apps won't trigger permission dialogs.
- Add `NSMicrophoneUsageDescription` to `Info.plist`.
- For menu bar app: set activation policy to "accessory" to hide dock icon.
- System tray appears in top-right menu bar.

### Windows

- Use `.skip_taskbar(true)` to hide taskbar icon for tray-only apps.
- System tray appears in bottom-right notification area.
- No special microphone permissions needed at build time.

### Linux

- Requires ALSA dev files: `libasound2-dev` (Debian/Ubuntu) or `alsa-lib-devel` (Fedora).
- Tray `Leave` event unsupported on Linux.
- Consider both X11 and Wayland clipboard paths (arboard handles this).

---

## Open Questions / Future Research

1. **Streaming transcription:** Can we stream audio chunks to Whisper API for faster response? (OpenAI doesn't support real-time streaming for Whisper as of 2025 knowledge)

2. **Local fallback:** Should we support whisper.cpp/whisper-rs for offline mode? Adds significant complexity and model size.

3. **Radix UI maintenance:** Monitor shadcn/ui's stance on Radix. They may migrate to React Aria or Base UI if Radix maintenance continues to lag.

4. **Mobile support:** Tauri 2.x supports iOS/Android, but `tauri-plugin-mic-recorder` does not. Would need native plugins for mobile audio capture.

---

## Sources

### HIGH Confidence (Official Documentation)
- [Tauri 2.0 Official Documentation](https://v2.tauri.app/)
- [Tauri System Tray Guide](https://v2.tauri.app/learn/system-tray/)
- [Tauri Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [Tauri Positioner Plugin](https://v2.tauri.app/plugin/positioner/)
- [Tauri Store Plugin](https://v2.tauri.app/plugin/store/)
- [Tauri GitHub Releases](https://github.com/tauri-apps/tauri/releases) - v2.9.5 confirmed
- [cpal GitHub](https://github.com/RustAudio/cpal) - v0.16.0
- [hound GitHub](https://github.com/ruuda/hound) - v3.5.1
- [arboard GitHub](https://github.com/1Password/arboard)
- [async-openai GitHub](https://github.com/64bit/async-openai)
- [Zustand npm](https://www.npmjs.com/package/zustand) - v5.0.10
- [shadcn/ui Tailwind v4 docs](https://ui.shadcn.com/docs/tailwind-v4)
- [TanStack React Query npm](https://www.npmjs.com/package/@tanstack/react-query) - v5.90.x

### MEDIUM Confidence (Community/Verified)
- [tauri-plugin-mic-recorder GitHub](https://github.com/ayangweb/tauri-plugin-mic-recorder) - v2.0.0, uses cpal+hound
- [React UI libraries comparison 2025](https://makersden.io/blog/react-ui-libs-2025-comparing-shadcn-radix-mantine-mui-chakra)
- [State Management Trends 2025](https://makersden.io/blog/react-state-management-in-2025)
- [Radix UI maintenance concerns](https://dev.to/mashuktamim/is-your-shadcn-ui-project-at-risk-a-deep-dive-into-radixs-future-45ei)

### LOW Confidence (Requires Validation)
- Exact async-openai version (check crates.io at project start)
- Tokio exact version (check crates.io at project start)
- reqwest exact version (0.13.1 as of research date)

---

*Stack research for: TTP (Talk To Paste) - Cross-platform voice transcription utility*
*Researched: 2025-01-29*
