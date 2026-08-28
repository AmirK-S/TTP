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
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const KEYCHAIN_SERVICE: &str = "com.ttp.desktop";

// ── In-process caches ───────────────────────────────────────────────────
//
// A keychain round-trip is not cheap and is not bounded. macOS re-evaluates
// the ACL whenever the requesting binary's code signature changes, and while
// it does so the call blocks — for seconds, and in the worst case until a
// user clicks an authorization dialog they may not have noticed.
//
// That cost was landing in the middle of dictations. `save_usage` signs the
// record (one secret read) and marks the migration flag (one keychain write),
// and it is called from `record_polish_success` and `record_transcription` —
// both on the path between the user finishing a sentence and their text
// appearing. Measured 2026-08-28: a polish stage took 10.1 seconds of which
// the Groq API call was 631ms. The other 9.5 seconds were here.
//
// Both values are safe to cache for the process lifetime. The per-machine
// secret is generated once and never rotates. The migration flag only ever
// transitions false → true, and only via `mark_legacy_migration_complete`,
// which updates the cache as it writes. Nothing else on the machine changes
// them underneath us.
//
// This does not weaken the tamper resistance the secrets exist for: an
// attacker who can read this process's memory has already lost the game, and
// could equally read the secret from any of the call sites that had it in a
// local before.

fn secret_cache() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    static CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn migration_cache() -> &'static Mutex<HashMap<String, bool>> {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read a 32-byte HMAC secret from the OS keychain, creating one with the
/// OS CSPRNG on first call. Returns the legacy hardcoded fallback if the
/// keychain is unavailable (e.g. the user denied access) so we degrade
/// gracefully into the v1.7.x behaviour rather than refusing to start.
pub fn get_or_create_hmac_secret(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
    if let Ok(cache) = secret_cache().lock() {
        if let Some(cached) = cache.get(account) {
            return *cached;
        }
    }

    let secret = read_or_create_hmac_secret(account, legacy_fallback);

    if let Ok(mut cache) = secret_cache().lock() {
        cache.insert(account.to_string(), secret);
    }
    secret
}

/// Uncached read. Split out so the caching wrapper above stays readable, and
/// so tests can reason about the keychain interaction in one place.
fn read_or_create_hmac_secret(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
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
    if let Ok(cache) = migration_cache().lock() {
        if let Some(cached) = cache.get(account) {
            return *cached;
        }
    }

    let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)) else {
        // Not cached: an unavailable keychain may become available later
        // (the user grants access), and answering `false` forever would keep
        // the legacy verification path alive past its migration.
        return false;
    };
    let complete = matches!(entry.get_password().as_deref(), Ok("1"));

    if let Ok(mut cache) = migration_cache().lock() {
        cache.insert(account.to_string(), complete);
    }
    complete
}

/// Persist the "legacy migration complete" marker for `account`. Called
/// from the licensing / usage stores after the first successful re-sign
/// with the per-machine secret.
pub fn mark_legacy_migration_complete(account: &str) {
    // Already known-complete: skip the keychain write entirely. This is the
    // hot one — `save_usage` calls it on every single write, so without this
    // every recorded transcription paid for a keychain round-trip to set a
    // flag that was already set.
    if let Ok(cache) = migration_cache().lock() {
        if cache.get(account) == Some(&true) {
            return;
        }
    }

    let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)) else {
        return;
    };
    if entry.set_password("1").is_ok() {
        if let Ok(mut cache) = migration_cache().lock() {
            cache.insert(account.to_string(), true);
        }
    }
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
