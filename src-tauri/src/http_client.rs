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
