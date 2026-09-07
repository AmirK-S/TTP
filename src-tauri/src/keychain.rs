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
use std::time::Instant;

/// Anything slower than this gets a line of its own.
///
/// A warm keychain round-trip is sub-millisecond; the pathological one is
/// seconds. 50 ms is comfortably above the former and two orders of magnitude
/// below the latter, so the threshold does not have to be defended precisely —
/// it only has to separate "cached" from "macOS is re-evaluating the ACL".
/// Below it we would be writing a line per usage record for no information.
const SLOW_KEYCHAIN_MS: u128 = 50;

/// Record a keychain interaction that was slow enough to be felt.
///
/// The keychain-on-the-critical-path defect cost a user 7.6 seconds of a
/// wedged state machine and was found by reading a gap between two trace
/// lines that happened to bracket it. Timing the call itself means the next
/// one names itself instead of having to be inferred from a hole.
fn record(op: &str, account: &str, started: Instant, extra: serde_json::Value) {
    let ms = started.elapsed().as_millis();
    if ms < SLOW_KEYCHAIN_MS {
        return;
    }
    let mut fields = serde_json::json!({
        "op": op,
        // The account name is a fixed slug from this crate
        // (`ttp_usage_hmac`, `ttp_license_hmac`), never anything the user
        // typed, and never the secret itself.
        "account": account,
        "ms": ms as u64,
    });
    if let (Some(dst), Some(src)) = (fields.as_object_mut(), extra.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    crate::trace::event("keychain.slow", fields);
}

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
    let started = Instant::now();
    let outcome = read_once(secret_cache(), account, || {
        read_or_create_hmac_secret(account, legacy_fallback)
    });
    if !outcome.was_cached {
        record(
            "secret_read",
            account,
            started,
            // Non-zero means this caller did not make a keychain call at all:
            // it waited for someone else's and took their answer. Before
            // single-flighting, each of those callers paid the full cost
            // itself — which is what eight `secret_read` lines in one session
            // of a process that caches for its whole lifetime were telling us.
            serde_json::json!({ "lock_wait_ms": outcome.waited_ms }),
        );
    }
    outcome.value
}

/// What [`read_once`] did, so the caller can say so on the trace.
pub(crate) struct ReadOnce<T> {
    pub value: T,
    /// The value was already in the cache when we looked: no keychain call,
    /// no waiting.
    pub was_cached: bool,
    /// How long we blocked behind another caller's in-flight read. Zero when
    /// we did the read ourselves.
    pub waited_ms: u64,
}

/// Read a value into `cache` under `account`, making **at most one** call to
/// `read` however many callers arrive together.
///
/// The bug this replaces: the previous shape checked the cache, *released the
/// lock*, and then did the expensive read. Every caller that arrived before
/// the first one finished missed the cache and made its own keychain call.
///
/// That is not theoretical. `warm_caches` runs on a background thread at
/// launch precisely so the first dictation does not pay for the ACL
/// evaluation — but with no single-flight it does not protect the dictation
/// at all, it just adds one more concurrent reader. `ttp-trace.log` for
/// 2026-08-31 has eight `keychain.slow {"op":"secret_read"}` lines for one
/// account in one session — 62.3 s, 13.6 s, 397.6 s, 97.4 s, 0.16 s, 56.0 s,
/// 0.11 s, 0.09 s — from a process whose cache is supposed to make that
/// number one. securityd serialises requests for the same item, so N
/// simultaneous misses do not overlap; they queue, and the last caller waits
/// for all of them.
///
/// Holding the lock across the read is the whole fix. A caller that arrives
/// during someone else's read now blocks on a mutex and is handed the answer,
/// instead of joining the queue at securityd.
pub(crate) fn read_once<T, F>(cache: &Mutex<HashMap<String, T>>, account: &str, read: F) -> ReadOnce<T>
where
    T: Clone,
    F: FnOnce() -> T,
{
    // Fast path: warm cache, no contention, no allocation of a wait.
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(account) {
            return ReadOnce { value: cached.clone(), was_cached: true, waited_ms: 0 };
        }
    }

    let waiting_since = Instant::now();
    // A poisoned lock must not put the app back on the uncoalesced path, so
    // recover rather than falling through to an unguarded read.
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let waited_ms = waiting_since.elapsed().as_millis() as u64;
    // Someone else won the race while we waited. Take their answer; do not
    // make a second call for a value that is now sitting in front of us.
    if let Some(cached) = guard.get(account) {
        return ReadOnce { value: cached.clone(), was_cached: false, waited_ms };
    }
    let value = read();
    guard.insert(account.to_string(), value.clone());
    ReadOnce { value, was_cached: false, waited_ms }
}

