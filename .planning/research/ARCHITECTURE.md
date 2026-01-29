# Architecture Research

**Domain:** Tauri desktop voice transcription utility (menu bar app)
**Researched:** 2026-01-29
**Confidence:** HIGH (verified via official Tauri v2 documentation)

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           FRONTEND (React/TypeScript)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ System Tray  │  │  Recording   │  │  Settings    │  │  Dictionary  │     │
│  │    Menu      │  │    UI        │  │    Panel     │  │   Editor     │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                 │                 │                 │              │
├─────────┴─────────────────┴─────────────────┴─────────────────┴──────────────┤
│                              IPC BOUNDARY                                     │
│                    (invoke commands / events / channels)                      │
├──────────────────────────────────────────────────────────────────────────────┤
│                            BACKEND (Rust)                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Global     │  │    Audio     │  │    API       │  │    Local     │     │
│  │   Hotkey     │  │   Capture    │  │   Client     │  │   Storage    │     │
│  │   Handler    │  │   Manager    │  │  (Whisper/   │  │  (Dictionary │     │
│  │              │  │              │  │   GPT)       │  │   + Config)  │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                 │                 │                 │              │
│  ┌──────┴─────────────────┴─────────────────┴─────────────────┴──────────────┐
│  │                        APPLICATION STATE (Mutex<AppState>)                 │
│  │   - recording_state: RecordingState (Idle/Recording/Processing)           │
│  │   - audio_buffer: Vec<u8>                                                 │
│  │   - settings: UserSettings                                                │
│  │   - dictionary: CustomDictionary                                          │
│  └───────────────────────────────────────────────────────────────────────────┘
├──────────────────────────────────────────────────────────────────────────────┤
│                           SYSTEM INTEGRATION                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   System     │  │  Microphone  │  │  Clipboard   │  │  File        │     │
│  │   Tray       │  │  (cpal)      │  │  (paste)     │  │  System      │     │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘     │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| System Tray Menu | App icon, status display, quick actions | `tauri::tray::TrayIconBuilder` + context menu |
| Global Hotkey Handler | Capture keyboard shortcuts when app unfocused | `tauri-plugin-global-shortcut` |
| Audio Capture Manager | Record microphone input to buffer | `tauri-plugin-mic-recorder` or custom `cpal` |
| API Client | Send audio to Whisper, text to GPT-4o-mini | `reqwest` async HTTP client |
| Local Storage | Persist dictionary, settings, corrections | `tauri::api::path::app_data_dir` + JSON/SQLite |
| Clipboard Manager | Write final text, trigger paste | `tauri-plugin-clipboard-manager` |
| Recording UI | Visual recording indicator | Minimal React component (optional overlay) |
| Settings Panel | Configure hotkey, API keys, behavior | React form with Tauri commands |
| Dictionary Editor | Manage custom word replacements | React UI + Rust persistence |

## Recommended Project Structure

```
TTP/
├── package.json              # Frontend dependencies
├── tsconfig.json             # TypeScript config
├── vite.config.ts            # Vite bundler config
├── index.html                # Entry HTML
├── src/                      # Frontend source
│   ├── main.tsx              # React entry point
│   ├── App.tsx               # Main app component
│   ├── components/
│   │   ├── TrayMenu.tsx      # Tray menu builder
│   │   ├── RecordingIndicator.tsx
│   │   ├── SettingsPanel.tsx
│   │   └── DictionaryEditor.tsx
│   ├── hooks/
│   │   ├── useRecordingState.ts    # Subscribe to recording events
│   │   ├── useSettings.ts          # Settings state management
│   │   └── useDictionary.ts        # Dictionary operations
│   ├── services/
│   │   └── tauri.ts          # Typed invoke wrappers
│   ├── types/
│   │   └── index.ts          # Shared TypeScript types
│   └── styles/
│       └── globals.css       # Tailwind/CSS
└── src-tauri/
    ├── Cargo.toml            # Rust dependencies
    ├── tauri.conf.json       # Tauri configuration
    ├── build.rs              # Build script
    ├── capabilities/
    │   └── default.json      # Permission capabilities
    ├── icons/                # App icons
    └── src/
        ├── main.rs           # Desktop entry (minimal)
        ├── lib.rs            # Main Tauri setup + commands
        ├── commands/
        │   ├── mod.rs
        │   ├── recording.rs  # start_recording, stop_recording
        │   ├── transcription.rs  # transcribe_audio, polish_text
        │   ├── settings.rs   # get_settings, save_settings
        │   └── dictionary.rs # get_dictionary, add_word, remove_word
        ├── state/
        │   ├── mod.rs
        │   └── app_state.rs  # AppState struct + RecordingState enum
        ├── services/
        │   ├── mod.rs
        │   ├── audio.rs      # Audio capture logic
        │   ├── whisper.rs    # Whisper API client
        │   ├── gpt.rs        # GPT-4o-mini client
        │   └── clipboard.rs  # Clipboard + paste simulation
        ├── storage/
        │   ├── mod.rs
        │   ├── config.rs     # User settings persistence
        │   └── dictionary.rs # Dictionary persistence
        └── tray/
            └── mod.rs        # System tray setup + handlers
```

