# Phase 2: Transcription Pipeline - Research

**Researched:** 2026-01-29
**Domain:** OpenAI Whisper/GPT-4o transcription, AI text polishing, macOS paste simulation
**Confidence:** HIGH

## Summary

Phase 2 transforms recorded audio into polished, auto-pasted text. The pipeline involves three core stages: (1) sending audio to OpenAI's transcription API, (2) polishing the transcription with GPT-4o-mini to remove filler words and handle self-corrections, and (3) auto-pasting into the active application with clipboard fallback.

The standard approach uses OpenAI's Whisper API (`whisper-1` model) for transcription, which accepts WAV files up to 25MB and returns text with punctuation. For polishing, GPT-4o-mini provides cost-effective cleanup while preserving the speaker's voice. Auto-paste on macOS requires the `enigo` crate for keyboard simulation with accessibility permissions checked via `tauri-plugin-macos-permissions`.

A critical decision: The newer `gpt-4o-transcribe` models (released March 2025) offer better accuracy and reduced hallucination compared to `whisper-1`, at the same price ($0.006/min). However, they lack timestamp support. Since TTP doesn't need timestamps, `gpt-4o-transcribe` is recommended for the transcription step.

**Primary recommendation:** Use `gpt-4o-transcribe` for transcription, GPT-4o-mini for polishing with a carefully crafted prompt, `enigo` for paste simulation with `tauri-plugin-macos-permissions` for permission handling, and preserve clipboard content before/after paste operations.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tauri-plugin-http` | 2.x | HTTP requests (reqwest re-export) | Official Tauri plugin, provides reqwest for multipart form uploads |
| `reqwest` | 0.12.x | HTTP client with multipart | Industry standard Rust HTTP client, multipart feature for audio upload |
| `enigo` | 0.2.x | Keyboard simulation | Cross-platform input simulation, supports macOS Cmd+V |
| `tauri-plugin-macos-permissions` | 2.3.x | Permission checking | Check/request accessibility permission required for paste simulation |
| `tauri-plugin-clipboard-manager` | 2.3.x | Clipboard access | Read/write clipboard for preservation and paste fallback |
| `tauri-plugin-notification` | 2.x | Toast notifications | System notifications for errors and fallback messages |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde_json` | 1.x | JSON serialization | Parsing OpenAI API responses |
| `base64` | 0.22.x | Base64 encoding | If sending audio as base64 (not recommended - use multipart) |
| `tokio` | 1.x | Async runtime | Required for async HTTP operations |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `gpt-4o-transcribe` | `whisper-1` | Whisper is older, more hallucination-prone, but has streaming support |
| `enigo` | `rdev` | rdev requires broader permissions; enigo is simpler for paste |
| Rust-side HTTP | JS-side fetch | JS fetch works but Rust gives better error handling, no CORS concerns |
| `tauri-plugin-upload` | Raw reqwest multipart | upload plugin is simpler but less control over headers |

**Installation:**

```bash
# Rust dependencies (add to Cargo.toml)
cargo add tauri-plugin-http --features reqwest
cargo add tauri-plugin-clipboard-manager
cargo add tauri-plugin-notification
cargo add tauri-plugin-macos-permissions
cargo add enigo
cargo add serde_json

# JavaScript dependencies
npm install @tauri-apps/plugin-http
npm install @tauri-apps/plugin-clipboard-manager
npm install @tauri-apps/plugin-notification
npm install tauri-plugin-macos-permissions-api
```

## Architecture Patterns

### Recommended Project Structure

```
src-tauri/src/
├── transcription/
│   ├── mod.rs           # Module exports
│   ├── whisper.rs       # OpenAI transcription API client
│   ├── polish.rs        # GPT-4o-mini text cleanup
│   └── pipeline.rs      # Orchestrates transcribe -> polish -> paste
├── paste/
│   ├── mod.rs
│   ├── clipboard.rs     # Clipboard read/write/preserve
│   ├── simulate.rs      # Keyboard simulation (Cmd+V)
│   └── permissions.rs   # Accessibility permission check/request
└── lib.rs               # Register new commands

src/
├── hooks/
│   └── useTranscription.ts  # Hook for transcription state/progress
└── windows/
    └── FloatingBar.tsx      # Show "Processing..." state
```

