// TTP - Accessibility permissions
// Checks if accessibility permission is granted for keyboard simulation

/// Check if accessibility permission is granted
///
/// Uses macOS Accessibility API to check if the app has permission.
///
/// Returns:
/// - `true` if permission is granted
/// - `false` if permission is denied
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Use ApplicationServices framework to check accessibility
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }

        unsafe { AXIsProcessTrusted() }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true // No accessibility check needed on other platforms
    }
}
