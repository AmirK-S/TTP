// TTP - Talk To Paste
// Lemon Squeezy License API client
//
// Errors returned from this module's public functions are translation keys
// (e.g. "error.license_activation_failed"). The frontend resolves them via
// i18next. Server-supplied detail (LS error strings, HTTP status, parse
// failures) is logged via `crate::logging::log_error` rather than surfaced
// directly to the user.

use super::types::{LicenseStatus, LsResponse};
use crate::http_client::shared as shared_http;
use crate::logging::log_error;
use std::time::Duration;

const ACTIVATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/activate";
const VALIDATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/validate";
const DEACTIVATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/deactivate";

const REQUEST_TIMEOUT_SECS: u64 = 15;

/// Activate a license key for a new instance (machine).
pub async fn activate(license_key: &str, instance_name: &str) -> Result<LicenseStatus, String> {
    let response = post_form(
        ACTIVATE_URL,
        &[
            ("license_key", license_key),
            ("instance_name", instance_name),
        ],
    )
    .await?;

    if !response.activated {
        log_error(&format!(
            "LS activation rejected: {}",
            response.error.as_deref().unwrap_or("<no detail>")
        ));
        return Err("error.license_activation_failed".to_string());
    }

    let instance = response.instance.ok_or_else(|| {
        log_error("LS activation: success flag set but no instance in response");
        "error.license_activation_failed".to_string()
    })?;
    let key = response.license_key.ok_or_else(|| {
        log_error("LS activation: success flag set but no license_key in response");
        "error.license_activation_failed".to_string()
    })?;

    Ok(LicenseStatus {
        instance_id: instance.id,
        status: key.status,
        expires_at: parse_timestamp(key.expires_at.as_deref()),
        activation_count: key.activation_usage,
        activation_limit: key.activation_limit,
    })
}

/// Validate that a license + instance is still active on the server.
pub async fn validate(license_key: &str, instance_id: &str) -> Result<LicenseStatus, String> {
    let response = post_form(
        VALIDATE_URL,
        &[
            ("license_key", license_key),
            ("instance_id", instance_id),
        ],
    )
    .await?;

    if !response.valid {
        log_error(&format!(
            "LS validation rejected: {}",
            response.error.as_deref().unwrap_or("<no detail>")
        ));
        return Err("error.license_validation_failed".to_string());
    }

    let key = response.license_key.ok_or_else(|| {
        log_error("LS validation: valid flag set but no license_key in response");
        "error.license_validation_failed".to_string()
    })?;
    let instance_id = response
        .instance
        .map(|i| i.id)
        .unwrap_or_else(|| instance_id.to_string());

    Ok(LicenseStatus {
        instance_id,
        status: key.status,
        expires_at: parse_timestamp(key.expires_at.as_deref()),
        activation_count: key.activation_usage,
        activation_limit: key.activation_limit,
    })
}

/// Deactivate the license for the current instance — frees an activation slot.
pub async fn deactivate(license_key: &str, instance_id: &str) -> Result<(), String> {
    let response = post_form(
        DEACTIVATE_URL,
        &[
            ("license_key", license_key),
            ("instance_id", instance_id),
        ],
    )
    .await?;

    if !response.deactivated {
        log_error(&format!(
            "LS deactivation rejected: {}",
            response.error.as_deref().unwrap_or("<no detail>")
        ));
        return Err("error.license_deactivation_failed".to_string());
    }
    Ok(())
}

async fn post_form(url: &str, params: &[(&str, &str)]) -> Result<LsResponse, String> {
    // Reuse the process-wide HTTP client; per-request timeout below replaces
    // the previous builder-level timeout so connection pooling stays shared.
    let client = shared_http();

    let response = client
        .post(url)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .header("Accept", "application/json")
        .form(params)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                log_error(&format!("LS request timeout: {}", e));
                "error.api_timeout".to_string()
            } else {
                log_error(&format!("LS network error: {}", e));
                "error.api_network".to_string()
            }
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        log_error(&format!("LS read body failed: {}", e));
        "error.license_server_error".to_string()
    })?;

    let parsed: LsResponse = serde_json::from_str(&body).map_err(|e| {
        log_error(&format!(
            "LS parse failed (HTTP {}): {}",
            status.as_u16(),
            e
        ));
        "error.license_server_error".to_string()
    })?;

    if !status.is_success() && parsed.error.is_none() {
        log_error(&format!("LS HTTP error: {}", status.as_u16()));
        return Err("error.license_server_error".to_string());
    }

    Ok(parsed)
}

fn parse_timestamp(value: Option<&str>) -> Option<i64> {
    value
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}
