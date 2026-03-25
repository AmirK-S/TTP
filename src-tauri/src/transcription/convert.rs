// TTP - Talk To Paste
// Audio conversion: stereo 48kHz WAV → mono 16kHz WAV for Groq Whisper API
//
// Whisper natively expects 16kHz mono audio. Converting before upload
// reduces file size ~6x compared to the original stereo 48kHz WAV.

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;
use std::io::Write;

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

/// Convert a mono 16kHz 16-bit WAV to OGG Opus for smaller upload size.
///
/// Returns the path to the `.ogg` file on success.
/// The input WAV is NOT deleted — caller handles cleanup.
pub fn convert_to_ogg_opus(wav_path: &str) -> Result<String, String> {
    use audiopus::{coder::Encoder, Application, Channels, SampleRate};
    use ogg::writing::PacketWriter;

    const SAMPLE_RATE: u32 = 16_000;
    const FRAME_MS: usize = 20;
    const FRAME_SIZE: usize = (SAMPLE_RATE as usize * FRAME_MS) / 1000; // 320 samples
    const BITRATE: i32 = 64_000;

    // Read WAV samples
    let reader = WavReader::open(wav_path)
        .map_err(|e| format!("Failed to read WAV for Opus encoding: {}", e))?;
    let spec = reader.spec();

    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "Expected mono 16kHz WAV, got {} ch {}Hz",
            spec.channels, spec.sample_rate
        ));
    }

    let samples: Vec<i16> = match spec.sample_format {
        SampleFormat::Int => reader.into_samples::<i16>().map(|s| s.unwrap_or(0)).collect(),
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .map(|s| {
                let v = s.unwrap_or(0.0);
                (v * 32767.0).clamp(-32768.0, 32767.0) as i16
            })
            .collect(),
    };

    // Create Opus encoder
    let mut encoder = Encoder::new(SampleRate::Hz16000, Channels::Mono, Application::Voip)
        .map_err(|e| format!("Failed to create Opus encoder: {}", e))?;
    encoder
        .set_bitrate(audiopus::Bitrate::BitsPerSecond(BITRATE))
        .map_err(|e| format!("Failed to set Opus bitrate: {}", e))?;

    // Output file
    let output_path = Path::new(wav_path)
        .with_extension("ogg")
        .to_string_lossy()
        .to_string();

    let out_file = std::fs::File::create(&output_path)
        .map_err(|e| format!("Failed to create OGG file: {}", e))?;
    let mut packet_writer = PacketWriter::new(out_file);

    let serial = 1u32;

    // Write OpusHead header (RFC 7845 Section 5.1)
    let mut opus_head: Vec<u8> = Vec::new();
    opus_head.extend_from_slice(b"OpusHead");  // Magic signature
    opus_head.push(1);                          // Version
    opus_head.push(1);                          // Channel count (mono)
    opus_head.extend_from_slice(&(0u16).to_le_bytes()); // Pre-skip (0 for simplicity)
    opus_head.extend_from_slice(&SAMPLE_RATE.to_le_bytes()); // Input sample rate
    opus_head.extend_from_slice(&(0i16).to_le_bytes());  // Output gain
    opus_head.push(0);                          // Channel mapping family

    let head_packet = ogg::writing::PacketWriteEndInfo::EndPage;
    packet_writer
        .write_packet(std::borrow::Cow::Owned(opus_head), serial, head_packet, 0)
        .map_err(|e| format!("Failed to write OpusHead: {}", e))?;

    // Write OpusTags header (RFC 7845 Section 5.2)
    let mut opus_tags: Vec<u8> = Vec::new();
    opus_tags.extend_from_slice(b"OpusTags");  // Magic signature
    let vendor = b"TTP";
    opus_tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    opus_tags.extend_from_slice(vendor);
    opus_tags.extend_from_slice(&(0u32).to_le_bytes()); // No user comments

    let tags_packet = ogg::writing::PacketWriteEndInfo::EndPage;
    packet_writer
        .write_packet(std::borrow::Cow::Owned(opus_tags), serial, tags_packet, 0)
        .map_err(|e| format!("Failed to write OpusTags: {}", e))?;

    // Encode audio in 20ms frames
    let mut encode_buf = vec![0u8; 4000]; // Max Opus packet size
    let mut granule_pos: u64 = 0;
    let total_frames = (samples.len() + FRAME_SIZE - 1) / FRAME_SIZE;

    for (i, chunk) in samples.chunks(FRAME_SIZE).enumerate() {
        // Pad last frame with silence if needed
        let frame: Vec<i16> = if chunk.len() < FRAME_SIZE {
            let mut padded = chunk.to_vec();
            padded.resize(FRAME_SIZE, 0);
            padded
        } else {
            chunk.to_vec()
        };

        let encoded_len = encoder
            .encode(&frame, &mut encode_buf)
            .map_err(|e| format!("Opus encode error: {}", e))?;

        granule_pos += FRAME_SIZE as u64;

        let is_last = i == total_frames - 1;
        let end_info = if is_last {
            ogg::writing::PacketWriteEndInfo::EndStream
        } else {
            ogg::writing::PacketWriteEndInfo::NormalPacket
        };

        packet_writer
            .write_packet(
                std::borrow::Cow::Owned(encode_buf[..encoded_len].to_vec()),
                serial,
                end_info,
                granule_pos,
            )
            .map_err(|e| format!("Failed to write Opus packet: {}", e))?;
    }

    // Flush remaining data
    let mut file = packet_writer.into_inner();
    file.flush().map_err(|e| format!("Failed to flush OGG file: {}", e))?;

    Ok(output_path)
}
