# Phase 5: Ensemble Transcription - Research

**Researched:** 2026-01-30
**Domain:** Multi-provider parallel transcription, LLM-based fusion, Rust async concurrency
**Confidence:** HIGH

## Summary

Phase 5 introduces ensemble transcription: sending audio to multiple providers (Groq, Gladia, OpenAI) simultaneously and using an LLM to fuse results into a single, highest-accuracy output. This approach, validated by recent research showing 10-40% WER improvements, leverages the complementary strengths of different ASR engines while using LLM contextual understanding to arbitrate disagreements.

The codebase already has all three provider clients implemented (`whisper.rs` for Groq/OpenAI, `gladia.rs` for Gladia). The ensemble mode requires: (1) parallel execution using Tokio's `join!` macro, (2) graceful handling of individual provider failures, (3) an LLM fusion prompt that combines transcriptions intelligently, and (4) a settings toggle to enable/disable ensemble mode with fallback to single provider.

Key architectural insight: Rather than voting or string matching, research shows LLMs excel at contextually weighing multiple hypotheses. The fusion prompt presents all transcriptions to GPT-4o-mini, which determines the most accurate combined result. This integrates naturally with the existing polish step - fusion and polish can be combined into a single LLM call for efficiency.

**Primary recommendation:** Use `tokio::join!` to call all three providers in parallel, collect results (tolerating individual failures), pass all successful transcriptions to a fusion-enabled polish prompt, and let GPT-4o-mini produce the final output. Add an "Ensemble Mode" toggle in settings that requires API keys for at least 2 providers.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | 1.x | Async runtime with `join!` macro | Built-in parallel execution, already in project |
| `futures` | 0.3.x | `join_all` for dynamic provider lists | Standard Rust async utilities |
| `reqwest` | 0.12.x | HTTP client (already present) | Shared client across providers |
| `gpt-4o-mini` | API | LLM fusion + polish | Cost-effective, sufficient for transcription fusion |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio::time::timeout` | 1.x | Per-provider timeout wrapper | Prevent slow provider from blocking |
| `serde_json` | 1.x | JSON serialization | Already present for API responses |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| LLM fusion | ROVER voting | ROVER is faster but LLMs handle context better |
| `join!` | `select!` | select cancels others on first completion - not wanted |
| Single fusion call | Separate fusion + polish | Two LLM calls doubles latency and cost |
| GPT-4o-mini | Local LLM | Local avoids API cost but adds deployment complexity |

**Installation:**
```bash
# No new dependencies - all already present
# Just need to restructure transcription module
```

## Architecture Patterns

### Recommended Project Structure

```
src-tauri/src/transcription/
├── mod.rs              # Module exports + ensemble orchestrator
├── whisper.rs          # Groq/OpenAI client (existing)
├── gladia.rs           # Gladia client (existing)
├── ensemble.rs         # NEW: Parallel execution + result collection
├── fusion.rs           # NEW: LLM fusion prompt and logic
├── polish.rs           # Updated: Combined fusion+polish prompt
└── pipeline.rs         # Updated: Ensemble mode support
```

### Pattern 1: Parallel Provider Execution with Graceful Degradation

**What:** Execute all available providers in parallel, continue even if some fail.
**When to use:** Ensemble transcription where we want results from all providers.

```rust
// Source: Tokio docs + ensemble architecture
// src-tauri/src/transcription/ensemble.rs
use tokio::time::{timeout, Duration};
use std::collections::HashMap;

/// Result from a single transcription provider
#[derive(Debug, Clone)]
pub struct ProviderResult {
    pub provider: String,
    pub text: String,
    pub latency_ms: u64,
}

