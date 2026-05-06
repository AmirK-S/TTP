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
