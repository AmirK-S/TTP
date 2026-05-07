// TTP - Talk To Paste
// Lemon Squeezy License API client

use super::types::{LicenseStatus, LsResponse};
use crate::http_client::shared as shared_http;
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
        return Err(human_error(response.error.as_deref(), "Activation failed"));
    }

    let instance = response
        .instance
        .ok_or_else(|| "Activation succeeded but server returned no instance".to_string())?;
    let key = response
        .license_key
        .ok_or_else(|| "Activation succeeded but server returned no license info".to_string())?;

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
        return Err(human_error(response.error.as_deref(), "License is no longer valid"));
    }

    let key = response
        .license_key
        .ok_or_else(|| "Validation succeeded but server returned no license info".to_string())?;
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
        return Err(human_error(
            response.error.as_deref(),
            "Deactivation failed",
        ));
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
                "Network timeout — check your internet connection".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let parsed: LsResponse = serde_json::from_str(&body).map_err(|e| {
        format!(
            "Unexpected response from license server (HTTP {}): {}",
            status.as_u16(),
            e
        )
    })?;

    if !status.is_success() && parsed.error.is_none() {
        return Err(format!("License server returned HTTP {}", status.as_u16()));
    }

    Ok(parsed)
}

fn human_error(server_msg: Option<&str>, fallback: &str) -> String {
    match server_msg {
        Some(msg) if !msg.is_empty() => msg.to_string(),
        _ => fallback.to_string(),
    }
}

fn parse_timestamp(value: Option<&str>) -> Option<i64> {
    value
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}
