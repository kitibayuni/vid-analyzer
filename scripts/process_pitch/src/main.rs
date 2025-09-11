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

/// First derivative (pitch modulation)
fn derivative(values: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    result.push(0.0);
    for i in 1..values.len() {
        result.push(values[i] - values[i - 1]);
    }
    result
}

/// Rolling variance
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

/// Rolling range (max - min)
fn rolling_range(values: &[f64], window: usize) -> Vec<f64> {
    let mut range = vec![0.0; values.len()];
    for i in 0..values.len() {
        let start = if i >= window { i + 1 - window } else { 0 };
        let slice = &values[start..=i];
        let min = slice.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = slice.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        range[i] = max - min;
    }
    range
}

/// Normalize to 0–1
fn normalize_01(values: &[f64]) -> Vec<f64> {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < 1e-8 {
        return vec![0.0; values.len()];
    }
    values.iter().map(|v| (v - min) / (max - min)).collect()
}

/// Percentile rank
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

    // Detect channels with pitch data
    let mut channels = Vec::new();
    for h in headers.iter() {
        if h.ends_with("_pitch_hz") {
            let base = h.strip_suffix("_pitch_hz").unwrap().to_string();
            channels.push(base);
        }
    }

    // Time info for window sizing
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
        let pitch_idx = headers.iter().position(|h| h == format!("{}_pitch_hz", ch).as_str()).unwrap();
        let pitch_vals: Vec<f64> = records.iter().map(|r| r[pitch_idx].parse::<f64>().unwrap_or(0.0)).collect();

        let mut col_map = HashMap::new();

        // Features
        let pitch_norm = normalize_01(&pitch_vals);
        let pitch_var = rolling_variance(&pitch_vals, rows_per_sec); // ~1s window
        let pitch_range = rolling_range(&pitch_vals, rows_per_sec);
        let pitch_mod = derivative(&pitch_vals);

        // Normalize each feature
        let pitch_var_norm = normalize_01(&pitch_var);
        let pitch_range_norm = normalize_01(&pitch_range);
        let pitch_mod_norm = normalize_01(&pitch_mod);

        // Engagement score (weighted blend)
        let engagement: Vec<f64> = pitch_var_norm.iter()
            .zip(pitch_range_norm.iter())
            .zip(pitch_mod_norm.iter())
            .map(|((v, r), m)| 0.4 * v + 0.3 * r + 0.3 * m)
            .collect();
        let engagement_norm = normalize_01(&engagement);

        // Store features
        col_map.insert("pitch_norm".to_string(), pitch_norm.clone());
        col_map.insert("pitch_variation".to_string(), pitch_var_norm.clone());
        col_map.insert("pitch_range".to_string(), pitch_range_norm.clone());
        col_map.insert("pitch_modulation".to_string(), pitch_mod_norm.clone());
        col_map.insert("pitch_engagement_score".to_string(), engagement_norm.clone());

        // EMAs for engagement
        col_map.insert("engagement_ema_1s".to_string(), ema(&engagement_norm, alpha_1s));
        col_map.insert("engagement_ema_5s".to_string(), ema(&engagement_norm, alpha_5s));
        col_map.insert("engagement_ema_10s".to_string(), ema(&engagement_norm, alpha_10s));

        // Percentile ranks
        for feat_name in ["engagement_ema_1s", "engagement_ema_5s", "engagement_ema_10s"] {
            let vals = col_map.get(feat_name).unwrap();
            col_map.insert(format!("{}_percentile", feat_name), percentile_rank(vals));
        }

        outputs.insert(ch.clone(), col_map);
    }

    // Write CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_pitch_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Headers
    let mut new_headers = headers.clone();
    for ch in channels.iter() {
        for feat in &[
            "pitch_norm",
            "pitch_variation",
            "pitch_range",
            "pitch_modulation",
            "pitch_engagement_score",
            "engagement_ema_1s",
            "engagement_ema_5s",
            "engagement_ema_10s",
            "engagement_ema_1s_percentile",
            "engagement_ema_5s_percentile",
            "engagement_ema_10s_percentile",
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
                "pitch_norm",
                "pitch_variation",
                "pitch_range",
                "pitch_modulation",
                "pitch_engagement_score",
                "engagement_ema_1s",
                "engagement_ema_5s",
                "engagement_ema_10s",
                "engagement_ema_1s_percentile",
                "engagement_ema_5s_percentile",
                "engagement_ema_10s_percentile",
            ] {
                row.push(feats.get(*feat_name).unwrap()[i].to_string());
            }
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV with pitch features and engagement scores saved successfully!");

    Ok(())
}
