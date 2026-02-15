// TTP - Talk To Paste
// Transcription module - Groq Whisper + AI polish

pub mod convert;
pub mod pipeline;
pub mod polish;
pub mod whisper;
pub mod backup;

pub use pipeline::{process_audio, process_recording};
pub use polish::polish_text;
pub use whisper::transcribe_audio;
