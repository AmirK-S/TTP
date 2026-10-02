// TTP - Talk To Paste
// LLM classification gate for dictionary auto-detection
//
// Before adding a correction to the dictionary, asks the LLM to classify it
// as LEARN (proper nouns, brands, technical terms) or IGNORE (grammar, style).

use crate::http_client::{retry_guidance, shared as shared_http};
use crate::logging::{log_error, log_warn};
use crate::transcription::polish::{MODEL, REASONING_EFFORT, REASONING_TOKEN_HEADROOM};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Groq chat completions API endpoint (OpenAI-compatible)
const CHAT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Request timeout in seconds (short — this is non-critical)
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Tokens for the visible answer: one word, LEARN or IGNORE.
///
/// This was the whole `max_tokens: 16` budget when the gate ran on
/// `llama-3.3-70b-versatile`. On gpt-oss (see [`MODEL`]) reasoning tokens
/// are spent first and count against the same cap, so 16 alone would be
/// eaten before the verdict is written: `content` comes back empty, the old
/// `contains("LEARN")` read that as IGNORE, and the dictionary would have
/// gone on learning nothing — silently, just by a different mechanism than
/// the 404. The reasoning headroom polish uses is added on top.
const VERDICT_TOKENS: u32 = 16;

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

/// Chat completion request body.
///
/// Same shape as polish's request minus `response_format`: the gate asks for
/// a bare word, not JSON, so the reply format it parses is unchanged by the
/// model migration.
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ChatMessage>,
    temperature: f32,
    /// `max_tokens` is deprecated on OpenAI-compatible endpoints and, on
    /// reasoning models, does not reliably account for reasoning tokens.
    max_completion_tokens: u32,
    reasoning_effort: &'static str,
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

/// Message content in chat response.
///
/// Optional: a reasoning model that runs out of budget mid-thought can return
/// `"content": null` (its reasoning lives in a separate field). That must be
/// a reported failure, not a deserialization error that hides the cause.
#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    #[serde(default)]
    content: Option<String>,
}

/// Build the request body. Pure, so the model and budget it sends are
/// unit-tested without a network call.
fn build_request(original: &str, correction: &str, context_sentence: &str) -> ChatRequest {
    let user_content = format!(
        "Original: \"{}\"\nCorrection: \"{}\"\nContext: \"{}\"",
        original, correction, context_sentence
    );
    ChatRequest {
        model: MODEL,
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
        max_completion_tokens: VERDICT_TOKENS + REASONING_TOKEN_HEADROOM,
        reasoning_effort: REASONING_EFFORT,
    }
}

/// Read the verdict out of the model's reply: `Some(true)` for LEARN,
/// `Some(false)` for IGNORE, `None` if it names neither.
///
/// Whichever keyword appears first wins, so "LEARN (brand name)" and
/// "IGNORE." both read correctly, and so does a reply that explains itself
/// ("IGNORE — not a LEARN case"), which the old `contains("LEARN")` read
/// backwards. A reply naming neither is an error, not an IGNORE: an empty or
/// garbled answer is exactly how a broken model looks, and folding it into
/// IGNORE is how the gate went quiet.
fn parse_verdict(content: &str) -> Option<bool> {
    let upper = content.to_uppercase();
    match (upper.find("LEARN"), upper.find("IGNORE")) {
        (Some(l), Some(i)) => Some(l < i),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    }
}

/// Parse a successful chat-completions body into a verdict.
fn parse_response_body(body: &str) -> Result<bool, String> {
    let chat_response: ChatResponse = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse classify response: {}", e))?;

    let content = chat_response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| "Empty response from classify API".to_string())?
        .message
        .content
        .unwrap_or_default();

    parse_verdict(&content).ok_or_else(|| {
        // Length only: the reply may quote the user's dictation back.
        format!(
            "Classify reply named neither LEARN nor IGNORE (model={}, reply_len={})",
            MODEL,
            content.chars().count()
        )
    })
}

/// How loudly to report a failed classify call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    /// Transient; the next correction will likely go through.
    Warn,
    /// Will fail the same way on every correction until someone acts.
    Error,
}

/// Describe a non-2xx reply. Pure — the policy is unit-tested as such.
///
/// The message keeps the status code verbatim and names the model: when the
/// gate stops learning, "which model were we asking for" is the first
/// question, and a name the provider has retired is the usual answer.
fn describe_http_failure(status: u16, body: &str, retry_after_ms: Option<u64>) -> (Severity, String) {
    match status {
        404 => (
            Severity::Error,
            format!(
                "[Classify] Model `{}` not found (404) — the provider has likely retired it. \
                 Dictionary auto-learning is OFF until `polish::MODEL` is updated. Body: {}",
                MODEL, body
            ),
        ),
        401 | 403 => (
            Severity::Error,
            format!(
                "[Classify] Groq rejected the API key ({}); dictionary auto-learning is off. Body: {}",
                status, body
            ),
        ),
        // Single attempt by design (see below): a rate limit costs this one
        // correction, not the feature, so it is a warning. The server's own
        // delay goes in the line so a burst is readable from the log.
        429 => (
            Severity::Warn,
            format!(
                "[Classify] Rate limited (429, model={}, retry_after_ms={}); correction not learned",
                MODEL,
                retry_after_ms.map_or_else(|| "none".to_string(), |ms| ms.to_string())
            ),
        ),
        _ => (
            Severity::Error,
            format!("[Classify] API error {} (model={}): {}", status, MODEL, body),
        ),
    }
}

fn log_at(severity: Severity, message: &str) {
    match severity {
        Severity::Warn => log_warn(message),
        Severity::Error => log_error(message),
    }
}

