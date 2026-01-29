// TTP - Talk To Paste
// Transcription module - OpenAI Whisper and GPT-4o-mini polish

pub mod pipeline;
pub mod polish;
pub mod whisper;

pub use pipeline::{process_audio, process_recording};
pub use polish::polish_text;
pub use whisper::transcribe_audio;
