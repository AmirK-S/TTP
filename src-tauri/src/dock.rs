// TTP - A Dock tile while a real window is open
//
// TTP runs as a menu bar agent (`LSUIElement`, `ActivationPolicy::Accessory`):
// no Dock tile and no entry in ⌘-Tab. That is right while it only listens for
// the hotkey, and wrong once Settings or onboarding is open. Amir, 2026-09-15:
// ⌘-Tab to Claude and there is no way back to Settings, the window is "lost"
// behind everything else.
//
// So the policy follows the windows: `Regular` while Settings or onboarding
// is visible, `Accessory` again when the last of them hides (the red button
// hides rather than closes — see `on_window_event` in lib.rs). Every way of
// showing those windows ends in `set_focus`, and every way of hiding them
// moves focus away, so the focus event is the one hook that sees them all.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{ActivationPolicy, AppHandle, Manager, Runtime};

/// Windows that make TTP a regular app while they are on screen. The pill and
/// the permission helper are panels over other apps and never count.
pub const APP_WINDOWS: &[&str] = &["settings", "onboarding"];

/// Whether the policy is currently `Regular`, so a focus change that does not
/// change the answer does not touch AppKit.
static REGULAR: AtomicBool = AtomicBool::new(false);

/// The policy for this set of visible windows.
pub fn policy_for(any_app_window_visible: bool) -> ActivationPolicy {
    if any_app_window_visible {
        ActivationPolicy::Regular
    } else {
        ActivationPolicy::Accessory
    }
}

/// Re-evaluate after an app window was shown, focused, hidden or destroyed.
pub fn sync<R: Runtime>(app: &AppHandle<R>) {
    let visible = APP_WINDOWS.iter().any(|label| {
        app.get_webview_window(label)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    });
    if REGULAR.swap(visible, Ordering::SeqCst) == visible {
        return;
    }
    let _ = app.set_activation_policy(policy_for(visible));
    crate::trace::event(
        "dock.policy",
        serde_json::json!({ "regular": visible }),
    );
    if visible {
        // Leaving Accessory can leave the window behind the previously active
        // app; bring it back in front now that it has a Dock tile.
        for label in APP_WINDOWS {
            if let Some(w) = app.get_webview_window(label) {
                if w.is_visible().unwrap_or(false) {
                    let _ = w.set_focus();
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_window_puts_ttp_in_the_dock_and_closing_it_takes_it_out() {
        assert!(matches!(policy_for(true), ActivationPolicy::Regular));
        assert!(matches!(policy_for(false), ActivationPolicy::Accessory));
    }

    #[test]
    fn the_pill_and_the_permission_helper_never_count() {
        assert!(!APP_WINDOWS.contains(&"pill"));
        assert!(!APP_WINDOWS.contains(&"permission-helper"));
    }
}
