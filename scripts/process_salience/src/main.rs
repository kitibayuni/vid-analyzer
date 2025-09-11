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

/// Calculate engagement score from saliency features
fn calculate_engagement(
    mean_sal: &[f64],
    max_sal: &[f64], 
    sal_entropy: &[f64],
    sal_change: &[f64],
    attention_conc: &[f64],
    attention_shift: &[f64],
    motion_intensity: &[f64],
    motion_change: &[f64]
) -> Vec<f64> {
    let mut engagement = Vec::with_capacity(mean_sal.len());
    
    for i in 0..mean_sal.len() {
        // Weighted engagement score based on multiple saliency and attention factors
        let saliency_component = 0.25 * mean_sal[i] + 0.15 * max_sal[i] + 0.10 * sal_entropy[i];
        let change_component = 0.15 * sal_change[i].abs() + 0.10 * attention_shift[i];
        let attention_component = 0.15 * attention_conc[i];
        let motion_component = 0.05 * motion_intensity[i] + 0.05 * motion_change[i].abs();
        
        let total_engagement = saliency_component + change_component + attention_component + motion_component;
        engagement.push(total_engagement.max(0.0)); // Ensure non-negative
    }
    
    engagement
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_saliency.csv> [--time_col TIME_COLUMN]", args[0]);
        eprintln!("Expected columns: time_sec, mean_saliency, max_saliency, saliency_entropy, saliency_change_rate,");
        eprintln!("                  attention_center_x, attention_center_y, attention_concentration, attention_shift_rate,");
        eprintln!("                  motion_intensity, motion_change_rate");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let time_col: &str = if args.len() > 3 && args[2] == "--time_col" {
        &args[3]
    } else {
        "time_sec"
    };

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    println!("[INFO] Processing {} rows from saliency CSV", records.len());

    // Find required column indices
    let required_cols = [
        (time_col, "time_sec"),
        ("mean_saliency", "mean_saliency"),
        ("max_saliency", "max_saliency"), 
        ("saliency_entropy", "saliency_entropy"),
        ("saliency_change_rate", "saliency_change_rate"),
        ("attention_concentration", "attention_concentration"),
        ("attention_shift_rate", "attention_shift_rate"),
        ("motion_intensity", "motion_intensity"),
        ("motion_change_rate", "motion_change_rate")
    ];

    let mut col_indices: HashMap<&str, usize> = HashMap::new();
    for (key, col_name) in &required_cols {
        let idx = headers.iter().position(|h| h == *col_name)
            .ok_or_else(|| format!("Required column '{}' not found", col_name))?;
        col_indices.insert(*key, idx);
    }

    // Extract column data
    let time_values: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get(time_col).unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let mean_saliency: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("mean_saliency").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let max_saliency: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("max_saliency").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let saliency_entropy: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("saliency_entropy").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let saliency_change_rate: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("saliency_change_rate").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let attention_concentration: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("attention_concentration").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let attention_shift_rate: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("attention_shift_rate").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let motion_intensity: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("motion_intensity").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    let motion_change_rate: Vec<f64> = records.iter()
        .map(|r| r.get(*col_indices.get("motion_change_rate").unwrap()).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    // Calculate time-based parameters
    let avg_delta = time_values.windows(2).map(|w| w[1] - w[0]).sum::<f64>() / (time_values.len() - 1) as f64;
    let rows_per_sec = (1.0 / avg_delta).round() as usize;
    println!("[INFO] Detected rows per second: {}", rows_per_sec);

    let alpha_1s = 2.0 / ((1.0 * rows_per_sec as f64) + 1.0);
    let alpha_3s = 2.0 / ((3.0 * rows_per_sec as f64) + 1.0);
    let alpha_10s = 2.0 / ((10.0 * rows_per_sec as f64) + 1.0);

    // Calculate raw engagement score
    let engagement_raw = calculate_engagement(
        &mean_saliency,
        &max_saliency,
        &saliency_entropy,
        &saliency_change_rate,
        &attention_concentration,
        &attention_shift_rate,
        &motion_intensity,
        &motion_change_rate
    );

    println!("[INFO] Raw engagement range: {:.6} to {:.6}", 
        engagement_raw.iter().cloned().fold(f64::INFINITY, f64::min),
        engagement_raw.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );

    // Normalize engagement score to 0-1
    let engagement_norm = normalize_01(&engagement_raw);

    // Calculate EMAs for different time windows
    let engagement_ema_1s = ema(&engagement_norm, alpha_1s);
    let engagement_ema_3s = ema(&engagement_norm, alpha_3s);
    let engagement_ema_10s = ema(&engagement_norm, alpha_10s);

    // Calculate percentile ranks
    let engagement_ema_1s_percentile = percentile_rank(&engagement_ema_1s);
    let engagement_ema_3s_percentile = percentile_rank(&engagement_ema_3s);
    let engagement_ema_10s_percentile = percentile_rank(&engagement_ema_10s);

    // Calculate variance features for engagement dynamics
    let window_1s = rows_per_sec;
    let window_5s = 5 * rows_per_sec;
    let engagement_variance_1s = rolling_variance(&engagement_norm, window_1s);
    let engagement_variance_5s = rolling_variance(&engagement_norm, window_5s);

    // Write output CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_engagement.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(&output_path)?;

    // Write headers
    let mut new_headers = headers.clone();
    let engagement_features = [
        "visual_engagement_score",
        "visual_engagement_ema_1s", 
        "visual_engagement_ema_3s",
        "visual_engagement_ema_10s",
        "visual_engagement_ema_1s_percentile",
        "visual_engagement_ema_3s_percentile", 
        "visual_engagement_ema_10s_percentile",
        "visual_engagement_variance_1s",
        "visual_engagement_variance_5s"
    ];
    
    for feat in &engagement_features {
        new_headers.push_field(feat);
    }
    wtr.write_record(&new_headers)?;

    // Write data rows
    for i in 0..records.len() {
        let mut row: Vec<String> = records[i].iter().map(|s| s.to_string()).collect();
        
        row.push(format!("{:.6}", engagement_norm[i]));
        row.push(format!("{:.6}", engagement_ema_1s[i]));
        row.push(format!("{:.6}", engagement_ema_3s[i]));
        row.push(format!("{:.6}", engagement_ema_10s[i]));
        row.push(format!("{:.6}", engagement_ema_1s_percentile[i]));
        row.push(format!("{:.6}", engagement_ema_3s_percentile[i]));
        row.push(format!("{:.6}", engagement_ema_10s_percentile[i]));
        row.push(format!("{:.6}", engagement_variance_1s[i]));
        row.push(format!("{:.6}", engagement_variance_5s[i]));
        
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("✅ Visual engagement analysis saved to: {}", output_path.display());
    println!("📊 Engagement Summary:");
    println!("   Mean engagement: {:.4}", engagement_norm.iter().sum::<f64>() / engagement_norm.len() as f64);
    println!("   Peak engagement: {:.4}", engagement_norm.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    println!("   Engagement variance: {:.6}", engagement_variance_5s.iter().sum::<f64>() / engagement_variance_5s.len() as f64);
    
    Ok(())
}
