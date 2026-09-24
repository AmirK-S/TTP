// TTP - Talk To Paste
// Dictionary module for learning proper noun corrections
//
// Enables TTP to learn from user corrections of proper nouns (names, places,
// specialized terms) and improve future transcription accuracy.

pub(crate) mod classify;
pub(crate) mod detection;
pub(crate) mod store;

pub(crate) use store::{add_dictionary_entry, add_entry, apply_dictionary, clear_dictionary, delete_dictionary_entry, get_dictionary, DictionaryEntry};