/// Under `cargo test`, nothing reaches the real keychain.
///
/// This is not belt-and-braces; it closes a hole in a proof. G0 established
/// that "no running test touches the keychain" by stubbing
/// `licensing::storage::machine_hmac_secret` to `panic!` and observing that
/// every test still passed. That result was true and environment-dependent:
/// `load_usage` and `load_license` both early-return when their JSON file
/// does not exist, so on a machine with no `usage.json` the keychain is
/// genuinely never reached — and on a machine that has actually run TTP, it
/// is. `cosmetics::unlocked()` calls `is_pro_or_trial_disk()`, which calls
/// `load_usage()`, which signs, which reads the keychain.
///
/// Measured on this machine: `cargo test --lib` after any change to the
/// library takes **200 seconds**, of which ~3 minutes is one securityd ACL
/// re-evaluation for the freshly linked test binary — the same 62-second
/// class of read this workstream exists to get off the dictation path. The
/// second run, against the same binary, takes 0.5 s.
///
/// A test suite whose cost and whose authorization prompts depend on whether
/// the developer has ever used the app is not a suite anyone can reason
/// about. Substituting the value here — at the single choke point both stores
/// go through — makes it impossible by construction rather than true by
/// circumstance.
///
/// What this does NOT weaken: the secret is still 32 bytes, still per-account,
/// still stable within a run, so every signing and verification path behaves
/// exactly as it does against a real per-machine secret. What it cannot cover
/// is the keychain interaction itself, which has no test coverage either way
/// and is guarded by the deliberately-`#[ignore]`d
/// `licensing::storage::keychain_backed_wrapper_round_trips`.
/// Set `TTP_TEST_REAL_KEYCHAIN=1` to opt one run back into the real thing.
///
/// Exactly one test wants that: `licensing::storage::
/// keychain_backed_wrapper_round_trips`, which asserts the production wrapper
/// really is wired to the keychain and so cannot be stubbed by definition. It
/// is `#[ignore]`d and run deliberately. Without this hatch, substituting the
/// secret here would quietly turn that test into a tautology — it would pass
/// against the stub and prove nothing, which is worse than deleting it.
#[cfg(test)]
const REAL_KEYCHAIN_ENV: &str = "TTP_TEST_REAL_KEYCHAIN";

#[cfg(test)]
fn read_or_create_hmac_secret(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
    if std::env::var(REAL_KEYCHAIN_ENV).is_ok() {
        return read_or_create_hmac_secret_real(account, legacy_fallback);
    }
    sha256_32(format!("ttp-test-hmac-secret::{}", account).as_bytes())
}

/// Uncached read. Split out so the caching wrapper above stays readable, and
/// so tests can reason about the keychain interaction in one place.
#[cfg(not(test))]
fn read_or_create_hmac_secret(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
    read_or_create_hmac_secret_real(account, legacy_fallback)
}

