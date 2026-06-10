// TTP - Talk To Paste
// Transcription module - Groq Whisper + AI polish

pub(crate) mod cleanup;
pub(crate) mod convert;
pub(crate) mod pipeline;
pub(crate) mod polish;
pub(crate) mod whisper;
pub(crate) mod backup;

pub(crate) use pipeline::{process_audio, process_recording};
pub(crate) use polish::polish_text;
pub(crate) use whisper::transcribe_audio;
