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

/// Pre-read this store's keychain material so the first dictation does not
/// pay for it. See [`crate::keychain::warm_caches`].
pub fn warm_keychain_cache() {
    crate::keychain::warm_caches(&[(KEYCHAIN_ACCOUNT, LEGACY_HMAC_SECRET)]);
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
///
/// Anti-replant: once this machine has successfully re-signed any record,
/// `legacy_migration_complete` returns true and the legacy path is refused.
/// Without this, an attacker who reads `LEGACY_HMAC_SECRET` out of the
/// binary could plant a forged file post-install (an installer hijack, a
/// pre-existing malicious `~/.config/ttp/` from another tool) and have it
/// accepted on the next launch.
fn verify_license_signature(record: &LicenseRecord) -> SigVerify {
    verify_license_signature_with(record, &machine_hmac_secret(), || {
        crate::keychain::legacy_migration_complete(KEYCHAIN_ACCOUNT)
    })
}

/// The verifier proper, with its two keychain-backed inputs passed in — the
/// same split `compute_license_signature` / `compute_license_signature_with`
/// already uses, so tests can exercise the real decision logic without the
/// OS keychain (and the authorization dialog it raises on every recompile).
///
/// `legacy_closed` is a closure, not a bool, deliberately: the migration flag
/// must stay unread when the machine secret already verified. A keychain read
/// on that path is what put 9.5 seconds into the middle of a dictation once
/// (see keychain.rs); eager evaluation here would quietly put it back.
fn verify_license_signature_with(
    record: &LicenseRecord,
    machine_secret: &[u8],
    legacy_closed: impl FnOnce() -> bool,
) -> SigVerify {
    let Some(stored_hex) = record.signature.as_deref() else {
        return SigVerify::NotPresent;
    };
    let Ok(stored_bytes) = hex::decode(stored_hex) else {
        return SigVerify::Invalid;
    };

    let input = license_signature_input(record);

    let mut mac = HmacSha256::new_from_slice(machine_secret).expect("32-byte secret");
    mac.update(input.as_bytes());
    if mac.verify_slice(&stored_bytes).is_ok() {
        return SigVerify::ValidMachine;
    }

    // Legacy fallback only allowed before the first successful migration.
    // After mark_legacy_migration_complete fires (in save_license below), the
    // ValidLegacy path is permanently closed for this account on this machine.
    if !legacy_closed() {
        let mut legacy = HmacSha256::new_from_slice(LEGACY_HMAC_SECRET).expect("32-byte secret");
        legacy.update(input.as_bytes());
        if legacy.verify_slice(&stored_bytes).is_ok() {
            return SigVerify::ValidLegacy;
        }
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

    // First successful machine-signed write on this machine closes the legacy
    // verification path forever. After this, a forged file signed with the
    // hardcoded `LEGACY_HMAC_SECRET` is rejected (`SigVerify::Invalid`).
    crate::keychain::mark_legacy_migration_complete(KEYCHAIN_ACCOUNT);
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests — pure-function coverage of the HMAC sign/verify path.
//
// These cover the linchpin of Pro entitlement: is_pro_disk() trusts whatever
// load_license() returns, and load_license() trusts whatever verify_license_
// signature() returns. Before this test module, the HMAC verifier shipped
// with zero regression coverage — a hash-input format drift across versions
// would have silently demoted every paid user back to Free.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_record() -> LicenseRecord {
        LicenseRecord {
            license_key: "TTP-TEST-0000-0001".into(),
            instance_id: "instance-uuid-1".into(),
            instance_name: "Test-Machine".into(),
            status: "active".into(),
            expires_at: Some(2_000_000_000),
            last_validated_at: 1_700_000_000,
            activation_count: Some(1),
            activation_limit: Some(3),
            signature: None,
        }
    }

    fn test_secret() -> [u8; 32] {
        // Stable per-test secret so we don't touch the OS keychain in unit
        // tests. The verifier is exercised via compute_license_signature_with.
        *b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1A\x1B\x1C\x1D\x1E\x1F\x20"
    }

    fn sign_with(record: &mut LicenseRecord, secret: &[u8]) {
        record.signature = Some(compute_license_signature_with(record, secret));
    }

    fn verify_with(record: &LicenseRecord, secret: &[u8]) -> bool {
        let Some(sig) = record.signature.as_deref() else { return false; };
        let Ok(bytes) = hex::decode(sig) else { return false; };
        let mut mac = HmacSha256::new_from_slice(secret).expect("32-byte secret");
        mac.update(license_signature_input(record).as_bytes());
        mac.verify_slice(&bytes).is_ok()
    }

    #[test]
    fn round_trip_sign_verify() {
        let secret = test_secret();
        let mut record = fresh_record();
        sign_with(&mut record, &secret);
        assert!(verify_with(&record, &secret), "sign+verify roundtrip must succeed");
    }

    #[test]
    fn tampered_license_key_rejected() {
        let secret = test_secret();
        let mut record = fresh_record();
        sign_with(&mut record, &secret);
        record.license_key = "TTP-FAKE-0000-9999".into();
        assert!(!verify_with(&record, &secret), "tampered license_key must fail verification");
    }

    #[test]
    fn tampered_status_rejected() {
        let secret = test_secret();
        let mut record = fresh_record();
        sign_with(&mut record, &secret);
        record.status = "expired".into();
        assert!(!verify_with(&record, &secret), "tampered status must fail verification");
    }

    #[test]
    fn tampered_expires_rejected() {
        let secret = test_secret();
        let mut record = fresh_record();
        sign_with(&mut record, &secret);
        record.expires_at = Some(9_999_999_999);
        assert!(!verify_with(&record, &secret), "tampered expires_at must fail verification");
    }

    #[test]
    fn tampered_instance_id_rejected() {
        let secret = test_secret();
        let mut record = fresh_record();
        sign_with(&mut record, &secret);
        record.instance_id = "instance-other-machine".into();
        assert!(!verify_with(&record, &secret), "tampered instance_id must fail verification");
    }

    #[test]
    fn different_secret_rejected() {
        let mut record = fresh_record();
        sign_with(&mut record, &test_secret());
        let other_secret = [0xAAu8; 32];
        assert!(!verify_with(&record, &other_secret), "secret mismatch must fail verification");
    }

    #[test]
    fn malformed_hex_signature_is_invalid_not_panic() {
        let mut record = fresh_record();
        record.signature = Some("not-valid-hex-zzz".into());
        // The real verifier, with the keychain's two inputs stubbed. The
        // panicking closure is the assertion: bad hex must bail out before
        // anything consults the migration flag.
        match verify_license_signature_with(&record, &test_secret(), || {
            panic!("must not read the migration flag for undecodable hex")
        }) {
            SigVerify::Invalid => { /* expected */ }
            _ => panic!("malformed hex must return SigVerify::Invalid"),
        }
    }

    #[test]
    fn empty_signature_rejected_as_invalid_hex() {
        let mut record = fresh_record();
        record.signature = Some(String::new());
        // Empty string decodes to empty bytes, which the HMAC verify_slice
        // rejects (wrong length). Both the machine and the legacy branch must
        // refuse it, so the flag is left open to prove neither accepts.
        let v = verify_license_signature_with(&record, &test_secret(), || false);
        assert!(
            matches!(v, SigVerify::Invalid),
            "empty signature must be Invalid (not panic, not ValidMachine)"
        );
    }

    #[test]
    fn absent_signature_is_not_present() {
        let record = fresh_record();
        assert!(
            matches!(
                verify_license_signature_with(&record, &test_secret(), || false),
                SigVerify::NotPresent
            ),
            "an unsigned pre-1.6.2 record must report NotPresent, not Invalid"
        );
    }

    #[test]
    fn machine_signed_record_is_valid_machine() {
        let mut record = fresh_record();
        sign_with(&mut record, &test_secret());
        assert!(
            matches!(
                verify_license_signature_with(&record, &test_secret(), || {
                    panic!("a machine-verified record must not read the migration flag")
                }),
                SigVerify::ValidMachine
            ),
            "record signed with the machine secret must verify as ValidMachine"
        );
    }

    #[test]
    fn legacy_signed_record_accepted_before_migration() {
        // A license.json written by v1.7.x, seen on the first run after
        // upgrade: the machine secret does not match, the migration has not
        // happened, so the legacy secret is allowed to rescue it.
        let mut record = fresh_record();
        sign_with(&mut record, LEGACY_HMAC_SECRET);
        assert!(
            matches!(
                verify_license_signature_with(&record, &test_secret(), || false),
                SigVerify::ValidLegacy
            ),
            "legacy-signed record must verify while the migration is still open"
        );
    }

    #[test]
    fn legacy_signed_record_refused_after_migration() {
        // The anti-replant property. Same bytes as the test above; the only
        // difference is that this machine has already re-signed something,
        // which closes the legacy path forever. Anyone who extracts
        // LEGACY_HMAC_SECRET from the binary and plants a forged file post-
        // install must land here.
        let mut record = fresh_record();
        sign_with(&mut record, LEGACY_HMAC_SECRET);
        assert!(
            matches!(
                verify_license_signature_with(&record, &test_secret(), || true),
                SigVerify::Invalid
            ),
            "legacy-signed record must be refused once the migration is complete"
        );
    }

    /// The one assertion that cannot be stubbed: that the production wrapper
    /// really is wired to the keychain, so a record signed by
    /// `compute_license_signature` verifies as ValidMachine end to end.
    /// Everything above stubs the secret; this proves the stub matches the
    /// real seam. It reads (and on a clean machine creates) the
    /// `license_hmac_secret` keychain entry, which raises a macOS
    /// authorization dialog whenever the test binary's code signature has
    /// changed — i.e. after every recompile. That is why it is ignored.
    ///
    /// Since the keychain is now diverted for the whole test configuration
    /// (see `crate::keychain::read_or_create_hmac_secret`), this test opts
    /// itself back in via `TTP_TEST_REAL_KEYCHAIN`. Without that it would pass
    /// against the substitute and prove nothing — a tautology wearing the
    /// name of a guard.
    ///
    /// Run deliberately, and expect to type a password:
    ///     cargo test --lib licensing::storage -- --ignored
    #[test]
    #[ignore = "reads the real macOS keychain; raises an authorization dialog"]
    fn keychain_backed_wrapper_round_trips() {
        // SAFETY: this test is `#[ignore]`d and run alone, so no other thread
        // is reading the environment. It is set before the first keychain
        // touch, so the process-lifetime cache is populated from the real
        // entry rather than the substitute.
        unsafe { std::env::set_var("TTP_TEST_REAL_KEYCHAIN", "1") };
        let mut record = fresh_record();
        record.signature = Some(compute_license_signature(&record));
        assert!(
            matches!(verify_license_signature(&record), SigVerify::ValidMachine),
            "production sign+verify must agree through the keychain secret"
        );
    }

    #[test]
    fn signature_input_format_is_pipe_delimited_and_stable() {
        // Pinning the exact wire format so a future refactor that reorders
        // fields immediately breaks this test rather than silently
        // invalidating every cached license in the field. If you intentionally
        // change the format, bump a schema version + add a migration path.
        let record = fresh_record();
        let input = license_signature_input(&record);
        assert_eq!(
            input,
            "TTP-TEST-0000-0001|active|2000000000|1700000000|1|3|instance-uuid-1"
        );
    }
}
