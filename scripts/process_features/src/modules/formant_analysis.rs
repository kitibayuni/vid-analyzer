use std::fs::File;
use std::io::BufReader;
use claxon::FlacReader;
use csv::Writer;
use rayon::prelude::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

// Simple formant detection using Linear Predictive Coding (LPC) approach
// This is a simplified implementation - for production use, consider more sophisticated methods
fn find_formants(samples: &[f64], sample_rate: usize) -> Vec<f64> {
    if samples.len() < 512 {
        return Vec::new();
    }
    
    // Apply pre-emphasis filter
    let mut pre_emphasized: Vec<f64> = Vec::with_capacity(samples.len());
    pre_emphasized.push(samples[0]);
    for i in 1..samples.len() {
        pre_emphasized.push(samples[i] - 0.97 * samples[i - 1]);
    }
    
    // Simple autocorrelation-based formant estimation
    let window_size = 1024.min(pre_emphasized.len());
    let mut autocorr = vec![0.0; window_size / 2];
    
    for lag in 0..autocorr.len() {
        let mut sum = 0.0;
        for i in 0..(window_size - lag) {
            if i + lag < pre_emphasized.len() {
                sum += pre_emphasized[i] * pre_emphasized[i + lag];
            }
        }
        autocorr[lag] = sum;
    }
    
    // Find peaks in autocorrelation (simplified formant detection)
    let mut formants = Vec::new();
    let min_formant_samples = sample_rate / 3000; // ~300 Hz minimum
    let max_formant_samples = sample_rate / 200;  // ~200 Hz maximum for F1
    
    for i in min_formant_samples..max_formant_samples.min(autocorr.len() - 1) {
        if autocorr[i] > autocorr[i - 1] && autocorr[i] > autocorr[i + 1] && autocorr[i] > 0.1 * autocorr[0] {
            let formant_freq = sample_rate as f64 / i as f64;
            if formant_freq >= 200.0 && formant_freq <= 3000.0 {
                formants.push(formant_freq);
            }
        }
    }
    
    // Sort and return up to 4 formants
    formants.sort_by(|a, b| a.partial_cmp(b).unwrap());
    formants.truncate(4);
    formants
}

