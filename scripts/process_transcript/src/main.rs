use csv::{ReaderBuilder, WriterBuilder};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

/// Count how many words occur within each 1-second bucket.
fn words_per_second(time_values: &[f64]) -> HashMap<i64, usize> {
    let mut counts = HashMap::new();
    for &t in time_values {
        let sec = t.floor() as i64;
        *counts.entry(sec).or_insert(0) += 1;
    }
    counts
}

/// Exponential Moving Average
fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut prev = values[0];
    result.push(prev);
    for &val in &values[1..] {
        let next = alpha * val + (1.0 - alpha) * prev;
        result.push(next);
        prev = next;
    }
    result
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input.csv>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let time_idx = headers.iter().position(|h| h == "time_sec").ok_or("Missing time_sec column")?;
    let word_idx = headers.iter().position(|h| h == "word").ok_or("Missing word column")?;

    let mut words: Vec<(f64, String)> = Vec::new();
    for result in rdr.records() {
        let rec = result?;
        let t: f64 = rec.get(time_idx).unwrap().parse().unwrap_or(0.0);
        let w = rec.get(word_idx).unwrap_or("").to_string();
        words.push((t, w));
    }

    // Compute word counts per second
    let times: Vec<f64> = words.iter().map(|(t, _)| *t).collect();
    let wps_map = words_per_second(&times);

    // Build continuous timeline (0.2s spacing)
    let start = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let end = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut timeline = Vec::new();
    let mut t = start;
    while t <= end {
        timeline.push(t);
        t = (t * 5.0 + 1.0).floor() / 5.0; // step 0.2s safely
        if t <= timeline.last().unwrap() + 1e-6 {
            t += 0.2;
        }
    }

    // Make series aligned with timeline
    let wps_series: Vec<f64> = timeline
        .iter()
        .map(|&tt| {
            let sec = tt.floor() as i64;
            wps_map.get(&sec).cloned().unwrap_or(0) as f64
        })
        .collect();

    // 5s EMA
    let alpha_5s = 2.0 / (5.0 + 1.0);
    let wps_ema_5s = ema(&wps_series, alpha_5s);

    // Output CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Headers
    wtr.write_record(&["time_sec", "words_per_sec", "words_per_sec_ema_5s"])?;

    // Write rows
    for (i, &tt) in timeline.iter().enumerate() {
        wtr.write_record(&[
            format!("{:.1}", tt),
            wps_series[i].to_string(),
            wps_ema_5s[i].to_string(),
        ])?;
    }

    wtr.flush()?;
    println!("Processed CSV with 0.2s timeline, words_per_sec, and 5s EMA saved!");

    Ok(())
}
