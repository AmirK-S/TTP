// TTP - Talk To Paste
// Shared `reqwest::Client` for all outbound HTTP (Groq transcription + polish + Lemon Squeezy).
//
// Why share a single Client?
// - `reqwest::Client` owns the connection pool. Building a new one per request
//   throws away kept-alive TCP/TLS sessions, so each call pays a fresh TLS
//   handshake (~100-300ms on slow networks).
// - One client = one warm pool, reused across whisper -> polish chained calls
//   (the dominant hot path) and across licensing checks.
//
// No global timeout is configured here on purpose: each call site needs a
// different per-request timeout (Whisper scales with file size; licensing is
// fixed at 15s). Per-request timeouts are applied via `.timeout(...)` on the
// RequestBuilder so the shared pool stays usable everywhere.
//
// TLS uses rustls (matches the `rustls-tls` feature in Cargo.toml).

use std::sync::OnceLock;
use std::time::Duration;

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Get the process-wide shared HTTP client.
///
/// Lazily initialized on first call. Subsequent calls return the same `Client`
/// (which is internally `Arc`-shared, so cloning it is cheap and all clones
/// share the connection pool).
///
/// Per-request timeouts MUST be set on the `RequestBuilder` (e.g. `.timeout(...)`)
/// because this client has no global timeout.
pub fn shared() -> &'static reqwest::Client {
    SHARED_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            // Keep up to 8 idle connections per host. We hit ~3 hosts
            // (api.groq.com, api.lemonsqueezy.com, plus updater) so 8 is
            // plenty for chained whisper->polish calls without bloat.
            .pool_max_idle_per_host(8)
            // Drop idle connections after 90s — long enough to reuse across
            // a typical user session but short enough to not hold sockets
            // open forever.
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            // Builder failure on a default rustls config is exceedingly rare
            // (would mean the rustls crypto provider failed to init). If it
            // ever happens we fall back to a fresh default client so callers
            // still work — they'll just lose pool reuse for this run.
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

// ---------------------------------------------------------------------------
// Rate-limit guidance
//
// A 429 from Groq is not an opaque failure: the response says how long to
// wait, in up to three places. Ignoring it and sleeping a locally invented
// number is how 54 polish calls burned through ~20 seconds on 2026-09-02
// against a limit that wanted between 0.21s and 8.34s (measured: the 55
// `Please try again in …` values in `ttp.log`, median 4.08s). Only 11 of
// those 55 delays were shorter than the 1.5s the old backoff spent in total,
// so four out of five retries were guaranteed to 429 again before they were
// sent.
//
// Shared here rather than in `polish.rs` because the Whisper path has the
// same retry loop and the same defect (see `transcription/whisper.rs`).

/// Where a retry delay came from. Trace slug — never shown to a user.
pub const SOURCE_HEADER: &str = "retry_after";
pub const SOURCE_RESET: &str = "ratelimit_reset";
pub const SOURCE_BODY: &str = "body";

/// A delay the server asked us to wait, and which field said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryGuidance {
    pub delay_ms: u64,
    /// Slug, for the trace: `retry_after` | `ratelimit_reset` | `body`.
    pub source: &'static str,
}

/// Parse one Go-style duration token as milliseconds: `6.35s`, `577.5ms`,
/// `2m59.56s`, `1h2m3s`.
///
/// Groq writes its `x-ratelimit-reset-*` headers and its rate-limit message
/// in this format, and mixes units inside one value. Rounds up: waking a
/// millisecond early re-triggers the same limit.
fn parse_duration_token(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() || s.starts_with('-') {
        return None;
    }

    let mut total_ms: f64 = 0.0;
    let mut number = String::new();
    let mut unit = String::new();

    // Walk the token accumulating <number><unit> pairs. Anything that is
    // neither (a letter that is not a unit, a second decimal point) makes the
    // whole token unparseable — better no guidance than a wrong wait.
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            if !unit.is_empty() {
                total_ms += apply_unit(&number, &unit)?;
                number.clear();
                unit.clear();
            }
            number.push(c);
        } else if c.is_ascii_alphabetic() {
            if number.is_empty() {
                return None;
            }
            unit.push(c.to_ascii_lowercase());
        } else {
            return None;
        }
    }

    if unit.is_empty() {
        // A trailing number with no unit ("6", "1m30") is ambiguous, and a
        // unit with no number was already rejected above. Only the caller
        // that knows the unit (`Retry-After` is delta-seconds) may assume one.
        return None;
    }
    total_ms += apply_unit(&number, &unit)?;

    if !total_ms.is_finite() || total_ms < 0.0 {
        return None;
    }
    Some(total_ms.ceil().min(u64::MAX as f64) as u64)
}