pub fn process(input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input FLAC file: {}", input_path);
    println!("Output CSV file: {}", output_path);

    // --- OPEN FLAC ---
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    let samplerate = flac.streaminfo().sample_rate as usize;
    let channels = flac.streaminfo().channels as usize;
    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- FORMANT ANALYSIS PARAMETERS ---
    let frame_len = (0.025 * samplerate as f64) as usize; // 25ms frames
    let hop_len = (0.010 * samplerate as f64) as usize;   // 10ms hop (15ms overlap)
    
    // --- CHUNK PARAMETERS ---
    let chunk_sec = 5.0;
    let chunk_samples = (chunk_sec * samplerate as f64) as usize;
    let overlap_samples = frame_len;
    println!(
        "Chunking: {:.1}s chunks ({} samples) with {} sample overlap",
        chunk_sec, chunk_samples, overlap_samples
    );
    println!("Frame length: {} samples ({:.1}ms), Hop length: {} samples ({:.1}ms)\n", 
             frame_len, frame_len as f64 * 1000.0 / samplerate as f64,
             hop_len, hop_len as f64 * 1000.0 / samplerate as f64);

    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();

    // Global status bar
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());

    // Overall channel progress
    let channel_bar = m.add(ProgressBar::new(channels as u64));
    channel_bar.set_style(
        ProgressStyle::default_bar()
            .template("Channels [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("|  "),
    );

    // --- LOAD SAMPLES INTO CHANNEL BUFFERS ---
    status_bar.set_message("[ == SLICING DATA INTO CHANNEL BUFFERS == ]");
    let total_samples = flac.streaminfo().samples.unwrap_or(0) as usize;
    let mut channel_buffers: Vec<Vec<f64>> =
        vec![Vec::with_capacity(total_samples / channels.max(1)); channels];

    for (i, sample) in flac.samples().enumerate() {
        let s = sample?;
        let chan = i % channels;
        channel_buffers[chan].push(s as f64 / i32::MAX as f64);
    }

    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.extend_from_slice(&[
            format!("chan{}_f1_hz", c + 1),
            format!("chan{}_f2_hz", c + 1),
            format!("chan{}_f3_hz", c + 1),
            format!("chan{}_f4_hz", c + 1),
        ]);
    }
    writer.write_record(&headers)?;

    let mut channel_results: Vec<Vec<(f64, Vec<f64>)>> = Vec::new();

    // --- PROCESS EACH CHANNEL ---
    for (chan_idx, samples) in channel_buffers.iter().enumerate() {
        let has_audio = samples.iter().any(|&v| v.abs() > 1e-6);
        if !has_audio {
            println!("Channel {} has no data, skipping.", chan_idx + 1);
            channel_results.push(Vec::new());
            channel_bar.inc(1);
            continue;
        }

        // --- PREPROCESS BAR ---
        let total_chunks = (samples.len() + chunk_samples - 1) / chunk_samples;
        let total_preprocess_steps = samples.len() + total_chunks;
        status_bar.set_message(format!("[ == PRE-PROCESSING CHANNEL {} == ]", chan_idx + 1));
        let preprocess_bar = m.add(ProgressBar::new(total_preprocess_steps as u64));
        preprocess_bar.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "[{{elapsed_precise}}] Ch.{} Pre-process  [{{wide_bar}}] {{pos}}/{{len}}",
                    chan_idx + 1
                ))
                .unwrap()
                .progress_chars("|  "),
        );

        // Step 1: normalize / touch memory
        let mut normalized_samples: Vec<f64> = Vec::with_capacity(samples.len());
        for &s in samples.iter() {
            normalized_samples.push(s);
            preprocess_bar.inc(1);
        }

        // Step 2: generate chunk indices
        status_bar.set_message("[ == COPYING MEMORY & PREPARING SLICES == ]");
        let mut chunk_indices = Vec::with_capacity(total_chunks);
        let mut start = 0;
        while start < normalized_samples.len() {
            let end = (start + chunk_samples + overlap_samples).min(normalized_samples.len());
            chunk_indices.push((start, end));
            preprocess_bar.inc(1);
            start += chunk_samples;
        }
        preprocess_bar.finish_with_message(format!("Channel {} pre-processed", chan_idx + 1));

        // --- CHUNK PROCESS BAR ---
        status_bar.set_message(format!(
            "[ == PROCESSING CHUNKS W/ FORMANT ANALYSIS CHANNEL {} == ]",
            chan_idx + 1
        ));
        let chunk_bar = m.add(ProgressBar::new(chunk_indices.len() as u64));
        chunk_bar.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "[{{elapsed_precise}}] Ch.{} Chunks [{{wide_bar}}] {{pos}}/{{len}} ({{eta}})",
                    chan_idx + 1
                ))
                .unwrap()
                .progress_chars("|  "),
        );

        // --- PARALLEL FORMANT PROCESSING ---
        let results: Vec<Vec<(f64, Vec<f64>)>> = chunk_indices
            .par_iter()
            .map(|(start, end)| {
                let chunk = &normalized_samples[*start..*end];
                let mut chunk_results = Vec::new();
                
                // Process frames within this chunk
                let mut frame_start = 0;
                while frame_start + frame_len <= chunk.len() {
                    let frame_end = frame_start + frame_len;
                    let frame = &chunk[frame_start..frame_end];
                    
                    // Calculate global time for this frame
                    let global_frame_start = *start + frame_start;
                    let global_time = global_frame_start as f64 / samplerate as f64;
                    
                    // Find formants for this frame
                    let formants = find_formants(frame, samplerate);
                    chunk_results.push((global_time, formants));
                    
                    frame_start += hop_len;
                }
                
                chunk_bar.inc(1);
                chunk_results
            })
            .collect();

        let merged: Vec<(f64, Vec<f64>)> = results.into_iter().flatten().collect();
        channel_results.push(merged);

        chunk_bar.finish_with_message(format!("Channel {} chunks done", chan_idx + 1));
        channel_bar.inc(1);
    }

    channel_bar.finish_with_message("All channels complete");
    status_bar.set_message("[ == ALL CHANNELS COMPLETE == ]");
    status_bar.finish();

    // --- WRITE CSV ---
    let max_len = channel_results.iter().map(|v| v.len()).max().unwrap_or(0);
    for i in 0..max_len {
        let mut row: Vec<String> = Vec::with_capacity(channels * 4 + 1);
        
        // Get time from first available channel
        let time_sec = channel_results
            .iter()
            .find_map(|chan| chan.get(i).map(|(t, _)| *t));
        row.push(time_sec.map(|t| format!("{:.4}", t)).unwrap_or_default());

        // Add formant data for each channel
        for chan in &channel_results {
            if let Some((_, formants)) = chan.get(i) {
                // Ensure we always output 4 formant columns per channel
                for f_idx in 0..4 {
                    if f_idx < formants.len() {
                        row.push(format!("{:.2}", formants[f_idx]));
                    } else {
                        row.push("".to_string());
                    }
                }
            } else {
                // No data for this frame - fill with empty strings
                for _ in 0..4 {
                    row.push("".to_string());
                }
            }
        }
        writer.write_record(&row)?;
    }
    writer.flush()?;

    println!("Done. Output saved to {}", output_path);
    Ok(())
}