use std::env;

use claxon::FlacReader;
use rubato::{FftFixedIn, Resampler};
use ndarray::Array1;
use ndarray_npy::write_npy;

/// Preprocess FLAC audio for HuBERT:
/// - Converts stereo -> mono
/// - Resamples to target_sr
/// - Normalizes amplitude
/// - Saves as .npy
fn preprocess_flac(input_path: &str, output_path: &str, target_sr: u32) -> Result<(), Box<dyn std::error::Error>> {
    // --- Open FLAC file ---
    let mut reader = FlacReader::open(input_path)?;
    let streaminfo = reader.streaminfo();
    let sample_rate = streaminfo.sample_rate;
    let channels = streaminfo.channels as usize;

    // --- Load samples ---
    let mut samples: Vec<f32> = Vec::with_capacity(streaminfo.samples.unwrap_or(0) as usize);
    for sample in reader.samples() {
        let s = sample? as f32 / i32::MAX as f32;
        samples.push(s);
    }

    // --- Convert stereo -> mono ---
    let mono: Vec<f32> = if channels > 1 {
        samples.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect()
    } else {
        samples
    };

    // --- Resample if needed ---
    let processed: Vec<f32> = if sample_rate != target_sr {
        let chunk_size = 1024;
        let mut resampler = FftFixedIn::<f32>::new(
            sample_rate as usize,
            target_sr as usize,
            chunk_size,
            2,
            1,
        )?;
        let chunks: Vec<Vec<f32>> = mono.chunks(chunk_size).map(|c| c.to_vec()).collect();
        let resampled_chunks: Vec<Vec<f32>> = resampler.process(&chunks, None)?;
        resampled_chunks.into_iter().flatten().collect()
    } else {
        mono
    };

    // --- Normalize ---
    let max_amp = processed.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    let normalized: Vec<f32> = processed.iter().map(|&s| s / max_amp.max(1e-8)).collect();

    // --- Save as .npy using ndarray-npy ---
    let array = Array1::from(normalized);
    write_npy(output_path, &array)?;

    println!("Saved preprocessed FLAC audio to {}", output_path);
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
