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

/// Convert a value, mean, std into a score
fn score(val: f64, mean: f64, std: f64) -> u8 {
    let diff = (val - mean).abs();
    if diff <= std {
        0
    } else if diff <= 2.0 * std {
        1
    } else if diff <= 3.0 * std {
        2
    } else {
        3
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input.csv> <column1> [column2 ...]", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let columns_to_process: Vec<String> = args[2..].to_vec();

    // Open CSV reader
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();

    // Load all records into memory
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    // Map column name to its index
    let mut col_indices: HashMap<String, usize> = HashMap::new();
    for col in &columns_to_process {
        if let Some(idx) = headers.iter().position(|h| h == col) {
            col_indices.insert(col.clone(), idx);
        } else {
            eprintln!("Warning: column '{}' not found in CSV", col);
        }
    }

    // Rolling window size
    let window_size = 10;

    // Compute scores for each column
    let mut score_columns: HashMap<String, Vec<u8>> = HashMap::new();
    for (col, &idx) in &col_indices {
        let values: Vec<f64> = records
            .iter()
            .map(|r| r.get(idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
            .collect();
        let stats = rolling_stats(&values, window_size);
        let scores: Vec<u8> = values
            .iter()
            .zip(stats.iter())
            .map(|(val, (mean, std))| score(*val, *mean, *std))
            .collect();
        score_columns.insert(col.clone(), scores);
    }

    // Prepare output path
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Write header
    let mut new_headers = headers.clone();
    for col in &columns_to_process {
        new_headers.push_field(&format!("{}_score", col));
    }
    wtr.write_record(&new_headers)?;

    // Write rows
    for (i, record) in records.iter().enumerate() {
        let mut row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        for col in &columns_to_process {
            let score_val = score_columns[col][i];
            row.push(score_val.to_string());
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV saved successfully!");

    Ok(())
}