/// Classify a correction as LEARN (true) or IGNORE (false) using the LLM.
///
/// Calls the Groq API to determine whether a detected correction should be
/// added to the dictionary. Returns `true` for proper nouns, brands, technical
/// terms, and accent fixes. Returns `false` for grammar, style, and common words.
///
/// Fails closed: if the LLM call fails for any reason, returns `Err` and the
/// caller does not add to the dictionary. Every failure is logged here first
/// (error for what will recur on every call — retired model, bad key, server
/// error, unreadable reply; warn for what is transient — rate limit, network),
/// because a gate that fails closed in silence looks exactly like a gate that
/// is working and finding nothing to learn.
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
    let request_body = build_request(original, correction, context_sentence);

    // Single attempt, no retries — this is non-critical. Unlike polish, no
    // one is waiting on this call, but a retry loop here would compete with
    // polish for the same free-tier TPM budget on the same model.
    let response = match shared_http()
        .post(CHAT_URL)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let message = format!(
                "[Classify] Request failed (model={}, timed_out={}): {}",
                MODEL,
                e.is_timeout(),
                e
            );
            log_warn(&message);
            return Err(message);
        }
    };

    let status = response.status();
    // Headers before the body: `text()` consumes the response, and
    // `retry-after` lives in the headers.
    let headers = response.headers().clone();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        let retry_after_ms = retry_guidance(&headers, &body).map(|g| g.delay_ms);
        let (severity, message) = describe_http_failure(status.as_u16(), &body, retry_after_ms);
        log_at(severity, &message);
        return Err(message);
    }

    parse_response_body(&body).inspect_err(|e| log_error(&format!("[Classify] {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The id Groq retired on 2026-08-16. Every classify call 404'd on it and
    /// the gate learned nothing, with no sign of it outside ttp.log.
    const RETIRED_MODEL: &str = "llama-3.3-70b-versatile";

    fn request_json() -> serde_json::Value {
        serde_json::to_value(build_request("Grok", "Groq", "I use Grok daily")).unwrap()
    }

    #[test]
    fn classify_asks_for_the_same_model_as_polish() {
        // One model id for the crate: when it has to change again, it changes
        // in `polish::MODEL` and the gate follows instead of 404ing alone.
        let body = request_json();
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["model"], "openai/gpt-oss-120b");
        assert_ne!(body["model"], RETIRED_MODEL);
    }

    #[test]
    fn the_verdict_budget_survives_reasoning() {
        let body = request_json();
        assert_eq!(body["reasoning_effort"], REASONING_EFFORT);
        let cap = body["max_completion_tokens"].as_u64().unwrap();
        assert!(cap > u64::from(REASONING_TOKEN_HEADROOM), "cap {cap} leaves no room for the verdict");
        // The deprecated field must not be sent alongside the modern one.
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn the_gate_still_asks_for_a_bare_word_not_json() {
        // The verdict parser reads text; a json_object response_format would
        // force the model to wrap it.
        assert!(request_json().get("response_format").is_none());
    }

    #[test]
    fn verdict_parsing() {
        assert_eq!(parse_verdict("LEARN"), Some(true));
        assert_eq!(parse_verdict("  learn\n"), Some(true));
        assert_eq!(parse_verdict("LEARN (brand name)"), Some(true));
        assert_eq!(parse_verdict("IGNORE"), Some(false));
        assert_eq!(parse_verdict("Ignore."), Some(false));
        // First keyword wins — the old `contains("LEARN")` said true here.
        assert_eq!(parse_verdict("IGNORE — not a LEARN case"), Some(false));
        assert_eq!(parse_verdict(""), None);
        assert_eq!(parse_verdict("maybe"), None);
    }

    #[test]
    fn a_gpt_oss_reply_parses() {
        // Shape of a Groq gpt-oss completion: reasoning in its own field,
        // the answer alone in `content`.
        let body = r#"{"id":"chatcmpl-x","object":"chat.completion","model":"openai/gpt-oss-120b",
            "choices":[{"index":0,"message":{"role":"assistant","reasoning":"Groq is a company name.","content":"LEARN"},"finish_reason":"stop"}]}"#;
        assert_eq!(parse_response_body(body), Ok(true));
        let body = body.replace("\"content\":\"LEARN\"", "\"content\":\"IGNORE\"");
        assert_eq!(parse_response_body(&body), Ok(false));
    }

    #[test]
    fn a_reply_truncated_by_reasoning_is_an_error_not_an_ignore() {
        let body = r#"{"choices":[{"index":0,"message":{"role":"assistant","reasoning":"Hmm, the user","content":null},"finish_reason":"length"}]}"#;
        let err = parse_response_body(body).unwrap_err();
        assert!(err.contains("neither LEARN nor IGNORE"), "{err}");
        assert!(parse_response_body(r#"{"choices":[]}"#).is_err());
        assert!(parse_response_body("not json").is_err());
    }

    #[test]
    fn a_retired_model_is_reported_as_an_error_that_names_it() {
        let body = r#"{"error":{"message":"The model `llama-3.3-70b-versatile` does not exist or you do not have access to it.","type":"invalid_request_error","code":"model_not_found"}}"#;
        let (severity, message) = describe_http_failure(404, body, None);
        assert_eq!(severity, Severity::Error);
        assert!(message.contains("404"));
        assert!(message.contains(MODEL));
        assert!(message.contains("model_not_found"));
    }

    #[test]
    fn failure_severity_policy() {
        assert_eq!(describe_http_failure(401, "", None).0, Severity::Error);
        assert_eq!(describe_http_failure(500, "", None).0, Severity::Error);
        let (severity, message) = describe_http_failure(429, "", Some(6352));
        assert_eq!(severity, Severity::Warn);
        assert!(message.contains("retry_after_ms=6352"), "{message}");
    }
}