/// The real keychain round-trip. Compiled in every configuration so the
/// `#[cfg(test)]` substitution above is a *diversion* and not a deletion —
/// the code the ignored test exercises is the same code release builds run.
fn read_or_create_hmac_secret_real(account: &str, legacy_fallback: &[u8; 32]) -> [u8; 32] {
    let entry = match keyring::Entry::new(KEYCHAIN_SERVICE, account) {
        Ok(e) => e,
        Err(e) => {
            // The keychain is unavailable and we quietly fall back to the
            // constant every copy of TTP ships with. The tamper resistance
            // the per-machine secret exists for is gone for this session, and
            // nothing anywhere said so.
            crate::trace::degraded(
                "keychain.secret",
                serde_json::json!({
                    "account": account,
                    "error": e.to_string(),
                    "fallback": "legacy_constant",
                }),
            );
            return *legacy_fallback;
        }
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
    if let Err(e) = getrandom::fill(&mut bytes) {
        crate::trace::degraded(
            "keychain.csprng",
            serde_json::json!({
                "account": account,
                "error": e.to_string(),
                "fallback": "legacy_constant",
            }),
        );
        // CSPRNG failure is exotic; fall back to the constant rather than
        // crashing the user's app over a tamper-resistance feature.
        return *legacy_fallback;
    }
    let encoded = hex::encode(bytes);
    if let Err(e) = entry.set_password(&encoded) {
        // A secret that could not be persisted is regenerated on the next
        // launch, which silently invalidates every record signed with this
        // one — the user's license cache and usage file stop verifying and
        // are treated as tampered.
        crate::trace::degraded(
            "keychain.secret_write",
            serde_json::json!({ "account": account, "error": e.to_string() }),
        );
    }
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

    // Constructing the entry is cheap and cannot block; `get_password` is the
    // call that can sit for a minute. Doing this part outside `read_once`
    // keeps the deliberate "do not cache an unavailable keychain" rule below,
    // which `read_once` would otherwise overwrite with a permanent `false`.
    let Some(read) = migration_flag_reader(account) else {
        // Not cached: an unavailable keychain may become available later
        // (the user grants access), and answering `false` forever would keep
        // the legacy verification path alive past its migration.
        crate::trace::degraded(
            "keychain.migration_flag",
            serde_json::json!({ "account": account, "assumed": false }),
        );
        return false;
    };

    let started = Instant::now();
    let outcome = read_once(migration_cache(), account, read);
    if !outcome.was_cached {
        record(
            "migration_flag_read",
            account,
            started,
            serde_json::json!({ "complete": outcome.value, "lock_wait_ms": outcome.waited_ms }),
        );
    }
    outcome.value
}

/// A closure that performs the one blocking keychain read, or `None` when the
/// keychain could not be opened at all.
#[cfg(not(test))]
fn migration_flag_reader(account: &str) -> Option<impl FnOnce() -> bool> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)).ok()?;
    Some(move || matches!(entry.get_password().as_deref(), Ok("1")))
}

/// See [`read_or_create_hmac_secret`]. Under test the keychain is not opened,
/// and the answer is the one an unavailable keychain gives — "not migrated",
/// which keeps the legacy verification path alive exactly as the documented
/// degraded case does.
#[cfg(test)]
fn migration_flag_reader(account: &str) -> Option<impl FnOnce() -> bool> {
    let _ = migration_flag_account(account);
    Some(|| false)
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

    let started = Instant::now();
    let Some(wrote) = write_migration_flag(account) else {
        return;
    };
    record(
        "migration_flag_write",
        account,
        started,
        serde_json::json!({ "ok": wrote }),
    );
    if wrote {
        if let Ok(mut cache) = migration_cache().lock() {
            cache.insert(account.to_string(), true);
        }
    }
}

/// Write the marker. `None` when the keychain could not be opened.
#[cfg(not(test))]
fn write_migration_flag(account: &str) -> Option<bool> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &migration_flag_account(account)).ok()?;
    Some(entry.set_password("1").is_ok())
}

/// See [`read_or_create_hmac_secret`]. A test must not write to the real
/// keychain any more than it may read from one.
#[cfg(test)]
fn write_migration_flag(account: &str) -> Option<bool> {
    let _ = migration_flag_account(account);
    None
}