/// Transcribe audio with all available providers in parallel
/// Returns results from all providers that succeeded (minimum 1 required)
pub async fn transcribe_ensemble(
    audio_path: &str,
    openai_key: Option<&str>,
    groq_key: Option<&str>,
    gladia_key: Option<&str>,
) -> Result<Vec<ProviderResult>, String> {
    use crate::transcription::{transcribe_audio, transcribe_audio_gladia};

    const PROVIDER_TIMEOUT: Duration = Duration::from_secs(45);

    // Build futures for available providers
    let mut futures: Vec<_> = Vec::new();

    // OpenAI (if key available)
    if let Some(key) = openai_key {
        let key = key.to_string();
        let path = audio_path.to_string();
        futures.push(("OpenAI".to_string(), Box::pin(async move {
            let start = std::time::Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio(&key, &path)).await;
            (result, start.elapsed().as_millis() as u64)
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>));
    }

    // Groq (if key available) - reuses whisper.rs with Groq URL
    if let Some(key) = groq_key {
        let key = key.to_string();
        let path = audio_path.to_string();
        futures.push(("Groq".to_string(), Box::pin(async move {
            let start = std::time::Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio(&key, &path)).await;
            (result, start.elapsed().as_millis() as u64)
        })));
    }

    // Gladia (if key available)
    if let Some(key) = gladia_key {
        let key = key.to_string();
        let path = audio_path.to_string();
        futures.push(("Gladia".to_string(), Box::pin(async move {
            let start = std::time::Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio_gladia(&key, &path)).await;
            (result, start.elapsed().as_millis() as u64)
        })));
    }

    if futures.is_empty() {
        return Err("No API keys configured for ensemble mode".to_string());
    }

    // Execute all in parallel using join_all
    let results = futures::future::join_all(
        futures.into_iter().map(|(name, fut)| async move {
            let (result, latency) = fut.await;
            (name, result, latency)
        })
    ).await;

    // Collect successful results
    let mut successful: Vec<ProviderResult> = Vec::new();
    for (provider, result, latency) in results {
        match result {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                successful.push(ProviderResult {
                    provider,
                    text: text.trim().to_string(),
                    latency_ms: latency,
                });
            }
            Ok(Err(e)) => {
                eprintln!("[Ensemble] {} failed: {}", provider, e);
            }
            Err(_) => {
                eprintln!("[Ensemble] {} timed out", provider);
            }
            _ => {
                eprintln!("[Ensemble] {} returned empty result", provider);
            }
        }
    }

    if successful.is_empty() {
        return Err("All transcription providers failed".to_string());
    }

    Ok(successful)
}
```

### Pattern 2: LLM Fusion Prompt

**What:** Present multiple transcriptions to LLM for intelligent fusion.
**When to use:** Combining outputs from 2+ providers into single best result.

```rust
// Source: Multi-ASR fusion research (arxiv.org/abs/2506.11089)
// src-tauri/src/transcription/fusion.rs

/// System prompt for multi-transcription fusion
/// Combines fusion with polish for single LLM call
pub const FUSION_SYSTEM_PROMPT: &str = r#"You are a transcription expert. You will receive multiple transcriptions of the same audio from different speech recognition systems.

Your task:
1. ANALYZE all transcriptions to identify the most accurate version
2. RESOLVE disagreements by choosing the most likely correct words based on:
   - Agreement across multiple systems (consensus is usually correct)
   - Acoustic plausibility (words that sound similar in speech)
   - Grammatical correctness and natural language flow
   - Context and semantic coherence
3. CLEAN UP the result: remove filler words (um, uh, like), fix grammar, add punctuation
4. PRESERVE the speaker's intent and any language mixing (French/English stays mixed)

Output ONLY the final, cleaned transcription. No explanations."#;

/// Build user prompt with all provider results
pub fn build_fusion_prompt(results: &[ProviderResult], dictionary: &[DictionaryEntry]) -> String {
    let mut prompt = String::from("Here are the transcriptions from different systems:\n\n");

    for result in results {
        prompt.push_str(&format!("=== {} ===\n{}\n\n", result.provider, result.text));
    }

    prompt.push_str("Provide the single best transcription.");

    // Append dictionary if available
    if !dictionary.is_empty() {
        prompt.push_str("\n\nPERSONAL DICTIONARY (use these exact spellings):\n");
        for entry in dictionary {
            prompt.push_str(&format!("- {} -> {}\n", entry.original, entry.correction));
        }
    }

    prompt
}
```

### Pattern 3: Simplified join! for Fixed Provider Set

**What:** Use `tokio::join!` directly when all three providers are known at compile time.
**When to use:** Cleaner code when provider set is static.

```rust
// Source: Tokio docs (docs.rs/tokio/latest/tokio/macro.join.html)
// Alternative simpler pattern for fixed providers

