// TTP - Reading the screen through the Accessibility API
//
// Runs on its own thread at recording start (see `mod.rs`). Everything here
// is bounded: every element gets a short messaging timeout, so an app that
// stops answering costs a fraction of a second instead of AX's default six,
// and the window walk stops on a node and a time budget.
//
// The timeout is set per element, never on the system-wide element: that one
// changes the default for every AX call in the process, including the paste
// verifier's.

use super::{
    extract_terms, head_chars, non_blank, redact_secrets, split_at_selection, CaptureStats,
    Captured, ScreenContext, TITLE_MAX,
};
use cocoa::base::{id, nil};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, CFTypeID, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::{HashSet, VecDeque};
use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Spelled as in `paste/accessibility.rs`, so the two extern blocks agree.
type AXUIElementRef = *mut c_void;
type AXError = i32;
const AX_OK: AXError = 0;
const AX_VALUE_CF_RANGE: u32 = 4;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyMultipleAttributeValues(
        element: AXUIElementRef,
        attributes: CFArrayRef,
        options: u32,
        values: *mut CFArrayRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: f32) -> AXError;
    fn AXUIElementGetTypeID() -> CFTypeID;
    fn AXValueCreate(value_type: u32, value: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, out: *mut c_void) -> bool;
    fn AXIsProcessTrusted() -> bool;
}

/// Seconds any single AX call may wait on the target app.
const MESSAGING_TIMEOUT_S: f32 = 0.15;
/// The window walk stops after this long…
const WALK_BUDGET: Duration = Duration::from_millis(180);
/// …or this many elements…
const WALK_NODES_MAX: usize = 900;
/// …or once this much text has been collected.
const WALK_CHARS_MAX: usize = 6000;
/// A single text on screen is looked at up to this length.
const WALK_TEXT_MAX: usize = 300;
/// A focused field longer than this (a terminal's whole scrollback) is read
/// around the cursor through `AXStringForRange` instead of in full.
const FIELD_FULL_READ_MAX: usize = 20_000;

/// How long the tree of a just-woken Electron app is given to build. The
/// capture runs while the user talks, so this costs them nothing.
const ELECTRON_WAKE_WAIT: Duration = Duration::from_millis(600);

/// Apps whose screen is never read: password managers, the system's own
/// credential prompts, and TTP itself.
const EXCLUDED_BUNDLES: &[&str] = &[
    "com.ttp.desktop",
    "com.apple.keychainaccess",
    "com.apple.Passwords",
    "com.apple.SecurityAgent",
    "com.apple.loginwindow",
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.agilebits.onepassword-osx",
    "com.bitwarden.desktop",
    "com.lastpass.LastPass",
    "org.keepassxc.keepassxc",
    "me.proton.pass.electron",
    "com.dashlane.dashlanephonefinal",
];

/// An owned AX element with a short timeout.
struct El(CFType);

impl El {
    /// Takes ownership of a +1 reference.
    unsafe fn owned(ptr: AXUIElementRef) -> Option<El> {
        if ptr.is_null() {
            return None;
        }
        let el = El(CFType::wrap_under_create_rule(ptr as CFTypeRef));
        AXUIElementSetMessagingTimeout(el.raw(), MESSAGING_TIMEOUT_S);
        Some(el)
    }

    fn raw(&self) -> AXUIElementRef {
        self.0.as_CFTypeRef() as AXUIElementRef
    }

