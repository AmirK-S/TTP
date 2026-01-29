// TTP - Talk To Paste
// Settings module - manages application settings and persistence

pub mod store;

pub use store::{get_settings, reset_settings, set_settings, Settings};
