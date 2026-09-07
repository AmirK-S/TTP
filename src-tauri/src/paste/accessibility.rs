// TTP - Accessibility text reading
//
// Reads the focused UI element back so `paste.verify` can say whether an
// injection *landed* rather than merely that it was posted. Native text
// fields answer AXValue; Chrome/web contenteditable answer AXStringForRange;
// a third class answers neither but does answer AXNumberOfCharacters, and for
// those a character count is still a real observation.
//
// ── Why the read reports *why* it failed ────────────────────────────────────
//
// Until Polaris this module returned `Option<String>` and the trace collapsed
// four very different outcomes into one `ax_readable:false`:
//
//   * the system-wide AXFocusedUIElement query errored (the target is not
//     answering AX at all),
//   * it succeeded and there was no focused element (nothing has keyboard
//     focus),
//   * an element was found and exposes no readable text attribute,
//   * we are not on macOS.
//
// Measured over the maintainer's corpus (26 Aug – 5 Sep 2026, re-counted on
// 6 Sep at 548 verifications): 219 of 548 — 40% — were `ax_readable:false`,
// every one of them with `ax_probe_ok:true` and `tcc_trusted:true`. So the
// blindness is not a permission problem, and with one boolean there was no way
// to tell which of the four it was, which app caused it, or what to build
// next. `FocusSnapshot` carries the reason and the raw `AXError` so the next
// harvest can attribute the remaining blind spot instead of guessing at it.
//
// All 219 have a *null baseline* — the read that failed is the one taken
// before injection — and not one is a lost read-back. That is what makes the
// third strategy below (`AXNumberOfCharacters` alone) the load-bearing one:
// the fix has to make the *before* read succeed more often, because there is
// no amount of patience after the fact that recovers a baseline that was never
// taken.

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

/// `AXError` values we name, because the trace records them and a bare
/// `-25212` in a log is not readable by anyone.
///
/// From `ApplicationServices/HIServices/AXError.h`. Only the ones the focused
/// read can actually produce are listed.
#[cfg(target_os = "macos")]
pub mod ax_error {
    /// `kAXErrorCannotComplete` — the target process did not answer. Seen when
    /// the frontmost app does not serve the accessibility API at all.
    pub const CANNOT_COMPLETE: i32 = -25204;
    /// `kAXErrorAttributeUnsupported` — the element exists and does not have
    /// that attribute.
    pub const ATTRIBUTE_UNSUPPORTED: i32 = -25205;
    /// `kAXErrorNoValue` — the attribute exists and is currently empty. On
    /// `AXFocusedUIElement` this is "nothing has keyboard focus", which is a
    /// different fact from "this app is not readable".
    pub const NO_VALUE: i32 = -25212;
}

/// Where a focused-field read got its answer, or where it gave up.
///
/// Written into `paste.verify` as `ax_before` / `ax_after`. The point of the
/// enum over a boolean is that the four failure variants need four different
/// fixes and used to be indistinguishable in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusSource {
    /// `AXValue` — native text fields.
    Value,
    /// `AXStringForRange` — Chrome and other web contenteditable.
    Range,
    /// `AXNumberOfCharacters` answered but neither text attribute did. We know
    /// how long the field is and not what is in it, which is still enough to
    /// see an insertion.
    LengthOnly,
    /// `AXFocusedUIElement` succeeded and returned nothing: no keyboard focus.
    NoFocus,
    /// `AXFocusedUIElement` itself failed. Carries its `AXError` alongside.
    FocusError,
    /// An element was found and exposes no readable text and no length.
    Unreadable,
    /// Not macOS. There is no equivalent read on Windows today.
    Unsupported,
}

impl FocusSource {
    /// Stable slug for the trace. Never change one of these without updating
    /// `docs/tracing.md` — they are grepped.
    pub fn as_str(self) -> &'static str {
        match self {
            FocusSource::Value => "value",
            FocusSource::Range => "range",
            FocusSource::LengthOnly => "length_only",
            FocusSource::NoFocus => "no_focus",
            FocusSource::FocusError => "focus_error",
            FocusSource::Unreadable => "unreadable",
            FocusSource::Unsupported => "unsupported",
        }
    }
}

