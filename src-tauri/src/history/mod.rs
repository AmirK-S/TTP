// TTP - Talk To Paste
// History module - manages transcription history and persistence

pub mod store;

pub use store::{add_history_entry, clear_history, get_history, HistoryEntry};
