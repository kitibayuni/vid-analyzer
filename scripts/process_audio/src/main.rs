use std::env;
use std::fs::File;
use std::io::BufReader;

use claxon::FlacReader;
use csv::Writer;
use pyin::{Framing, PadMode, PYINExecutor};
use rayon::prelude::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- CLI ARGUMENTS ---
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input_audio.flac>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    println!("Input FLAC file: {}", input_path);

    // --- OPEN FLAC ---
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    let samplerate = flac.streaminfo().sample_rate as usize;
    let channels = flac.streaminfo().channels as usize;

    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- PYIN PARAMETERS ---
    let frame_min = 60.0;
    let frame_max = 600.0;
    let frame_len = (0.025 * samplerate as f64) as usize;
    let (window_len, hop_len, resolution) = (None, None, None);

    // --- CHUNK PARAMETERS ---
    let chunk_sec = 5.0;
    let chunk_samples = (chunk_sec * samplerate as f64) as usize;
    let overlap_samples = frame_len;

    println!(
        "Chunking: {:.1}s chunks ({} samples) with {} sample overlap",
        chunk_sec, chunk_samples, overlap_samples
    );

    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();

    // Global status bar at the top
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(
        ProgressStyle::default_bar()
            .template("{msg}")
            .unwrap(),
    );

    // Overall channel progress bar
    let channel_bar = m.add(ProgressBar::new(channels as u64));
    channel_bar.set_style(
        ProgressStyle::default_bar()
            .template("Channels [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("█  "),
    );

    // --- LOAD SAMPLES INTO CHANNEL BUFFERS ---
    status_bar.set_message("[ == SLICING DATA INTO CHANNEL BUFFERS == ]");
    let mut channel_buffers: Vec<Vec<f64>> = vec![Vec::new(); channels];
    for (i, sample) in flac.samples().enumerate() {
        let s = sample?;
        let chan = i % channels;
        channel_buffers[chan].push(s as f64 / i32::MAX as f64);
    }

    // --- CSV SETUP ---
    let mut writer = Writer::from_path("pitch_output.csv")?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.push(format!("chan{}_pitch_hz", c + 1));
    }
    writer.write_record(&headers)?;

    let mut channel_results: Vec<Vec<(f64, Option<f64>)>> = Vec::new();

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
    status_bar.set_message(format!("[ == PRE-PROCESSING CHANNEL {} == ]", chan_idx + 1));
    let total_chunks = (samples.len() + chunk_samples - 1) / chunk_samples;
    let total_preprocess_steps = samples.len() + total_chunks; // normalization + chunk indexing
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
        preprocess_bar.inc(1); // counts toward the preprocess progress
    }

    // Step 2: generate chunk indices
    status_bar.set_message("[ == COPYING MEMORY & PREPARING SLICES == ]");
    let mut chunk_indices = Vec::new();
    let mut start = 0;
    while start < normalized_samples.len() {
        let end = (start + chunk_samples + overlap_samples).min(normalized_samples.len());
        // Don't allocate a new vector here; just store indices
        chunk_indices.push((start, end));
        preprocess_bar.inc(1); // each chunk counted toward progress
        start += chunk_samples;
    }

    preprocess_bar.finish_with_message(format!("Channel {} pre-processed", chan_idx + 1));

        // --- CHUNK PROCESS BAR ---
        status_bar.set_message(format!("[ == PROCESSING CHUNKS W/ PYIN CHANNEL {} == ]", chan_idx + 1));
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

        // Process chunks in parallel
        let results: Vec<Vec<(f64, Option<f64>)>> = chunk_indices
            .par_iter()
            .map(|(start, end)| {
                let chunk = &normalized_samples[*start..*end];
                let mut pyin_executor = PYINExecutor::new(
                    frame_min,
                    frame_max,
                    samplerate as u32,
                    frame_len,
                    window_len,
                    hop_len,
                    resolution,
                );
                let framing = Framing::Center(PadMode::Constant(0.0));
                let (timestamps, f0, _, _) = pyin_executor.pyin(chunk, f64::NAN, framing);

                let res: Vec<(f64, Option<f64>)> = timestamps
                    .iter()
                    .zip(f0.iter())
                    .map(|(&t, &pitch)| {
                        let global_time = t + (*start as f64) / samplerate as f64;
                        let pitch_opt = if pitch.is_nan() { None } else { Some(pitch) };
                        (global_time, pitch_opt)
                    })
                    .collect();

                chunk_bar.inc(1);
                res
            })
            .collect();

        let merged: Vec<(f64, Option<f64>)> = results.into_iter().flatten().collect();
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
        let mut row: Vec<String> = Vec::new();
        let time_sec = channel_results
            .iter()
            .find_map(|chan| chan.get(i).map(|(t, _)| *t));
        row.push(time_sec.map(|t| format!("{:.4}", t)).unwrap_or_default());

        for chan in &channel_results {
            if let Some((_, pitch)) = chan.get(i) {
                row.push(pitch.map(|p| format!("{:.2}", p)).unwrap_or_else(|| "".to_string()));
            } else {
                row.push("no data".to_string());
            }
        }
        writer.write_record(&row)?;
    }
    writer.flush()?;

    println!("Done. Output saved to pitch_output.csv");

    Ok(())
}
