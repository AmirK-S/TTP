# Phase 1: Foundation + Recording - Research

**Researched:** 2026-01-29
**Domain:** Tauri 2 menu bar/tray app with global shortcuts, audio recording, and secure credential storage
**Confidence:** HIGH

## Summary

Phase 1 establishes the core foundation for a cross-platform voice transcription utility using Tauri 2. The phase covers three main technical domains: (1) system tray/menu bar app architecture, (2) global keyboard shortcuts with push-to-talk and toggle modes, and (3) audio capture with secure API key storage.

The standard approach uses Tauri 2.9.x with React/TypeScript frontend and Rust backend. Key plugins include `tauri-plugin-global-shortcut` for keyboard shortcuts (supports both key press and release detection via `ShortcutState`), `tauri-plugin-mic-recorder` for audio capture (wraps cpal+hound), and `tauri-plugin-keyring` for secure credential storage in system keychain.

A critical finding: The fn key (Wispr Flow's default) cannot be reliably captured as a global shortcut on macOS - it's intercepted by the system before applications can see it. The recommended approach is to use a modifier combination like `Ctrl+Shift+Space` or `Option+Space` as the default, with user customization available.

**Primary recommendation:** Use the official Tauri plugins ecosystem for shortcuts, audio, and credential storage. Implement double-tap detection with a 300ms timer pattern. Create a transparent floating bar window for recording feedback.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tauri` | 2.9.x | Desktop app framework | Official framework with stable 2.0, native tray APIs, small binaries (~10MB) |
| `tauri-plugin-global-shortcut` | 2.x | Keyboard shortcuts | Official plugin, supports `ShortcutState::Pressed` and `ShortcutState::Released` for push-to-talk |
| `tauri-plugin-mic-recorder` | 2.0.0 | Audio recording | Wraps cpal+hound, simpler API, outputs WAV files |
| `tauri-plugin-keyring` | latest | Secure credential storage | Uses system keychain (macOS Keychain, Windows Credential Manager) |
| `tauri-plugin-positioner` | 2.3.0 | Window positioning | Official plugin for tray-relative window positioning |
| `rodio` | 0.21.x | Audio playback | Standard Rust audio playback for sound effects |
| React | 18.x | Frontend UI | Mature ecosystem, works perfectly with Tauri's webview |
| TypeScript | 5.x | Type safety | Industry standard for frontend |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tauri-plugin-macos-permissions` | 2.0.x | Permission checking | Check/request microphone permission on macOS |
| Zustand | 5.x | State management | Frontend state for recording status, settings |
| shadcn/ui | latest | UI components | Floating bar, settings panel |
| Tailwind CSS | 4.x | Styling | Utility-first CSS |
| Lucide React | latest | Icons | Tray menu and UI icons |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tauri-plugin-mic-recorder` | Raw `cpal` + `hound` | More control but significantly more code; use if streaming audio needed |
| `tauri-plugin-keyring` | `tauri-plugin-store` (encrypted) | Store plugin doesn't use OS keychain; less secure for API keys |
| Zustand | React Context | Context lacks persistence, devtools, cross-window sync |

**Installation:**

```bash
# Rust dependencies (Cargo.toml)
cargo add tauri --features tray-icon
cargo add tauri-plugin-global-shortcut
cargo add tauri-plugin-mic-recorder
cargo add tauri-plugin-keyring
cargo add tauri-plugin-positioner --features tray-icon
cargo add rodio --no-default-features --features symphonia-wav
cargo add serde --features derive
cargo add serde_json

# For macOS permission checking
cargo add tauri-plugin-macos-permissions

# JavaScript dependencies
npm install @tauri-apps/api @tauri-apps/plugin-global-shortcut
npm install tauri-plugin-mic-recorder-api tauri-plugin-keyring-api
npm install zustand lucide-react sonner
npx shadcn@latest init
```

## Architecture Patterns

### Recommended Project Structure

```
TTP/
├── src/                      # React frontend
│   ├── main.tsx              # Entry point
│   ├── App.tsx               # Main app (minimal - tray-driven)
│   ├── windows/
│   │   ├── FloatingBar.tsx   # Recording indicator overlay
│   │   └── Settings.tsx      # Settings panel
│   ├── components/
│   │   ├── RecordingIndicator.tsx
│   │   └── ApiKeySetup.tsx
│   ├── hooks/
│   │   ├── useRecordingState.ts
│   │   └── useSettings.ts
│   └── stores/
│       └── app-store.ts      # Zustand store
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/
    │   └── default.json      # Permissions
    ├── icons/
    │   ├── icon-idle.png     # Tray icon (idle)
    │   └── icon-recording.png # Tray icon (recording)
    ├── sounds/
    │   ├── start.wav         # Recording start sound
    │   └── stop.wav          # Recording stop sound
    └── src/
        ├── lib.rs            # Main Tauri setup
        ├── tray.rs           # System tray setup
        ├── shortcuts.rs      # Global shortcut handling
        ├── recording.rs      # Audio recording logic
        ├── credentials.rs    # API key storage
        └── state.rs          # App state management
```

### Pattern 1: Event-Based Recording State Machine

**What:** Backend emits events to frontend on state changes; frontend subscribes.
**When to use:** Recording state changes (idle -> recording -> processing -> idle).

```rust
// Source: Tauri official docs - Events
// Backend: src-tauri/src/state.rs
use tauri::{AppHandle, Emitter};
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

pub struct AppState {
    pub recording_state: RecordingState,
    pub last_shortcut_time: Option<std::time::Instant>,
}

impl AppState {
    pub fn set_state(&mut self, state: RecordingState, app: &AppHandle) {
        self.recording_state = state.clone();
        app.emit("recording-state-changed", &state).ok();
    }
}
```

```typescript
// Source: Tauri official docs - Events
// Frontend: src/hooks/useRecordingState.ts
import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

type RecordingState = 'Idle' | 'Recording' | 'Processing';

export function useRecordingState() {
  const [state, setState] = useState<RecordingState>('Idle');

  useEffect(() => {
    const unlisten = listen<RecordingState>('recording-state-changed', (event) => {
      setState(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  return state;
}
```

### Pattern 2: Global Shortcut with Press/Release Detection

**What:** Detect both key press and release for push-to-talk functionality.
**When to use:** Implementing hold-to-record behavior.

```rust
// Source: https://v2.tauri.app/plugin/global-shortcut/
// Backend: src-tauri/src/shortcuts.rs
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri::{AppHandle, Manager};
use std::sync::Mutex;

pub fn setup_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Default: Ctrl+Shift+Space (cross-platform friendly)
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                let state = app.state::<Mutex<crate::state::AppState>>();
                let mut app_state = state.lock().unwrap();

                match event.state() {
                    ShortcutState::Pressed => {
                        handle_shortcut_pressed(&mut app_state, app);
                    }
                    ShortcutState::Released => {
                        handle_shortcut_released(&mut app_state, app);
                    }
                }
            })
            .build()
    )?;

    app.global_shortcut().register(shortcut)?;
    Ok(())
}
```

### Pattern 3: Double-Tap Detection with Timer

**What:** Detect double-tap to toggle hands-free recording mode.
**When to use:** Implementing Wispr Flow-style toggle recording.

```rust
// Double-tap detection pattern
const DOUBLE_TAP_THRESHOLD_MS: u128 = 300;

fn handle_shortcut_pressed(state: &mut AppState, app: &AppHandle) {
    let now = std::time::Instant::now();

    // Check for double-tap
    let is_double_tap = state.last_shortcut_time
        .map(|last| now.duration_since(last).as_millis() < DOUBLE_TAP_THRESHOLD_MS)
        .unwrap_or(false);

    state.last_shortcut_time = Some(now);

    if is_double_tap {
        // Toggle hands-free mode
        match state.recording_state {
            RecordingState::Idle => {
                state.hands_free_mode = true;
                start_recording(state, app);
            }
            RecordingState::Recording if state.hands_free_mode => {
                stop_recording(state, app);
                state.hands_free_mode = false;
            }
            _ => {}
        }
    } else {
        // Push-to-talk: start on press
        if matches!(state.recording_state, RecordingState::Idle) {
            state.hands_free_mode = false;
            start_recording(state, app);
        }
    }
}

fn handle_shortcut_released(state: &mut AppState, app: &AppHandle) {
    // Only stop if push-to-talk (not hands-free)
    if !state.hands_free_mode && matches!(state.recording_state, RecordingState::Recording) {
        stop_recording(state, app);
    }
}
```

### Pattern 4: Transparent Floating Window

**What:** Always-on-top transparent window for recording indicator.
**When to use:** Visual feedback like Wispr Flow's "Flow bar".

```json
// Source: https://v2.tauri.app/learn/window-customization/
// tauri.conf.json - window configuration
{
  "app": {
    "windows": [
      {
        "label": "floating-bar",
        "title": "",
        "width": 200,
        "height": 40,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "visible": false,
        "resizable": false,
        "center": false
      }
    ]
  }
}
```

```rust
// Show/hide floating bar on recording state change
fn show_floating_bar(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("floating-bar") {
        // Position at bottom center of screen
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_floating_bar(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("floating-bar") {
        let _ = window.hide();
    }
}
```

### Anti-Patterns to Avoid

- **Capturing fn key as shortcut:** macOS intercepts fn before apps can see it. Use modifier combinations instead.
- **WebView audio capture:** Leads to permission persistence bugs. Use Rust-side audio via `tauri-plugin-mic-recorder`.
- **Storing API key in frontend:** Accessible via WebView dev tools. Store in Rust via keyring plugin.
- **Polling for state:** Wastes CPU. Use Tauri events to push state changes.
- **Synchronous I/O in commands:** Blocks main thread. All I/O must be async.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Global keyboard shortcuts | Custom key listener | `tauri-plugin-global-shortcut` | Platform-specific APIs, press/release detection already implemented |
| Audio recording | Raw cpal+hound code | `tauri-plugin-mic-recorder` | Handles device enumeration, format conversion, file output |
| Credential storage | File-based encryption | `tauri-plugin-keyring` | Uses OS keychain, security best practices |
| Permission checking (macOS) | Native API calls | `tauri-plugin-macos-permissions` | Handles all macOS permission types |
| System tray | Custom tray implementation | Tauri's built-in tray API | Native integration, proper event handling |
| Audio playback | Manual stream handling | `rodio` | Simple API for sound effects |

**Key insight:** Tauri's plugin ecosystem handles platform-specific complexity. Hand-rolling these features adds weeks of debugging for edge cases already solved.

## Common Pitfalls

### Pitfall 1: fn Key Cannot Be Captured

**What goes wrong:** Attempting to use fn key as global shortcut fails silently - macOS intercepts it at system level.
**Why it happens:** The fn/Globe key on Mac is handled by the system before applications receive it, used for emoji picker, dictation, etc.
**How to avoid:** Use a modifier combination like `Ctrl+Shift+Space` or `Option+Space` as default. Document this limitation.
**Warning signs:** Shortcut works for other keys but not fn.

### Pitfall 2: macOS Microphone Permission Not Prompting (Signed Builds)

**What goes wrong:** Audio recording works in dev mode but fails silently in signed builds - no permission dialog appears.
**Why it happens:** macOS requires `NSMicrophoneUsageDescription` in Info.plist AND proper entitlements for signed apps.
**How to avoid:**
1. Add to `src-tauri/Info.plist`:
   ```xml
   <key>NSMicrophoneUsageDescription</key>
   <string>TTP needs microphone access to transcribe your voice.</string>
   ```
2. Test signed builds locally before distribution.
3. Use `tauri-plugin-macos-permissions` to check permission status.
**Warning signs:** Works in `tauri dev`, fails in `tauri build`.

### Pitfall 3: Tray Icon Disappears or Crashes on macOS

**What goes wrong:** Tray icon randomly disappears, or clicking menu after closing all windows crashes the app.
**Why it happens:** Known Tauri bugs with tray icons on macOS; menu event handlers crash when no windows exist.
**How to avoid:**
1. Keep an invisible window alive on macOS.
2. Pin to specific Tauri version (2.9.x) known to work.
3. Test tray behavior extensively with all windows closed.
**Warning signs:** App works as windowed app but crashes as tray-only app.

### Pitfall 4: Cross-Platform Build Fails

**What goes wrong:** Building Windows installer from macOS fails with cryptic errors.
**Why it happens:** Tauri relies on native toolchains that don't support cross-compilation. MSI requires Windows-only WiX toolkit.
**How to avoid:** Use GitHub Actions from day one with separate runners per platform. Never attempt cross-compilation.
**Warning signs:** "WiX is not available" errors, builds succeed but produce broken binaries.

### Pitfall 5: Double-Tap Detection Conflicts with Push-to-Talk

**What goes wrong:** Quick presses intended as push-to-talk accidentally trigger toggle mode.
**Why it happens:** Timer-based double-tap detection fires when user does rapid short recordings.
**How to avoid:**
1. Require minimum hold time (50-100ms) for push-to-talk before starting.
2. Only count as double-tap if both presses are very short (< 150ms each).
3. Add visual/audio feedback for mode switching.
**Warning signs:** Users report "getting stuck in recording mode".

## Code Examples

### System Tray Setup with Context Menu

```rust
// Source: https://v2.tauri.app/learn/system-tray/
// src-tauri/src/tray.rs
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
    Manager, AppHandle,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &quit])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .menu_on_left_click(false) // Right-click for menu
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "settings" => {
                // Open settings window
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Left click could toggle recording or show status
                let app = tray.app_handle();
                // Handle left click
            }
        })
        .build(app)?;

    Ok(())
}
```

### Secure API Key Storage

```rust
// Source: https://github.com/HuakunShen/tauri-plugin-keyring
// src-tauri/src/credentials.rs
use tauri::Manager;
use tauri_plugin_keyring::KeyringExt;