/// One read of the focused field.
///
/// `text` and `chars` are independent: a field can report its length without
/// reporting its contents, and that case is worth keeping because a length
/// change is still an observation of the paste landing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSnapshot {
    pub text: Option<String>,
    pub chars: Option<usize>,
    pub source: FocusSource,
    /// The `AXError` from the call that decided `source`. Zero when the read
    /// succeeded.
    pub ax_err: i32,
}

impl FocusSnapshot {
    /// A snapshot with nothing in it — the read failed for `source`.
    pub fn blind(source: FocusSource, ax_err: i32) -> Self {
        Self { text: None, chars: None, source, ax_err }
    }

    /// Is there anything here to compare a later snapshot against?
    ///
    /// This is the predicate that decides whether a paste into this target is
    /// verifiable *at all*, and it is known before we inject — which is why
    /// the pipeline can honestly say `pasted_unverified` at
    /// `dictation.finish` instead of waiting for a verifier that has nothing
    /// to look at.
    pub fn observable(&self) -> bool {
        self.text.is_some() || self.chars.is_some()
    }
}

/// What a before/after pair of snapshots proves about an injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteVerdict {
    /// The field changed. The characters landed.
    Observed,
    /// The field was readable and did not change. The keystrokes were
    /// swallowed — this is the shape the stuck-Globe bug produces.
    Swallowed,
    /// We could not look. **Not** evidence of failure, and not evidence of
    /// success either.
    Unverified,
}

impl PasteVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            PasteVerdict::Observed => "observed",
            PasteVerdict::Swallowed => "swallowed",
            PasteVerdict::Unverified => "unverified",
        }
    }
}

/// The verdict plus the kind of evidence behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verification {
    pub verdict: PasteVerdict,
    /// `text` — we compared the field's contents.
    /// `length` — we could only compare its character count.
    /// `none` — we could not compare anything; `reason` says why.
    pub evidence: &'static str,
    /// Only meaningful when `verdict` is `Unverified`.
    pub reason: &'static str,
}

/// Decide what a before/after pair proves. Pure, so it is tested.
///
/// The ordering matters and is the whole point of the function:
///
/// * **No baseline is not a change.** The previous implementation compared
///   `Option<String>` directly, so an unreadable *before* and a readable
///   *after* produced `changed:true` — a self-report wearing an observation's
///   clothes. One dictation in the 540-dictation corpus has exactly that shape
///   (`before_chars:null`, `ax_readable:true`). It now returns `Unverified`
///   with reason `no_baseline`.
/// * **Text beats length.** When both sides yield text we compare text, which
///   catches a same-length replacement that a count would miss.
/// * **Length is real evidence, but only for `Observed`.** A count that did not
///   move does not prove a swallow: typing over a selection of equal length
///   leaves the count identical. So a length-only comparison can promote to
///   `Observed` and must never accuse.
/// * **A comparison needs the same shape on both sides.** Which strategy
///   answers is a property of the element, so text-then-length (or
///   length-then-text) means focus moved between the reads and the two numbers
///   describe different fields. `shape_changed`, checked before the counts.
pub fn classify(before: &FocusSnapshot, after: &FocusSnapshot) -> Verification {
    if !before.observable() {
        return Verification {
            verdict: PasteVerdict::Unverified,
            evidence: "none",
            reason: "no_baseline",
        };
    }
    if !after.observable() {
        return Verification {
            verdict: PasteVerdict::Unverified,
            evidence: "none",
            reason: "read_back_failed",
        };
    }

    if let (Some(b), Some(a)) = (&before.text, &after.text) {
        return if b == a {
            Verification { verdict: PasteVerdict::Swallowed, evidence: "text", reason: "" }
        } else {
            Verification { verdict: PasteVerdict::Observed, evidence: "text", reason: "" }
        };
    }

    // Text on one side and a bare length on the other is not a comparison.
    //
    // This has to be checked *before* the counts, not after: a text-bearing
    // snapshot always carries `chars` as well, so the mixed pair would
    // otherwise be caught by the length comparison below and promoted to
    // `Observed` off two numbers that do not describe the same thing. Which
    // strategy answers is a property of the element — `AXValue` either exists
    // on it or does not — so a strategy that changed between the two reads
    // means the *element* changed, i.e. focus moved. Comparing one field's
    // length against another field's is not an observation, in either
    // direction, and the injection went to whichever element had focus at the
    // time, which we no longer know.
    if before.text.is_some() != after.text.is_some() {
        return Verification {
            verdict: PasteVerdict::Unverified,
            evidence: "none",
            reason: "shape_changed",
        };
    }

    match (before.chars, after.chars) {
        (Some(b), Some(a)) if a != b => {
            Verification { verdict: PasteVerdict::Observed, evidence: "length", reason: "" }
        }
        (Some(_), Some(_)) => Verification {
            verdict: PasteVerdict::Unverified,
            evidence: "length",
            // The count is unchanged. That is consistent with a swallow and
            // equally consistent with a same-length replacement, and we do not
            // have enough to tell them apart. Accusing on this would put false
            // `paste-swallowed` findings into the analyser.
            reason: "length_unchanged",
        },
        // Defensive: `observable()` guarantees text-or-chars on both sides and
        // the shape check above has already handled text-vs-length, so the
        // only pair left is a hand-built snapshot with text and no count.
        // Nothing to compare either way.
        _ => Verification {
            verdict: PasteVerdict::Unverified,
            evidence: "none",
            reason: "shape_changed",
        },
    }
}

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

