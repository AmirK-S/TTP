// TTP - Talk To Paste
// Whisper transcription API client - OpenAI gpt-4o-transcribe

use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

/// OpenAI transcription API endpoint
const TRANSCRIPTION_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Request timeout in seconds
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Transcribe audio file using OpenAI gpt-4o-transcribe API
///
/// Uses the gpt-4o-transcribe model which provides better accuracy than whisper-1.
/// Implements retry logic with exponential backoff (500ms, 1000ms, 1500ms).
///
/// # Arguments
/// * `api_key` - OpenAI API key
/// * `audio_path` - Path to the audio file (WAV format)
///
/// # Returns
/// * `Ok(String)` - Transcription text on success
/// * `Err(String)` - Error message on failure
pub async fn transcribe_audio(api_key: &str, audio_path: &str) -> Result<String, String> {
    // Read audio file bytes
    let audio_bytes = fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Get filename from path for the multipart form
    let filename = Path::new(audio_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav")
        .to_string();

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
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
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to set MIME type: {}", e))?;

        let form = Form::new()
            .text("model", "gpt-4o-transcribe")
            .text("response_format", "text")
            .text("prompt", "Transcribe exactly what is spoken, preserving all languages (English, French, etc.) without translating.")
            .part("file", file_part);

        // Make the request
        match client
            .post(TRANSCRIPTION_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    // Parse response text
                    return response
                        .text()
                        .await
                        .map(|text| text.trim().to_string())
                        .map_err(|e| format!("Failed to read transcription response: {}", e));
                } else {
                    // HTTP error - capture for potential retry
                    let error_body = response.text().await.unwrap_or_default();
                    last_error = format!("Transcription API error: {} - {}", status, error_body);

                    // Don't retry on client errors (4xx) except rate limits (429)
                    if status.is_client_error() && status.as_u16() != 429 {
                        return Err(last_error);
                    }
                }
            }
            Err(e) => {
                // Network error - will retry
                last_error = format!("Transcription request failed: {}", e);
            }
        }
    }

    // All retries exhausted
    Err(last_error)
}
