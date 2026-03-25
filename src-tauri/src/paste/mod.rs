// TTP - Paste module
// Handles clipboard operations and keyboard simulation for auto-paste

pub mod accessibility;
pub mod clipboard;
pub mod permissions;
pub mod simulate;

pub use accessibility::read_focused_text;
pub use clipboard::ClipboardGuard;
pub use permissions::check_accessibility;
#[cfg(target_os = "macos")]
pub use permissions::{check_accessibility_with_prompt, probe_accessibility, reset_accessibility_tcc};
pub use simulate::simulate_paste;
