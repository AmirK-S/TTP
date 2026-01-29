// TTP - Paste module
// Handles clipboard operations and keyboard simulation for auto-paste

pub mod clipboard;
pub mod permissions;
pub mod simulate;

pub use clipboard::ClipboardGuard;
pub use permissions::check_accessibility;
pub use simulate::simulate_paste;
