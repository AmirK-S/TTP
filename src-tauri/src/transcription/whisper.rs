// Groq Whisper transcription API client

use crate::http_client::shared as shared_http;
use crate::logging::log_error;
use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::time::sleep;

/// Compute a retry sleep with ±25% jitter from `base_ms`.
///
/// Jitter avoids retry stampedes when many clients hit the same upstream
/// rate-limit at once. Source of randomness is the low bits of the current
/// monotonic clock — perfectly fine for jitter (we don't need crypto-grade
/// entropy and pulling in `rand` for this would be overkill).
fn jittered_backoff_ms(base_ms: u64) -> u64 {
    // Take ~10 bits of entropy from the nanosecond portion of the clock.
    let entropy = Instant::now().elapsed().subsec_nanos() ^ base_ms as u32;
    // Map to [0, 1) then to [-0.5, 0.5), then scale to ±25% of base.
    let frac = (entropy & 0x3FF) as f32 / 1024.0; // [0, 1)
    let signed = frac - 0.5;                       // [-0.5, 0.5)
    let delta = signed * 0.5 * base_ms as f32;     // ±25% of base
    (base_ms as f32 + delta).max(1.0) as u64
}

/// Groq transcription API endpoint (uses whisper-large-v3)
const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Base request timeout in seconds (scales up with file size)
const BASE_TIMEOUT_SECS: u64 = 30;

/// Transcribe audio file using Groq (whisper-large-v3 model)
///
/// Implements retry logic with exponential backoff (500ms, 1000ms, 1500ms).
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `audio_path` - Path to the audio file (WAV format)
///
/// # Returns
/// * `Ok(String)` - Transcription text on success
/// * `Err(String)` - Error message on failure
pub async fn transcribe_audio(api_key: &str, audio_path: &str, prompt: Option<&str>) -> Result<String, String> {
    transcribe_with_provider(api_key, audio_path, GROQ_TRANSCRIPTION_URL, "whisper-large-v3", "Groq", prompt).await
}

/// Internal function to transcribe audio with a specific provider
///
/// Implements retry logic with exponential backoff (500ms, 1000ms, 1500ms).
async fn transcribe_with_provider(
    api_key: &str,
    audio_path: &str,
    transcription_url: &str,
    model: &str,
    _provider_name: &str,
    prompt: Option<&str>,
) -> Result<String, String> {
    // Convert model to owned String for Form::text (requires 'static)
    let model = model.to_string();

    // Read audio file bytes
    let audio_bytes = fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Get filename and MIME type from path for the multipart form
    let filename = Path::new(audio_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav")
        .to_string();

    let mime_type = "audio/wav";

    // Scale timeout based on file size: base + 2s per MB.
    // Applied per-request below so the shared client (which has no global
    // timeout) can be reused across calls of different sizes.
    let file_mb = audio_bytes.len() as u64 / (1024 * 1024);
    let timeout_secs = BASE_TIMEOUT_SECS + file_mb * 2;

    // Reuse the process-wide HTTP client so we keep TCP/TLS connections warm
    // across whisper -> polish -> next-recording calls.
    let client = shared_http();

    // Retry loop with exponential backoff
    let mut last_error = String::new();
    for attempt in 0..MAX_RETRIES {
        // Calculate backoff delay with ±25% jitter: ~500ms, ~1000ms, ~1500ms.
        // Jitter prevents thundering-herd retries when Groq rate-limits us.
        if attempt > 0 {
            let delay_ms = jittered_backoff_ms(500 * (attempt as u64));
            sleep(Duration::from_millis(delay_ms)).await;
        }

        // Build multipart form - need to recreate each attempt since Part consumes bytes
        let file_part = Part::bytes(audio_bytes.clone())
            .file_name(filename.clone())
            .mime_str(mime_type)
            .map_err(|e| format!("Failed to set MIME type: {}", e))?;

        let mut form = Form::new()
            .text("model", model.clone())
            .text("response_format", "text")
            .text("temperature", "0")
            .part("file", file_part);

        if let Some(prompt_value) = prompt {
            form = form.text("prompt", prompt_value.to_string());
        }

        // Make the request. Per-request timeout scales with audio size and
        // is applied here (not on the shared client) so other call sites
        // keep their own timeouts.
        match client
            .post(transcription_url)
            .timeout(Duration::from_secs(timeout_secs))
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    // Parse response text
                    let text = response
                        .text()
                        .await
                        .map(|text| text.trim().to_string())
                        .map_err(|e| format!("Failed to read transcription response: {}", e))?;
                    return Ok(text);
                } else {
                    // HTTP error - capture for potential retry. Status code is
                    // included in the error string verbatim so the pipeline can
                    // detect 429/401/403 and surface a friendlier message.
                    let error_body = response.text().await.unwrap_or_default();
                    let status_code = status.as_u16();
                    last_error = format!("Transcription API error: {} - {}", status, error_body);
                    log_error(&format!("API error {}: {}", status, &error_body[..error_body.len().min(300)]));

                    // Don't retry on client errors (4xx) except rate limits (429)
                    if status.is_client_error() && status_code != 429 {
                        return Err(last_error);
                    }
                }
            }
            Err(e) => {
                // Network error - will retry
                last_error = format!("Transcription request failed: {}", e);
                log_error(&format!("Network error: {}", e));
            }
        }
    }

    // All retries exhausted
    Err(last_error)
}
