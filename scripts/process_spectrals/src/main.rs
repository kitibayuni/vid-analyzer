use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

// --- Weights for engagement calculation ---
const W_CENTROID: f64 = 0.3;
const W_BANDWIDTH: f64 = 0.25;
const W_FLUX: f64 = 0.25;
const W_FLATNESS: f64 = 0.2;

// --- Utility functions ---
fn parse_numeric(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("nan") || t.eq_ignore_ascii_case("none") {
        0.0
    } else {
        t.parse::<f64>().unwrap_or(0.0)
    }
}

fn normalize(values: &[f64]) -> Vec<f64> {
    let finite: Vec<f64> = values.iter().cloned().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return vec![0.0; values.len()];
    }
    let min = finite.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = finite.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return vec![0.0; values.len()];
    }
    values
        .iter()
        .map(|&v| if v.is_finite() { (v - min) / (max - min) } else { 0.0 })
        .collect()
}

fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let mut result = Vec::with_capacity(values.len());
    let mut prev = values[0];
    result.push(prev);
    for &v in &values[1..] {
        let next = alpha * v + (1.0 - alpha) * prev;
        result.push(next);
        prev = next;
    }
    result
}

fn percentile_rank(values: &[f64]) -> Vec<f64> {
    let mut sorted: Vec<f64> = values
        .iter()
        .filter(|v| v.is_finite())
        .cloned()
        .collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    values
        .iter()
        .map(|&v| {
            let count = sorted.iter().filter(|&&x| x <= v).count();
            count as f64 / values.len() as f64
        })
        .collect()
}

