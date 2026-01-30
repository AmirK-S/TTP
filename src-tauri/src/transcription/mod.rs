// TTP - Talk To Paste
// Transcription module - supports Gladia, Groq, and OpenAI

pub mod gladia;
pub mod pipeline;
pub mod polish;
pub mod whisper;

pub use gladia::transcribe_audio_gladia;
pub use pipeline::{process_audio, process_recording};
pub use polish::polish_text;
pub use whisper::transcribe_audio;