### Pattern 1: Pipeline with State Transitions

**What:** Recording completion triggers a multi-stage pipeline with visible state transitions.
**When to use:** Processing recorded audio through transcription -> polish -> paste.

```rust
// Source: Phase 2 architecture
// src-tauri/src/transcription/pipeline.rs
use crate::state::{AppState, RecordingState};
use tauri::{AppHandle, Emitter, Manager};
use std::sync::Mutex;

#[derive(Clone, serde::Serialize)]
pub struct TranscriptionProgress {
    pub stage: String,  // "transcribing", "polishing", "pasting"
    pub message: String,
}

pub async fn process_recording(app: &AppHandle, audio_path: &str) -> Result<String, String> {
    let state = app.state::<Mutex<AppState>>();

    // Stage 1: Transcription
    emit_progress(app, "transcribing", "Transcribing audio...");
    let raw_text = transcribe_audio(app, audio_path).await?;

    if raw_text.trim().is_empty() {
        emit_progress(app, "complete", "No speech detected");
        return Err("No speech detected".to_string());
    }

    // Stage 2: Polish
    emit_progress(app, "polishing", "Processing...");
    let polished = polish_text(app, &raw_text).await
        .unwrap_or_else(|_| raw_text.clone()); // Fallback to raw on polish failure

    // Stage 3: Paste
    emit_progress(app, "pasting", "Inserting text...");
    paste_text(app, &polished).await?;

    // Update state to idle
    if let Ok(mut app_state) = state.lock() {
        app_state.set_state(RecordingState::Idle, app);
    }

    Ok(polished)
}

fn emit_progress(app: &AppHandle, stage: &str, message: &str) {
    app.emit("transcription-progress", TranscriptionProgress {
        stage: stage.to_string(),
        message: message.to_string(),
    }).ok();
}
```

### Pattern 2: OpenAI API Multipart Upload

**What:** Send audio file to OpenAI transcription API using multipart/form-data.
**When to use:** Transcribing recorded audio.

```rust
// Source: OpenAI API docs + reqwest docs
// src-tauri/src/transcription/whisper.rs
use tauri_plugin_http::reqwest;
use std::fs;

const OPENAI_TRANSCRIPTION_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

pub async fn transcribe_audio(api_key: &str, audio_path: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    // Read audio file
    let audio_bytes = fs::read(audio_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Create multipart form
    let file_part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to create file part: {}", e))?;

    let form = reqwest::multipart::Form::new()
        .text("model", "gpt-4o-transcribe")  // Or "whisper-1"
        .text("response_format", "text")
        .part("file", file_part);

    // Send request with retries
    let mut last_error = String::new();
    for attempt in 0..3 {
        match client
            .post(OPENAI_TRANSCRIPTION_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form.try_clone().unwrap())
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    return response.text().await
                        .map_err(|e| format!("Failed to read response: {}", e));
                }
                last_error = format!("API error: {}", response.status());
            }
            Err(e) => {
                last_error = format!("Request failed: {}", e);
            }
        }

        if attempt < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64)).await;
        }
    }

    Err(last_error)
}
```

### Pattern 3: GPT-4o-mini Polish with Structured Prompt

**What:** Clean transcription using GPT-4o-mini with specific instructions.
**When to use:** Removing filler words, fixing grammar, handling self-corrections.

