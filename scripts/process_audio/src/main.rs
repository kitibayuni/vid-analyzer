use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

use claxon::FlacReader;
use csv::Writer;
use pyin::{Framing, PadMode, PYINExecutor};
use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- CLI ARGUMENT HANDLING --- //
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input_audio.flac>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];
    println!("🔍 Input FLAC file: {}", input_path);

    // --- OPEN FLAC --- //
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    let samplerate = flac.streaminfo().sample_rate as usize;
    println!("🎵 Sample rate: {} Hz", samplerate);

    // --- PYIN PARAMS SETUP --- //
    let frame_min = 60.0;
    let frame_max = 600.0;
    let frame_len = (0.025 * samplerate as f64) as usize;
    println!("🧠 PYIN frame length: {} samples", frame_len);

    let (window_len, hop_len, resolution) = (None, None, None);

    // --- CHUNKING SETUP --- //
    let chunk_sec = 5.0; // adjusted for fewer, larger chunks
    let chunk_samples = (chunk_sec * samplerate as f64) as usize;
    let overlap_samples = frame_len;
    println!(
        "📦 Chunking: {:.1}s chunks ({} samples), {} sample overlap",
        chunk_sec, chunk_samples, overlap_samples
    );

    // --- LOAD SAMPLES INTO MEMORY --- //
    let samples: Vec<f64> = flac
        .samples()
        .map(|s| s.unwrap() as f64 / i16::MAX as f64)
        .collect();
    println!("📊 Total samples loaded: {}", samples.len());

    // --- SPLIT INTO OVERLAPPING CHUNKS --- //
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let end = (start + chunk_samples + overlap_samples).min(samples.len());
        chunks.push((&samples[start..end], start));
        start += chunk_samples;
    }
    println!("🧩 Total chunks: {}", chunks.len());

    // --- INIT CSV WRITER --- //
    let mut writer = Writer::from_path("pitch_output.csv")?;
    writer.write_record(&["time_sec", "pitch_hz"])?;

    // --- PROGRESS BAR --- //
    let pb = ProgressBar::new(chunks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} chunks ({eta})")
            .unwrap()
            .progress_chars("█  "),
    );

    // --- PROCESS CHUNKS IN PARALLEL --- //
    let results: Vec<Vec<(f64, Option<f64>)>> = chunks
        .par_iter()
        .enumerate()
        .map(|(_i, (chunk, start_sample))| {
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

            let global_results: Vec<(f64, Option<f64>)> = timestamps
                .iter()
                .zip(f0.iter())
                .map(|(&t, &pitch)| {
                    let global_time = t + (*start_sample as f64) / samplerate as f64;
                    let pitch_opt = if pitch.is_nan() { None } else { Some(pitch) };
                    (global_time, pitch_opt)
                })
                .collect();

            pb.inc(1); // update progress bar after each chunk
            global_results
        })
        .collect();

    pb.finish_with_message("✅ Processing complete");

    // --- WRITE TO CSV --- //
    println!("💾 Writing pitch_output.csv ...");
    for chunk_result in results {
        for (time_sec, pitch_opt) in chunk_result {
            writer.write_record(&[
                format!("{:.4}", time_sec),
                pitch_opt
                    .map(|p| format!("{:.2}", p))
                    .unwrap_or_else(|| "".to_string()),
            ])?;
        }
    }
    writer.flush()?;
    println!("✅ Done. Output saved.");

    Ok(())
}
