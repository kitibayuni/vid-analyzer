use std::env;
use claxon::FlacReader;
use rubato::{FftFixedIn, Resampler};
use ndarray::Array1;
use ndarray_npy::write_npy;

/// Preprocess FLAC audio for HuBERT:
/// - Converts to mono
/// - Resamples to target_sr
/// - Normalizes amplitude
/// - Saves as 1D .npy
fn preprocess_flac(input_path: &str, output_path: &str, target_sr: u32) -> Result<(), Box<dyn std::error::Error>> {
    // --- Open FLAC ---
    let mut reader = FlacReader::open(input_path)?;
    let streaminfo = reader.streaminfo();
    let sample_rate = streaminfo.sample_rate;
    let channels = streaminfo.channels as usize;

    // --- Read samples ---
    let mut samples: Vec<f32> = Vec::new();
    let bits_per_sample = streaminfo.bits_per_sample;
    let max_value = (1i32 << (bits_per_sample - 1)) as f32;
    if channels == 0 || sample_rate == 0 {
        return Err("Invalid audio file: zero channels or sample rate".into());
    }
    for sample in reader.samples() {
        let s = sample? as f32 / max_value;
        samples.push(s);
    }

    // --- Split into channels ---
    let mut channel_buffers: Vec<Vec<f32>> = vec![Vec::new(); channels];
    for (i, s) in samples.iter().enumerate() {
        channel_buffers[i % channels].push(*s);
    }

    // --- Resample if needed ---
    let mono_samples: Vec<f32> = if sample_rate != target_sr {
        let chunk_size = 1024;
        let chunk_size = (sample_rate as usize).min(4096); // Adaptive chunk size
        let mut resampler = FftFixedIn::<f32>::new(
            sample_rate as usize,
            target_sr as usize,
            chunk_size,
            1, // Single thread to avoid potential issues
            channels,
        )?;

        

        let resampled_channels: Vec<Vec<f32>> = resampler.process(&channel_buffers, None)?;
        let n_samples = resampled_channels[0].len();
        // Convert to mono
        (0..n_samples)
            .map(|i| resampled_channels.iter().map(|c| c[i]).sum::<f32>() / channels as f32)
            .collect()
    } else {
        // No resampling, just convert to mono
        let n_samples = channel_buffers[0].len();
        (0..n_samples)
            .map(|i| channel_buffers.iter().map(|c| c[i]).sum::<f32>() / channels as f32)
            .collect()
    };

    if mono_samples.is_empty() {
        return Err("No audio data found".into());
    }

    // --- Normalize ---
    let rms = (mono_samples.iter().map(|&s| s * s).sum::<f32>() / mono_samples.len() as f32).sqrt();
    let target_rms = 0.1; // Conservative target RMS level
    let gain = if rms > 1e-8 { target_rms / rms } else { 1.0 };
    let normalized: Vec<f32> = mono_samples.iter().map(|&s| (s * gain).clamp(-1.0, 1.0)).collect();

    // --- Check format ---
    println!("Audio stats: length={}, min={:.6}, max={:.6}, mean={:.6}", 
         normalized.len(),
         normalized.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
         normalized.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)),
         normalized.iter().sum::<f32>() / normalized.len() as f32);

    // --- Save as .npy ---
    let array = Array1::from(normalized);
    write_npy(output_path, &array)?;

    println!("Saved preprocessed audio to {}", output_path);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <input_audio.flac> <output.npy> <target_sample_rate>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];
    let target_sr: u32 = args[3].parse()?;

    preprocess_flac(input_path, output_path, target_sr)?;

    Ok(())
}
