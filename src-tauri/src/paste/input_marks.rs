// TTP - Who else was typing while we injected
//
// 0024-8368 (2026-09-26): 156 characters typed into the Claude composer, 21
// arrived, and the message was sent a fraction of a second later. The trace
// could say what TTP posted and what the field held, and nothing about what
// else reached the keyboard queue at the same moment — a key the user pressed,
// or another program injecting. Both are hypotheses for how the chunks were
// lost, and neither could be checked.
//
// The hotkey's HID event tap already sees every key-down. It tallies them here
// by origin, counts only, never which key except Return, so `paste.verify` can
// say how many foreign key-downs overlapped the injection and when the user
// hit Return.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Key-downs from the hardware (including Karabiner's virtual keyboard).
static USER_KEYS: AtomicU64 = AtomicU64::new(0);
/// Key-downs synthesised by another process.
static FOREIGN_KEYS: AtomicU64 = AtomicU64::new(0);
/// Wall-clock ms of the last hardware Return / keypad Enter.
static LAST_USER_RETURN_MS: AtomicU64 = AtomicU64::new(0);

const KVK_RETURN: u16 = 36;
const KVK_KEYPAD_ENTER: u16 = 76;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Called by the event tap for every key-down. `source_pid` is the event's
/// `kCGEventSourceUnixProcessID`: 0 for hardware, ours for our own injection.
pub fn note_key_down(source_pid: i64, keycode: u16) {
    if source_pid == std::process::id() as i64 {
        return;
    }
    if source_pid == 0 {
        USER_KEYS.fetch_add(1, Ordering::Relaxed);
        if keycode == KVK_RETURN || keycode == KVK_KEYPAD_ENTER {
            LAST_USER_RETURN_MS.store(now_ms(), Ordering::Relaxed);
        }
    } else {
        FOREIGN_KEYS.fetch_add(1, Ordering::Relaxed);
    }
}

/// A point to measure from: take one before injecting, diff it afterwards.
#[derive(Debug, Clone, Copy)]
pub struct InputMark {
    user: u64,
    foreign: u64,
    at_ms: u64,
}

pub fn mark() -> InputMark {
    InputMark {
        user: USER_KEYS.load(Ordering::Relaxed),
        foreign: FOREIGN_KEYS.load(Ordering::Relaxed),
        at_ms: now_ms(),
    }
}

impl InputMark {
    /// Trace fields for everything that happened since this mark.
    pub fn since(&self) -> serde_json::Value {
        let ret = LAST_USER_RETURN_MS.load(Ordering::Relaxed);
        serde_json::json!({
            "user_keys": USER_KEYS.load(Ordering::Relaxed).saturating_sub(self.user),
            "foreign_keys": FOREIGN_KEYS.load(Ordering::Relaxed).saturating_sub(self.foreign),
            // How long after the injection began the user pressed Return.
            "user_return_ms": (ret >= self.at_ms).then(|| ret - self.at_ms),
        })
    }
}
