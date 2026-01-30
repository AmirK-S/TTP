// TTP - Talk To Paste
// Gladia transcription API client - best for multilingual code-switching

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

/// Gladia upload endpoint
const GLADIA_UPLOAD_URL: &str = "https://api.gladia.io/v2/upload";

/// Gladia pre-recorded transcription endpoint
const GLADIA_TRANSCRIBE_URL: &str = "https://api.gladia.io/v2/pre-recorded";

/// Maximum polling attempts (30 seconds total with 1s intervals)
const MAX_POLL_ATTEMPTS: u32 = 30;

/// Request timeout in seconds
const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Upload response from Gladia
#[derive(Debug, Deserialize)]
struct UploadResponse {
    audio_url: String,
}

/// Transcription initiation response
#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    id: String,
    result_url: String,
}

/// Transcription result response
#[derive(Debug, Deserialize)]
struct ResultResponse {
    status: String,
    result: Option<TranscriptionResult>,
}

/// Transcription result with utterances
#[derive(Debug, Deserialize)]
struct TranscriptionResult {
    transcription: TranscriptionData,
}

#[derive(Debug, Deserialize)]
struct TranscriptionData {
    full_transcript: String,
}

/// Transcription request body
#[derive(Debug, Serialize)]
struct TranscribeRequest {
    audio_url: String,
    detect_language: bool,
    enable_code_switching: bool,
}

/// Transcribe audio file using Gladia API
///
/// Gladia excels at multilingual code-switching (e.g., French/English mixed).
/// Uses a 3-step process: upload -> transcribe -> poll for results.
///
/// # Arguments
/// * `api_key` - Gladia API key
/// * `audio_path` - Path to the audio file (WAV format)
///
/// # Returns
/// * `Ok(String)` - Transcription text on success
/// * `Err(String)` - Error message on failure
pub async fn transcribe_audio_gladia(api_key: &str, audio_path: &str) -> Result<String, String> {
    println!("[Gladia] Starting transcription for: {}", audio_path);

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Step 1: Upload audio file
    println!("[Gladia] Step 1: Uploading audio file...");
    let audio_url = upload_audio(&client, api_key, audio_path).await?;
    println!("[Gladia] Upload complete, got URL: {}", audio_url);

    // Step 2: Initiate transcription
    println!("[Gladia] Step 2: Initiating transcription...");
    let result_url = initiate_transcription(&client, api_key, &audio_url).await?;
    println!("[Gladia] Transcription initiated, polling: {}", result_url);

    // Step 3: Poll for results
    println!("[Gladia] Step 3: Polling for results...");
    let transcript = poll_for_results(&client, api_key, &result_url).await?;
    println!("[Gladia] Transcription complete: {}", transcript);

    Ok(transcript)
}

/// Upload audio file to Gladia
async fn upload_audio(client: &reqwest::Client, api_key: &str, audio_path: &str) -> Result<String, String> {
    // Read audio file bytes
    let audio_bytes = fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Get filename from path
    let filename = Path::new(audio_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav")
        .to_string();

    // Build multipart form
    let file_part = Part::bytes(audio_bytes)
        .file_name(filename)
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to set MIME type: {}", e))?;

    let form = Form::new().part("audio", file_part);

    // Upload
    let response = client
        .post(GLADIA_UPLOAD_URL)
        .header("x-gladia-key", api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Upload request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("Upload failed: {} - {}", status, error_body));
    }

    let upload_response: UploadResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse upload response: {}", e))?;

    Ok(upload_response.audio_url)
}

/// Initiate transcription with Gladia
async fn initiate_transcription(client: &reqwest::Client, api_key: &str, audio_url: &str) -> Result<String, String> {
    let request_body = TranscribeRequest {
        audio_url: audio_url.to_string(),
        detect_language: true,
        enable_code_switching: true,
    };

    let response = client
        .post(GLADIA_TRANSCRIBE_URL)
        .header("x-gladia-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Transcription request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("Transcription initiation failed: {} - {}", status, error_body));
    }

    let transcribe_response: TranscribeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse transcription response: {}", e))?;

    Ok(transcribe_response.result_url)
}

/// Poll for transcription results
async fn poll_for_results(client: &reqwest::Client, api_key: &str, result_url: &str) -> Result<String, String> {
    for attempt in 0..MAX_POLL_ATTEMPTS {
        // Wait between polls (except first attempt)
        if attempt > 0 {
            sleep(Duration::from_secs(1)).await;
        }

        let response = client
            .get(result_url)
            .header("x-gladia-key", api_key)
            .send()
            .await
            .map_err(|e| format!("Poll request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(format!("Poll failed: {} - {}", status, error_body));
        }

        let result_response: ResultResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse poll response: {}", e))?;

        match result_response.status.as_str() {
            "done" => {
                if let Some(result) = result_response.result {
                    let transcript = result.transcription.full_transcript.trim().to_string();
                    return Ok(transcript);
                } else {
                    return Err("Transcription done but no result".to_string());
                }
            }
            "error" => {
                return Err("Transcription failed on Gladia side".to_string());
            }
            "queued" | "processing" => {
                println!("[Gladia] Status: {}, attempt {}/{}", result_response.status, attempt + 1, MAX_POLL_ATTEMPTS);
                continue;
            }
            other => {
                println!("[Gladia] Unknown status: {}", other);
                continue;
            }
        }
    }

    Err("Transcription timed out after 30 seconds".to_string())
}
