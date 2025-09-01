use std::env;
use std::fs::File;
use std::io::BufReader;

use claxon::FlacReader;
use pyin::{PYINExecutor, Framing, PadMode};
use csv::Writer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- CLI ARGUMENT HANDLING --- //
    let args: Vec<String> = env::args().collect();

    // checking # of args
    if args.len() != 2 {
        eprintln!("Usage: {} <input_audio.flac>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    println!("🔍 Input FLAC file: {}", input_path);

    // --- PROCESS .FLAC --- //
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    // take the flac's stream info's sample rate
    let samplerate = flac.streaminfo().sample_rate as usize;
    println!("🎵 Sample rate: {} Hz", samplerate);

    // --- INITIATE PYIN --- //
    let frame_min = 60.0;
    let frame_max = 600.0;
    let frame_len = (0.025 * samplerate as f64) as usize;
    println!("🧠 PYIN initialized with frame length: {} samples", frame_len);

    let (window_len, hop_len, resolution) = (None, None, None);

    let mut pyin_executor = PYINExecutor::new(
        frame_min,
        frame_max,
        samplerate as u32,
        frame_len,
        window_len,
        hop_len,
        resolution,
    );

    // predefine framing behavior
    let framing = Framing::Center(PadMode::Constant(0.0));

    // --- CHUNKING --- //
    let chunk_sec = 0.5;
    let chunk_samples = (chunk_sec * samplerate as f64) as usize;
    let overlap_samples = frame_len;

    println!(
        "📦 Chunking enabled: {}s chunks ({} samples), with {} sample overlap",
        chunk_sec, chunk_samples, overlap_samples
    );

    let mut buffer: Vec<f64> = Vec::with_capacity(chunk_samples + overlap_samples);
    let mut chunk_start_time = 0.0;

    // --- CSV WRITER --- //
    let mut writer = Writer::from_path("pitch_output.csv")?;
    writer.write_record(&["time_sec", "pitch_hz"])?;
    println!("📄 Writing pitch data to pitch_output.csv");

    // --- WRITE ITERATION OVER CHUNKS --- //
    let mut sample_iter = flac.samples();
    let mut sample_counter = 0;
    let mut chunk_count = 0;

    while let Some(sample) = sample_iter.next() {
        let s = sample.unwrap() as f64 / i16::MAX as f64; // normalize
        buffer.push(s);
        sample_counter += 1;

        if buffer.len() >= chunk_samples + overlap_samples {
            println!("🧩 Processing chunk #{} ({} samples)", chunk_count + 1, buffer.len());

            let framing = Framing::Center(PadMode::Constant(0.0));
            let (timestamps, f0, _, _) = pyin_executor.pyin(&buffer, f64::NAN, framing);

            println!("   ➕ PYIN returned {} pitch estimates", f0.len());

            for (t, pitch) in timestamps.iter().zip(f0.iter()) {
                let global_time = t + chunk_start_time;
                writer.write_record(&[
                    format!("{:.4}", global_time),
                    if pitch.is_nan() {
                        "".to_string()
                    } else {
                        format!("{:.2}", pitch)
                    },
                ])?;
            }

            let overlap: Vec<f64> = buffer[buffer.len() - overlap_samples..].to_vec();
            buffer.clear();
            buffer.extend(overlap);
            chunk_start_time += chunk_sec;
            chunk_count += 1;
        }
    }

    // --- PROCESS FINAL CHUNK IF LEFTOVER --- //
    if !buffer.is_empty() {
        println!("🧩 Final chunk: {} samples", buffer.len());
        let (timestamps, f0, _, _) = pyin_executor.pyin(&buffer, f64::NAN, framing);

        println!("   ➕ PYIN returned {} pitch estimates", f0.len());

        for (t, pitch) in timestamps.iter().zip(f0.iter()) {
            let global_time = t + chunk_start_time;
            writer.write_record(&[
                format!("{:.4}", global_time),
                if pitch.is_nan() {
                    "".to_string()
                } else {
                    format!("{:.2}", pitch)
                },
            ])?;
        }
    }

    writer.flush()?;

    println!("✅ Done! {} total samples processed", sample_counter);
    println!("📈 Output saved to pitch_output.csv");
    Ok(())
}
