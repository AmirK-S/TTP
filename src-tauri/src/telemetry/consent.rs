// TTP - Talk To Paste
// Consent module - gates Sentry DSN based on user telemetry preference

/// Hardcoded Sentry DSN (public-facing, only allows writing events).
const SENTRY_DSN: &str = "https://e74857f6958a0049cf61eefbdc40d3e6@o4510885403033600.ingest.de.sentry.io/4510885412274256";

/// Returns a parsed Sentry DSN if the user has opted into telemetry,
/// or `None` if telemetry is disabled (default).
///
/// When `None` is passed to `sentry::init()`, the client is entirely
/// disabled and makes zero network requests.
pub fn get_sentry_dsn() -> Option<sentry::types::Dsn> {
    let settings = crate::settings::get_settings();
    if settings.telemetry_enabled {
        SENTRY_DSN.parse().ok()
    } else {
        None
    }
}