const SERVICE_NAME: &str = "TTP";
const API_KEY_USER: &str = "openai-api-key";

#[tauri::command]
pub async fn get_api_key(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    app.keyring()
        .set_password(SERVICE_NAME, API_KEY_USER, &key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_api_key(app: tauri::AppHandle) -> Result<bool, String> {
    let result = app.keyring()
        .get_password(SERVICE_NAME, API_KEY_USER)
        .map_err(|e| e.to_string())?;
    Ok(result.is_some())
}
```

```typescript
// Frontend: src/components/ApiKeySetup.tsx
import { getPassword, setPassword } from "tauri-plugin-keyring-api";

export async function checkApiKey(): Promise<boolean> {
  try {
    const key = await getPassword("TTP", "openai-api-key");
    return key !== null && key.length > 0;
  } catch {
    return false;
  }
}

export async function saveApiKey(key: string): Promise<void> {
  await setPassword("TTP", "openai-api-key", key);
}
```

### Audio Recording with Plugin

```rust
// Source: https://github.com/ayangweb/tauri-plugin-mic-recorder
// src-tauri/src/recording.rs
// Recording is handled by the plugin - just need to start/stop

#[tauri::command]
pub async fn start_mic_recording() -> Result<(), String> {
    // Plugin handles this via JS API
    Ok(())
}

#[tauri::command]
pub async fn stop_mic_recording() -> Result<String, String> {
    // Plugin returns file path
    Ok(String::new())
}
```

```typescript
// Frontend: Recording control via plugin
import { startRecording, stopRecording } from "tauri-plugin-mic-recorder-api";

export async function beginRecording() {
  await startRecording();
}

export async function endRecording(): Promise<string> {
  // Returns path to WAV file
  return await stopRecording();
}
```

### Sound Effect Playback

```rust
// src-tauri/src/sounds.rs
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub fn play_sound(app: &tauri::AppHandle, sound_name: &str) {
    let resource_path = app.path()
        .resolve(format!("sounds/{}.wav", sound_name), tauri::path::BaseDirectory::Resource)
        .expect("failed to resolve sound path");

    std::thread::spawn(move || {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(file) = File::open(&resource_path) {
                let source = Decoder::new(BufReader::new(file)).unwrap();
                let sink = Sink::try_new(&stream_handle).unwrap();
                sink.append(source);
                sink.sleep_until_end();
            }
        }
    });
}

pub fn play_start_sound(app: &tauri::AppHandle) {
    play_sound(app, "start");
}

pub fn play_stop_sound(app: &tauri::AppHandle) {
    play_sound(app, "stop");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tauri 1.x plugins | Tauri 2.x plugin system | 2024 | New permission model, mobile support path |
| `tauri-plugin-stronghold` for secrets | `tauri-plugin-keyring` | 2025 | Stronghold deprecated in Tauri v3, keyring uses native OS storage |
| Custom global shortcut code | `tauri-plugin-global-shortcut` with ShortcutState | 2024 | Press/release detection now built-in |

**Deprecated/outdated:**
- `tauri-plugin-stronghold`: Will be removed in Tauri v3. Use `tauri-plugin-keyring` instead.
- Tauri 1.x: No mobile path, older plugin system. Always use Tauri 2.x.
- `rust-clipboard`: Unmaintained. Use `arboard` if needed.

## Open Questions

1. **fn Key Alternative for macOS**
   - What we know: fn key cannot be captured as global shortcut
   - What's unclear: Best default key combination that doesn't conflict with common apps
   - Recommendation: Use `Option+Space` (less likely to conflict than Ctrl+Shift combinations); make configurable

2. **Audio Format for Whisper API**
   - What we know: Whisper accepts WAV, and `tauri-plugin-mic-recorder` outputs WAV
   - What's unclear: Whether compression (WebM/Opus) before upload would improve latency
   - Recommendation: Start with WAV for simplicity; optimize in later phase if needed

3. **Floating Bar Positioning**
   - What we know: Can position anywhere with transparent window
   - What's unclear: Best default position (center-bottom like Wispr, or near cursor?)
   - Recommendation: Center-bottom for v1; cursor-following could be Phase 3 enhancement

## Sources

### Primary (HIGH confidence)
- [Tauri v2 System Tray Documentation](https://v2.tauri.app/learn/system-tray/) - Tray setup, menu events
- [Tauri v2 Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/) - ShortcutState for press/release
- [Tauri v2 Window Customization](https://v2.tauri.app/learn/window-customization/) - Transparent windows, always on top
- [Tauri v2 Create Project](https://v2.tauri.app/start/create-project/) - Project scaffolding
- [Tauri v2 GitHub Actions](https://v2.tauri.app/distribute/pipelines/github/) - CI/CD workflow
- [tauri-plugin-keyring GitHub](https://github.com/HuakunShen/tauri-plugin-keyring) - Secure credential storage
- [tauri-plugin-mic-recorder GitHub](https://github.com/ayangweb/tauri-plugin-mic-recorder) - Audio recording
- [tauri-plugin-macos-permissions GitHub](https://github.com/ayangweb/tauri-plugin-macos-permissions) - Permission checking
- [rodio Documentation](https://docs.rs/rodio) - Audio playback

### Secondary (MEDIUM confidence)
- [Wispr Flow](https://wisprflow.ai) - Reference UX patterns
- [wispr-flow-lite GitHub](https://github.com/tommyyau/wispr-flow-lite) - Open source reference implementation
- [Apple Fn Key Support](https://support.apple.com/en-us/102439) - fn key limitations

### Tertiary (LOW confidence)
- Double-tap detection timing (300ms) - Community consensus, may need tuning

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Official Tauri plugins with documented APIs
- Architecture patterns: HIGH - Based on official Tauri documentation
- Pitfalls: HIGH - Verified via GitHub issues and official docs
- fn key limitation: MEDIUM - Verified by Apple docs but not Tauri-specific testing

**Research date:** 2026-01-29
**Valid until:** 2026-03-01 (Tauri ecosystem relatively stable)

---

*Phase: 01-foundation-recording*
*Research completed: 2026-01-29*
