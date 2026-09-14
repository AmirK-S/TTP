// TTP - Talk To Paste
// "Drag TTP into the list above": the permission helper.
//
// Granting Accessibility or Input Monitoring normally means finding TTP in
// Finder, or hunting for the "+" button. Instead, System Settings opens on the
// right page and a small panel sits on its bottom edge with TTP's icon, which
// the user drags into the list. Research and the pitfalls this code avoids are
// in `docs/permission-drag-research.md`.
//
// Pieces:
//   * `show_permission_helper` opens the pane, creates the panel window
//     (rendered by `src/windows/PermissionHelper.tsx`) and starts a tracker.
//   * The tracker, every 150 ms: stops if the permission is granted (and says
//     so to the onboarding window), otherwise keeps the panel on the bottom of
//     the System Settings window while System Settings (or TTP) is in front,
//     and hides it otherwise.
//   * The drag itself is started from the panel by `tauri-plugin-drag`, with
//     the path from `app_bundle_path`.
//
// Microphone is not here on purpose: that list has no "+" and only ever shows
// apps that asked, so the system prompt is the only way.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "permission-helper";
const WIDTH: f64 = 460.0;
const HEIGHT: f64 = 92.0;
/// Width of the System Settings sidebar; the panel centres on the content area
/// to its right, which is where the list it points at lives.
const SIDEBAR_WIDTH: f64 = 215.0;
const BOTTOM_MARGIN: f64 = 14.0;
const TICK: Duration = Duration::from_millis(150);
/// A helper nobody used is closed rather than left floating for the session.
const GIVE_UP_AFTER: Duration = Duration::from_secs(600);

const SETTINGS_BUNDLE_ID: &str = "com.apple.systempreferences";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    Accessibility,
    InputMonitoring,
}

impl PermissionKind {
    fn pane_url(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Self::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        }
    }

    fn granted(self) -> bool {
        match self {
            Self::Accessibility => crate::paste::check_accessibility(),
            Self::InputMonitoring => crate::fnkey::has_input_monitoring(),
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::InputMonitoring => "inputMonitoring",
        }
    }
}

/// Bumped every time a helper opens or closes, so a tracker from an earlier
/// request can tell it has been replaced and stop.
static SESSION: AtomicU64 = AtomicU64::new(0);
/// 0 = none, 1 = accessibility, 2 = input monitoring.
static CURRENT_KIND: AtomicU8 = AtomicU8::new(0);

/// The `.app` bundle TTP is running from, or `None` for a bare dev binary —
/// in which case there is nothing to drag, and TCC would grant the terminal
/// that launched it anyway.
pub fn bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.extension().map_or(false, |e| e == "app"))
        .map(|p| p.to_path_buf())
}

#[tauri::command]
pub fn app_bundle_path() -> Option<String> {
    bundle_path().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn permission_helper_kind() -> Option<PermissionKind> {
    match CURRENT_KIND.load(Ordering::Relaxed) {
        1 => Some(PermissionKind::Accessibility),
        2 => Some(PermissionKind::InputMonitoring),
        _ => None,
    }
}

/// Open System Settings on `kind`'s page and, when TTP runs from a bundle,
/// the drag panel beside it. Returns `"granted"`, `"panel"`, or
/// `"settings_only"` (no bundle to drag).
#[tauri::command]
pub fn show_permission_helper(app: AppHandle, kind: PermissionKind) -> Result<String, String> {
    if kind.granted() {
        return Ok("granted".into());
    }

    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(kind.pane_url(), None::<&str>)
            .map_err(|e| e.to_string())?;
    }

    let bundle = bundle_path();
    let bundle_str = bundle.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    crate::trace::event(
        "permission.helper_shown",
        serde_json::json!({
            "kind": kind.slug(),
            "bundle": bundle.is_some(),
            // A translocated or DMG-mounted app is a read-only temporary path;
            // a grant made to it does not follow the app to /Applications.
            "translocated": bundle_str.contains("/AppTranslocation/") || bundle_str.starts_with("/Volumes/"),
        }),
    );
    if bundle.is_none() {
        return Ok("settings_only".into());
    }

    CURRENT_KIND.store(
        match kind {
            PermissionKind::Accessibility => 1,
            PermissionKind::InputMonitoring => 2,
        },
        Ordering::Relaxed,
    );
    let session = SESSION.fetch_add(1, Ordering::Relaxed) + 1;

    if let Some(window) = app.get_webview_window(LABEL) {
        // Reused for the other permission: tell the panel to re-read its kind.
        let _ = window.emit("permission-helper-kind", kind);
    } else {
        WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("index.html".into()))
            .title("TTP")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .visible_on_all_workspaces(true)
            // Without this the first click only activates TTP, and the drag
            // never starts.
            .accept_first_mouse(true)
            .build()
            .map_err(|e| format!("Failed to create permission helper: {}", e))?;
    }

    // The onboarding window floats above everything and would cover the very
    // list the user has to drop into.
    if let Some(onboarding) = app.get_webview_window("onboarding") {
        let _ = onboarding.set_always_on_top(false);
    }

    let app_for_tracker = app.clone();
    std::thread::Builder::new()
        .name("ttp-permission-helper".into())
        .spawn(move || track(app_for_tracker, kind, session))
        .map_err(|e| e.to_string())?;

    Ok("panel".into())
}