async fn transcribe_all_providers(
    audio_path: &str,
    openai_key: &str,
    groq_key: &str,
    gladia_key: &str,
) -> (Result<String, String>, Result<String, String>, Result<String, String>) {
    tokio::join!(
        transcribe_openai(openai_key, audio_path),
        transcribe_groq(groq_key, audio_path),
        transcribe_gladia(gladia_key, audio_path),
    )
}
```

### Pattern 4: Settings Integration

**What:** Add ensemble mode toggle to settings with provider requirements.
**When to use:** User configuration for ensemble vs. single provider mode.

```rust
// src-tauri/src/settings/store.rs (additions)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub ai_polish_enabled: bool,
    pub shortcut: String,
    pub transcription_provider: TranscriptionProvider,
    #[serde(default)]
    pub ensemble_enabled: bool,  // NEW: Enable multi-provider ensemble
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_polish_enabled: true,
            shortcut: "Alt+Space".to_string(),
            transcription_provider: TranscriptionProvider::Gladia,
            ensemble_enabled: false,  // Off by default
        }
    }
}
```

### Anti-Patterns to Avoid

- **Sequential provider calls:** Using `await` sequentially wastes time. Always use `join!` for parallel execution.
- **Failing fast on first error:** Don't use `try_join!` - we want all results even if some fail.
- **String-matching fusion:** Don't try to implement ROVER or edit-distance voting - LLM handles this better.
- **Separate fusion and polish calls:** Combining them saves an API call and ~500ms latency.
- **Blocking on slowest provider:** Use timeouts to prevent Gladia's polling from blocking everything.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parallel execution | Manual thread spawning | `tokio::join!` / `join_all` | Proper async, no thread overhead |
| Word alignment | Levenshtein/DTW alignment | LLM contextual fusion | LLM understands semantics, not just strings |
| ROVER voting | Custom voting algorithm | GPT-4o-mini fusion | Research shows LLM outperforms voting |
| Provider timeout | Manual timer management | `tokio::time::timeout` | Clean cancellation, proper async |
| Error aggregation | Custom error types | `Vec<ProviderResult>` with filtering | Simple, pragmatic approach |

**Key insight:** Multi-ASR research shows that "the textual LLM outperforms the ensemble approach and all individual ASRs" - LLM fusion is not just simpler, it's more accurate than traditional methods.

## Common Pitfalls

### Pitfall 1: Gladia Timeout Blocking Other Providers

**What goes wrong:** Gladia's polling can take 30 seconds, blocking the entire ensemble.
**Why it happens:** Gladia uses async polling while Groq/OpenAI return immediately.
**How to avoid:**
1. Use per-provider timeouts with `tokio::time::timeout`
2. Set aggressive timeout (15-20s) for ensemble mode
3. Continue with partial results if one provider times out
**Warning signs:** Ensemble mode is consistently slow even when 2 providers finish quickly.

### Pitfall 2: API Key Not Available for All Providers

**What goes wrong:** User enables ensemble but only has one API key configured.
**Why it happens:** Ensemble requires multiple keys but UI doesn't enforce this.
**How to avoid:**
1. Check available keys before enabling ensemble in settings
2. Require minimum 2 configured keys to enable ensemble
3. Show which providers will be used based on available keys
**Warning signs:** Ensemble produces same quality as single provider.

### Pitfall 3: LLM Hallucination in Fusion

**What goes wrong:** LLM adds words not present in any transcription.
**Why it happens:** Low temperature not enforced, or prompt allows creative freedom.
**How to avoid:**
1. Use temperature 0.1 or lower for fusion
2. Explicit prompt instruction: "Use ONLY words from the provided transcriptions"
3. Include constraint: "Do not add information not present in any transcription"
**Warning signs:** Fused output contains words none of the providers produced.

### Pitfall 4: Double Polish

**What goes wrong:** Text is polished twice - once in fusion, once in polish step.
**Why it happens:** Ensemble path still calls polish_text after fusion.
**How to avoid:**
1. Fusion prompt includes polish instructions
2. Skip separate polish step when ensemble is enabled
3. Single code path: ensemble returns already-polished text
**Warning signs:** Overly formal output, lost speaker voice.

### Pitfall 5: Memory Bloat with Audio Bytes

**What goes wrong:** Audio file read 3x into memory for 3 providers.
**Why it happens:** Each provider call reads audio_path independently.
**How to avoid:**
1. Read audio bytes once, pass to all providers
2. Use `Arc<Vec<u8>>` for shared ownership
3. Or accept the 3x read if audio is small (<1MB typical)
**Warning signs:** Memory spikes during ensemble transcription.

### Pitfall 6: Race Condition with Provider Settings

**What goes wrong:** User changes provider setting while ensemble is running.
**Why it happens:** Settings read isn't atomic with transcription execution.
**How to avoid:**
1. Capture settings at start of recording
2. Don't re-read settings during pipeline
3. Already implemented pattern in existing code
**Warning signs:** Intermittent unexpected provider usage.

## Code Examples

### Complete Ensemble Pipeline Integration

```rust
// src-tauri/src/transcription/pipeline.rs (updated)

