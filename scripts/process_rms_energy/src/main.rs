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
    derivative(&derivative(values))
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

/// Z-score normalization
fn zscore(values: &[f64]) -> Vec<f64> {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let std = (values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();
    values.iter().map(|x| if std != 0.0 { (x - mean) / std } else { 0.0 }).collect()
}

/// Normalize a vector to 0–1
fn normalize_01(values: &[f64]) -> Vec<f64> {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < 1e-8 {
        return vec![0.0; values.len()];
    }
    values.iter().map(|v| (v - min) / (max - min)).collect()
}

/// Compute percentile rank of a vector
fn percentile_rank(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values.iter().map(|v| {
        let count = sorted.iter().filter(|&&x| x <= *v).count();
        count as f64 / values.len() as f64
    }).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input.csv> [--time_col TIME_COLUMN]", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let mut time_col = "time_sec".to_string();
    if args.len() > 3 && args[2] == "--time_col" {
        time_col = args[3].clone();
    }

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    // Detect all channels automatically
    let mut channels = Vec::new();
    for h in headers.iter() {
        if h.ends_with("_rms") {
            let base = h.strip_suffix("_rms").unwrap().to_string();
            if headers.iter().any(|x| x == format!("{}_energy", base).as_str()) {
                channels.push(base);
            }
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

    let alpha_1s = 2.0 / ((1.0 * rows_per_sec as f64) + 1.0);
    let alpha_5s = 2.0 / ((5.0 * rows_per_sec as f64) + 1.0);
    let alpha_10s = 2.0 / ((10.0 * rows_per_sec as f64) + 1.0);

    let mut outputs: HashMap<String, HashMap<String, Vec<f64>>> = HashMap::new();

    for ch in channels.iter() {
        let rms_idx = headers.iter().position(|h| h == format!("{}_rms", ch).as_str()).unwrap();
        let energy_idx = headers.iter().position(|h| h == format!("{}_energy", ch).as_str()).unwrap();

        let rms_vals: Vec<f64> = records.iter().map(|r| r[rms_idx].parse::<f64>().unwrap_or(0.0)).collect();
        let energy_vals: Vec<f64> = records.iter().map(|r| r[energy_idx].parse::<f64>().unwrap_or(0.0)).collect();

        let mut col_map = HashMap::new();

        // Weighted engagement: RMS 70%, Energy 30%
        let engagement_raw: Vec<f64> = rms_vals.iter().zip(energy_vals.iter())
            .map(|(r, e)| 0.7*r + 0.3*e).collect();

        let engagement_norm = normalize_01(&engagement_raw);
        col_map.insert("engagement_score".to_string(), engagement_norm.clone());

        // Rolling EMAs for 1s, 5s, 10s
        col_map.insert("engagement_ema_1s".to_string(), ema(&engagement_norm, alpha_1s));
        col_map.insert("engagement_ema_5s".to_string(), ema(&engagement_norm, alpha_5s));
        col_map.insert("engagement_ema_10s".to_string(), ema(&engagement_norm, alpha_10s));

        // Percentile ranks
        col_map.insert("engagement_ema_1s_percentile".to_string(), percentile_rank(&col_map["engagement_ema_1s"]));
        col_map.insert("engagement_ema_5s_percentile".to_string(), percentile_rank(&col_map["engagement_ema_5s"]));
        col_map.insert("engagement_ema_10s_percentile".to_string(), percentile_rank(&col_map["engagement_ema_10s"]));

        outputs.insert(ch.clone(), col_map);
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
    for ch in channels.iter() {
        for feat in &[
            "engagement_score",
            "engagement_ema_1s",
            "engagement_ema_5s",
            "engagement_ema_10s",
            "engagement_ema_1s_percentile",
            "engagement_ema_5s_percentile",
            "engagement_ema_10s_percentile"
        ] {
            new_headers.push_field(&format!("{}_{}", ch, feat));
        }
    }
    wtr.write_record(&new_headers)?;

    // Rows
    for i in 0..records.len() {
        let mut row: Vec<String> = records[i].iter().map(|s| s.to_string()).collect();
        for ch in channels.iter() {
            let feats = outputs.get(ch).unwrap();
            for feat_name in &[
                "engagement_score",
                "engagement_ema_1s",
                "engagement_ema_5s",
                "engagement_ema_10s",
                "engagement_ema_1s_percentile",
                "engagement_ema_5s_percentile",
                "engagement_ema_10s_percentile"
            ] {
                row.push(feats.get(*feat_name).unwrap()[i].to_string());
            }
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV with engagement scores and percentile ranks saved successfully!");

    Ok(())
}
