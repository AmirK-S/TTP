// TTP - Talk To Paste
// Shared OS-keychain utilities. Used for both Groq API credentials and
// per-machine HMAC secrets that authenticate the local license + usage caches.
//
// Why this matters for HMAC: previously the secrets were 32-byte constants
// baked into the binary, so anyone who reverse-engineered TTP recovered the
// same secret on every install. A forged license.json signed with the
// constant would be accepted everywhere. Per-machine secrets break that —
// a forgery only works on the machine it was signed for.

use sha2::{Digest, Sha256};

const KEYCHAIN_SERVICE: &str = "com.ttp.desktop";

/// Read a 32-byte HMAC secret from the OS keychain, creating one with the
/// OS CSPRNG on first call. Returns the legacy hardcoded fallback if the
/// keychain is unavailable (e.g. the user denied access) so we degrade
/// gracefully into the v1.7.x behaviour rather than refusing to start.
pub fn get_or_create_hmac_secret(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
    let entry = match keyring::Entry::new(KEYCHAIN_SERVICE, account) {
        Ok(e) => e,
        Err(_) => return *legacy_fallback,
    };

    if let Ok(stored) = entry.get_password() {
        if let Ok(bytes) = hex::decode(&stored) {
            if bytes.len() == 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&bytes);
                return out;
            }
        }
        // Corrupt entry — recreate.
    }

    let mut bytes = [0u8; 32];
    if getrandom::fill(&mut bytes).is_err() {
        // CSPRNG failure is exotic; fall back to the constant rather than
        // crashing the user's app over a tamper-resistance feature.
        return *legacy_fallback;
    }
    let encoded = hex::encode(bytes);
    let _ = entry.set_password(&encoded);
    bytes
}

/// Per-account "legacy migration complete" flag.
///
/// Once a record on this machine has been re-signed with the per-machine
/// secret, the verifier MUST refuse to accept records signed with the
/// `LEGACY_HMAC_SECRET` constant. Without this, an attacker who reverse-
/// engineers the legacy constant out of the binary can plant a forged
/// `license.json` AFTER a clean install — the user's first launch sees it,
/// verifies via the legacy fallback path, and trusts it.
///
/// With the flag set: post-migration, the legacy path is a no-op. The
/// attacker would have to also somehow plant a record signed with the
/// per-machine secret, which they cannot read (it's in the keychain).
///
/// Stored as a keychain entry whose password is the literal string `"1"`.
/// Absence of the entry (or any other value) means "not yet migrated".
const MIGRATION_FLAG_SUFFIX: &str = "_legacy_migration_complete";

fn migration_flag_account(account: &str) -> String {
    format!("{}{}", account, MIGRATION_FLAG_SUFFIX)
}

/// Returns true once `mark_legacy_migration_complete` has been called for
/// `account` on this machine.
pub fn legacy_migration_complete(account: &str) -> bool {
    let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)) else {
        return false;
    };
    matches!(entry.get_password().as_deref(), Ok("1"))
}

/// Persist the "legacy migration complete" marker for `account`. Called
/// from the licensing / usage stores after the first successful re-sign
/// with the per-machine secret.
pub fn mark_legacy_migration_complete(account: &str) {
    let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)) else {
        return;
    };
    let _ = entry.set_password("1");
}

/// Hash arbitrary input to a 32-byte key. Used to derive a stable secret
/// from the legacy constant when we want to pre-seed something.
#[allow(dead_code)]
pub fn sha256_32(input: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(input);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}
