// TTP - Talk To Paste
// Audio conversion: stereo 48kHz WAV → mono 16kHz WAV for Groq Whisper API
//
// Whisper natively expects 16kHz mono audio. Converting before upload
// reduces file size ~6x, allowing recordings up to ~14 minutes.

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

/// Target sample rate for Whisper (16kHz)
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Convert a WAV file to mono 16kHz for optimal Whisper API upload.
///
/// Returns the path to the converted file (same directory, `_16k.wav` suffix).
/// The original file is NOT deleted — caller handles cleanup.
pub fn convert_to_mono_16khz(input_path: &str) -> Result<String, String> {
    let reader = WavReader::open(input_path)
        .map_err(|e| format!("Failed to read WAV: {}", e))?;
    let spec = reader.spec();

    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    // Already mono 16kHz — no conversion needed
    if channels == 1 && sample_rate == TARGET_SAMPLE_RATE {
        return Ok(input_path.to_string());
    }

    // Read all samples as i16
    let samples: Vec<i16> = match spec.sample_format {
        SampleFormat::Int => {
            reader.into_samples::<i16>()
                .map(|s| s.unwrap_or(0))
                .collect()
        }
        SampleFormat::Float => {
            reader.into_samples::<f32>()
                .map(|s| {
                    let v = s.unwrap_or(0.0);
                    (v * 32767.0).clamp(-32768.0, 32767.0) as i16
                })
                .collect()
        }
    };

    // Stereo → mono: average channels
    let mono_samples: Vec<i16> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| {
                let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
                (sum / channels as i32) as i16
            })
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz using linear interpolation
    let ratio = TARGET_SAMPLE_RATE as f64 / sample_rate as f64;
    let output_len = (mono_samples.len() as f64 * ratio) as usize;
    let resampled: Vec<i16> = (0..output_len)
        .map(|i| {
            let src_pos = i as f64 / ratio;
            let idx = src_pos as usize;
            let frac = src_pos - idx as f64;

            if idx + 1 < mono_samples.len() {
                let a = mono_samples[idx] as f64;
                let b = mono_samples[idx + 1] as f64;
                (a + (b - a) * frac) as i16
            } else if idx < mono_samples.len() {
                mono_samples[idx]
            } else {
                0
            }
        })
        .collect();

    // Write converted WAV
    let output_path = Path::new(input_path)
        .with_extension("16k.wav")
        .to_string_lossy()
        .to_string();

    let out_spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(&output_path, out_spec)
        .map_err(|e| format!("Failed to create converted WAV: {}", e))?;

    for sample in &resampled {
        writer
            .write_sample(*sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(output_path)
}
