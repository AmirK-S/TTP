// TTP - Paste module
// Handles clipboard operations and keyboard simulation for auto-paste

pub mod accessibility;
pub mod clipboard;
pub mod frontmost;
pub mod outcome;
pub mod permissions;
pub mod simulate;

pub use accessibility::{
    classify, probe_focused_text, read_focused_text, FocusSnapshot, FocusSource, PasteVerdict,
    Verification,
};
pub use frontmost::frontmost_bundle_id;
pub use outcome::{
    describe_verification, finish_outcome, read_verdict, record_verdict, PasteVerdictSlot,
};
pub use clipboard::ClipboardGuard;
pub use permissions::check_accessibility;
#[cfg(target_os = "macos")]
pub use permissions::{check_accessibility_with_prompt, probe_accessibility, reset_accessibility_tcc};
pub use simulate::{describe_held_modifiers, last_injection_modifiers, simulate_paste, simulate_typing};