### Structure Rationale

- **`src/components/`:** React components separated by concern; tray, recording UI, and settings are distinct features.
- **`src/hooks/`:** Custom hooks encapsulate Tauri IPC subscriptions; keeps components clean.
- **`src/services/tauri.ts`:** Typed wrappers around `invoke()` for type safety between frontend and backend.
- **`src-tauri/commands/`:** Each command module handles a feature domain; avoids monolithic `lib.rs`.
- **`src-tauri/state/`:** Centralized state management with `Mutex<AppState>` for thread-safe access.
- **`src-tauri/services/`:** Business logic separated from command handlers; testable in isolation.
- **`src-tauri/storage/`:** Persistence layer abstracted; can swap JSON for SQLite later.
- **`src-tauri/tray/`:** System tray is complex enough to warrant its own module.

## Architectural Patterns

### Pattern 1: Command-Based IPC

**What:** All frontend-to-backend communication uses Tauri commands via `invoke()`.
**When to use:** Any operation that requires Rust execution (file I/O, API calls, state changes).
**Trade-offs:** Type-safe but requires serialization; use `tauri::ipc::Response` for large data.

**Example:**
```typescript
// Frontend: src/services/tauri.ts
import { invoke } from '@tauri-apps/api/core';

export async function startRecording(): Promise<void> {
  return invoke('start_recording');
}

export async function getTranscription(): Promise<string> {
  return invoke('get_transcription');
}
```

```rust
// Backend: src-tauri/src/commands/recording.rs
use tauri::State;
use std::sync::Mutex;
use crate::state::AppState;

#[tauri::command]
pub async fn start_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().map_err(|e| e.to_string())?;
    app_state.start_recording().map_err(|e| e.to_string())
}
```

### Pattern 2: Event-Based State Sync

**What:** Backend emits events to notify frontend of state changes; frontend subscribes.
**When to use:** Real-time UI updates (recording state, transcription progress).
**Trade-offs:** Not type-safe; use sparingly for truly async notifications.

**Example:**
```typescript
// Frontend: src/hooks/useRecordingState.ts
import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

type RecordingState = 'idle' | 'recording' | 'processing';

export function useRecordingState() {
  const [state, setState] = useState<RecordingState>('idle');

  useEffect(() => {
    const unlisten = listen<RecordingState>('recording-state-changed', (event) => {
      setState(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  return state;
}
```

```rust
// Backend: src-tauri/src/state/app_state.rs
use tauri::{AppHandle, Emitter};

impl AppState {
    pub fn set_recording_state(&mut self, state: RecordingState, app: &AppHandle) {
        self.recording_state = state;
        app.emit("recording-state-changed", &state).ok();
    }
}
```

### Pattern 3: Global Hotkey with State Guard

**What:** Global hotkey triggers action only if state permits (not already recording).
**When to use:** Push-to-talk and toggle recording modes.
**Trade-offs:** Requires careful state machine design to prevent race conditions.

**Example:**
```rust
// Backend: src-tauri/src/lib.rs
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

fn setup_hotkey(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                let state = app.state::<Mutex<AppState>>();
                let mut app_state = state.lock().unwrap();

                match event.state {
                    ShortcutState::Pressed => {
                        if app_state.can_start_recording() {
                            app_state.start_recording();
                        }
                    }
                    ShortcutState::Released => {
                        if app_state.is_recording() {
                            app_state.stop_recording();
                            // Trigger transcription pipeline
                        }
                    }
                }
            })
            .build()
    )?;
    Ok(())
}
```

## Data Flow