/// Read the focused UI element, reporting *how* the read went.
///
/// Strategies, in order:
/// 1. `AXValue` — native text fields (TextEdit, Notes, VS Code, Terminal).
/// 2. `AXStringForRange` — Chrome and other web contenteditable.
/// 3. `AXNumberOfCharacters` alone — length without contents. Weaker, and
///    still enough to see an insertion.
///
/// Requires Accessibility permission (already granted for paste simulation).
///
/// **No subprocess, no sleep.** This runs on the dictation's critical path,
/// between the user's last word and their text appearing. The previous
/// implementation ended with an `osascript` round-trip that asked System
/// Events for the frontmost process name and then asked Chrome to evaluate
/// JavaScript. Measured on the maintainer's machine on 2026-09-05: the
/// frontmost-process query costs ~100 ms warm and 740 ms cold, and the Chrome
/// evaluation returns an error — "JavaScript via AppleScript is disabled" —
/// because that is Chrome's default and it has not been turned on. It is also
/// unreachable in practice: it sat *after* an early `return None` on the
/// focus query, and across 540 corpus dictations the blind reads cost the same
/// ~8 ms as the readable ones, so it never once ran. It was two seconds of
/// latency waiting for a target app to be slow enough to reach it, in exchange
/// for an answer the API is configured to refuse. Removed rather than moved
/// off the path; if a Chrome-specific read is ever wanted, `ax_before` in the
/// trace will say how often `focus_error` names Chrome first.
#[cfg(target_os = "macos")]
pub fn probe_focused_text() -> FocusSnapshot {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return FocusSnapshot::blind(FocusSource::FocusError, 0);
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

        if err != AX_ERROR_SUCCESS {
            // `kAXErrorNoValue` here means "nothing has keyboard focus", which
            // is a different fact from "this app does not answer AX"
            // (`kAXErrorCannotComplete`). Both used to render as
            // `ax_readable:false`.
            let source = if err == ax_error::NO_VALUE {
                FocusSource::NoFocus
            } else {
                FocusSource::FocusError
            };
            return FocusSnapshot::blind(source, err);
        }
        if focused.is_null() {
            return FocusSnapshot::blind(FocusSource::NoFocus, err);
        }

        let element = focused as AXUIElementRef;

        // Strategy 1: AXValue (native text fields)
        if let Some(text) = read_ax_value(element) {
            let chars = text.chars().count();
            CFRelease(focused);
            return FocusSnapshot {
                text: Some(text),
                chars: Some(chars),
                source: FocusSource::Value,
                ax_err: AX_ERROR_SUCCESS,
            };
        }

        // Strategy 2 and 3 share the AXNumberOfCharacters call, so ask once.
        let (count, count_err) = read_ax_char_count(element);

        if let Some(count) = count {
            if let Some(text) = read_ax_string_for_range(element, count) {
                let chars = text.chars().count();
                CFRelease(focused);
                return FocusSnapshot {
                    text: Some(text),
                    chars: Some(chars),
                    source: FocusSource::Range,
                    ax_err: AX_ERROR_SUCCESS,
                };
            }
            // Strategy 3: the element told us how long it is and refused to
            // tell us what is in it. Keep the number — comparing it before and
            // after an injection is a real observation, just a coarser one.
            CFRelease(focused);
            return FocusSnapshot {
                text: None,
                chars: Some(count),
                source: FocusSource::LengthOnly,
                ax_err: AX_ERROR_SUCCESS,
            };
        }

        CFRelease(focused);
        FocusSnapshot::blind(FocusSource::Unreadable, count_err)
    }
}

