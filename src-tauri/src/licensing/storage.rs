// TTP - Talk To Paste
// Persisted license cache (~/.config/ttp/license.json)

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;

/// Legacy hardcoded HMAC secret from v1.6.x–v1.7.3. Kept so license.json
/// files signed by previous versions still verify on first run after
/// upgrade — we then re-sign them with the per-machine key below so the
/// legacy secret stops mattering on this install.
const LEGACY_HMAC_SECRET: &[u8; 32] = b"TTPOfflineGraceCache_v1_Casual__";

const KEYCHAIN_ACCOUNT: &str = "license_hmac_secret";

/// Per-machine HMAC secret stored in the OS keychain. Generated once with
/// the OS CSPRNG on first use, then reused across launches. Replaces the
/// shared LEGACY_HMAC_SECRET so a forged license.json can't be signed
/// once and replayed across machines.
fn machine_hmac_secret() -> [u8; 32] {
    crate::keychain::get_or_create_hmac_secret(KEYCHAIN_ACCOUNT, LEGACY_HMAC_SECRET)
}

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

fn license_signature_input(record: &LicenseRecord) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        record.license_key,
        record.status,
        record.expires_at.unwrap_or(0),
        record.last_validated_at,
        record.activation_count.unwrap_or(0),
        record.activation_limit.unwrap_or(0),
        record.instance_id,
    )
}

fn compute_license_signature_with(record: &LicenseRecord, secret: &[u8]) -> String {
    let input = license_signature_input(record);
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC secret has correct length");
    mac.update(input.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn compute_license_signature(record: &LicenseRecord) -> String {
    compute_license_signature_with(record, &machine_hmac_secret())
}

enum SigVerify {
    NotPresent,
    Invalid,
    ValidMachine,
    ValidLegacy,
}

/// Verify a stored signature using HMAC's constant-time `verify_slice` to
/// defeat timing attacks (the previous `==` compare exited on first byte
/// mismatch). Tries the per-machine secret first; falls back to the legacy
/// hardcoded secret so license.json files written by v1.7.x still verify
/// on first run after upgrade. The caller re-signs ValidLegacy files so
/// the legacy path stops being needed on this install.
fn verify_license_signature(record: &LicenseRecord) -> SigVerify {
    let Some(stored_hex) = record.signature.as_deref() else {
        return SigVerify::NotPresent;
    };
    let Ok(stored_bytes) = hex::decode(stored_hex) else {
        return SigVerify::Invalid;
    };

    let input = license_signature_input(record);

    let machine_secret = machine_hmac_secret();
    let mut mac = HmacSha256::new_from_slice(&machine_secret).expect("32-byte secret");
    mac.update(input.as_bytes());
    if mac.verify_slice(&stored_bytes).is_ok() {
        return SigVerify::ValidMachine;
    }

    let mut legacy = HmacSha256::new_from_slice(LEGACY_HMAC_SECRET).expect("32-byte secret");
    legacy.update(input.as_bytes());
    if legacy.verify_slice(&stored_bytes).is_ok() {
        return SigVerify::ValidLegacy;
    }
    SigVerify::Invalid
}

pub fn load_license() -> Option<LicenseRecord> {
    let path = license_path()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let record: LicenseRecord = serde_json::from_str(&content).ok()?;

    match verify_license_signature(&record) {
        SigVerify::ValidMachine => Some(record),
        SigVerify::ValidLegacy => {
            // File was signed by v1.7.x's hardcoded secret — re-sign with the
            // per-machine key so the legacy path isn't needed again on this
            // install.
            let _ = save_license(&record);
            Some(record)
        }
        SigVerify::NotPresent => {
            // Pre-1.6.2 file with no signature — accept once, sign now.
            let _ = save_license(&record);
            Some(record)
        }
        SigVerify::Invalid => {
            // Genuine tamper or corruption — discard.
            crate::logging::log_warn("license signature invalid — discarding cache");
            None
        }
    }
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
