// TTP - Talk To Paste
// Lemon Squeezy license management

mod api;
mod storage;
mod types;

use crate::logging::log_error;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use storage::{LicenseRecord, clear_license, load_license, save_license};

/// Maximum days a cached license can be trusted offline before requiring re-validation.
const OFFLINE_GRACE_DAYS: i64 = 14;

/// Free tier: max AI Polish calls per calendar month.
pub const FREE_POLISH_PER_MONTH: u32 = 30;
/// Free tier: max dictionary entries (existing entries above this are grandfathered).
pub const FREE_DICTIONARY_LIMIT: usize = 20;
/// Free tier: max history entries (existing entries above this are grandfathered).
pub const FREE_HISTORY_LIMIT: usize = 50;
/// Length of the auto-trial granted on first launch, in days. Was 7 in
/// v1.6.0–v2.1.10. Shortened to 3 in v2.2.0 after observing that uninstall+
/// reinstall trivially refreshes the trial (the v2.1.4 uninstaller wipes
/// the keychain HMAC secret + usage.json signed counter the protection
/// depended on). A shorter trial reduces the value of that abuse loop
/// (3 min reinstall friction for 3 days of unlimited polish is a bad
/// trade) without sacrificing honest evaluation time — at typical use
/// (3–5 dictation sessions/day) 3 days is 9–15 sessions, enough to know.
pub const TRIAL_DAYS: i64 = 3;

/// Public license info returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct LicenseInfo {
    pub is_pro: bool,
    pub license_key: Option<String>,
    pub status: Option<String>,
    pub expires_at: Option<i64>,
    pub last_validated_at: Option<i64>,
    pub activation_count: Option<u32>,
    pub activation_limit: Option<u32>,
}

impl LicenseInfo {
    fn from_record(record: Option<LicenseRecord>, is_pro: bool) -> Self {
        match record {
            Some(r) => Self {
                is_pro,
                license_key: Some(r.license_key),
                status: Some(r.status),
                expires_at: r.expires_at,
                last_validated_at: Some(r.last_validated_at),
                activation_count: r.activation_count,
                activation_limit: r.activation_limit,
            },
            None => Self {
                is_pro: false,
                license_key: None,
                status: None,
                expires_at: None,
                last_validated_at: None,
                activation_count: None,
                activation_limit: None,
            },
        }
    }
}

/// In-memory cached license state, refreshed by validate/activate calls.
#[derive(Debug, Default)]
pub struct LicenseState {
    pub record: Option<LicenseRecord>,
}

/// Compute whether the current cached record grants Pro access right now.
fn is_pro_for(record: &LicenseRecord) -> bool {
    if record.status != "active" {
        return false;
    }
    let now = chrono::Utc::now().timestamp();
    if let Some(expires) = record.expires_at {
        if expires <= now {
            return false;
        }
    }
    let age_secs = now - record.last_validated_at;
    age_secs < OFFLINE_GRACE_DAYS * 86_400
}

fn emit_license_changed(app: &AppHandle, info: &LicenseInfo) {
    let _ = app.emit("license-changed", info);
}

/// Recover from a poisoned mutex by taking the inner value rather than
/// panicking. License state is just a cache; if a previous holder panicked,
/// we'd rather rebuild the state than crash the whole app.
fn lock_or_recover<'a>(
    state: &'a State<Mutex<LicenseState>>,
) -> std::sync::MutexGuard<'a, LicenseState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log_error("License state mutex was poisoned — recovering");
            poisoned.into_inner()
        }
    }
}

#[tauri::command]
pub fn get_license_info(state: State<Mutex<LicenseState>>) -> LicenseInfo {
    let guard = lock_or_recover(&state);
    let is_pro = guard.record.as_ref().map(is_pro_for).unwrap_or(false);
    LicenseInfo::from_record(guard.record.clone(), is_pro)
}

#[tauri::command]
pub fn is_pro(state: State<Mutex<LicenseState>>) -> bool {
    let guard = lock_or_recover(&state);
    guard.record.as_ref().map(is_pro_for).unwrap_or(false)
}

