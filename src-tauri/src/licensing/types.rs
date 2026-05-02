// TTP - Talk To Paste
// Lemon Squeezy license API types

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct LicenseStatus {
    pub instance_id: String,
    pub status: String,
    pub expires_at: Option<i64>,
    pub activation_count: Option<u32>,
    pub activation_limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct LsResponse {
    #[serde(default)]
    pub activated: bool,
    #[serde(default)]
    pub valid: bool,
    #[serde(default)]
    pub deactivated: bool,
    pub error: Option<String>,
    pub license_key: Option<LsLicenseKey>,
    pub instance: Option<LsInstance>,
    pub meta: Option<LsMeta>,
}

#[derive(Debug, Deserialize)]
pub struct LsLicenseKey {
    pub status: String,
    pub expires_at: Option<String>,
    pub activation_limit: Option<u32>,
    pub activation_usage: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct LsInstance {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LsMeta {
    pub store_id: Option<u64>,
    pub product_id: Option<u64>,
    pub variant_id: Option<u64>,
}
