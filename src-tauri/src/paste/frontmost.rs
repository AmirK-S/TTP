// TTP - Which application the paste was aimed at
//
// The trace could say `paste.verify {"ax_readable":false}` 211 times across a
// 540-dictation corpus and not name a single application responsible, because
// nothing on the paste path ever recorded where the text was going. That made
// the largest hole in the product's observability un-attributable: the blind
// reads arrive in long runs — one of 80 consecutive, spanning a full day —
// which is exactly the signature of "the maintainer worked in one app all
// afternoon", and there was no way to say which app.
//
// This records the bundle identifier of the frontmost application, and only
// that. Not the window title, not the document name, not the URL: those are
// user content and the trace deliberately keeps user content behind the
// verbose-diagnostics switch.
//
// `NSWorkspace.frontmostApplication` rather than an AX query, for two reasons
// measured on 2026-09-05: the system-wide `AXFocusedApplication` returns
// `kAXErrorCannotComplete` (-25204) against an app that does not serve the
// accessibility API — precisely the apps we most need named — and the
// AppleScript route (`System Events` → frontmost process) costs ~100 ms warm
// and 740 ms cold, which is unaffordable between the user's last word and
// their text appearing. The NSWorkspace read is an in-process property access.

/// Bundle identifier of the frontmost application, e.g. `com.google.Chrome`.
///
/// `None` when there is no frontmost application, when it has no bundle
/// identifier (a bare executable), or on platforms without the API.
#[cfg(target_os = "macos")]
pub fn frontmost_bundle_id() -> Option<String> {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }
        let bundle_id: id = msg_send![app, bundleIdentifier];
        if bundle_id == nil {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_bundle_id() -> Option<String> {
    None
}