// --- Main ---
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input.csv>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    let mut reader = ReaderBuilder::new().from_path(input_path)?;
    let headers = reader.headers()?.clone();
    let records: Vec<StringRecord> = reader.records().collect::<Result<_, _>>()?;

    // Find channels
    let mut channels = Vec::new();
    for h in headers.iter() {
        if let Some(ch) = h.strip_prefix("chan") {
            if ch.contains("_spectral_centroid") {
                let base = h.trim_end_matches("_spectral_centroid").to_string();
                channels.push(base);
            }
        }
    }
    if channels.is_empty() {
        eprintln!("No spectral feature channels found.");
        std::process::exit(1);
    }

    // Extract time
    let time_idx = headers
        .iter()
        .position(|h| h == "time_sec")
        .ok_or("Missing time_sec column")?;
    let time_values: Vec<f64> = records
        .iter()
        .map(|r| parse_numeric(r.get(time_idx).unwrap_or("0")))
        .collect();

    // Estimate sampling rate
    let avg_delta = time_values
        .windows(2)
        .map(|w| w[1] - w[0])
        .sum::<f64>()
        / (time_values.len() - 1) as f64;
    let rows_per_sec = (1.0 / avg_delta).round() as usize;

    let alpha_1s = 2.0 / (1.0 * rows_per_sec as f64 + 1.0);
    let alpha_5s = 2.0 / (5.0 * rows_per_sec as f64 + 1.0);
    let alpha_10s = 2.0 / (10.0 * rows_per_sec as f64 + 1.0);

    let mut results: HashMap<String, HashMap<String, Vec<f64>>> = HashMap::new();

    for ch in &channels {
        let mut feature_series: HashMap<String, Vec<f64>> = HashMap::new();

        // Extract raw features
        for feat in &["spectral_centroid", "spectral_bandwidth", "spectral_flux", "spectral_flatness"] {
            let col = format!("{}_{}", ch, feat);
            if let Some(idx) = headers.iter().position(|h| h == &col) {
                let vals: Vec<f64> = records
                    .iter()
                    .map(|r| parse_numeric(r.get(idx).unwrap_or("0")))
                    .collect();
                feature_series.insert(feat.trim_start_matches("spectral_").to_string(), vals);
            }
        }

        let n = time_values.len();

        // Normalize each feature
        for vals in feature_series.values_mut() {
            *vals = normalize(vals);
        }

        // Prepare fallbacks
        let empty_centroid = vec![0.0; n];
        let empty_bandwidth = vec![0.0; n];
        let empty_flux = vec![0.0; n];
        let empty_flatness = vec![0.0; n];

        let centroid = feature_series.get("centroid").unwrap_or(&empty_centroid);
        let bandwidth = feature_series.get("bandwidth").unwrap_or(&empty_bandwidth);
        let flux = feature_series.get("flux").unwrap_or(&empty_flux);
        let flatness = feature_series.get("flatness").unwrap_or(&empty_flatness);

        // Compute engagement score
        let engagement: Vec<f64> = (0..n)
            .map(|i| {
                W_CENTROID * centroid[i]
                    + W_BANDWIDTH * bandwidth[i]
                    + W_FLUX * flux[i]
                    + W_FLATNESS * flatness[i]
            })
            .collect();

        let mut channel_data: HashMap<String, Vec<f64>> = HashMap::new();

        // Save normalized features
        channel_data.insert("centroid".to_string(), centroid.clone());
        channel_data.insert("bandwidth".to_string(), bandwidth.clone());
        channel_data.insert("flux".to_string(), flux.clone());
        channel_data.insert("flatness".to_string(), flatness.clone());
        channel_data.insert("engagement".to_string(), engagement.clone());

        // For each feature + engagement: compute EMA + percentile
        let feats = vec!["centroid", "bandwidth", "flux", "flatness", "engagement"];
        for f in feats {
            if let Some(vals) = channel_data.get(f).cloned() {
                let ema1 = ema(&vals, alpha_1s);
                let ema5 = ema(&vals, alpha_5s);
                let ema10 = ema(&vals, alpha_10s);
                channel_data.insert(format!("{}_ema_1s", f), ema1.clone());
                channel_data.insert(format!("{}_ema_5s", f), ema5.clone());
                channel_data.insert(format!("{}_ema_10s", f), ema10.clone());
                channel_data.insert(format!("{}_ema_1s_pct", f), percentile_rank(&ema1));
                channel_data.insert(format!("{}_ema_5s_pct", f), percentile_rank(&ema5));
                channel_data.insert(format!("{}_ema_10s_pct", f), percentile_rank(&ema10));
            }
        }

        results.insert(ch.clone(), channel_data);
    }

    // Write output CSV
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_spectral_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut writer = WriterBuilder::new().from_path(&output_path)?;

    // Headers
    let mut out_headers = vec!["time_sec".to_string()];
    for ch in &channels {
        let feats = vec![
            "centroid",
            "bandwidth",
            "flux",
            "flatness",
            "engagement",
            "centroid_ema_1s",
            "centroid_ema_5s",
            "centroid_ema_10s",
            "centroid_ema_1s_pct",
            "centroid_ema_5s_pct",
            "centroid_ema_10s_pct",
            "bandwidth_ema_1s",
            "bandwidth_ema_5s",
            "bandwidth_ema_10s",
            "bandwidth_ema_1s_pct",
            "bandwidth_ema_5s_pct",
            "bandwidth_ema_10s_pct",
            "flux_ema_1s",
            "flux_ema_5s",
            "flux_ema_10s",
            "flux_ema_1s_pct",
            "flux_ema_5s_pct",
            "flux_ema_10s_pct",
            "flatness_ema_1s",
            "flatness_ema_5s",
            "flatness_ema_10s",
            "flatness_ema_1s_pct",
            "flatness_ema_5s_pct",
            "flatness_ema_10s_pct",
            "engagement_ema_1s",
            "engagement_ema_5s",
            "engagement_ema_10s",
            "engagement_ema_1s_pct",
            "engagement_ema_5s_pct",
            "engagement_ema_10s_pct",
        ];
        for f in feats {
            out_headers.push(format!("{}_{}", ch, f));
        }
    }
    writer.write_record(&out_headers)?;

    // Rows
    for i in 0..time_values.len() {
        let mut row = vec![format!("{:.6}", time_values[i])];
        for ch in &channels {
            if let Some(cd) = results.get(ch) {
                for key in &out_headers[1..] {
                    if key.starts_with(ch) {
                        let suffix = key.trim_start_matches(&format!("{}_", ch));
                        let val = cd.get(suffix).and_then(|v| v.get(i)).copied().unwrap_or(0.0);
                        row.push(format!("{:.6}", val));
                    }
                }
            }
        }
        writer.write_record(&row)?;
    }

    writer.flush()?;
    println!("Output written to: {}", output_path.display());
    Ok(())
}
