// TTP - Accessibility text reading
// Reads the text content of the currently focused UI element using macOS Accessibility API
// Supports both native text fields (AXValue) and Chrome contenteditable (AXStringForRange)

#[cfg(target_os = "macos")]
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use std::ffi::c_void;

#[cfg(target_os = "macos")]
type AXUIElementRef = *mut c_void;
#[cfg(target_os = "macos")]
type AXError = i32;
#[cfg(target_os = "macos")]
const AX_ERROR_SUCCESS: AXError = 0;

#[cfg(target_os = "macos")]
/// AXValueType for CFRange
const K_AX_VALUE_TYPE_CF_RANGE: u32 = 4;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        parameterized_attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueCreate(value_type: u32, value_ptr: *const c_void) -> CFTypeRef;
}

/// Read the text content of the currently focused UI element
///
/// Tries multiple strategies:
/// 1. AXValue — works for native text fields (TextEdit, Notes, VS Code, etc.)
/// 2. AXStringForRange — works for Chrome/web contenteditable elements
///
/// Requires Accessibility permission (already granted for paste simulation).
#[cfg(target_os = "macos")]
pub fn read_focused_text() -> Option<String> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        // Get the focused UI element
        let focused_attr = CFString::new("AXFocusedUIElement");
        let mut focused: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            focused_attr.as_concrete_TypeRef(),
            &mut focused,
        );
        CFRelease(system_wide as CFTypeRef);

        if err != AX_ERROR_SUCCESS || focused.is_null() {
            return None;
        }

        // Strategy 1: Try AXValue (native text fields)
        if let Some(text) = read_ax_value(focused as AXUIElementRef) {
            CFRelease(focused);
            return Some(text);
        }

        // Strategy 2: Try AXStringForRange (Chrome contenteditable, web apps)
        if let Some(text) = read_ax_string_for_range(focused as AXUIElementRef) {
            CFRelease(focused);
            return Some(text);
        }

        CFRelease(focused);

        // Strategy 3: Try Chrome JavaScript execution via osascript
        read_chrome_active_element()
    }
}

/// Try reading text from Chrome's active element via AppleScript + JavaScript
/// Requires "Allow JavaScript from Apple Events" enabled in Chrome (View > Developer)
#[cfg(target_os = "macos")]
fn read_chrome_active_element() -> Option<String> {
    // Check if Chrome is frontmost
    let check = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first process whose frontmost is true")
        .output()
        .ok()?;

    let app_name = String::from_utf8_lossy(&check.stdout).trim().to_string();
    if app_name != "Google Chrome" {
        return None;
    }

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"Google Chrome\" to tell active tab of front window to execute javascript \"document.activeElement.innerText || document.activeElement.value || ''\"")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Try reading text via AXValue attribute
#[cfg(target_os = "macos")]
unsafe fn read_ax_value(element: AXUIElementRef) -> Option<String> {
    let value_attr = CFString::new("AXValue");
    let mut value: CFTypeRef = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        value_attr.as_concrete_TypeRef(),
        &mut value,
    );

    if err != AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }

    let cf_type_id = core_foundation::base::CFGetTypeID(value);
    if cf_type_id == CFString::type_id() {
        let cf_string = CFString::wrap_under_create_rule(value as CFStringRef);
        Some(cf_string.to_string())
    } else {
        CFRelease(value);
        None
    }
}

/// Try reading text via AXNumberOfCharacters + AXStringForRange
/// This works for Chrome contenteditable and other web-based text areas
#[cfg(target_os = "macos")]
unsafe fn read_ax_string_for_range(element: AXUIElementRef) -> Option<String> {
    use core_foundation::number::CFNumber;

    // Get text length
    let num_attr = CFString::new("AXNumberOfCharacters");
    let mut num_ref: CFTypeRef = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        num_attr.as_concrete_TypeRef(),
        &mut num_ref,
    );

    if err != AX_ERROR_SUCCESS || num_ref.is_null() {
        return None;
    }

    let cf_type_id = core_foundation::base::CFGetTypeID(num_ref);
    if cf_type_id != CFNumber::type_id() {
        CFRelease(num_ref);
        return None;
    }

    let cf_number = CFNumber::wrap_under_create_rule(num_ref as *const _ as _);
    let count = cf_number.to_i64()?;

    if count <= 0 {
        return None;
    }

    // Create CFRange(0, count) and wrap as AXValue
    #[repr(C)]
    struct CFRange {
        location: i64,
        length: i64,
    }

    let range = CFRange {
        location: 0,
        length: count,
    };
    let range_value = AXValueCreate(
        K_AX_VALUE_TYPE_CF_RANGE,
        &range as *const _ as *const c_void,
    );

    if range_value.is_null() {
        return None;
    }

    // Get string for range
    let string_attr = CFString::new("AXStringForRange");
    let mut string_ref: CFTypeRef = std::ptr::null_mut();
    let err = AXUIElementCopyParameterizedAttributeValue(
        element,
        string_attr.as_concrete_TypeRef(),
        range_value,
        &mut string_ref,
    );
    CFRelease(range_value);

    if err != AX_ERROR_SUCCESS || string_ref.is_null() {
        return None;
    }

    let cf_type_id = core_foundation::base::CFGetTypeID(string_ref);
    if cf_type_id == CFString::type_id() {
        let cf_string = CFString::wrap_under_create_rule(string_ref as CFStringRef);
        Some(cf_string.to_string())
    } else {
        CFRelease(string_ref);
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_focused_text() -> Option<String> {
    None
}