    fn attr(&self, name: &str) -> Option<CFType> {
        let name = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(self.raw(), name.as_concrete_TypeRef(), &mut out)
        };
        if err != AX_OK || out.is_null() {
            return None;
        }
        Some(unsafe { CFType::wrap_under_create_rule(out) })
    }

    fn string(&self, name: &str) -> Option<String> {
        self.attr(name)?.downcast::<CFString>().map(|s| s.to_string())
    }

    fn element(&self, name: &str) -> Option<El> {
        let value = self.attr(name)?;
        as_element(&value)
    }

    fn range(&self, name: &str) -> Option<(usize, usize)> {
        let value = self.attr(name)?;
        cf_range(&value)
    }

    fn number(&self, name: &str) -> Option<usize> {
        let n = self.attr(name)?.downcast::<CFNumber>()?.to_i64()?;
        usize::try_from(n).ok()
    }

    fn string_for_range(&self, location: usize, length: usize) -> Option<String> {
        #[repr(C)]
        struct Range {
            location: isize,
            length: isize,
        }
        let range = Range {
            location: location as isize,
            length: length as isize,
        };
        unsafe {
            let param = AXValueCreate(AX_VALUE_CF_RANGE, &range as *const _ as *const c_void);
            if param.is_null() {
                return None;
            }
            let param = CFType::wrap_under_create_rule(param);
            let name = CFString::new("AXStringForRange");
            let mut out: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyParameterizedAttributeValue(
                self.raw(),
                name.as_concrete_TypeRef(),
                param.as_CFTypeRef(),
                &mut out,
            );
            if err != AX_OK || out.is_null() {
                return None;
            }
            CFType::wrap_under_create_rule(out)
                .downcast::<CFString>()
                .map(|s| s.to_string())
        }
    }

    /// Several attributes in one round trip. Missing ones come back `None`.
    fn many(&self, names: &CFArray<CFString>) -> Option<Vec<Option<CFType>>> {
        let mut out: CFArrayRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyMultipleAttributeValues(
                self.raw(),
                names.as_concrete_TypeRef(),
                0,
                &mut out,
            )
        };
        if err != AX_OK || out.is_null() {
            return None;
        }
        let values: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(out) };
        // A missing attribute is an AXValue holding an error; it is neither a
        // string nor an array nor an element, so the readers below skip it.
        Some(values.iter().map(|v| Some(v.clone())).collect())
    }
}

fn as_element(value: &CFType) -> Option<El> {
    if value.type_of() != unsafe { AXUIElementGetTypeID() } {
        return None;
    }
    let el = El(value.clone());
    unsafe { AXUIElementSetMessagingTimeout(el.raw(), MESSAGING_TIMEOUT_S) };
    Some(el)
}

fn cf_range(value: &CFType) -> Option<(usize, usize)> {
    #[repr(C)]
    #[derive(Default)]
    struct Range {
        location: isize,
        length: isize,
    }
    let mut range = Range::default();
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef(),
            AX_VALUE_CF_RANGE,
            &mut range as *mut _ as *mut c_void,
        )
    };
    if !ok || range.location < 0 || range.length < 0 {
        return None;
    }
    Some((range.location as usize, range.length as usize))
}

/// The frontmost app: pid, bundle id, name, and whether it is built on
/// Electron.
struct Frontmost {
    pid: i32,
    bundle_id: Option<String>,
    name: Option<String>,
    electron: bool,
}

