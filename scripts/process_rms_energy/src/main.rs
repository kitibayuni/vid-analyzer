use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

/// Exponential Moving Average
fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
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

/// First derivative
fn derivative(values: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    result.push(0.0);
    for i in 1..values.len() {
        result.push(values[i] - values[i - 1]);
    }
    result
}

/// Second derivative
fn second_derivative(values: &[f64]) -> Vec<f64> {
    let first = derivative(values);
    derivative(&first)
}

/// Rolling variance over a window of N samples
fn rolling_variance(values: &[f64], window: usize) -> Vec<f64> {
    let mut var = vec![0.0; values.len()];
    for i in 0..values.len() {
        let start = if i >= window { i + 1 - window } else { 0 };
        let window_slice = &values[start..=i];
        let mean = window_slice.iter().sum::<f64>() / window_slice.len() as f64;
        let v = window_slice
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / window_slice.len() as f64;
        var[i] = v;
    }
    var
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <input.csv> <column1> [column2 ...] [--time_col TIME_COLUMN]",
            args[0]
        );
        std::process::exit(1);
    }

    let input_path = &args[1];
    let mut columns_to_process = Vec::new();
    let mut time_col = "time_sec".to_string();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--time_col" if i + 1 < args.len() => {
                time_col = args[i + 1].clone();
                i += 2;
            }
            _ => {
                columns_to_process.push(args[i].clone());
                i += 1;
            }
        }
    }

    if columns_to_process.is_empty() {
        eprintln!("[!] No columns specified to process.");
        std::process::exit(1);
    }

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    // Map column names to indices
    let mut col_indices = HashMap::new();
    for col in &columns_to_process {
        if let Some(idx) = headers.iter().position(|h| h == col) {
            col_indices.insert(col.clone(), idx);
        } else {
            eprintln!("Warning: column '{}' not found in CSV", col);
        }
    }

    // Determine time column for rows/sec
    let time_idx = headers.iter().position(|h| h == &time_col).ok_or("Time column not found")?;
    let time_values: Vec<f64> = records
        .iter()
        .map(|r| r.get(time_idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();
    let avg_delta = time_values.windows(2).map(|w| w[1] - w[0]).sum::<f64>() / (time_values.len() - 1) as f64;
    let rows_per_sec = (1.0 / avg_delta).round() as usize;
    println!("[INFO] Detected rows per second: {}", rows_per_sec);

    // Convert desired ms to alpha
    let alpha_100 = 2.0 / ((0.100 * rows_per_sec as f64) + 1.0);
    let alpha_175 = 2.0 / ((0.175 * rows_per_sec as f64) + 1.0);
    let alpha_400 = 2.0 / ((0.400 * rows_per_sec as f64) + 1.0);
    let var_window = (0.400 * rows_per_sec as f64).round() as usize;

    let mut outputs: HashMap<String, HashMap<String, Vec<f64>>> = HashMap::new();

    for (col, &idx) in &col_indices {
        let values: Vec<f64> = records
            .iter()
            .map(|r| r.get(idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
            .collect();

        let mut col_map = HashMap::new();
        // EMAs
        col_map.insert("ema_100ms".to_string(), ema(&values, alpha_100));
        let ema_175 = ema(&values, alpha_175);
        col_map.insert("ema_175ms".to_string(), ema_175.clone());
        let ema_400 = ema(&values, alpha_400);
        col_map.insert("ema_400ms".to_string(), ema_400.clone());

        // Derived features
        col_map.insert("d1_175ms".to_string(), derivative(&ema_175));
        col_map.insert("d2_100ms".to_string(), second_derivative(&ema(&values, alpha_100)));
        col_map.insert("var_400ms".to_string(), rolling_variance(&ema_400, var_window));

        outputs.insert(col.clone(), col_map);
    }

    // Write CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Headers
    let mut new_headers = headers.clone();
    for col in &columns_to_process {
        for feat in &["ema_100ms", "ema_175ms", "ema_400ms", "d1_175ms", "d2_100ms", "var_400ms"] {
            new_headers.push_field(&format!("{}_{}", col, feat));
        }
    }
    wtr.write_record(&new_headers)?;

    // Rows
    for i in 0..records.len() {
        let mut row: Vec<String> = records[i].iter().map(|s| s.to_string()).collect();
        for col in &columns_to_process {
            let feats = outputs.get(col).unwrap();
            for feat_name in &["ema_100ms", "ema_175ms", "ema_400ms", "d1_175ms", "d2_100ms", "var_400ms"] {
                row.push(feats.get(*feat_name).unwrap()[i].to_string());
            }
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV saved successfully!");

    Ok(())
}
