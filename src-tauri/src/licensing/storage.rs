// TTP - Talk To Paste
// Persisted license cache (~/.config/ttp/license.json)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
}

fn license_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("license.json"))
}

pub fn load_license() -> Option<LicenseRecord> {
    let path = license_path()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_license(record: &LicenseRecord) -> Result<(), String> {
    let path = license_path().ok_or("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(record)
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