use crate::settings::get_settings;
use crate::transcription::ensemble::{transcribe_ensemble, ProviderResult};
use crate::transcription::fusion::{fuse_and_polish};
use crate::transcription::polish::polish_text;

pub async fn process_recording(app: &AppHandle, audio_path: String) -> Result<String, String> {
    set_state(app, RecordingState::Processing);

    let settings = get_settings();

    // Check for minimum audio size (existing)
    // ...

    let final_text = if settings.ensemble_enabled {
        // Ensemble mode: parallel transcription + LLM fusion
        process_ensemble(app, &audio_path, &settings).await?
    } else {
        // Single provider mode (existing behavior)
        process_single_provider(app, &audio_path, &settings).await?
    };

    // Stage 3: Paste (existing)
    // ...

    Ok(final_text)
}

async fn process_ensemble(
    app: &AppHandle,
    audio_path: &str,
    settings: &Settings,
) -> Result<String, String> {
    emit_progress(app, "transcribing", "Transcribing (ensemble)...");

    // Get all available API keys
    let openai_key = get_api_key_internal(app).ok().flatten();
    let groq_key = get_groq_api_key_internal(app).ok().flatten();
    let gladia_key = get_gladia_api_key_internal(app).ok().flatten();

    // Run all providers in parallel
    let results = transcribe_ensemble(
        audio_path,
        openai_key.as_deref(),
        groq_key.as_deref(),
        gladia_key.as_deref(),
    ).await?;

    println!("[Ensemble] Got {} provider results", results.len());
    for r in &results {
        println!("[Ensemble] {}: {} chars in {}ms", r.provider, r.text.len(), r.latency_ms);
    }

    // If only one result, use it directly (skip fusion overhead)
    if results.len() == 1 {
        let text = &results[0].text;
        if settings.ai_polish_enabled {
            emit_progress(app, "polishing", "Processing...");
            return polish_text(&get_api_key_internal(app)?.unwrap(), text).await
                .unwrap_or_else(|_| text.clone());
        }
        return Ok(text.clone());
    }

    // Multiple results: fuse with LLM (includes polish)
    emit_progress(app, "polishing", "Fusing transcriptions...");
    let openai_key = get_api_key_internal(app)?
        .ok_or("OpenAI API key required for ensemble fusion")?;

    fuse_and_polish(&openai_key, &results).await
}
```

### Fusion + Polish Combined Call

```rust
// src-tauri/src/transcription/fusion.rs

use crate::dictionary::get_dictionary;
use super::polish::{ChatRequest, ChatMessage, ChatResponse};

const CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

pub async fn fuse_and_polish(
    api_key: &str,
    results: &[ProviderResult],
) -> Result<String, String> {
    let dictionary = get_dictionary();
    let user_prompt = build_fusion_prompt(results, &dictionary);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let request = ChatRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: FUSION_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        temperature: 0.1,  // Very low for consistency
        max_tokens: 4096,
    };

    let response = client
        .post(CHAT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Fusion request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Fusion API error: {}", response.status()));
    }

    let chat_response: ChatResponse = response.json().await
        .map_err(|e| format!("Failed to parse fusion response: {}", e))?;

    chat_response.choices
        .first()
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| "Empty fusion response".to_string())
}
```

### Settings UI Addition

```typescript
// src/windows/Settings.tsx (additions to Transcription section)

