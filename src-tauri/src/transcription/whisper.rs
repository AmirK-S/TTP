// Groq Whisper transcription API client

use crate::logging::log_error;
use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

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
pub async fn transcribe_audio(api_key: &str, audio_path: &str, prompt: Option<&str>, language: Option<&str>) -> Result<String, String> {
    transcribe_with_provider(api_key, audio_path, GROQ_TRANSCRIPTION_URL, "whisper-large-v3", "Groq", prompt, language).await
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
    language: Option<&str>,
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

    // Scale timeout based on file size: base + 2s per MB
    let file_mb = audio_bytes.len() as u64 / (1024 * 1024);
    let timeout_secs = BASE_TIMEOUT_SECS + file_mb * 2;

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Retry loop with exponential backoff
    let mut last_error = String::new();
    for attempt in 0..MAX_RETRIES {
        // Calculate backoff delay: 500ms, 1000ms, 1500ms
        if attempt > 0 {
            let delay_ms = 500 * (attempt as u64);
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

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        // Make the request
        match client
            .post(transcription_url)
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
                    // HTTP error - capture for potential retry
                    let error_body = response.text().await.unwrap_or_default();
                    last_error = format!("Transcription API error: {} - {}", status, error_body);
                    log_error(&format!("API error {}: {}", status, &error_body[..error_body.len().min(300)]));

                    // Don't retry on client errors (4xx) except rate limits (429)
                    if status.is_client_error() && status.as_u16() != 429 {
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