#[tauri::command]
pub async fn activate_license(
    license_key: String,
    app: AppHandle,
) -> Result<LicenseInfo, String> {
    let key = license_key.trim().to_string();
    if key.is_empty() {
        return Err("error.license_key_empty".to_string());
    }

    let instance_name = format!("TTP - {}", device_label());
    let result = api::activate(&key, &instance_name)
        .await
        .map_err(|e| {
            log_error(&format!("License activate failed: {}", e));
            e
        })?;

    let now = chrono::Utc::now().timestamp();
    let record = LicenseRecord {
        license_key: key,
        instance_id: result.instance_id,
        instance_name,
        status: result.status,
        expires_at: result.expires_at,
        last_validated_at: now,
        activation_count: result.activation_count,
        activation_limit: result.activation_limit,
        signature: None, // populated by save_license
    };
    save_license(&record)?;

    let state = app.state::<Mutex<LicenseState>>();
    let mut guard = lock_or_recover(&state);
    guard.record = Some(record.clone());
    let is_pro = is_pro_for(&record);
    drop(guard);

    let info = LicenseInfo::from_record(Some(record), is_pro);
    emit_license_changed(&app, &info);
    Ok(info)
}

#[tauri::command]
pub async fn deactivate_license(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<LicenseState>>();
    let snapshot = {
        let guard = lock_or_recover(&state);
        guard.record.clone()
    };

    let Some(record) = snapshot else {
        return Err("error.license_none_to_deactivate".to_string());
    };

    if let Err(e) = api::deactivate(&record.license_key, &record.instance_id).await {
        log_error(&format!("License deactivate failed: {}", e));
        // Continue anyway — user wants the local key gone even if server is unreachable.
    }

    clear_license()?;
    {
        let mut guard = lock_or_recover(&state);
        guard.record = None;
    }

    let info = LicenseInfo::from_record(None, false);
    emit_license_changed(&app, &info);
    Ok(())
}

#[tauri::command]
pub async fn validate_license(app: AppHandle) -> Result<LicenseInfo, String> {
    let state = app.state::<Mutex<LicenseState>>();
    let snapshot = {
        let guard = lock_or_recover(&state);
        guard.record.clone()
    };

    let Some(mut record) = snapshot else {
        return Ok(LicenseInfo::from_record(None, false));
    };

    match api::validate(&record.license_key, &record.instance_id).await {
        Ok(result) => {
            record.status = result.status;
            record.expires_at = result.expires_at;
            record.activation_count = result.activation_count;
            record.activation_limit = result.activation_limit;
            record.last_validated_at = chrono::Utc::now().timestamp();
            save_license(&record)?;

            let mut guard = lock_or_recover(&state);
            guard.record = Some(record.clone());
            let is_pro = is_pro_for(&record);
            drop(guard);

            let info = LicenseInfo::from_record(Some(record), is_pro);
            emit_license_changed(&app, &info);
            Ok(info)
        }
        Err(e) => {
            log_error(&format!("License validate failed (offline grace applies): {}", e));
            // Don't clear local state on transient failure — let offline grace handle it.
            let is_pro = is_pro_for(&record);
            Ok(LicenseInfo::from_record(Some(record), is_pro))
        }
    }
}

/// Initialize the license state on app startup. Loads from disk and kicks off a
/// background validation if a license is present.
pub fn init(app: &AppHandle) {
    let record = load_license();
    let state = LicenseState {
        record: record.clone(),
    };
    app.manage(Mutex::new(state));

    // Best-effort online refresh in the background so the cached status stays fresh.
    if record.is_some() {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = validate_license(app_handle).await;
        });
    }
}

/// Disk-only check: does the cached license currently grant Pro access?
/// Doesn't require AppHandle — safe to call from anywhere (pipeline, dictionary, etc).
pub fn is_pro_disk() -> bool {
    load_license().as_ref().map(is_pro_for).unwrap_or(false)
}

/// Disk-only check: is the user currently within the auto-trial window?
/// Caller passes a pre-loaded UsageRecord to avoid double IO.
pub fn is_in_trial_disk(usage: &crate::usage::UsageRecord) -> bool {
    let Some(started) = usage.trial_started_at else {
        return false;
    };
    let elapsed_secs = chrono::Utc::now().timestamp().saturating_sub(started);
    elapsed_secs < TRIAL_DAYS * 86_400
}

/// Combined check: Pro license OR active trial.
pub fn is_pro_or_trial_disk() -> bool {
    if is_pro_disk() {
        return true;
    }
    let usage = crate::usage::load_usage();
    is_in_trial_disk(&usage)
}

fn device_label() -> String {
    if let Ok(output) = std::process::Command::new("hostname").output() {
        if let Ok(name) = String::from_utf8(output.stdout) {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    std::env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|u| format!("Mac-{}", u))
        .unwrap_or_else(|| "Mac".to_string())
}
