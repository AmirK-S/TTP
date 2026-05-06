// TTP - Talk To Paste
// Sentry PII scrubbing - strips API keys, file paths, and sensitive data
// from all Sentry events before they leave the device.

use std::sync::OnceLock;

/// A regex that matches nothing — used as a fallback if a hand-written
/// pattern fails to compile, so the scrubber can never panic and take
/// down the Sentry pipeline (which runs from event/breadcrumb hooks on
/// arbitrary threads, where a panic kills the whole app).
fn never_match() -> regex::Regex {
    regex::Regex::new(r"$.^").expect("trivial regex must compile")
}

/// Compiled regex for Groq API keys (gsk_...)
fn api_key_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"gsk_[a-zA-Z0-9]{20,}").unwrap_or_else(|_| never_match()))
}

/// Compiled regex for file paths (macOS, Linux, Windows)
fn file_path_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(/Users/[^\s:]+|/home/[^\s:]+|[A-Z]:\\[^\s:]+)")
            .unwrap_or_else(|_| never_match())
    })
}

/// Compiled regex for email addresses. Conservative: requires a TLD of at
/// least two letters and disallows whitespace inside the address.
fn email_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b")
            .unwrap_or_else(|_| never_match())
    })
}

/// Scrub PII from a string: redacts API keys, file paths, and emails.
fn scrub_string(s: &str) -> String {
    let s = api_key_regex()
        .replace_all(s, "[REDACTED_API_KEY]")
        .to_string();
    let s = file_path_regex()
        .replace_all(&s, "[REDACTED_PATH]")
        .to_string();
    email_regex().replace_all(&s, "[REDACTED_EMAIL]").to_string()
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
