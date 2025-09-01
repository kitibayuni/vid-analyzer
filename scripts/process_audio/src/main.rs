use std::env;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

use claxon::FlacReader;
use csv::Writer;
use pyin::{Framing, PadMode, PYINExecutor};
use rayon::prelude::*; // notes: for parallel iteration

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- CLI ARGUMENT HANDLING --- //
    // notes: get command line args, expect exactly 1 input file path
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input_audio.flac>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];
    println!("🔍 Input FLAC file: {}", input_path);

    // --- OPEN FLAC --- //
    // notes: open file, wrap in BufReader, initialize FLAC decoder
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    // notes: get sample rate info from flac metadata
    let samplerate = flac.streaminfo().sample_rate as usize;
    println!("🎵 Sample rate: {} Hz", samplerate);

    // --- PYIN PARAMS SETUP (used inside closure later) --- //
    let frame_min = 60.0;
    let frame_max = 600.0;
    let frame_len = (0.025 * samplerate as f64) as usize;
    println!("🧠 PYIN frame length: {} samples", frame_len);

    let (window_len, hop_len, resolution) = (None, None, None);

    // --- CHUNKING SETUP --- //
    let chunk_sec = 0.5;
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

    // --- PROCESS CHUNKS IN PARALLEL --- //
    let results: Vec<Vec<(f64, Option<f64>)>> = chunks
        .par_iter()
        .enumerate()
        .map(|(i, (chunk, start_sample))| {
            println!("⏳ Chunk #{} ({} samples)", i + 1, chunk.len());
            let timer = Instant::now();

            // --- create new PYIN executor per thread (can't share mutable) --- //
            let mut pyin_executor = PYINExecutor::new(
                frame_min,
                frame_max,
                samplerate as u32,
                frame_len,
                window_len,
                hop_len,
                resolution,
            );

            // --- recreate framing inside closure (no move) --- //
            let framing = Framing::Center(PadMode::Constant(0.0));

            // --- pitch estimation --- //
            let (timestamps, f0, _, _) = pyin_executor.pyin(chunk, f64::NAN, framing);

            // --- attach global time offset --- //
            let global_results: Vec<(f64, Option<f64>)> = timestamps
                .iter()
                .zip(f0.iter())
                .map(|(&t, &pitch)| {
                    let global_time = t + (*start_sample as f64) / samplerate as f64;
                    let pitch_opt = if pitch.is_nan() { None } else { Some(pitch) };
                    (global_time, pitch_opt)
                })
                .collect();

            println!(
                "   ✅ Returned {} pitch points in {:.2?}",
                global_results.len(),
                timer.elapsed()
            );

            global_results
        })
        .collect();

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