```rust
// Source: OpenAI Chat API + prompt engineering research
// src-tauri/src/transcription/polish.rs
use tauri_plugin_http::reqwest;
use serde::{Deserialize, Serialize};

const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

const POLISH_SYSTEM_PROMPT: &str = r#"You are a transcription editor. Your job is to clean up voice transcriptions while preserving the speaker's voice and intent.

Rules:
1. Remove ALL filler words: um, uh, like (when used as filler), you know, sort of, kind of, basically, literally (when meaningless)
2. Fix obvious grammar errors but preserve casual speech patterns
3. Add proper punctuation and capitalization
4. Handle self-corrections: when someone says "Tuesday no wait Wednesday" or "Send it Monday. Actually make that Tuesday", keep ONLY the final corrected version
5. Preserve the speaker's tone - don't make casual speech formal
6. If uncertain whether something is a correction, preserve the original

Return ONLY the cleaned text, nothing else."#;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

pub async fn polish_text(api_key: &str, raw_text: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let request = ChatRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: POLISH_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: raw_text.to_string(),
            },
        ],
        temperature: 0.3,  // Lower temperature for more consistent output
        max_tokens: 1024,
    };

    let response = client
        .post(OPENAI_CHAT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Polish request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Polish API error: {}", response.status()));
    }

    let chat_response: ChatResponse = response.json().await
        .map_err(|e| format!("Failed to parse polish response: {}", e))?;

    chat_response.choices
        .first()
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| "Empty response from polish API".to_string())
}
```

### Pattern 4: Clipboard Preservation and Paste Simulation

**What:** Save clipboard, paste text, restore original clipboard.
**When to use:** Auto-pasting transcription while preserving user's clipboard.

```rust
// Source: enigo docs, tauri-plugin-clipboard-manager docs
// src-tauri/src/paste/clipboard.rs
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub struct ClipboardGuard {
    original_content: Option<String>,
    app: AppHandle,
}

impl ClipboardGuard {
    pub fn new(app: &AppHandle) -> Self {
        // Save current clipboard content
        let original_content = app.clipboard()
            .read_text()
            .ok()
            .flatten();

        Self {
            original_content,
            app: app.clone(),
        }
    }

    pub fn write_text(&self, text: &str) -> Result<(), String> {
        self.app.clipboard()
            .write_text(text)
            .map_err(|e| format!("Failed to write to clipboard: {}", e))
    }

    pub fn restore(self) -> Result<(), String> {
        if let Some(original) = self.original_content {
            self.app.clipboard()
                .write_text(&original)
                .map_err(|e| format!("Failed to restore clipboard: {}", e))?;
        }
        Ok(())
    }
}
```

```rust
// src-tauri/src/paste/simulate.rs
use enigo::{Enigo, Key, Keyboard, Settings};

pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create enigo: {}", e))?;

    // Small delay to ensure app focus
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Cmd+V on macOS
    enigo.key(Key::Meta, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press Meta: {}", e))?;
    enigo.key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Failed to click V: {}", e))?;
    enigo.key(Key::Meta, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release Meta: {}", e))?;

    Ok(())
}
```

### Pattern 5: Accessibility Permission Check

**What:** Check and request accessibility permission before paste simulation.
**When to use:** Before first paste attempt on macOS.

```typescript
// Source: tauri-plugin-macos-permissions docs
// src/hooks/useAccessibilityPermission.ts
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission
} from "tauri-plugin-macos-permissions-api";
import { useState, useEffect, useCallback } from "react";

export function useAccessibilityPermission() {
  const [hasPermission, setHasPermission] = useState<boolean | null>(null);

  const checkPermission = useCallback(async () => {
    try {
      const authorized = await checkAccessibilityPermission();
      setHasPermission(authorized);
      return authorized;
    } catch (error) {
      console.error("Failed to check accessibility permission:", error);
      return false;
    }
  }, []);

  const requestPermission = useCallback(async () => {
    try {
      await requestAccessibilityPermission();
      // Re-check after request (user may have granted in System Settings)
      return await checkPermission();
    } catch (error) {
      console.error("Failed to request accessibility permission:", error);
      return false;
    }
  }, [checkPermission]);

  useEffect(() => {
    checkPermission();
  }, [checkPermission]);

  return { hasPermission, checkPermission, requestPermission };
}
```

### Anti-Patterns to Avoid

