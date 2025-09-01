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

    // --- PROCESS .FLAC --- //
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    // take the flac's stream info's sample rate
    let samplerate = flac.streaminfo().sample_rate as usize;

    // --- INITIATE PYIN --- //
    // notes: this creates
    // frames that overlap &
    // which will be calc'd
    // after generation.

    let frame_min = 60.0;
    let frame_max = 600.0;
    let frame_len = (0.025 * samplerate as f64) as usize;

    // tuple for
    let (window_len, hop_len, resolution) = (None, None, None);

    // pyin executor
    let mut pyin_executor = PYINExecutor::new(
        frame_min,
        frame_max,
        samplerate as i32,
        frame_len,
        window_len,
        hop_len,
        resolution,
    );

    // framing, so that the sample is in the middle of the frame
    let framing = Framing::Center(PadMode::Constant(0.0));

    // --- CHUNKING --- //
    // notes: so that
    // memory will not
    // load the entire
    // audio track at
    // once.

    let chunk_sec = 30.0;
    let chunk_samples = (chunk_sec * samplerate as f64) as usize;
    let overlap_samples = frame_len;

    // pre-allocating a vector buffer to hold chunk_samples and overlap_samples
    let mut buffer: Vec<f64> = Vec::with_capacity(chunk_samples + overlap_samples);
    let mut chunk_start_time = 0.0; // mutable to adjust for next chunk

    // CSV WRITER //
    let mut writer = Writer::from_path("pitch_output.csv")?;
    writer.write_record(&["time_sec", "pitch_hz"])?;

    // --- WRITE ITERATION OVER CHUNKS --- //
    let mut sample_iter = flac.samples::<i32>();
    while let Some(sample) = sample_iter.next() {
        let s = sample.unwrap() as f64 / i16::MAX as f64; // normalize
        buffer.push(s);

        if buffer.len() >= chunk_samples + overlap_samples {
            // Run PYIN
            let (timestamps, f0, _, _) = pyin_executor.pyin(&buffer, f64::NAN, framing.clone());

            // Write results with global time
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

            // Retain overlap for next chunk
            let overlap: Vec<f64> = buffer[buffer.len() - overlap_samples..].to_vec();
            buffer.clear();
            buffer.extend(overlap);
            chunk_start_time += chunk_sec;
        }
    }

    // --- PROCESS FINAL CHUNK IF LEFTOVER --- //
    if !buffer.is_empty() {
        let (timestamps, f0, _, _) = pyin_executor.pyin(&buffer, f64::NAN, framing);
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

    println!("pitch_output.csv generated!");
    Ok(())
}