/// Populate the caches ahead of first use.
///
/// The caches above remove the *repeated* keychain cost, but not the first
/// one, and the first one is the expensive one: it is where macOS evaluates
/// the ACL, and where it puts up an authorization dialog if it wants one.
/// Left to happen lazily, that bill lands on whichever code path touches the
/// keychain first — which is a dictation, with the user watching an empty
/// text field. Measured on the first dictation after a relaunch: 3.8 seconds
/// between a 312ms API response and the stage completing.
///
/// Call this at startup from a blocking task. Nothing waits on it; it either
/// finishes before the first dictation, in which case the cost is invisible,
/// or it does not, in which case we are no worse off than before.
pub fn warm_caches(accounts: &[(&str, &[u8; 32])]) {
    for (account, fallback) in accounts {
        let started = Instant::now();
        let _ = get_or_create_hmac_secret(account, fallback);
        let _ = legacy_migration_complete(account);
        // Always emitted, however fast. This is the bill for the whole
        // session's keychain use and the number that says whether warming
        // worked: a large value here is good news, because it means the cost
        // was paid on a thread nobody was waiting on. The same number showing
        // up on `keychain.api_key` or `usage.recorded` instead is the bug.
        crate::trace::event(
            "keychain.warmed",
            serde_json::json!({
                "account": account,
                "ms": started.elapsed().as_millis() as u64,
            }),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// The previous shape, kept verbatim so the test below can be seen to
    /// discriminate rather than merely to pass. It checks the cache, releases
    /// the lock, and only then does the expensive work — which is how eight
    /// `keychain.slow` lines appeared in one session of a process that caches
    /// for its whole lifetime.
    fn read_check_then_act<T, F>(cache: &Mutex<HashMap<String, T>>, account: &str, read: F) -> T
    where
        T: Clone,
        F: FnOnce() -> T,
    {
        if let Ok(guard) = cache.lock() {
            if let Some(cached) = guard.get(account) {
                return cached.clone();
            }
        }
        let value = read();
        if let Ok(mut guard) = cache.lock() {
            guard.insert(account.to_string(), value.clone());
        }
        value
    }

    /// Eight threads want the same secret at once — `warm_caches` on its
    /// background thread, and every `load_usage` / `save_usage` a dictation
    /// makes. Exactly one of them may call the keychain.
    #[test]
    fn concurrent_readers_make_one_keychain_call() {
        let cache: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
        let calls = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = cache.clone();
            let n = calls.clone();
            handles.push(std::thread::spawn(move || {
                read_once(&c, "usage_hmac_secret", || {
                    n.fetch_add(1, Ordering::SeqCst);
                    // Stand in for securityd re-evaluating the ACL. The real
                    // one measured 62,304 ms.
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    42u32
                })
                .value
            }));
        }
        let values: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "every caller after the first must take the winner's answer, not queue at securityd"
        );
        assert!(values.iter().all(|&v| v == 42), "all callers get the same secret");
    }

    /// The discriminator: the same eight threads against the shape this
    /// replaced. If this ever stops failing to coalesce, the test above has
    /// stopped proving anything.
    #[test]
    fn the_previous_shape_did_not_coalesce() {
        let cache: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
        let calls = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = cache.clone();
            let n = calls.clone();
            handles.push(std::thread::spawn(move || {
                read_check_then_act(&c, "usage_hmac_secret", || {
                    n.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    42u32
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(
            calls.load(Ordering::SeqCst) > 1,
            "the old shape lets concurrent callers each make their own call — \
             if this no longer holds, so does the defect it describes"
        );
    }

    /// A caller that arrives after the value is cached must not report itself
    /// as having done a read; otherwise `keychain.slow` would print a line per
    /// usage record and the signal would drown.
    #[test]
    fn a_warm_read_reports_itself_as_cached() {
        let cache: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
        let first = read_once(&cache, "acct", || 7u32);
        assert!(!first.was_cached);
        assert_eq!(first.waited_ms, 0);

        let second = read_once(&cache, "acct", || panic!("must not read twice"));
        assert!(second.was_cached);
        assert_eq!(second.value, 7);
    }

    /// Different accounts are independent — the licence secret and the usage
    /// secret must not shadow each other.
    #[test]
    fn accounts_do_not_share_an_entry() {
        let cache: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
        assert_eq!(read_once(&cache, "usage", || 1u32).value, 1);
        assert_eq!(read_once(&cache, "license", || 2u32).value, 2);
        assert_eq!(read_once(&cache, "usage", || 99u32).value, 1);
    }

    /// A poisoned cache must not send callers back to the uncoalesced path.
    /// The alternative to recovering here is the 62-second read, per caller,
    /// for the rest of the session.
    #[test]
    fn a_poisoned_cache_still_coalesces() {
        let cache: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
        let c = cache.clone();
        let _ = std::thread::spawn(move || {
            let _g = c.lock().unwrap();
            panic!("poison the cache");
        })
        .join();

        assert!(cache.lock().is_err(), "precondition: the lock is poisoned");
        assert_eq!(read_once(&cache, "acct", || 5u32).value, 5);
        assert_eq!(
            read_once(&cache, "acct", || panic!("must not read twice")).value,
            5
        );
    }
}
