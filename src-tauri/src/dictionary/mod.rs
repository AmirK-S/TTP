// TTP - Talk To Paste
// Dictionary module for learning proper noun corrections
//
// Enables TTP to learn from user corrections of proper nouns (names, places,
// specialized terms) and improve future transcription accuracy.

pub mod detection;
pub mod store;

pub use store::{add_entry, clear_dictionary, delete_dictionary_entry, get_dictionary, DictionaryEntry};
