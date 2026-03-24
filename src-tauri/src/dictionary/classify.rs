// TTP - Talk To Paste
// LLM classification gate for dictionary auto-detection
//
// Before adding a correction to the dictionary, asks the LLM to classify it
// as LEARN (proper nouns, brands, technical terms) or IGNORE (grammar, style).

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Groq chat completions API endpoint (OpenAI-compatible)
const CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Request timeout in seconds (short — this is non-critical)
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// System prompt for correction classification
const CLASSIFY_SYSTEM_PROMPT: &str = r#"You classify corrections from a speech-to-text app. Given an original transcribed word and the user's correction, respond with EXACTLY one word: LEARN or IGNORE.

LEARN: The correction is a proper noun (person name, company, brand, place), technical term, acronym, or accent fix that the speech engine misspelled.
IGNORE: The correction is a grammar fix, conjugation change, style preference, capitalization change, punctuation edit, or common word substitution.

Examples:
"Whysper" → "Whisper" → LEARN (brand name)
"Grok" → "Groq" → LEARN (company name)
"parris" → "Paris" → LEARN (place name)
"resultats" → "résultats" → LEARN (accent fix)
"AmirKs" → "AmirKS" → LEARN (personal name)
"fait" → "fais" → IGNORE (verb conjugation)
"dont" → "don't" → IGNORE (contraction)
"commence" → "start" → IGNORE (synonym)
"bonjour" → "Bonjour" → IGNORE (capitalization)
"les" → "des" → IGNORE (article swap)"#;

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

/// Classify a correction as LEARN (true) or IGNORE (false) using the LLM.
///
/// Calls the Groq API to determine whether a detected correction should be
/// added to the dictionary. Returns `true` for proper nouns, brands, technical
/// terms, and accent fixes. Returns `false` for grammar, style, and common words.
///
/// Fails closed: if the LLM call fails for any reason, returns `false`
/// (do not add to dictionary).
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `original` - The original transcribed word
/// * `correction` - The user's correction
/// * `context_sentence` - The surrounding sentence for context
pub async fn classify_correction(
    api_key: &str,
    original: &str,
    correction: &str,
    context_sentence: &str,
) -> Result<bool, String> {
    let user_content = format!(
        "Original: \"{}\"\nCorrection: \"{}\"\nContext: \"{}\"",
        original, correction, context_sentence
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_body = ChatRequest {
        model: "llama-3.3-70b-versatile".to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: CLASSIFY_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content,
            },
        ],
        temperature: 0.0,
        max_tokens: 16,
    };

    // Single attempt, no retries — this is non-critical
    let response = client
        .post(CHAT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Classify request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("Classify API error: {} - {}", status, error_body));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse classify response: {}", e))?;

    let content = chat_response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| "Empty response from classify API".to_string())?;

    let trimmed = content.trim().to_uppercase();
    Ok(trimmed.contains("LEARN"))
}
