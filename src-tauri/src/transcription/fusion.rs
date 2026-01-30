// TTP - Talk To Paste
// LLM fusion - combines multiple transcriptions into optimal output

use crate::dictionary::{get_dictionary, DictionaryEntry};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

use super::ensemble::ProviderResult;

/// OpenAI chat completions API endpoint
const CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Request timeout in seconds (slightly longer than polish for fusion complexity)
const REQUEST_TIMEOUT_SECS: u64 = 20;

/// System prompt for multi-transcription fusion
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

/// Build the fusion user prompt from multiple provider results
///
/// Formats all transcriptions with provider labels and appends dictionary if available.
fn build_fusion_prompt(results: &[ProviderResult], dictionary: &[DictionaryEntry]) -> String {
    let mut prompt = String::from("Here are the transcriptions from different speech recognition systems:\n\n");

    for result in results {
        prompt.push_str(&format!(
            "=== {} ({}ms) ===\n{}\n\n",
            result.provider, result.latency_ms, result.text
        ));
    }

    prompt.push_str("Please fuse these into the most accurate single transcription.");

    // Append dictionary if not empty (same format as polish.rs)
    if !dictionary.is_empty() {
        prompt.push_str("\n\nPERSONAL DICTIONARY (use these exact spellings):\n");
        for entry in dictionary {
            prompt.push_str(&format!("- {} -> {}\n", entry.original, entry.correction));
        }
    }

    prompt
}

/// Fuse multiple transcriptions into a single optimal output using GPT-4o-mini
///
/// Takes results from multiple providers and uses an LLM to combine them,
/// resolving disagreements and producing the most accurate transcription.
///
/// # Arguments
/// * `api_key` - OpenAI API key
/// * `results` - Results from multiple transcription providers
///
/// # Returns
/// * `Ok(String)` - Fused transcription on success
/// * `Err(String)` - Error message on failure
pub async fn fuse_and_polish(api_key: &str, results: &[ProviderResult]) -> Result<String, String> {
    if results.is_empty() {
        return Err("No transcription results to fuse".to_string());
    }

    // Load dictionary for personalized corrections
    let dictionary = get_dictionary();
    let user_prompt = build_fusion_prompt(results, &dictionary);

    if !dictionary.is_empty() {
        println!("[Fusion] Using {} dictionary entries", dictionary.len());
    }

    println!(
        "[Fusion] Fusing {} provider results: {}",
        results.len(),
        results
            .iter()
            .map(|r| r.provider.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

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
                content: FUSION_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
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
                        .map_err(|e| format!("Failed to parse fusion response: {}", e))?;

                    // Extract content from first choice
                    let content = chat_response
                        .choices
                        .into_iter()
                        .next()
                        .map(|choice| choice.message.content)
                        .ok_or_else(|| "Empty response from fusion API".to_string())?;

                    // Trim whitespace and return
                    let trimmed = content.trim().to_string();
                    if trimmed.is_empty() {
                        return Err("Empty response from fusion API".to_string());
                    }

                    println!("[Fusion] Fusion complete: {} chars", trimmed.len());
                    return Ok(trimmed);
                } else {
                    // HTTP error - capture for potential retry
                    let error_body = response.text().await.unwrap_or_default();
                    last_error = format!("Fusion API error: {} - {}", status, error_body);

                    // Don't retry on client errors (4xx) except rate limits (429)
                    if status.is_client_error() && status.as_u16() != 429 {
                        return Err(last_error);
                    }
                }
            }
            Err(e) => {
                // Network error - will retry
                last_error = format!("Fusion request failed: {}", e);
            }
        }
    }

    // All retries exhausted
    Err(last_error)
}
