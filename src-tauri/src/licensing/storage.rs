// TTP - Talk To Paste
// Persisted license cache (~/.config/ttp/license.json)

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;

/// HMAC secret used to detect tampering of the local license cache.
/// Reverse-engineerable from the binary — this is "casual tamper-resistant",
/// not crypto. Raises the bar above plain text editing.
const HMAC_SECRET: &[u8; 32] = b"TTPOfflineGraceCache_v1_Casual__";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRecord {
    pub license_key: String,
    pub instance_id: String,
    pub instance_name: String,
    pub status: String,
    pub expires_at: Option<i64>,
    pub last_validated_at: i64,
    pub activation_count: Option<u32>,
    pub activation_limit: Option<u32>,
    #[serde(default)]
    pub signature: Option<String>,
}

fn license_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("license.json"))
}

fn compute_license_signature(record: &LicenseRecord) -> String {
    let input = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        record.license_key,
        record.status,
        record.expires_at.unwrap_or(0),
        record.last_validated_at,
        record.activation_count.unwrap_or(0),
        record.activation_limit.unwrap_or(0),
        record.instance_id,
    );
    let mut mac =
        HmacSha256::new_from_slice(HMAC_SECRET).expect("HMAC_SECRET has correct length");
    mac.update(input.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_license_signature(record: &LicenseRecord) -> bool {
    let Some(ref stored) = record.signature else {
        return false;
    };
    compute_license_signature(record) == *stored
}

pub fn load_license() -> Option<LicenseRecord> {
    let path = license_path()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let record: LicenseRecord = serde_json::from_str(&content).ok()?;

    if record.signature.is_some() {
        if verify_license_signature(&record) {
            return Some(record);
        }
        // Signature present but mismatched — discard rather than crash.
        // Tamper detected; treat as no license.
        crate::logging::log_warn("license signature invalid — discarding cache");
        return None;
    }

    // Legacy 1.6.x file with no signature — accept once, re-sign on disk so
    // future edits get caught. Existing legitimate users aren't broken.
    let _ = save_license(&record);
    Some(record)
}

pub fn save_license(record: &LicenseRecord) -> Result<(), String> {
    let path = license_path().ok_or("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let mut record = record.clone();
    record.signature = Some(compute_license_signature(&record));

    let json = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Failed to serialize license: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write license file: {}", e))?;
    Ok(())
}

pub fn clear_license() -> Result<(), String> {
    let Some(path) = license_path() else {
        return Ok(());
    };
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete license file: {}", e))?;
    }
    Ok(())
}
