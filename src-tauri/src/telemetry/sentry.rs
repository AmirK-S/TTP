// TTP - Talk To Paste
// Sentry PII scrubbing - strips API keys, file paths, and sensitive data
// from all Sentry events before they leave the device.

use std::sync::OnceLock;

/// Compiled regex for Groq API keys (gsk_...)
fn api_key_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"gsk_[a-zA-Z0-9]{20,}").unwrap())
}

/// Compiled regex for file paths (macOS, Linux, Windows)
fn file_path_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(/Users/[^\s:]+|/home/[^\s:]+|[A-Z]:\\[^\s:]+)").unwrap()
    })
}

/// Scrub PII from a string: redacts API keys and file paths.
fn scrub_string(s: &str) -> String {
    let result = api_key_regex()
        .replace_all(s, "[REDACTED_API_KEY]")
        .to_string();
    file_path_regex()
        .replace_all(&result, "[REDACTED_PATH]")
        .to_string()
}

/// Returns true if a key name likely contains sensitive data.
fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("key")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("path")
        || lower.contains("text")
        || lower.contains("transcription")
}

/// Scrub PII from a Sentry event before it is sent.
///
/// Strips: server_name (hostname), API keys, file paths, and sensitive
/// keys from exceptions, messages, breadcrumbs, extra data, and tags.
pub fn scrub_event_pii(event: &mut sentry::protocol::Event<'_>) {
    // Strip server_name (contains hostname)
    event.server_name = None;

    // Scrub exception values
    for exception in event.exception.values.iter_mut() {
        if let Some(ref mut value) = exception.value {
            *value = scrub_string(value);
        }
    }

    // Scrub event message
    if let Some(ref mut msg) = event.message {
        *msg = scrub_string(msg);
    }

    // Scrub breadcrumb messages and data
    for breadcrumb in event.breadcrumbs.values.iter_mut() {
        if let Some(ref mut msg) = breadcrumb.message {
            *msg = scrub_string(msg);
        }
        breadcrumb.data.retain(|k, _| !is_sensitive_key(k));
    }

    // Scrub extra data
    event.extra.retain(|k, _| !is_sensitive_key(k));

    // Scrub tags
    event.tags.retain(|k, _| !is_sensitive_key(k));
}

/// Scrub PII from a Sentry breadcrumb before it is recorded.
pub fn scrub_breadcrumb_pii(breadcrumb: &mut sentry::protocol::Breadcrumb) {
    if let Some(ref mut msg) = breadcrumb.message {
        *msg = scrub_string(msg);
    }
    breadcrumb.data.retain(|k, _| !is_sensitive_key(k));
}