- **Blocking main thread during API calls:** All HTTP requests must be async. Use `tokio::spawn` for background processing.
- **Not preserving clipboard:** Always save and restore clipboard content to avoid losing user's data.
- **Hardcoded correction phrases:** Don't use regex for "no wait" patterns - let GPT handle contextual detection.
- **Single retry on API failure:** OpenAI APIs can have transient failures. Use 2-3 retries with exponential backoff.
- **Pasting without permission check:** Always verify accessibility permission before attempting paste simulation.
- **Sending large audio files:** Keep recordings under 25MB. For longer recordings, implement chunking.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP multipart upload | Custom boundary handling | `reqwest::multipart` | Boundary generation, encoding edge cases |
| Keyboard simulation | CGEvent/AppleScript | `enigo` crate | Cross-platform, handles modifier keys correctly |
| Permission checking | Direct system API calls | `tauri-plugin-macos-permissions` | Consistent API, handles prompting |
| Disfluency detection | Regex patterns for corrections | GPT-4o-mini with prompt | Context-aware, handles edge cases |
| Clipboard management | Direct NSPasteboard | `tauri-plugin-clipboard-manager` | Cross-platform, proper Tauri integration |
| Retry logic | Simple loop | Exponential backoff pattern | Prevents rate limiting, handles transient failures |

**Key insight:** Self-correction detection ("Tuesday no wait Wednesday") is surprisingly complex when done with pattern matching. GPT handles it naturally through context understanding.

## Common Pitfalls

### Pitfall 1: Accessibility Permission Not Granted

**What goes wrong:** Paste simulation fails silently - text goes to clipboard but nothing appears in the target app.
**Why it happens:** macOS requires explicit Accessibility permission for keyboard simulation. First-time users won't have this granted.
**How to avoid:**
1. Check permission on app startup and before first paste
2. Show clear instructions for granting permission in System Settings
3. Always implement clipboard fallback with notification
4. Use `tauri-plugin-macos-permissions` to check/request

**Warning signs:** Text appears in clipboard (Cmd+V works manually) but auto-paste does nothing.

### Pitfall 2: OpenAI API Key Not Available When Needed

**What goes wrong:** Pipeline fails because API key wasn't retrieved before async operations.
**Why it happens:** Keyring access is async, and key might not be fetched before transcription starts.
**How to avoid:**
1. Fetch API key at recording start (not recording end)
2. Validate key exists before starting recording
3. Cache key in app state for duration of recording session
**Warning signs:** Intermittent "unauthorized" or "missing API key" errors.

### Pitfall 3: Audio File Too Large

**What goes wrong:** Transcription fails with "file too large" error from OpenAI.
**Why it happens:** WAV files can exceed 25MB for recordings over ~2-3 minutes at high sample rates.
**How to avoid:**
1. Limit recording duration (e.g., 5 minutes max)
2. Use appropriate sample rate (16kHz is sufficient for speech)
3. Consider compression to WebM/Opus for longer recordings
4. Show warning when approaching limit
**Warning signs:** Failures only on longer recordings.

### Pitfall 4: Polish Prompt Overcorrects

**What goes wrong:** GPT changes the speaker's intended meaning or makes casual speech too formal.
**Why it happens:** Prompt instructions too aggressive, temperature too high.
**How to avoid:**
1. Use lower temperature (0.2-0.3)
2. Explicit instruction to preserve tone
3. Test with varied input styles
4. Include "when uncertain, preserve original" instruction
**Warning signs:** Users complain output doesn't sound like them.

### Pitfall 5: Clipboard Restoration Race Condition

**What goes wrong:** Original clipboard content lost or restored before paste completes.
**Why it happens:** Async paste followed by immediate clipboard restore.
**How to avoid:**
1. Use small delay after paste before restoring (100-200ms)
2. Only restore if original was non-empty
3. Consider not restoring immediately - restore on next recording start
**Warning signs:** Clipboard contains transcription text when user expects their previous content.

### Pitfall 6: Notification Spam

