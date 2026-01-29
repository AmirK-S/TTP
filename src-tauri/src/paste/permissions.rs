// TTP - Accessibility permissions
// Checks if accessibility permission is granted for keyboard simulation
//
// Note: tauri-plugin-macos-permissions is NOT available as a Cargo crate
// (it's JavaScript-only). We check permission via enigo's behavior instead.
// If enigo can be initialized, accessibility permission is granted.

use enigo::{Enigo, Settings};

/// Check if accessibility permission is granted
///
/// This works by attempting to create an Enigo instance.
/// On macOS, if accessibility permission is not granted,
/// enigo will fail to initialize.
///
/// Returns:
/// - `true` if permission is granted (enigo initialized successfully)
/// - `false` if permission is denied (enigo failed to initialize)
///
/// Note: This is a practical approach since enigo needs accessibility
/// permission to function. The actual permission prompt appears when
/// the user first tries to use keyboard simulation, not when we check.
pub fn check_accessibility() -> bool {
    // Try to create an Enigo instance
    // If this succeeds, we have accessibility permission
    Enigo::new(&Settings::default()).is_ok()
}