/// Convert one `<number><unit>` pair to milliseconds.
fn apply_unit(number: &str, unit: &str) -> Option<f64> {
    let value: f64 = number.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let factor = match unit {
        "ms" => 1.0,
        "s" => 1000.0,
        "m" => 60_000.0,
        "h" => 3_600_000.0,
        // `us`/`µs`/`ns` never appear in a rate-limit reply and would round to
        // zero anyway; treat anything else as unparseable.
        _ => return None,
    };
    Some(value * factor)
}

/// Parse a `Retry-After` header value.
///
/// RFC 9110 allows delta-seconds or an HTTP-date. Only delta-seconds is
/// honoured (Groq sends that, sometimes fractional). The date form is
/// deliberately unparsed: acting on it means trusting the local clock against
/// the server's, and a skewed clock turns a 3-second wait into a 3-hour one.
/// Returning `None` falls through to the other two sources, and if they are
/// silent too the caller degrades immediately — the safe direction.
fn parse_retry_after_header(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() || s.starts_with('-') {
        return None;
    }
    if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let seconds: f64 = s.parse().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some((seconds * 1000.0).ceil().min(u64::MAX as f64) as u64)
}

/// Pull the phrase Groq puts in the error body: `Please try again in 6.3525s`.
fn parse_body_guidance(body: &str) -> Option<u64> {
    const MARKER: &str = "try again in ";
    let idx = body.find(MARKER)? + MARKER.len();
    let token: String = body[idx..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || c.is_ascii_alphabetic() || *c == '.')
        .collect();
    // A trailing '.' is sentence punctuation, not a decimal point.
    let token = token.trim_end_matches('.');
    parse_duration_token(token)
}