#[tauri::command]
pub fn close_permission_helper(app: AppHandle) {
    SESSION.fetch_add(1, Ordering::Relaxed);
    close(&app, "dismissed");
}

fn close(app: &AppHandle, reason: &str) {
    CURRENT_KIND.store(0, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.close();
        crate::trace::event("permission.helper_closed", serde_json::json!({ "reason": reason }));
    }
    if let Some(onboarding) = app.get_webview_window("onboarding") {
        let _ = onboarding.set_always_on_top(true);
    }
}

fn track(app: AppHandle, kind: PermissionKind, session: u64) {
    let started = Instant::now();
    let mut visible = false;
    loop {
        std::thread::sleep(TICK);
        if SESSION.load(Ordering::Relaxed) != session {
            return;
        }
        if kind.granted() {
            crate::trace::event(
                "permission.helper_granted",
                serde_json::json!({ "kind": kind.slug(), "secs": started.elapsed().as_secs() }),
            );
            let _ = app.emit("permission-granted", kind);
            close(&app, "granted");
            return;
        }
        if started.elapsed() > GIVE_UP_AFTER {
            close(&app, "timed_out");
            return;
        }

        let Some(window) = app.get_webview_window(LABEL) else { return };
        match unsafe { settings_window_frame() } {
            Some(frame) => {
                let content_x = frame.x + SIDEBAR_WIDTH;
                let content_w = (frame.w - SIDEBAR_WIDTH).max(WIDTH);
                let x = content_x + (content_w - WIDTH) / 2.0;
                let y = frame.y + frame.h - HEIGHT - BOTTOM_MARGIN;
                let _ = window.set_position(LogicalPosition::new(x, y));
                if !visible {
                    let _ = window.show();
                    visible = true;
                }
            }
            None if visible => {
                let _ = window.hide();
                visible = false;
            }
            None => {}
        }
    }
}

struct Frame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// The System Settings main window, in global top-left points — but only while
/// System Settings or TTP itself is in front, so the panel does not hover over
/// whatever the user switched to.
///
/// Window owners and bounds are readable without Screen Recording permission;
/// only titles are not, and nothing here reads a title. "In front" is read
/// from the window list itself (it comes back front-to-back) rather than from
/// `NSWorkspace`, whose accessors want the main thread — and this runs on the
/// tracker's own thread.
unsafe fn settings_window_frame() -> Option<Frame> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> id;
    }
    const ON_SCREEN_ONLY: u32 = 1;
    const EXCLUDE_DESKTOP: u32 = 16;

    let pool = NSAutoreleasePool::new(nil);
    let result = (|| {
        let own_pid = std::process::id() as i32;

        let bundle_id = NSString::alloc(nil).init_str(SETTINGS_BUNDLE_ID);
        let apps: id = msg_send![class!(NSRunningApplication), runningApplicationsWithBundleIdentifier: bundle_id];
        let count: usize = msg_send![apps, count];
        if count == 0 {
            return None;
        }
        let settings_app: id = msg_send![apps, objectAtIndex: 0usize];
        let settings_pid: i32 = msg_send![settings_app, processIdentifier];

        let windows = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0);
        if windows == nil {
            return None;
        }
        let key_pid = NSString::alloc(nil).init_str("kCGWindowOwnerPID");
        let key_layer = NSString::alloc(nil).init_str("kCGWindowLayer");
        let key_bounds = NSString::alloc(nil).init_str("kCGWindowBounds");
        let (kx, ky, kw, kh) = (
            NSString::alloc(nil).init_str("X"),
            NSString::alloc(nil).init_str("Y"),
            NSString::alloc(nil).init_str("Width"),
            NSString::alloc(nil).init_str("Height"),
        );
        let number = |dict: id, key: id| -> f64 {
            let n: id = msg_send![dict, objectForKey: key];
            if n == nil { 0.0 } else { msg_send![n, doubleValue] }
        };

        let mut best: Option<Frame> = None;
        let mut front_pid: Option<i32> = None;
        let n: usize = msg_send![windows, count];
        for i in 0..n {
            let info: id = msg_send![windows, objectAtIndex: i];
            let pid = number(info, key_pid) as i32;
            let layer = number(info, key_layer) as i32;
            // The list is front-to-back, so the first ordinary window belongs
            // to whatever the user is looking at.
            if layer == 0 && front_pid.is_none() {
                front_pid = Some(pid);
            }
            if pid != settings_pid || layer != 0 {
                continue;
            }
            let bounds: id = msg_send![info, objectForKey: key_bounds];
            if bounds == nil {
                continue;
            }
            let frame = Frame {
                x: number(bounds, kx),
                y: number(bounds, ky),
                w: number(bounds, kw),
                h: number(bounds, kh),
            };
            if best.as_ref().map_or(true, |b| frame.w * frame.h > b.w * b.h) {
                best = Some(frame);
            }
        }
        let _: () = msg_send![windows, release];
        match front_pid {
            Some(pid) if pid == settings_pid || pid == own_pid => best,
            _ => None,
        }
    })();
    pool.drain();
    result
}
