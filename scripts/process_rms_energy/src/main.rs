use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

/// Compute rolling mean and standard deviation for a slice of values.
fn rolling_stats(values: &[f64], window: usize) -> Vec<(f64, f64)> {
    let mut stats = vec![(0.0, 0.0); values.len()];
    for i in 0..values.len() {
        let start = if i >= window { i + 1 - window } else { 0 };
        let window_slice = &values[start..=i];
        let mean = window_slice.iter().sum::<f64>() / window_slice.len() as f64;
        let variance = window_slice
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / window_slice.len() as f64;
        stats[i] = (mean, variance.sqrt());
    }
    stats
}

/// Continuous score based on number of standard deviations above mean
fn score_continuous(val: f64, mean: f64, std: f64, clamp_negative: bool) -> f64 {
    if std == 0.0 {
        return 0.0;
    }
    let z = (val - mean) / std;
    let z = if clamp_negative && z < 0.0 { 0.0 } else { z };

    if z <= 1.0 {
        z
    } else if z <= 2.0 {
        1.0 + (z - 1.0)
    } else if z <= 3.0 {
        2.0 + (z - 2.0) * 1.5
    } else {
        3.0 + (z - 3.0) * 2.0
    }
}

/// Normalize a vector to 0-1 range
fn normalize_0_1(values: &[f64]) -> Vec<f64> {
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (max_val - min_val).abs() < 1e-9 {
        vec![0.0; values.len()]
    } else {
        values.iter().map(|v| (v - min_val) / (max_val - min_val)).collect()
    }
}

/// Compute mean engagement per interval (given in seconds)
fn interval_mean(values: &[f64], interval_len_sec: usize, rows_per_second: usize) -> Vec<f64> {
    let window_size = interval_len_sec * rows_per_second;
    let mut means = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        let start = if i >= window_size { i + 1 - window_size } else { 0 };
        let window_slice = &values[start..=i];
        let mean = window_slice.iter().sum::<f64>() / window_slice.len() as f64;
        means.push(mean);
    }
    means
}

/// Compute percentile ranks of values
fn percentile_ranks(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values
        .iter()
        .map(|v| {
            let count = sorted.iter().filter(|&x| x <= v).count();
            count as f64 / values.len() as f64
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <input.csv> <engagement_column> [--time_col TIME_COLUMN]",
            args[0]
        );
        std::process::exit(1);
    }

    let input_path = &args[1];
    let engagement_col = &args[2];
    let mut time_col = "time_sec".to_string();

    if args.len() > 4 && args[3] == "--time_col" {
        time_col = args[4].clone();
    }

    // Open CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    // Get indices
    let eng_idx = headers.iter()
        .position(|h| h == engagement_col)
        .ok_or("Engagement column not found")?;
    let time_idx = headers.iter()
        .position(|h| h == &time_col)
        .ok_or("Time column not found")?;

    // Parse columns
    let engagement_values: Vec<f64> = records.iter()
        .map(|r| r.get(eng_idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();
    let time_values: Vec<f64> = records.iter()
        .map(|r| r.get(time_idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    // Compute rows per second
    let mut deltas = Vec::new();
    for i in 1..time_values.len() {
        deltas.push(time_values[i] - time_values[i - 1]);
    }
    let avg_delta = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let rows_per_second = (1.0 / avg_delta).round() as usize;
    println!("[INFO] Detected rows per second: {}", rows_per_second);

    // Normalize engagement 0-1
    let normalized_engagement = normalize_0_1(&engagement_values);

    // Compute mean engagement per 1s, 5s, 10s
    let mean_1s = interval_mean(&normalized_engagement, 1, rows_per_second);
    let mean_5s = interval_mean(&normalized_engagement, 5, rows_per_second);
    let mean_10s = interval_mean(&normalized_engagement, 10, rows_per_second);

    // Compute percentile ranks
    let pr_1s = percentile_ranks(&mean_1s);
    let pr_5s = percentile_ranks(&mean_5s);
    let pr_10s = percentile_ranks(&mean_10s);

    // Prepare output CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_ranked.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Write headers
    let mut new_headers = headers.clone();
    new_headers.push_field("engagement_0_1");
    new_headers.push_field("mean_engagement_1s");
    new_headers.push_field("mean_engagement_5s");
    new_headers.push_field("mean_engagement_10s");
    new_headers.push_field("percentile_1s");
    new_headers.push_field("percentile_5s");
    new_headers.push_field("percentile_10s");
    wtr.write_record(&new_headers)?;

    // Write rows
    for i in 0..records.len() {
        let mut row: Vec<String> = records[i].iter().map(|s| s.to_string()).collect();
        row.push(normalized_engagement[i].to_string());
        row.push(mean_1s[i].to_string());
        row.push(mean_5s[i].to_string());
        row.push(mean_10s[i].to_string());
        row.push(pr_1s[i].to_string());
        row.push(pr_5s[i].to_string());
        row.push(pr_10s[i].to_string());
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV saved successfully!");

    Ok(())
}
