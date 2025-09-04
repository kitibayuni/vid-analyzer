use std::env;
use std::fs::File;
use std::io::BufReader;

use claxon::FlacReader;
use csv::Writer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- CLI ARGUMENTS ---
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_audio.flac> <output.csv>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];

    println!("Input FLAC file: {}", input_path);
    println!("Output CSV file: {}", output_path);

    // --- OPEN FLAC ---
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    let samplerate = flac.streaminfo().sample_rate as usize;
    let channels = flac.streaminfo().channels as usize;
    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- FRAME PARAMETERS ---
    let frame_len = (0.025 * samplerate as f64) as usize; // 25ms frames
    println!("Calculating RMS & Energy using {}-sample frames (~25ms)", frame_len);

    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());

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
        headers.push(format!("chan{}_rms", c + 1));
        headers.push(format!("chan{}_energy", c + 1));
    }
    writer.write_record(&headers)?;

    // --- CALCULATE RMS & ENERGY PER CHANNEL ---
    status_bar.set_message("[ == CALCULATING RMS & ENERGY == ]");

    let max_len = channel_buffers.iter().map(|v| v.len()).max().unwrap_or(0);
    let frame_hop = frame_len; // non-overlapping frames

    for start in (0..max_len).step_by(frame_hop) {
        let time_sec = start as f64 / samplerate as f64;
        let mut row: Vec<String> = vec![format!("{:.4}", time_sec)];

        for chan in &channel_buffers {
            if start >= chan.len() {
                row.push("".to_string());
                row.push("".to_string());
                continue;
            }
            let end = (start + frame_len).min(chan.len());
            let frame = &chan[start..end];

            // RMS
            let rms = (frame.iter().map(|&s| s * s).sum::<f64>() / frame.len() as f64).sqrt();
            // Energy
            let energy = frame.iter().map(|&s| s * s).sum::<f64>();

            row.push(format!("{:.6}", rms));
            row.push(format!("{:.6}", energy));
        }
        writer.write_record(&row)?;
    }

    writer.flush()?;
    status_bar.finish_with_message("[ == RMS & ENERGY CSV COMPLETE == ]");
    println!("Done. Output saved to {}", output_path);

    Ok(())
}
