// TTP - Talk To Paste
// GPT-4o-mini text polish API client

use crate::dictionary::{get_dictionary, DictionaryEntry};
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
pub const POLISH_SYSTEM_PROMPT: &str = r#"Clean up this voice transcription. Output the COMPLETE cleaned text.

RULES:
1. Keep ALL content - do NOT remove or shorten anything
2. NEVER translate - keep original language(s) exactly (French stays French, English stays English, mixed stays mixed)
3. Remove only filler words: um, uh, like (as filler), you know, basically
4. Fix grammar but keep casual tone
5. Add punctuation
6. Self-corrections only: "Tuesday no wait Wednesday" → "Wednesday"

Output the FULL cleaned transcription, nothing else."#;

/// Build the polish system prompt, optionally including dictionary terms
///
/// If dictionary contains entries, appends a PERSONAL DICTIONARY section
/// instructing the AI to use those exact spellings.
pub fn build_polish_prompt(dictionary: &[DictionaryEntry]) -> String {
    if dictionary.is_empty() {
        return POLISH_SYSTEM_PROMPT.to_string();
    }

    let mut prompt = POLISH_SYSTEM_PROMPT.to_string();
    prompt.push_str("\n\nPERSONAL DICTIONARY (use these exact spellings):\n");

    for entry in dictionary {
        prompt.push_str(&format!("- {} -> {}\n", entry.original, entry.correction));
    }

    prompt
}

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
    // Load dictionary for personalized corrections
    let dictionary = get_dictionary();
    let system_prompt = build_polish_prompt(&dictionary);

    if !dictionary.is_empty() {
        println!("[Polish] Using {} dictionary entries", dictionary.len());
    }

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
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: raw_text.to_string(),
            },
        ],
        temperature: 0.1, // Very low for consistency
        max_tokens: 4096, // Ensure full output
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
