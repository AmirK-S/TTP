// TTP - Talk To Paste
// GPT-4o-mini text polish API client

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

/// OpenAI chat completions API endpoint
const CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Request timeout in seconds
const REQUEST_TIMEOUT_SECS: u64 = 15;

/// System prompt for transcription polishing
/// Based on CONTEXT.md decisions for filler removal, self-correction, and tone preservation
pub const POLISH_SYSTEM_PROMPT: &str = r#"You are a transcription editor. Your job is to clean up voice transcriptions while preserving the speaker's voice and intent.

Rules:
1. Remove ALL filler words: um, uh, like (when used as filler), you know, sort of, kind of, basically, literally (when meaningless)
2. Fix obvious grammar errors but preserve casual speech patterns — do NOT make casual speech formal
3. Add proper punctuation and capitalization (periods, commas, question marks)
4. Handle self-corrections: when someone says "Tuesday no wait Wednesday" or "Send it Monday. Actually make that Tuesday", keep ONLY the final corrected version
5. Preserve the speaker's exact tone — don't elevate formality
6. If uncertain whether something is a correction, preserve the original verbatim

Return ONLY the cleaned text, nothing else."#;

/// Chat completion request body
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

/// Chat message structure
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Chat completion response body
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

/// Individual choice in chat response
#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

/// Message content in chat response
#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

/// Polish raw transcription text using GPT-4o-mini
///
/// Removes filler words, fixes grammar, handles self-corrections,
/// and adds proper punctuation while preserving the speaker's tone.
///
/// # Arguments
/// * `api_key` - OpenAI API key
/// * `raw_text` - Raw transcription text to polish
///
/// # Returns
/// * `Ok(String)` - Polished text on success
/// * `Err(String)` - Error message on failure
pub async fn polish_text(api_key: &str, raw_text: &str) -> Result<String, String> {
    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Build request body
    let request_body = ChatRequest {
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
        temperature: 0.3, // Low temperature for consistency
        max_tokens: 1024,
    };

    // Retry loop with exponential backoff
    let mut last_error = String::new();
    for attempt in 0..MAX_RETRIES {
        // Calculate backoff delay: 500ms, 1000ms, 1500ms
        if attempt > 0 {
            let delay_ms = 500 * (attempt as u64);
            sleep(Duration::from_millis(delay_ms)).await;
        }

        // Make the request
        match client
            .post(CHAT_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    // Parse response JSON
                    let chat_response: ChatResponse = response
                        .json()
                        .await
                        .map_err(|e| format!("Failed to parse polish response: {}", e))?;

                    // Extract content from first choice
                    let content = chat_response
                        .choices
                        .into_iter()
                        .next()
                        .map(|choice| choice.message.content)
                        .ok_or_else(|| "Empty response from polish API".to_string())?;

                    // Trim whitespace and return
                    let trimmed = content.trim().to_string();
                    if trimmed.is_empty() {
                        return Err("Empty response from polish API".to_string());
                    }
                    return Ok(trimmed);
                } else {
                    // HTTP error - capture for potential retry
                    let error_body = response.text().await.unwrap_or_default();
                    last_error = format!("Polish API error: {} - {}", status, error_body);

                    // Don't retry on client errors (4xx) except rate limits (429)
                    if status.is_client_error() && status.as_u16() != 429 {
                        return Err(last_error);
                    }
                }
            }
            Err(e) => {
                // Network error - will retry
                last_error = format!("Polish request failed: {}", e);
            }
        }
    }

    // All retries exhausted
    Err(last_error)
}
