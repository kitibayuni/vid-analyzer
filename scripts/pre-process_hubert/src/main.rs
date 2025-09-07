use std::env;
use hound::{WavReader, SampleFormat};
use ndarray::Array1;
use ndarray_npy::write_npy;
use rubato::{FftFixedIn, Resampler};

/// Convert WAV to HuBERT-ready npy:
/// - Accepts PCM16/PCM32/Float32
/// - Converts to mono float32
/// - Resamples to target_sr
/// - Normalizes amplitude
fn preprocess_wav(input_path: &str, output_path: &str, target_sr: u32) -> Result<(), Box<dyn std::error::Error>> {
    // --- Open WAV ---
    let mut reader = WavReader::open(input_path)?;
    let spec = reader.spec();
    println!("WAV Spec: {:?}", spec);
    
    // --- Read samples as f32 ---
    let mut samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        (SampleFormat::Int, 16) => reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect(),
        (SampleFormat::Int, 32) => reader.samples::<i32>().map(|s| s.unwrap() as f32 / 2147483648.0).collect(),
        _ => return Err("Unsupported WAV format".into()),
    };
    
    if samples.is_empty() {
        return Err("No audio data found".into());
    }
    println!("Read {} raw samples from WAV", samples.len());
    
    // --- Downmix to mono if needed ---
    if spec.channels > 1 {
        samples = samples.chunks_exact(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / spec.channels as f32)
            .collect();
        println!("Downmixed to mono, {} samples", samples.len());
    }
    
    // --- Resample if needed ---
    let mut processed = samples.clone();
    if spec.sample_rate != target_sr {
        println!("Resampling from {} Hz to {} Hz", spec.sample_rate, target_sr);
        
        // Create resampler with proper chunk size
        let chunk_size = 1024;
        let mut resampler = FftFixedIn::<f32>::new(
            spec.sample_rate as usize,
            target_sr as usize,
            chunk_size,
            1, // mono channel
            1, // threads
        )?;
        
        let mut resampled_data = Vec::new();
        let mut input_chunks = processed.chunks(chunk_size);
        
        // Process chunks
        while let Some(chunk) = input_chunks.next() {
            let mut chunk_vec = chunk.to_vec();
            
            // Pad the last chunk if necessary
            if chunk_vec.len() < chunk_size {
                chunk_vec.resize(chunk_size, 0.0);
            }
            
            // Process this chunk
            let input_channels = vec![chunk_vec];
            let output_channels = resampler.process(&input_channels, None)?;
            resampled_data.extend_from_slice(&output_channels[0]);
        }
        
        processed = resampled_data;
        println!("Resampled to {} Hz, {} samples", target_sr, processed.len());
    }
    
    // --- Normalize ---
    if processed.is_empty() {
        return Err("No audio data after processing".into());
    }
    
    let rms = (processed.iter().map(|&s| s * s).sum::<f32>() / processed.len() as f32).sqrt();
    let target_rms = 0.1;
    let gain = if rms > 1e-8 { target_rms / rms } else { 1.0 };
    let normalized: Vec<f32> = processed.iter().map(|&s| (s * gain).clamp(-1.0, 1.0)).collect();
    
    // --- Stats ---
    println!(
        "Audio stats: length={}, min={:.6}, max={:.6}, mean={:.6}",
        normalized.len(),
        normalized.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
        normalized.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)),
        normalized.iter().sum::<f32>() / normalized.len() as f32
    );
    
    // --- Save ---
    let array = Array1::from(normalized);
    write_npy(output_path, &array)?;
    println!("Saved preprocessed audio to {}", output_path);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <input.wav> <output.npy> <target_sr>", args[0]);
        std::process::exit(1);
    }
    
    let input_path = &args[1];
    let output_path = &args[2];
    let target_sr: u32 = args[3].parse()?;
    
    preprocess_wav(input_path, output_path, target_sr)?;
    Ok(())
}