{/* Ensemble Mode Toggle */}
<div className="flex items-center justify-between mt-6 pt-6 border-t border-gray-200 dark:border-gray-700">
  <div className="flex-1 pr-4">
    <p className="text-gray-900 dark:text-white font-medium">
      Ensemble Mode
    </p>
    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
      Use multiple providers for higher accuracy (requires OpenAI key for fusion)
    </p>
    {ensembleEnabled && (
      <p className="text-xs text-blue-500 mt-1">
        Active providers: {availableProviders.join(', ')}
      </p>
    )}
  </div>
  <Toggle
    enabled={ensembleEnabled}
    onChange={handleEnsembleToggle}
    disabled={loading || availableProviderCount < 2}
  />
</div>

{ensembleEnabled && availableProviderCount < 2 && (
  <p className="text-xs text-amber-600 dark:text-amber-400 mt-2">
    Configure at least 2 provider API keys to enable ensemble mode
  </p>
)}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single ASR provider | Multi-ASR ensemble | 2025 | 10-40% WER reduction |
| ROVER voting | LLM-based fusion | 2025 | Better context handling |
| Sequential API calls | Parallel with `join!` | Standard | 2-3x faster total time |
| Separate fusion/polish | Combined single call | Optimization | 50% fewer API calls |

**Deprecated/outdated:**
- ROVER and traditional voting: Still works but LLM fusion is more accurate
- Edit-distance alignment: Replaced by LLM contextual understanding
- Sequential provider calls: Inefficient when parallel is possible

## Open Questions

1. **Optimal Provider Combination**
   - What we know: Different providers excel at different audio types
   - What's unclear: Which 2-provider combination gives best cost/accuracy tradeoff
   - Recommendation: Default to Gladia + OpenAI (best multilingual + accuracy)

2. **Fusion Fallback When OpenAI Unavailable**
   - What we know: Fusion requires OpenAI key for GPT-4o-mini
   - What's unclear: Can Groq's LLM or local model substitute?
   - Recommendation: Require OpenAI key for ensemble; fallback to single provider if unavailable

3. **Cost vs. Accuracy Tradeoff**
   - What we know: Ensemble uses 3x transcription + 1x fusion API calls
   - What's unclear: Is cost justified for typical voice notes?
   - Recommendation: Default OFF; power users can enable. Show estimated cost in UI.

4. **Timeout Tuning**
   - What we know: Gladia can take 5-30s; Groq/OpenAI are <5s
   - What's unclear: Optimal per-provider timeout values
   - Recommendation: Start with 15s global timeout, tune based on user feedback

## Sources

### Primary (HIGH confidence)
- [Tokio join! macro](https://docs.rs/tokio/latest/tokio/macro.join.html) - Parallel execution patterns
- [Tokio try_join! macro](https://docs.rs/tokio/latest/tokio/macro.try_join.html) - Error handling patterns
- [futures join_all](https://docs.rs/futures/latest/futures/future/fn.join_all.html) - Dynamic future collections
- Existing TTP codebase - Provider implementations, pipeline architecture

### Secondary (MEDIUM confidence)
- [Multi-ASR Fusion with LLM (arxiv:2506.11089)](https://arxiv.org/abs/2506.11089) - LLM fusion methodology
- [Multi-stage LLM ASR Correction (arxiv:2310.11532)](https://arxiv.org/html/2310.11532v2) - 10-20% WER improvements
- [NextLevel AI Multi-Model Strategy](https://nextlevel.ai/best-speech-to-text-models/) - 40% error reduction with multi-model
- [Northflank STT Benchmarks 2026](https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks) - Current model comparisons

### Tertiary (LOW confidence)
- Fusion prompt wording - Based on research patterns, needs empirical tuning
- Per-provider timeout values - Reasonable estimates, not production-tested

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Using existing Tokio patterns and codebase infrastructure
- Architecture patterns: HIGH - Based on official Tokio docs and existing code structure
- LLM fusion approach: MEDIUM - Research-backed but prompt needs empirical tuning
- Pitfalls: MEDIUM - Some based on general async patterns, not TTP-specific testing

**Research date:** 2026-01-30
**Valid until:** 2026-03-01 (Provider APIs stable; may update if new fusion research emerges)

---

*Phase: 05-ensemble-transcription*
*Research completed: 2026-01-30*