/// How long the server asked us to wait before retrying, in milliseconds.
///
/// Reads, in order of authority: `Retry-After`, then Groq's
/// `x-ratelimit-reset-*` headers (the shortest of the ones present — the
/// first window to reopen is the one that matters), then the `try again in …`
/// phrase in the error body. `None` means the response gave no usable
/// guidance; it does NOT mean "retry immediately".
///
/// The returned delay is measured from when the *server* wrote it, so it is
/// already conservative by the response's transit time. No pad is added.
pub fn retry_guidance(headers: &reqwest::header::HeaderMap, body: &str) -> Option<RetryGuidance> {
    if let Some(ms) = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after_header)
    {
        return Some(RetryGuidance {
            delay_ms: ms,
            source: SOURCE_HEADER,
        });
    }

    let reset = ["x-ratelimit-reset-tokens", "x-ratelimit-reset-requests"]
        .iter()
        .filter_map(|name| headers.get(*name))
        .filter_map(|v| v.to_str().ok())
        .filter_map(parse_duration_token)
        .min();
    if let Some(ms) = reset {
        return Some(RetryGuidance {
            delay_ms: ms,
            source: SOURCE_RESET,
        });
    }

    parse_body_guidance(body).map(|ms| RetryGuidance {
        delay_ms: ms,
        source: SOURCE_BODY,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    /// Verbatim from `ttp.log`, 2026-09-02 00:20:10 — the burst that started
    /// this. The delay in it (6.3525s) is four times what the old backoff
    /// waited across all three attempts combined.
    const REAL_429_BODY: &str = r#"{"error":{"message":"Rate limit reached for model `openai/gpt-oss-120b` in organization `org_01kg76940xeaq94fsmzgzh3mxx` service tier `on_demand` on tokens per minute (TPM): Limit 8000, Used 7739, Requested 1108. Please try again in 6.3525s. Need more tokens? Upgrade to Dev Tier today at https://console.groq.com/settings/billing","type":"tokens","code":"rate_limit_exceeded"}}"#;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn body_guidance_parses_the_real_groq_429() {
        let g = retry_guidance(&HeaderMap::new(), REAL_429_BODY).expect("guidance");
        assert_eq!(g.delay_ms, 6353, "6.3525s, rounded up");
        assert_eq!(g.source, SOURCE_BODY);
    }

    #[test]
    fn body_guidance_handles_every_unit_groq_emits() {
        // All three shapes appear in the 55 logged rate-limit messages.
        for (msg, expected) in [
            ("Please try again in 577.5ms.", 578),
            ("Please try again in 6.3525s.", 6353),
            ("Please try again in 2.662499999s.", 2663),
            ("Please try again in 2m59.56s.", 179_560),
        ] {
            assert_eq!(parse_body_guidance(msg), Some(expected), "{}", msg);
        }
    }

    #[test]
    fn malformed_body_guidance_yields_none_not_zero() {
        // A zero here would mean "retry immediately", which is the defect.
        for msg in [
            "",
            "Rate limit reached.",
            "Please try again in soon.",
            "Please try again in .",
            "Please try again in 5 minutes",
            "Please try again in -3s",
            "Please try again in 1.2.3s",
            "Please try again in 6",
        ] {
            assert_eq!(parse_body_guidance(msg), None, "{:?} should not parse", msg);
        }
    }

    #[test]
    fn retry_after_header_is_delta_seconds_and_wins() {
        let h = headers(&[
            ("retry-after", "3"),
            ("x-ratelimit-reset-tokens", "9s"),
        ]);
        let g = retry_guidance(&h, REAL_429_BODY).expect("guidance");
        assert_eq!(g.delay_ms, 3000);
        assert_eq!(g.source, SOURCE_HEADER, "header outranks reset and body");
    }

    #[test]
    fn retry_after_header_accepts_fractional_seconds() {
        let g = retry_guidance(&headers(&[("retry-after", "2.5")]), "").expect("guidance");
        assert_eq!(g.delay_ms, 2500);
    }

    #[test]
    fn retry_after_zero_is_honoured_as_zero_not_missing() {
        let g = retry_guidance(&headers(&[("retry-after", "0")]), "").expect("guidance");
        assert_eq!(g.delay_ms, 0);
        assert_eq!(g.source, SOURCE_HEADER);
    }

    #[test]
    fn malformed_retry_after_falls_through_to_the_body() {
        // HTTP-date form, empty, negative, junk. Each must not be mistaken for
        // a delta-seconds value, and must not block the body from being read.
        for raw in ["Wed, 21 Oct 2015 07:28:00 GMT", "", "  ", "-5", "soon", "3s"] {
            let g = retry_guidance(&headers(&[("retry-after", raw)]), REAL_429_BODY);
            assert_eq!(
                g.map(|g| (g.delay_ms, g.source)),
                Some((6353, SOURCE_BODY)),
                "retry-after {:?} should fall through",
                raw
            );
        }
    }

    #[test]
    fn ratelimit_reset_headers_take_the_shorter_window() {
        let h = headers(&[
            ("x-ratelimit-reset-requests", "2m59.56s"),
            ("x-ratelimit-reset-tokens", "6.35s"),
        ]);
        let g = retry_guidance(&h, "").expect("guidance");
        assert_eq!(g.delay_ms, 6350, "the first window to reopen");
        assert_eq!(g.source, SOURCE_RESET);
    }

    #[test]
    fn compound_duration_tokens_sum_their_parts() {
        assert_eq!(parse_duration_token("1h2m3s"), Some(3_723_000));
        assert_eq!(parse_duration_token("2m59.56s"), Some(179_560));
        assert_eq!(parse_duration_token("500ms"), Some(500));
    }

    #[test]
    fn absurd_durations_saturate_instead_of_panicking() {
        // A garbage or hostile value must produce a number the caller can
        // reject against its cap, not an overflow.
        let ms = parse_duration_token("999999999999999999999s").expect("parsed");
        assert!(ms > 0);
        assert_eq!(retry_guidance(&headers(&[("retry-after", "999999999999999999999")]), "")
            .map(|g| g.delay_ms > 0), Some(true));
    }

    #[test]
    fn no_guidance_anywhere_is_none() {
        assert_eq!(retry_guidance(&HeaderMap::new(), ""), None);
        assert_eq!(
            retry_guidance(&HeaderMap::new(), r#"{"error":{"message":"Too many requests"}}"#),
            None
        );
    }
}
