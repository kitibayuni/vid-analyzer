use hound;
use csv::Writer;

pub fn process(input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input WAV file: {}", input_path);
    println!("Output CSV file: {}", output_path);

    // --- OPEN WAV ---
    let reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();
    let samplerate = spec.sample_rate as usize;
    let channels = spec.channels as usize;
    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- LOAD SAMPLES ---
    let mut channel_buffers: Vec<Vec<f64>> = vec![Vec::new(); channels];
    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = 2f64.powi(spec.bits_per_sample as i32 - 1) - 1.0;
            for (i, sample) in reader.into_samples::<i32>().enumerate() {
                let s = sample? as f64 / max_val;
                let chan = i % channels;
                channel_buffers[chan].push(s);
            }
        }
        hound::SampleFormat::Float => {
            for (i, sample) in reader.into_samples::<f32>().enumerate() {
                let s = sample? as f64;
                let chan = i % channels;
                channel_buffers[chan].push(s);
            }
        }
    }

    // --- FRAME PARAMETERS ---
    let frame_len = (0.025 * samplerate as f64) as usize; // 25ms frames
    let frame_hop = frame_len; // non-overlapping

    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.push(format!("chan{}_zcr", c + 1));
    }
    writer.write_record(&headers)?;

    // --- CALCULATE ZCR ---
    let max_len = channel_buffers.iter().map(|v| v.len()).max().unwrap_or(0);

    for start in (0..max_len).step_by(frame_hop) {
        let time_sec = start as f64 / samplerate as f64;
        let mut row: Vec<String> = vec![format!("{:.4}", time_sec)];

        for chan in &channel_buffers {
            if start >= chan.len() {
                row.push("".to_string());
                continue;
            }
            let end = (start + frame_len).min(chan.len());
            let frame = &chan[start..end];

            let zcr = calculate_zero_crossing_rate(frame);
            row.push(format!("{:.6}", zcr));
        }

        writer.write_record(&row)?;
    }

    writer.flush()?;
    println!("Done. Output saved to {}", output_path);
    Ok(())
}

fn calculate_zero_crossing_rate(frame: &[f64]) -> f64 {
    if frame.len() < 2 { return 0.0; }
    let mut crossings = 0;
    for i in 1..frame.len() {
        if (frame[i] >= 0.0) != (frame[i-1] >= 0.0) { crossings += 1; }
    }
    crossings as f64 / (frame.len() - 1) as f64
}
