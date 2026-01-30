// TTP - Talk To Paste
// Transcription module - supports Gladia, Groq, and OpenAI

pub mod ensemble;
pub mod fusion;
pub mod gladia;
pub mod pipeline;
pub mod polish;
pub mod whisper;

pub use ensemble::{transcribe_ensemble, ProviderResult};
pub use fusion::{fuse_and_polish, FUSION_SYSTEM_PROMPT};
pub use gladia::transcribe_audio_gladia;
pub use pipeline::{process_audio, process_recording};
pub use polish::polish_text;
pub use whisper::{transcribe_audio, transcribe_audio_groq, transcribe_audio_openai};
