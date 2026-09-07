// TTP - App Nap suppression while a dictation is in flight
//
// TTP ships as an `LSUIElement` agent: no Dock tile, no menu bar presence
// beyond the tray item. That is exactly the profile macOS App Naps most
// aggressively, and a napped process does not slow down — it stops. Timers
// stop firing, run loops stop turning, worker threads stop being scheduled.
//
// This was observed rather than theorised. A dictation on 27 August showed a
// 4.2-second hole between two adjacent trivial statements in the pipeline
// (`ui.completed` → `usage.recorded`, which is one JSON write apart), and the
// Fn poll timer — an NSTimer on the main run loop, a completely separate
// scheduling context — reported `timer_stall {"gap_ms":4178}` covering the
// same window. Two independent contexts cannot both stall for the same 4.2
// seconds because one function was slow. The process was descheduled.
//
// The consequence is not cosmetic. The very next line was:
//
//     hotkey.tap_rearmed {"reason":"timeout"}
//
// macOS disables a CGEventTap whose callback stops responding, and before
// the recovery path existed that left the Fn key dead until restart. So App
// Nap does not merely delay a dictation: it is a route into the exact
// "TTP just stopped working" failure this whole investigation started from.
// An earlier dictation showed the same shape at 12 minutes instead of 4
// seconds.
//
// `beginActivityWithOptions:reason:` is the documented way to tell the OS
// that a stretch of work is user-initiated and must not be suspended. We
// hold one from the moment recording starts until the state machine returns
// to Idle — precisely the window in which the user is standing there waiting
// for their words to appear.

#[cfg(target_os = "macos")]
mod imp {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Mutex;

    /// `NSActivityUserInitiated` with the idle-system-sleep bit cleared.
    ///
    /// The plain `NSActivityUserInitiated` also asserts
    /// `NSActivityIdleSystemSleepDisabled`, which would keep the whole
    /// machine awake for as long as we hold the token. A dictation must not
    /// stop a laptop from sleeping — we only want to not be napped
    /// ourselves, so that bit is masked off.
    const NS_ACTIVITY_IDLE_SYSTEM_SLEEP_DISABLED: u64 = 1 << 20;
    const NS_ACTIVITY_USER_INITIATED: u64 = 0x00FF_FFFF | NS_ACTIVITY_IDLE_SYSTEM_SLEEP_DISABLED;
    const OPTIONS: u64 = NS_ACTIVITY_USER_INITIATED & !NS_ACTIVITY_IDLE_SYSTEM_SLEEP_DISABLED;

    /// The live activity token, as a raw pointer.
    ///
    /// Stored as `usize` because the Objective-C object is not `Send`, and we
    /// begin and end from whichever thread drives the state machine. We only
    /// ever pass it straight back to `endActivity:`, never message it
    /// otherwise, so treating it as an opaque handle is sound.
    static TOKEN: Mutex<Option<usize>> = Mutex::new(None);

    /// Take an activity assertion. Idempotent: a second call while one is
    /// already held is a no-op, so a re-entered Recording state cannot leak
    /// a token.
    pub fn begin(reason: &str) {
        let Ok(mut slot) = TOKEN.lock() else { return };
        if slot.is_some() {
            return;
        }
        unsafe {
            let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
            if process_info == nil {
                return;
            }
            let reason_str = NSString::alloc(nil).init_str(reason);
            let token: id = msg_send![
                process_info,
                beginActivityWithOptions: OPTIONS
                reason: reason_str
            ];
            if token == nil {
                return;
            }
            // The returned token is autoreleased; retain it so it survives
            // until we hand it back to endActivity:.
            let token: id = msg_send![token, retain];
            *slot = Some(token as usize);
        }
    }

    /// Release the activity assertion. Safe to call when none is held.
    pub fn end() {
        let Ok(mut slot) = TOKEN.lock() else { return };
        let Some(raw) = slot.take() else { return };
        unsafe {
            let token = raw as id;
            let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
            if process_info != nil {
                let _: () = msg_send![process_info, endActivity: token];
            }
            let _: () = msg_send![token, release];
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    // App Nap is a macOS concept. Windows throttles background processes far
    // less aggressively and offers no equivalent per-activity assertion, so
    // there is nothing to hold here.
    pub fn begin(_reason: &str) {}
    pub fn end() {}
}

/// Begin an activity assertion for the duration of a dictation.
pub fn begin_dictation() {
    imp::begin("TTP dictation in flight — the user is waiting for text");
}

/// End the dictation activity assertion.
pub fn end_dictation() {
    imp::end();
}