/// Text of the focused element, or `None`.
///
/// Kept for callers that only want the text and have nowhere to report a
/// reason — today that is the dictionary correction watcher.
#[cfg(target_os = "macos")]
pub fn read_focused_text() -> Option<String> {
    probe_focused_text().text
}

/// `AXNumberOfCharacters`, plus the error when it is not available.
///
/// Split out because both the range read and the length-only fallback need it
/// and an AX round-trip against an unresponsive target is not free.
#[cfg(target_os = "macos")]
unsafe fn read_ax_char_count(element: AXUIElementRef) -> (Option<usize>, i32) {
    use core_foundation::number::CFNumber;

    let num_attr = CFString::new("AXNumberOfCharacters");
    let mut num_ref: CFTypeRef = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        num_attr.as_concrete_TypeRef(),
        &mut num_ref,
    );

    if err != AX_ERROR_SUCCESS || num_ref.is_null() {
        return (None, err);
    }

    let cf_type_id = core_foundation::base::CFGetTypeID(num_ref);
    if cf_type_id != CFNumber::type_id() {
        CFRelease(num_ref);
        return (None, err);
    }

    let cf_number = CFNumber::wrap_under_create_rule(num_ref as *const _ as _);
    match cf_number.to_i64() {
        // A zero-length field is a legitimate answer, not a failure: an empty
        // box the user is about to dictate into is the single most common
        // paste target there is, and treating it as unreadable would blind the
        // verifier in exactly that case.
        Some(n) if n >= 0 => (Some(n as usize), AX_ERROR_SUCCESS),
        _ => (None, err),
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

/// Try reading text via `AXStringForRange`, given a length already fetched.
///
/// This works for Chrome contenteditable and other web-based text areas.
#[cfg(target_os = "macos")]
unsafe fn read_ax_string_for_range(element: AXUIElementRef, count: usize) -> Option<String> {
    if count == 0 {
        // Nothing to ask for. The caller still has the count, and an empty
        // range makes some targets return `kAXErrorIllegalArgument`.
        return Some(String::new());
    }

    // Create CFRange(0, count) and wrap as AXValue
    #[repr(C)]
    struct CFRange {
        location: i64,
        length: i64,
    }

    let range = CFRange {
        location: 0,
        length: count as i64,
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

/// No equivalent read exists on Windows, so every paste there is unverified
/// and the trace says so rather than reporting a false negative.
#[cfg(not(target_os = "macos"))]
pub fn probe_focused_text() -> FocusSnapshot {
    FocusSnapshot::blind(FocusSource::Unsupported, 0)
}

#[cfg(not(target_os = "macos"))]
pub fn read_focused_text() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> FocusSnapshot {
        FocusSnapshot {
            text: Some(t.to_string()),
            chars: Some(t.chars().count()),
            source: FocusSource::Value,
            ax_err: 0,
        }
    }

    fn length(n: usize) -> FocusSnapshot {
        FocusSnapshot {
            text: None,
            chars: Some(n),
            source: FocusSource::LengthOnly,
            ax_err: 0,
        }
    }

    /// The regression this whole workstream exists for.
    ///
    /// The old verifier compared `Option<String>` directly, so an unreadable
    /// baseline against a readable read-back produced `changed:true`. That is
    /// a self-report presented as an observation, and one dictation in the
    /// corpus (`before_chars:null`, `ax_readable:true`) has exactly the shape.
    #[test]
    fn no_baseline_is_never_an_observation() {
        let before = FocusSnapshot::blind(FocusSource::FocusError, ax_error_cannot_complete());
        let v = classify(&before, &text("bonjour"));
        assert_eq!(v.verdict, PasteVerdict::Unverified);
        assert_eq!(v.reason, "no_baseline");
        assert_eq!(v.evidence, "none");
    }

    /// `paste-swallowed` must not fire because we stopped being able to look.
    #[test]
    fn losing_the_read_back_is_unverified_not_swallowed() {
        let v = classify(&text("bonjour"), &FocusSnapshot::blind(FocusSource::NoFocus, 0));
        assert_eq!(v.verdict, PasteVerdict::Unverified);
        assert_eq!(v.reason, "read_back_failed");
    }

    #[test]
    fn text_that_changed_is_observed() {
        let v = classify(&text("bon"), &text("bonjour"));
        assert_eq!(v.verdict, PasteVerdict::Observed);
        assert_eq!(v.evidence, "text");
    }

    /// The stuck-Globe shape: readable target, nothing arrived.
    #[test]
    fn readable_and_identical_is_swallowed() {
        let v = classify(&text("bonjour"), &text("bonjour"));
        assert_eq!(v.verdict, PasteVerdict::Swallowed);
        assert_eq!(v.evidence, "text");
    }

    /// Typing over a selection can leave the field *shorter*. `changed`, not
    /// `grew`.
    #[test]
    fn a_shorter_field_still_counts_as_observed() {
        let v = classify(&text("a very long selection"), &text("hi"));
        assert_eq!(v.verdict, PasteVerdict::Observed);
    }

    /// The second means of verification: no readable text on either side, but
    /// the element reports its length and the length moved.
    #[test]
    fn length_only_change_is_observed() {
        let v = classify(&length(12), &length(19));
        assert_eq!(v.verdict, PasteVerdict::Observed);
        assert_eq!(v.evidence, "length");
    }

    /// A count that did not move is consistent with a swallow *and* with a
    /// same-length replacement. Accusing here would put false findings into
    /// the analyser, so it stays unverified.
    #[test]
    fn length_only_unchanged_never_accuses() {
        let v = classify(&length(12), &length(12));
        assert_eq!(v.verdict, PasteVerdict::Unverified);
        assert_eq!(v.reason, "length_unchanged");
    }

    /// Text on one side and a bare length on the other is not a comparison.
    #[test]
    fn mismatched_shapes_are_unverified() {
        let v = classify(&text("bonjour"), &length(14));
        assert_eq!(v.verdict, PasteVerdict::Unverified);
        assert_eq!(v.reason, "shape_changed");
    }

    /// An empty field is a legitimate baseline — it is the commonest paste
    /// target there is — and must not read as "unreadable".
    #[test]
    fn an_empty_field_is_a_usable_baseline() {
        assert!(text("").observable());
        assert!(length(0).observable());
        let v = classify(&text(""), &text("bonjour"));
        assert_eq!(v.verdict, PasteVerdict::Observed);
    }

    #[test]
    fn a_blind_snapshot_is_not_observable() {
        assert!(!FocusSnapshot::blind(FocusSource::Unreadable, -25205).observable());
        assert!(!FocusSnapshot::blind(FocusSource::Unsupported, 0).observable());
    }

    /// The slugs go into the trace and into `docs/tracing.md`. Pin them.
    #[test]
    fn trace_slugs_are_stable() {
        assert_eq!(FocusSource::Value.as_str(), "value");
        assert_eq!(FocusSource::Range.as_str(), "range");
        assert_eq!(FocusSource::LengthOnly.as_str(), "length_only");
        assert_eq!(FocusSource::NoFocus.as_str(), "no_focus");
        assert_eq!(FocusSource::FocusError.as_str(), "focus_error");
        assert_eq!(FocusSource::Unreadable.as_str(), "unreadable");
        assert_eq!(FocusSource::Unsupported.as_str(), "unsupported");
        assert_eq!(PasteVerdict::Observed.as_str(), "observed");
        assert_eq!(PasteVerdict::Swallowed.as_str(), "swallowed");
        assert_eq!(PasteVerdict::Unverified.as_str(), "unverified");
    }

    #[cfg(target_os = "macos")]
    fn ax_error_cannot_complete() -> i32 {
        ax_error::CANNOT_COMPLETE
    }
    #[cfg(not(target_os = "macos"))]
    fn ax_error_cannot_complete() -> i32 {
        -25204
    }
}
