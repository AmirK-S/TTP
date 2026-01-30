// TTP - Talk To Paste
// Ensemble transcription - parallel execution across multiple providers

use std::time::Instant;
use tokio::time::{timeout, Duration};

use super::gladia::transcribe_audio_gladia;
use super::whisper::{transcribe_audio_openai, transcribe_audio_groq};

/// Provider timeout (30 seconds - Gladia can be slow with polling)
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(30);

/// Result from a single transcription provider
#[derive(Debug, Clone)]
pub struct ProviderResult {
    /// Provider name (e.g., "OpenAI", "Groq", "Gladia")
    pub provider: String,
    /// Transcribed text
    pub text: String,
    /// Latency in milliseconds
    pub latency_ms: u64,
}

/// Transcribe audio with all available providers in parallel
///
/// Executes transcription with all providers that have API keys configured.
/// Returns results from all providers that succeeded (minimum 1 required for success).
/// Individual provider failures are logged but don't fail the overall operation.
///
/// # Arguments
/// * `audio_path` - Path to the audio file (WAV format)
/// * `openai_key` - Optional OpenAI API key
/// * `groq_key` - Optional Groq API key
/// * `gladia_key` - Optional Gladia API key
///
/// # Returns
/// * `Ok(Vec<ProviderResult>)` - Results from all successful providers
/// * `Err(String)` - Error if no providers succeed or no keys configured
pub async fn transcribe_ensemble(
    audio_path: &str,
    openai_key: Option<&str>,
    groq_key: Option<&str>,
    gladia_key: Option<&str>,
) -> Result<Vec<ProviderResult>, String> {
    // Count available providers
    let provider_count = [openai_key.is_some(), groq_key.is_some(), gladia_key.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();

    if provider_count == 0 {
        return Err("No API keys configured for ensemble mode".to_string());
    }

    println!("[Ensemble] Starting parallel transcription with {} providers", provider_count);

    // Build futures for available providers
    let openai_future = async {
        if let Some(key) = openai_key {
            let start = Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio_openai(key, audio_path)).await;
            Some(("OpenAI", result, start.elapsed().as_millis() as u64))
        } else {
            None
        }
    };

    let groq_future = async {
        if let Some(key) = groq_key {
            let start = Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio_groq(key, audio_path)).await;
            Some(("Groq", result, start.elapsed().as_millis() as u64))
        } else {
            None
        }
    };

    let gladia_future = async {
        if let Some(key) = gladia_key {
            let start = Instant::now();
            let result = timeout(PROVIDER_TIMEOUT, transcribe_audio_gladia(key, audio_path)).await;
            Some(("Gladia", result, start.elapsed().as_millis() as u64))
        } else {
            None
        }
    };

    // Execute all in parallel using tokio::join!
    let (openai_result, groq_result, gladia_result) = tokio::join!(
        openai_future,
        groq_future,
        gladia_future
    );

    // Collect successful results
    let mut successful: Vec<ProviderResult> = Vec::new();

    // Process OpenAI result
    if let Some((provider, result, latency)) = openai_result {
        match result {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                println!("[Ensemble] {} succeeded in {}ms", provider, latency);
                successful.push(ProviderResult {
                    provider: provider.to_string(),
                    text: text.trim().to_string(),
                    latency_ms: latency,
                });
            }
            Ok(Ok(_)) => {
                println!("[Ensemble] {} returned empty result", provider);
            }
            Ok(Err(e)) => {
                println!("[Ensemble] {} failed: {}", provider, e);
            }
            Err(_) => {
                println!("[Ensemble] {} timed out after {:?}", provider, PROVIDER_TIMEOUT);
            }
        }
    }

    // Process Groq result
    if let Some((provider, result, latency)) = groq_result {
        match result {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                println!("[Ensemble] {} succeeded in {}ms", provider, latency);
                successful.push(ProviderResult {
                    provider: provider.to_string(),
                    text: text.trim().to_string(),
                    latency_ms: latency,
                });
            }
            Ok(Ok(_)) => {
                println!("[Ensemble] {} returned empty result", provider);
            }
            Ok(Err(e)) => {
                println!("[Ensemble] {} failed: {}", provider, e);
            }
            Err(_) => {
                println!("[Ensemble] {} timed out after {:?}", provider, PROVIDER_TIMEOUT);
            }
        }
    }

    // Process Gladia result
    if let Some((provider, result, latency)) = gladia_result {
        match result {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                println!("[Ensemble] {} succeeded in {}ms", provider, latency);
                successful.push(ProviderResult {
                    provider: provider.to_string(),
                    text: text.trim().to_string(),
                    latency_ms: latency,
                });
            }
            Ok(Ok(_)) => {
                println!("[Ensemble] {} returned empty result", provider);
            }
            Ok(Err(e)) => {
                println!("[Ensemble] {} failed: {}", provider, e);
            }
            Err(_) => {
                println!("[Ensemble] {} timed out after {:?}", provider, PROVIDER_TIMEOUT);
            }
        }
    }

    println!("[Ensemble] Completed with {}/{} providers successful", successful.len(), provider_count);

    if successful.is_empty() {
        return Err("All transcription providers failed".to_string());
    }

    Ok(successful)
}
