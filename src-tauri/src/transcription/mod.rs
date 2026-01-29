// TTP - Talk To Paste
// Transcription module - OpenAI Whisper and GPT-4o-mini polish

pub mod whisper;
pub mod polish;

pub use whisper::transcribe_audio;
pub use polish::polish_text;