### Recording → Transcription → Paste Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            USER ACTION                                       │
│                                                                              │
│  [Press Hotkey] ──────────────────────────────────────────────────────────┐ │
│                                                                           │ │
│  ┌─────────────────────────────────────────────────────────────────────┐  │ │
│  │ 1. HOTKEY DETECTION                                                  │  │ │
│  │    global_shortcut plugin → captures Ctrl+Shift+R press             │  │ │
│  │    → Emit "recording-state-changed" (recording)                     │  │ │
│  └─────────────────────────────┬───────────────────────────────────────┘  │ │
│                                ↓                                           │ │
│  ┌─────────────────────────────────────────────────────────────────────┐  │ │
│  │ 2. AUDIO CAPTURE                                                     │  │ │
│  │    mic_recorder plugin OR cpal → stream to buffer                    │  │ │
│  │    AppState.audio_buffer = Vec<u8> (PCM/WAV data)                   │  │ │
│  │    [User holds key...]                                               │  │ │
│  └─────────────────────────────┬───────────────────────────────────────┘  │ │
│                                ↓                                           │ │
│  [Release Hotkey] ←───────────────────────────────────────────────────────┘ │
│                                ↓                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ 3. STOP RECORDING + ENCODE                                           │    │
│  │    → Emit "recording-state-changed" (processing)                    │    │
│  │    audio_buffer → encode to WAV/MP3                                 │    │
│  └─────────────────────────────┬───────────────────────────────────────┘    │
│                                ↓                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ 4. WHISPER API TRANSCRIPTION                                         │    │
│  │    POST /v1/audio/transcriptions                                    │    │
│  │    audio file → raw transcription text                              │    │
│  └─────────────────────────────┬───────────────────────────────────────┘    │
│                                ↓                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ 5. DICTIONARY APPLICATION                                            │    │
│  │    raw text → apply custom word replacements                        │    │
│  │    "john" → "John", "tauri" → "Tauri", etc.                        │    │
│  └─────────────────────────────┬───────────────────────────────────────┘    │
│                                ↓                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ 6. GPT-4o-mini POLISH (Optional)                                     │    │
│  │    POST /v1/chat/completions                                        │    │
│  │    "Fix grammar, punctuation, keep meaning" → polished text         │    │
│  └─────────────────────────────┬───────────────────────────────────────┘    │
│                                ↓                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ 7. CLIPBOARD + PASTE                                                 │    │
│  │    clipboard_manager.write_text(polished_text)                      │    │
│  │    Simulate Cmd+V / Ctrl+V keystroke                                │    │
│  │    → Emit "recording-state-changed" (idle)                          │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  [Text appears in user's active application]                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### State Management

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AppState (Mutex-protected)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  recording_state: RecordingState                                            │
│  ├── Idle          → Can start recording                                    │
│  ├── Recording     → Audio being captured, can stop                         │
│  └── Processing    → Transcription in progress, cannot start/stop           │
│                                                                              │
│  audio_buffer: Option<Vec<u8>>                                              │
│  └── Populated during Recording, consumed during Processing                 │
│                                                                              │
│  settings: UserSettings                                                     │
│  ├── hotkey: String ("Ctrl+Shift+R")                                       │
│  ├── recording_mode: RecordingMode (PushToTalk | Toggle)                   │
│  ├── auto_paste: bool                                                       │
│  ├── enable_polish: bool                                                    │
│  └── openai_api_key: String                                                │
│                                                                              │
│  dictionary: HashMap<String, String>                                        │
│  └── "whisper" → "Whisper", "api" → "API", etc.                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Data Flows

1. **Hotkey → Recording:** Global shortcut triggers state change, audio capture begins.
2. **Recording → Processing:** Hotkey release (push-to-talk) or second press (toggle) stops capture.
3. **Processing → API:** Audio buffer sent to Whisper API, response flows to dictionary/GPT.
4. **API → Clipboard:** Final text written to clipboard, paste simulated.
5. **Settings Change → State:** User settings update propagated to AppState via command.
6. **Dictionary Update → Storage:** Dictionary changes persisted to local JSON/SQLite.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single user (target) | Monolithic Tauri app; all processing local except API calls |
| Multiple recordings/day | Consider audio file caching for corrections feature |
| Large dictionary (1000+ words) | SQLite instead of JSON; indexed lookups |
| Offline mode | Local Whisper model (whisper.cpp) instead of API |

### Scaling Priorities

1. **First bottleneck:** API latency (500ms-2s for Whisper). Mitigate with streaming UI feedback.
2. **Second bottleneck:** Large audio files. Mitigate with compression before upload.

## Anti-Patterns

### Anti-Pattern 1: Frontend Audio Capture

**What people do:** Capture audio in JavaScript using Web Audio API.
**Why it's wrong:** More complex, less reliable, requires additional permissions bridging.
**Do this instead:** Use Rust-based audio capture (`cpal` or `tauri-plugin-mic-recorder`).

### Anti-Pattern 2: Synchronous Commands

**What people do:** Use synchronous Rust functions for I/O operations.
**Why it's wrong:** Blocks main thread, freezes UI.
**Do this instead:** All I/O commands should be `async`.

### Anti-Pattern 3: Storing API Keys in Frontend

**What people do:** Store OpenAI API key in React state or localStorage.
**Why it's wrong:** Accessible via WebView developer tools; security risk.
**Do this instead:** Store in Rust state, loaded from secure storage (keyring or encrypted file).

### Anti-Pattern 4: Polling for State

**What people do:** Frontend polls backend every 100ms for recording state.
**Why it's wrong:** Wastes CPU, introduces latency, increases complexity.
**Do this instead:** Use Tauri events to push state changes to frontend.

### Anti-Pattern 5: Monolithic lib.rs

**What people do:** Put all Rust code in a single `lib.rs` file.
**Why it's wrong:** Becomes unmanageable quickly; hard to test.
**Do this instead:** Modular structure with `commands/`, `services/`, `state/` modules.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| OpenAI Whisper API | `reqwest` async POST with multipart form | Use `whisper-1` model; WAV format preferred |
| OpenAI GPT-4o-mini | `reqwest` async POST with JSON | Keep prompt short; stream if needed |
| OS Clipboard | `tauri-plugin-clipboard-manager` | Write text, then simulate paste keystroke |
| System Tray | `tauri::tray` module | Icon + context menu; no window needed for tray-only apps |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Frontend ↔ Backend | Tauri IPC (invoke/events) | JSON serialization; keep payloads small |
| Commands ↔ Services | Rust function calls | Services are stateless; commands manage state |
| Services ↔ Storage | Rust traits for abstraction | Allows swapping storage implementations |
| Hotkey ↔ State | Direct `Mutex<AppState>` access | Hotkey handler runs in Rust; no IPC needed |

## Build Order Implications

Based on component dependencies:

1. **Phase 1: Foundation**
   - Project scaffolding (Tauri + React)
   - Basic Rust state structure
   - System tray with quit action
   - *Reason:* Everything depends on base project structure

2. **Phase 2: Core Input**
   - Global hotkey registration
   - Audio capture (start/stop)
   - Recording state machine
   - *Reason:* Cannot test transcription without audio input

3. **Phase 3: Transcription Pipeline**
   - Whisper API integration
   - Basic clipboard write
   - End-to-end flow without polish
   - *Reason:* Core value proposition; proves concept

4. **Phase 4: Enhancement**
   - GPT polish integration
   - Dictionary system
   - Auto-paste simulation
   - *Reason:* Builds on working transcription

5. **Phase 5: Polish**
   - Settings UI
   - Corrections learning
   - Platform-specific fixes (macOS permissions, Windows paths)
   - *Reason:* UX improvements after core works

## Sources

- [Tauri Architecture (Official)](https://v2.tauri.app/concept/architecture/) - HIGH confidence
- [Tauri IPC Documentation](https://v2.tauri.app/concept/inter-process-communication/) - HIGH confidence
- [Tauri Project Structure](https://v2.tauri.app/start/project-structure/) - HIGH confidence
- [Tauri Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/) - HIGH confidence
- [Tauri System Tray](https://v2.tauri.app/learn/system-tray/) - HIGH confidence
- [Tauri State Management](https://v2.tauri.app/develop/state-management/) - HIGH confidence
- [Tauri Clipboard Plugin](https://v2.tauri.app/plugin/clipboard/) - HIGH confidence
- [tauri-plugin-mic-recorder](https://github.com/ayangweb/tauri-plugin-mic-recorder) - MEDIUM confidence
- [Handy STT (Reference Implementation)](https://github.com/cjpais/Handy) - MEDIUM confidence

---
*Architecture research for: Tauri voice transcription desktop utility*
*Researched: 2026-01-29*