fn frontmost() -> Option<Frontmost> {
    unsafe fn nsstring(s: id) -> Option<String> {
        if s == nil {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![s, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        let bundle_id = nsstring(msg_send![app, bundleIdentifier]);
        let name = nsstring(msg_send![app, localizedName]);
        let url: id = msg_send![app, bundleURL];
        let path = if url == nil { None } else { nsstring(msg_send![url, path]) };
        let electron = path.is_some_and(|p| {
            std::path::Path::new(&p)
                .join("Contents/Frameworks/Electron Framework.framework")
                .exists()
        });
        Some(Frontmost {
            pid,
            bundle_id,
            name,
            electron,
        })
    }
}

/// Electron apps (Slack, Notion, VS Code, Discord) build their accessibility
/// tree only when asked. `AXManualAccessibility` is the switch Electron
/// documents for tools that are not screen readers; unlike
/// `AXEnhancedUserInterface` it does not change how windows animate. Asked
/// once per process. The tree builds asynchronously: on 2026-09-14 the first
/// dictation into the Claude desktop app right after the switch saw 13 nodes,
/// no focused field and no text. `capture` waits [`ELECTRON_WAKE_WAIT`] after
/// flipping it.
fn wake_electron(app: &El, pid: i32) -> bool {
    static WOKEN: Mutex<Option<HashSet<i32>>> = Mutex::new(None);
    let Ok(mut woken) = WOKEN.lock() else {
        return false;
    };
    let set = woken.get_or_insert_with(HashSet::new);
    if !set.insert(pid) {
        return false;
    }
    let name = CFString::new("AXManualAccessibility");
    unsafe {
        AXUIElementSetAttributeValue(
            app.raw(),
            name.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
    }
    true
}

fn is_secure(role: Option<&str>, subrole: Option<&str>) -> bool {
    role == Some("AXSecureTextField") || subrole == Some("AXSecureTextField")
}

pub fn capture() -> Captured {
    let started = Instant::now();
    if !unsafe { AXIsProcessTrusted() } {
        return Captured::Skipped("no_accessibility");
    }
    let Some(front) = frontmost() else {
        return Captured::Skipped("no_frontmost_app");
    };
    if front
        .bundle_id
        .as_deref()
        .is_some_and(|b| EXCLUDED_BUNDLES.contains(&b))
    {
        return Captured::Skipped("excluded_app");
    }
    let Some(app) = (unsafe { El::owned(AXUIElementCreateApplication(front.pid)) }) else {
        return Captured::Skipped("no_app_element");
    };
    let woke_electron = front.electron && wake_electron(&app, front.pid);
    if woke_electron {
        std::thread::sleep(ELECTRON_WAKE_WAIT);
    }

    let mut ctx = ScreenContext {
        bundle_id: front.bundle_id.clone(),
        app_name: front.name.clone(),
        ..Default::default()
    };
    let mut stats = CaptureStats {
        field: "none",
        electron: front.electron,
        woke_electron,
        ..Default::default()
    };
    // Field text with no known cursor still names things worth spelling.
    let mut loose_field_text: Option<String> = None;

    if let Some(focused) = app.element("AXFocusedUIElement") {
        let role = focused.string("AXRole");
        let subrole = focused.string("AXSubrole");
        if is_secure(role.as_deref(), subrole.as_deref()) {
            return Captured::Skipped("secure_field");
        }
        read_field(&focused, &mut ctx, &mut stats, &mut loose_field_text);
    }

    let window = app.element("AXFocusedWindow");
    if let Some(window) = &window {
        ctx.window_title = window
            .string("AXTitle")
            .and_then(|t| non_blank(head_chars(&t, TITLE_MAX), false));
    }

    let mut window_texts = Vec::new();
    if let Some(window) = &window {
        // Budget counted from here, not from `started`: the Electron wait
        // above would otherwise have spent it.
        match walk_window(window, Instant::now(), &mut stats) {
            Walk::Texts(texts) => window_texts = texts,
            Walk::Secure => return Captured::Skipped("secure_field"),
        }
    }

    let mut sources: Vec<&str> = Vec::new();
    sources.extend(ctx.selected.as_deref());
    sources.extend(ctx.before_cursor.as_deref());
    sources.extend(ctx.after_cursor.as_deref());
    sources.extend(loose_field_text.as_deref());
    sources.extend(ctx.window_title.as_deref());
    sources.extend(window_texts.iter().map(String::as_str));
    ctx.terms = extract_terms(&sources);

    stats.ms = started.elapsed().as_millis() as u64;
    Captured::Context(ctx, stats)
}

/// Read the focused field around its cursor.
fn read_field(
    focused: &El,
    ctx: &mut ScreenContext,
    stats: &mut CaptureStats,
    loose: &mut Option<String>,
) {
    let cursor = focused.range("AXSelectedTextRange");

    // Native fields answer AXValue; a very long one is read around the cursor
    // below instead of pulled whole.
    let count = focused.number("AXNumberOfCharacters");
    let small = count.map_or(true, |n| n <= FIELD_FULL_READ_MAX);
    if small {
        if let Some(value) = focused.string("AXValue") {
            stats.field = "value";
            match cursor {
                Some((loc, len)) => set_split(ctx, &value, loc, len),
                None => {
                    *loose = non_blank(redact_secrets(&head_chars(&value, WALK_TEXT_MAX * 2)), false)
                }
            }
            return;
        }
    }

    // Web fields and long buffers: ask for the text around the cursor only.
    let (Some((loc, len)), Some(count)) = (cursor, count) else {
        return;
    };
    let start = loc.saturating_sub(super::BEFORE_MAX * 2);
    let end = (loc + len + super::AFTER_MAX * 2).min(count);
    if end <= start {
        return;
    }
    if let Some(text) = focused.string_for_range(start, end - start) {
        stats.field = "range";
        set_split(ctx, &text, loc - start, len);
    }
}

/// Split first, redact after: the cursor offsets index the text as the app
/// holds it, and redaction changes lengths.
fn set_split(ctx: &mut ScreenContext, text: &str, loc: usize, len: usize) {
    let (before, selected, after) = split_at_selection(text, loc, len);
    let redact = |part: Option<String>| part.map(|p| redact_secrets(&p));
    ctx.before_cursor = redact(before);
    ctx.selected = redact(selected);
    ctx.after_cursor = redact(after);
}

enum Walk {
    Texts(Vec<String>),
    /// A password field is on screen: read nothing from this window.
    Secure,
}

/// Collect visible text from the window, breadth first, on a budget.
fn walk_window(window: &El, started: Instant, stats: &mut CaptureStats) -> Walk {
    let names = CFArray::from_CFTypes(&[
        CFString::new("AXRole"),
        CFString::new("AXSubrole"),
        CFString::new("AXValue"),
        CFString::new("AXTitle"),
        CFString::new("AXDescription"),
        CFString::new("AXChildren"),
    ]);
    let mut texts = Vec::new();
    let mut chars = 0usize;
    let mut queue: VecDeque<El> = VecDeque::new();
    queue.push_back(El(window.0.clone()));

    while let Some(node) = queue.pop_front() {
        if stats.window_nodes >= WALK_NODES_MAX
            || started.elapsed() >= WALK_BUDGET
            || chars >= WALK_CHARS_MAX
        {
            stats.window_truncated = true;
            break;
        }
        stats.window_nodes += 1;
        let Some(values) = node.many(&names) else {
            continue;
        };
        let get = |i: usize| values.get(i).cloned().flatten();
        let string = |i: usize| get(i).and_then(|v| v.downcast::<CFString>()).map(|s| s.to_string());

        let role = string(0);
        let subrole = string(1);
        if is_secure(role.as_deref(), subrole.as_deref()) {
            return Walk::Secure;
        }

        let text = match role.as_deref() {
            Some("AXStaticText") => string(2).or_else(|| string(3)),
            Some("AXTextField" | "AXTextArea" | "AXComboBox") => string(2),
            Some("AXHeading" | "AXLink" | "AXCell" | "AXRow") => string(3).or_else(|| string(4)),
            _ => None,
        };
        if let Some(text) = text.and_then(|t| non_blank(head_chars(&t, WALK_TEXT_MAX), false)) {
            chars += text.chars().count();
            texts.push(text);
        }

        let children = get(5).filter(|v| v.type_of() == CFArray::<CFType>::type_id()).map(|v| {
            // Type checked on the line above; `get` rule because `v` keeps
            // its own reference.
            unsafe { CFArray::<CFType>::wrap_under_get_rule(v.as_CFTypeRef() as CFArrayRef) }
        });
        if let Some(children) = children {
            for child in children.iter() {
                if queue.len() >= WALK_NODES_MAX {
                    break;
                }
                if let Some(el) = as_element(&child) {
                    queue.push_back(el);
                }
            }
        }
    }
    stats.window_chars = chars;
    Walk::Texts(texts)
}

#[cfg(test)]
mod live {
    /// Reads whatever app is frontmost and prints it. Needs Accessibility for
    /// the process running the test, so it is opt-in:
    /// `cargo test --lib screen_context::macos::live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn prints_a_live_capture() {
        for _ in 0..2 {
            println!("{:#?}", super::capture());
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
    }
}
