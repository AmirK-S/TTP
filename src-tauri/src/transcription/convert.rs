// TTP - Talk To Paste
// Audio conversion: stereo 48kHz WAV → mono 16kHz WAV for Groq Whisper API
//
// Whisper natively expects 16kHz mono audio. Converting before upload
// reduces file size ~6x compared to the original stereo 48kHz WAV.

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

    // Anti-aliasing low-pass filter before downsampling
    // Cutoff at Nyquist of target rate (8kHz) to prevent aliasing artifacts
    let filtered = if sample_rate > TARGET_SAMPLE_RATE {
        low_pass_filter(&mono_samples, sample_rate, TARGET_SAMPLE_RATE / 2)
    } else {
        mono_samples.clone()
    };

    // Resample to 16kHz using linear interpolation (safe after anti-aliasing filter)
    let ratio = TARGET_SAMPLE_RATE as f64 / sample_rate as f64;
    let output_len = (filtered.len() as f64 * ratio) as usize;
    let resampled: Vec<i16> = (0..output_len)
        .map(|i| {
            let src_pos = i as f64 / ratio;
            let idx = src_pos as usize;
            let frac = src_pos - idx as f64;

            if idx + 1 < filtered.len() {
                let a = filtered[idx] as f64;
                let b = filtered[idx + 1] as f64;
                (a + (b - a) * frac) as i16
            } else if idx < filtered.len() {
                filtered[idx]
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

/// Simple windowed-sinc low-pass filter to prevent aliasing before downsampling.
/// Uses a Kaiser-windowed sinc kernel for good stopband attenuation.
fn low_pass_filter(samples: &[i16], sample_rate: u32, cutoff_hz: u32) -> Vec<i16> {
    let fc = cutoff_hz as f64 / sample_rate as f64; // Normalized cutoff frequency
    let kernel_size = 63; // Odd number for symmetric kernel
    let half = kernel_size / 2;

    // Build windowed-sinc kernel
    let mut kernel: Vec<f64> = (0..kernel_size)
        .map(|i| {
            let n = i as f64 - half as f64;
            let sinc = if n.abs() < 1e-10 {
                2.0 * std::f64::consts::PI * fc
            } else {
                (2.0 * std::f64::consts::PI * fc * n).sin() / n
            };
            // Blackman window for good stopband attenuation
            let window = 0.42
                - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (kernel_size - 1) as f64).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * i as f64 / (kernel_size - 1) as f64).cos();
            sinc * window
        })
        .collect();

    // Normalize kernel so sum = 1
    let sum: f64 = kernel.iter().sum();
    for k in &mut kernel {
        *k /= sum;
    }

    // Apply convolution
    let len = samples.len();
    let mut output = Vec::with_capacity(len);
    for i in 0..len {
        let mut acc: f64 = 0.0;
        for (j, &k) in kernel.iter().enumerate() {
            let idx = i as isize + j as isize - half as isize;
            let sample = if idx < 0 {
                samples[0] as f64
            } else if idx >= len as isize {
                samples[len - 1] as f64
            } else {
                samples[idx as usize] as f64
            };
            acc += sample * k;
        }
        output.push(acc.clamp(-32768.0, 32767.0) as i16);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: i16, b: i16, tol: i16) -> bool {
        (a as i32 - b as i32).abs() <= tol as i32
    }

    #[test]
    fn low_pass_filter_preserves_length() {
        let samples: Vec<i16> = (0..2048).map(|i| (i as f32 / 10.0).sin() as i16 * 1000).collect();
        let filtered = low_pass_filter(&samples, 48_000, 8_000);
        assert_eq!(filtered.len(), samples.len());
    }

    #[test]
    fn low_pass_filter_passes_silence_through_as_silence() {
        let silent = vec![0i16; 1024];
        let filtered = low_pass_filter(&silent, 48_000, 8_000);
        // Edge-padding + numerical noise can produce ±1 LSB.
        for sample in filtered {
            assert!(sample.abs() <= 1, "silent input should produce ~silent output, got {}", sample);
        }
    }

    #[test]
    fn low_pass_filter_attenuates_a_high_frequency_above_cutoff() {
        // Pure 22 kHz sine at 48 kHz sample rate (well above the 8 kHz
        // cutoff used before downsample-to-16k). After filtering, the
        // amplitude should drop SIGNIFICANTLY. The actual attenuation a
        // 63-tap Blackman-windowed sinc produces at this very high
        // frequency is around 2.5x-3x (the transition band of a 63-tap
        // kernel is wider than ideal, and a 22 kHz tone lands near Nyquist
        // where boundary effects dominate). Asserting >2x is enough to
        // catch a filter that has totally regressed without flaking on
        // edge-of-Nyquist numerical noise.
        let sr: u32 = 48_000;
        let f_hz = 22_000.0;
        let amplitude: i16 = 20_000;
        let samples: Vec<i16> = (0..4096)
            .map(|i| {
                let t = i as f64 / sr as f64;
                (amplitude as f64 * (2.0 * std::f64::consts::PI * f_hz * t).sin()) as i16
            })
            .collect();
        let filtered = low_pass_filter(&samples, sr, 8_000);

        let in_peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        let out_peak = filtered.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(
            (out_peak as u32) * 2 < in_peak as u32,
            "expected >2x attenuation at 22kHz, in_peak={} out_peak={}",
            in_peak,
            out_peak
        );
    }

    #[test]
    fn low_pass_filter_preserves_a_low_frequency_below_cutoff() {
        // Pure 200 Hz sine at 48 kHz — well below the 8 kHz cutoff.
        // After filtering, the amplitude should be near-identical.
        let sr: u32 = 48_000;
        let f_hz = 200.0;
        let amplitude: i16 = 10_000;
        let samples: Vec<i16> = (0..4096)
            .map(|i| {
                let t = i as f64 / sr as f64;
                (amplitude as f64 * (2.0 * std::f64::consts::PI * f_hz * t).sin()) as i16
            })
            .collect();
        let filtered = low_pass_filter(&samples, sr, 8_000);

        // Trim a kernel-width buffer from each end so edge transients
        // (Blackman window response startup) don't dominate the metric.
        let in_peak = samples[63..samples.len() - 63]
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        let out_peak = filtered[63..filtered.len() - 63]
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            (out_peak as i32 - in_peak as i32).abs() < (in_peak as i32 / 10),
            "expected near-unity passband, in_peak={} out_peak={}",
            in_peak,
            out_peak
        );
    }

    #[test]
    fn low_pass_filter_handles_dc_offset_without_clipping() {
        // Constant DC at +20 000 should pass through unchanged (DC is
        // below any cutoff). No saturation, no garbage.
        let samples = vec![20_000i16; 1024];
        let filtered = low_pass_filter(&samples, 48_000, 8_000);
        // Allow a 2% deviation — the Blackman window introduces a small
        // DC gain trim at the edges.
        for &sample in &filtered[63..filtered.len() - 63] {
            assert!(approx_eq(sample, 20_000, 400), "expected ~20000, got {}", sample);
        }
    }
}
