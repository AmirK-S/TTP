// TTP - Accessibility permissions
// Checks if accessibility permission is granted for keyboard simulation

/// Check if accessibility permission is granted
///
/// Uses AXIsProcessTrustedWithOptions to check if the app has permission.
/// This is more reliable than AXIsProcessTrusted() because after an app update,
/// macOS may invalidate the trust entry (the binary hash changes) while still
/// showing the app as enabled in System Preferences. AXIsProcessTrusted() can
/// return true for a stale entry, but actual AX calls will fail.
///
/// Returns:
/// - `true` if permission is granted
/// - `false` if permission is denied
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        check_accessibility_impl(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        true // No accessibility check needed on other platforms
    }
}

/// Check accessibility and optionally prompt the user to grant it.
///
/// When `prompt` is true, macOS will show the system dialog asking the user
/// to grant accessibility access if it is not currently trusted. This is
/// especially useful after an app update where the previous trust entry became
/// stale — the prompt gives the user a direct path to re-enable.
#[cfg(target_os = "macos")]
pub fn check_accessibility_with_prompt(prompt: bool) -> bool {
    check_accessibility_impl(prompt)
}

#[cfg(target_os = "macos")]
fn check_accessibility_impl(prompt: bool) -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(
            options: core_foundation::base::CFTypeRef,
        ) -> bool;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = if prompt {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };

    let options = CFDictionary::from_CFType_pairs(&[(key, value)]);

    unsafe { AXIsProcessTrustedWithOptions(options.as_CFTypeRef()) }
}

/// Perform a real accessibility probe to detect stale trust entries.
///
/// After an app update, AXIsProcessTrusted/WithOptions may still return true
/// because the old entry exists in the TCC database. But actual AX API calls
/// will fail. This function tries a real AX call to verify the permission
/// actually works.
///
/// Returns:
/// - `true` if accessibility genuinely works
/// - `false` if the permission is missing or stale
#[cfg(target_os = "macos")]
pub fn probe_accessibility() -> bool {
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::CFString;
    use std::ffi::c_void;

    type AXUIElementRef = *mut c_void;
    type AXError = i32;
    const AX_ERROR_API_DISABLED: AXError = -25211;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: core_foundation::string::CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
    }

    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return false;
        }

        // Try to get the focused application — this will fail with
        // kAXErrorAPIDisabled (-25211) if trust is stale/missing.
        let attr = CFString::new("AXFocusedApplication");
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            attr.as_concrete_TypeRef(),
            &mut value,
        );
        CFRelease(system_wide as CFTypeRef);

        if !value.is_null() {
            CFRelease(value);
        }

        // Success or "not implemented" (some contexts) means AX is working.
        // API_DISABLED means the trust is stale or revoked.
        err != AX_ERROR_API_DISABLED
    }
}

/// Reset the stale TCC accessibility entry for this app.
///
/// When an app update changes the binary, the old TCC entry becomes stale.
/// This function uses `tccutil` to reset the accessibility entry for this
/// app's bundle ID, clearing the stale state so the user gets a clean
/// re-prompt.
/// This is the most destructive thing TTP does to a user's machine, and it
/// used to leave a `log_warn` and nothing else. `docs/tracing.md` promises
/// `paste.accessibility` with `tcc_trusted` vs `ax_probe_ok`, but that line is
/// written during a dictation — long after a reset that happens at launch. A
/// user whose granted permission was wiped on startup had nothing in
/// `ttp-trace.log` explaining why they were suddenly being asked for it again.
///
/// So the event is emitted **here**, at the one function that runs `tccutil`,
/// rather than at the three call sites — the same reasoning as the
/// `start_recording` / `stop_recording` wrappers in `audio_capture`: tracing
/// the boundary cannot miss a caller, and a fourth caller added later is
/// traced by default.
///
/// It is emitted **before** the command runs, carrying the two probe values
/// that justified the decision, so the record exists even if the process does
/// not survive what happens next. The outcome follows on its own line.
#[cfg(target_os = "macos")]
pub fn reset_accessibility_tcc() -> Result<(), String> {
    // Get the bundle identifier
    let bundle_id = get_bundle_id().ok_or("Could not determine bundle identifier")?;

    // The state that led here. Re-probed rather than passed in, so the record
    // describes the moment of the reset and not the caller's older reading.
    crate::trace::event(
        "permission.tcc_reset",
        serde_json::json!({
            "bundle_id": bundle_id,
            "api_trusted": check_accessibility(),
            "ax_probe_ok": probe_accessibility(),
            "version": env!("CARGO_PKG_VERSION"),
        }),
    );

    // Reset TCC entry for this bundle
    let output = std::process::Command::new("tccutil")
        .args(["reset", "Accessibility", &bundle_id])
        .output()
        .map_err(|e| {
            crate::trace::event(
                "permission.tcc_reset_result",
                serde_json::json!({ "ok": false, "error": e.to_string() }),
            );
            format!("Failed to run tccutil: {}", e)
        })?;

    if output.status.success() {
        crate::trace::event(
            "permission.tcc_reset_result",
            serde_json::json!({ "ok": true }),
        );
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::trace::event(
            "permission.tcc_reset_result",
            // The user's grant is gone either way at this point; whether
            // tccutil said so matters for telling "reset and re-prompted"
            // apart from "asked to reset and was refused".
            serde_json::json!({ "ok": false, "stderr": stderr.trim() }),
        );
        Err(format!("tccutil failed: {}", stderr))
    }
}

/// Get the app's bundle identifier
#[cfg(target_os = "macos")]
fn get_bundle_id() -> Option<String> {
    Some("com.ttp.desktop".to_string())
}