**What goes wrong:** Too many notifications appear for successful operations.
**Why it happens:** Notifying on every success instead of just failures.
**How to avoid:**
1. Only notify on errors or fallback situations
2. "Text copied - paste manually" when auto-paste fails
3. "No speech detected" when transcription is empty
4. Never notify on successful auto-paste
**Warning signs:** Users disable notifications entirely.

## Code Examples

### Complete Pipeline Integration

```rust
// src-tauri/src/transcription/pipeline.rs
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::credentials::get_api_key_internal;
use crate::paste::{ClipboardGuard, check_accessibility, simulate_paste};
use super::{transcribe_audio, polish_text};

pub async fn process_recording(app: &AppHandle, audio_path: String) -> Result<String, String> {
    // Get API key
    let api_key = get_api_key_internal(app).await?
        .ok_or("API key not configured")?;

    // Stage 1: Transcribe
    app.emit("transcription-progress", ("transcribing", "Transcribing...")).ok();
    let raw_text = transcribe_audio(&api_key, &audio_path).await?;

    if raw_text.trim().is_empty() {
        notify_subtle(app, "No speech detected");
        return Err("No speech detected".to_string());
    }

    // Stage 2: Polish
    app.emit("transcription-progress", ("polishing", "Processing...")).ok();
    let polished = match polish_text(&api_key, &raw_text).await {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Polish failed, using raw: {}", e);
            raw_text
        }
    };

    // Stage 3: Paste
    app.emit("transcription-progress", ("pasting", "")).ok();

    // Save clipboard, write new text
    let clipboard_guard = ClipboardGuard::new(app);
    clipboard_guard.write_text(&polished)?;

    // Try auto-paste if we have permission
    let paste_succeeded = if check_accessibility() {
        simulate_paste().is_ok()
    } else {
        false
    };

    // Small delay before potential clipboard restore
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    if !paste_succeeded {
        // Don't restore clipboard - user needs to paste manually
        notify_fallback(app, "Text copied - paste with Cmd+V");
    } else {
        // Restore original clipboard content
        clipboard_guard.restore().ok();
    }

    app.emit("transcription-progress", ("complete", "")).ok();

    Ok(polished)
}

fn notify_subtle(app: &AppHandle, message: &str) {
    app.notification()
        .builder()
        .title("TTP")
        .body(message)
        .show()
        .ok();
}

fn notify_fallback(app: &AppHandle, message: &str) {
    app.notification()
        .builder()
        .title("TTP")
        .body(message)
        .show()
        .ok();
}
```

### Frontend Progress Display

```typescript
// src/hooks/useTranscription.ts
import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

type TranscriptionStage = 'idle' | 'transcribing' | 'polishing' | 'pasting' | 'complete';

interface TranscriptionProgress {
  stage: TranscriptionStage;
  message: string;
}

export function useTranscription() {
  const [stage, setStage] = useState<TranscriptionStage>('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    const unlisten = listen<[string, string]>('transcription-progress', (event) => {
      const [newStage, newMessage] = event.payload;
      setStage(newStage as TranscriptionStage);
      setMessage(newMessage);
    });

    return () => { unlisten.then(fn => fn()); };
  }, []);

  return { stage, message, isProcessing: stage !== 'idle' && stage !== 'complete' };
}
```

### Tauri Commands Registration

```rust
// src-tauri/src/lib.rs (additions)
mod transcription;
mod paste;

use transcription::pipeline::process_recording;

#[tauri::command]
async fn transcribe_and_paste(app: tauri::AppHandle, audio_path: String) -> Result<String, String> {
    process_recording(&app, audio_path).await
}

// In run():
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    transcribe_and_paste,
])
```

### Capabilities Configuration

