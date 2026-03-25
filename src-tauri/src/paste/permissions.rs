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
#[cfg(target_os = "macos")]
pub fn reset_accessibility_tcc() -> Result<(), String> {
    // Get the bundle identifier
    let bundle_id = get_bundle_id().ok_or("Could not determine bundle identifier")?;

    // Reset TCC entry for this bundle
    let output = std::process::Command::new("tccutil")
        .args(["reset", "Accessibility", &bundle_id])
        .output()
        .map_err(|e| format!("Failed to run tccutil: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("tccutil failed: {}", stderr))
    }
}

/// Get the app's bundle identifier
#[cfg(target_os = "macos")]
fn get_bundle_id() -> Option<String> {
    Some("com.ttp.desktop".to_string())
}