```json
// src-tauri/capabilities/default.json (additions)
{
  "permissions": [
    "clipboard-manager:allow-read-text",
    "clipboard-manager:allow-write-text",
    "notification:default",
    "macos-permissions:default",
    {
      "identifier": "http:default",
      "allow": [
        { "url": "https://api.openai.com/*" }
      ]
    }
  ]
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `whisper-1` for transcription | `gpt-4o-transcribe` | March 2025 | Better accuracy, fewer hallucinations, same price |
| GPT-4 for polish | GPT-4o-mini | 2024 | 10x cheaper, sufficient for text cleanup |
| AppleScript for paste | `enigo` crate | 2024 | More reliable, cross-platform |
| Manual permission prompts | `tauri-plugin-macos-permissions` | 2025 | Consistent API, automatic prompting |

**Deprecated/outdated:**
- `whisper-1`: Still works but `gpt-4o-transcribe` is recommended for new projects
- `gpt-4o`: Overkill for text polish; use `gpt-4o-mini` instead
- Tauri v1 HTTP API: Use v2 plugin with reqwest

## Open Questions

1. **Clipboard Restoration Timing**
   - What we know: Need delay between paste and restore to avoid race condition
   - What's unclear: Optimal delay duration (100ms? 200ms? varies by app?)
   - Recommendation: Start with 150ms, make configurable if issues arise

2. **Smart Spacing Implementation**
   - What we know: Context says "add space if cursor follows a word"
   - What's unclear: How to detect cursor context from paste simulation
   - Recommendation: Defer to Phase 4 with native APIs; for now, let user handle spacing

3. **Multi-language Polish Prompts**
   - What we know: Whisper handles 100+ languages
   - What's unclear: Whether single English prompt works for non-English text polish
   - Recommendation: English prompt works for light cleanup; language-specific prompts may be needed for Phase 3+

4. **gpt-4o-transcribe vs gpt-4o-mini-transcribe**
   - What we know: Full model is more accurate but costs 2x
   - What's unclear: Whether accuracy difference matters for typical voice notes
   - Recommendation: Start with `gpt-4o-transcribe` for quality; offer `gpt-4o-mini-transcribe` as cost-saving option later

## Sources

### Primary (HIGH confidence)
- [OpenAI Audio Transcription API](https://platform.openai.com/docs/api-reference/audio/createSpeech) - API endpoint, parameters, models
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat) - GPT-4o-mini for polish
- [Tauri v2 Clipboard Plugin](https://v2.tauri.app/plugin/clipboard/) - Clipboard read/write
- [Tauri v2 HTTP Plugin](https://v2.tauri.app/plugin/http-client/) - reqwest integration
- [Tauri v2 Notification Plugin](https://v2.tauri.app/plugin/notification/) - Toast notifications
- [tauri-plugin-macos-permissions](https://github.com/ayangweb/tauri-plugin-macos-permissions) - Permission checking
- [enigo Permissions](https://github.com/enigo-rs/enigo/blob/main/Permissions.md) - Accessibility requirements
- [reqwest multipart](https://docs.rs/reqwest/latest/reqwest/multipart/) - File upload handling

### Secondary (MEDIUM confidence)
- [OpenAI gpt-4o-transcribe announcement](https://openai.com/index/introducing-our-next-generation-audio-models/) - New model capabilities
- [OpenAI Transcription Pricing](https://costgoat.com/pricing/openai-transcription) - Current pricing ($0.006/min)
- [Transcript cleanup best practices](https://thescottking.com/how-to-clean-up-raw-transcriptions) - Prompt engineering patterns
- [Disfluency detection research](https://research.google/blog/identifying-disfluencies-in-natural-speech/) - Self-correction handling

### Tertiary (LOW confidence)
- Polish prompt temperature (0.3) - Based on general GPT best practices, may need tuning
- Clipboard restore delay (150ms) - Reasonable estimate, not empirically tested

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Official Tauri plugins and OpenAI APIs with documented behavior
- Architecture patterns: HIGH - Based on official documentation and established patterns
- Pitfalls: MEDIUM - Some based on general knowledge, not TTP-specific testing
- Self-correction handling: MEDIUM - Relies on GPT contextual understanding

**Research date:** 2026-01-29
**Valid until:** 2026-03-01 (OpenAI APIs stable, may update if new transcription models released)

---

*Phase: 02-transcription-pipeline*
*Research completed: 2026-01-29*
