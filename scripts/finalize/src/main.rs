use csv::WriterBuilder;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::error::Error;

// Add ordered float for precise floating point comparisons
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat<T>(T);

impl<T: PartialOrd> Eq for OrderedFloat<T> {}
impl<T: PartialOrd> Ord for OrderedFloat<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[derive(Debug)]
struct CsvRow {
    data: HashMap<String, Option<f64>>,
}

// Weighted contributions
const WEIGHTS: &[(&str, f64)] = &[
    ("cat", 0.22),
    ("transc", 0.18),
    ("vocal_rms", 0.13),
    ("vocal_spectral", 0.10),
    ("nonvocal_spectral", 0.03),
    ("nonvocal_rms", 0.05),
    ("video", 0.25),
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 7 {
        eprintln!("Usage: {} --vocals <file> --nonvocals <file> --video <file>", args[0]);
        return Ok(());
    }

    let mut vocals_file = "";
    let mut nonvocals_file = "";
    let mut video_file = "";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--vocals" => vocals_file = &args[i + 1],
            "--nonvocals" => nonvocals_file = &args[i + 1],
            "--video" => video_file = &args[i + 1],
            _ => {}
        }
        i += 2;
    }

    let vocals_rows = load_csv(vocals_file)?;
    let nonvocals_rows = load_csv(nonvocals_file)?;
    let video_rows = load_csv(video_file)?;

    let vocal_features = process_vocal_features(&vocals_rows);
    let nonvocal_features = process_nonvocal_features(&nonvocals_rows);
    let video_features = process_video_features(&video_rows);

    let combined = combine_features_filtered(
        &vocals_rows,
        &vocal_features,
        &nonvocals_rows,
        &nonvocal_features,
        &video_rows,
        &video_features,
    );

    // Collect all column names
    let mut all_columns_set = BTreeSet::new();
    for row in &combined {
        for k in row.keys() {
            all_columns_set.insert(k.clone());
        }
    }
    let all_columns: Vec<String> = all_columns_set.into_iter().collect();

    // Write CSV
    let mut wtr = WriterBuilder::new().from_path("total_engagement.csv")?;
    wtr.write_record(&all_columns)?; // headers

    for row in combined {
        let record: Vec<String> = all_columns
            .iter()
            .map(|col| row.get(col).unwrap_or(&0.0).to_string())
            .collect();
        wtr.write_record(record)?;
    }
    wtr.flush()?;

    println!("CSV exported to total_engagement.csv");
    Ok(())
}

// Load CSV and convert empty/missing values to Option<f64>
fn load_csv(file_path: &str) -> Result<Vec<CsvRow>, Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new().from_path(file_path)?;
    let headers = rdr.headers()?.clone();
    let mut rows = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let mut data: HashMap<String, Option<f64>> = HashMap::new();
        for (i, field) in record.iter().enumerate() {
            let key = &headers[i];
            let val = if field.trim().is_empty() {
                None
            } else {
                match field.parse::<f64>() {
                    Ok(v) => Some(v),
                    Err(_) => None,
                }
            };
            data.insert(key.to_string(), val);
        }
        rows.push(CsvRow { data });
    }
    Ok(rows)
}

// Extract multi-channel vocal features (1s & 10s EMA pct included)
fn process_vocal_features(rows: &[CsvRow]) -> HashMap<String, Vec<f64>> {
    extract_features(rows, &[
        "cat_engage_ema_1s_pct",
        "cat_engage_ema_10s_pct",
        "transc_engage_ema_1s",
        "transc_engage_ema_10s",
        "rms_energy_engage_ema_1s_pct",
        "rms_energy_engage_ema_10s_pct",
        "spectral_engage_ema_1s_pct",
        "spectral_engage_ema_10s_pct",
    ])
}

// Extract multi-channel non-vocal features
fn process_nonvocal_features(rows: &[CsvRow]) -> HashMap<String, Vec<f64>> {
    extract_features(rows, &[
        "rms_energy_engage_ema_1s_pct",
        "rms_energy_engage_ema_10s_pct",
        "spectral_engage_ema_1s_pct",
        "spectral_engage_ema_10s_pct",
    ])
}

// Extract video features, keeping attention and visual_engage columns
fn process_video_features(rows: &[CsvRow]) -> HashMap<String, Vec<f64>> {
    let mut features: HashMap<String, Vec<f64>> = HashMap::new();
    for key in rows[0].data.keys() {
        if key.contains("attention") || key.contains("visual_engage") {
            let vals: Vec<f64> = rows
                .iter()
                .filter_map(|r| r.data.get(key).and_then(|v| *v))
                .collect();
            if !vals.is_empty() {
                features.insert(key.clone(), interpolate_nans(&vals));
            }
        }
    }
    features
}

// Generic extraction with multi-channel averaging
fn extract_features(rows: &[CsvRow], keys: &[&str]) -> HashMap<String, Vec<f64>> {
    let mut features = HashMap::new();
    for key in keys {
        let vals: Vec<f64> = rows
            .iter()
            .map(|row| {
                let channel_vals = row
                    .data
                    .iter()
                    .filter(|(k, _)| k.contains(key))
                    .filter_map(|(_, v)| *v)
                    .filter(|v| *v != 0.0)
                    .collect::<Vec<f64>>();
                if !channel_vals.is_empty() {
                    channel_vals.iter().sum::<f64>() / channel_vals.len() as f64
                } else {
                    f64::NAN
                }
            })
            .collect();
        features.insert(key.to_string(), interpolate_nans(&vals));
    }
    features
}

// Linear interpolation for NaNs
fn interpolate_nans(vals: &[f64]) -> Vec<f64> {
    let mut out = vals.to_vec();
    let n = out.len();
    for i in 0..n {
        if out[i].is_nan() {
            let prev = (0..i).rev().find(|&j| !out[j].is_nan());
            let next = (i + 1..n).find(|&j| !out[j].is_nan());
            out[i] = match (prev, next) {
                (Some(p), Some(nxt)) => out[p] + (out[nxt] - out[p]) * ((i - p) as f64 / (nxt - p) as f64),
                (Some(p), None) => out[p],
                (None, Some(nxt)) => out[nxt],
                _ => 0.0,
            };
        }
    }
    out
}

// Combine features but only include engage EMA pct columns and attention data
fn combine_features_filtered(
    vocals_rows: &[CsvRow],
    vocal_features: &HashMap<String, Vec<f64>>,
    nonvocals_rows: &[CsvRow],
    nonvocal_features: &HashMap<String, Vec<f64>>,
    video_rows: &[CsvRow],
    video_features: &HashMap<String, Vec<f64>>,
) -> Vec<HashMap<String, f64>> {
    // Create time-indexed maps for each dataset
    let vocals_by_time = create_time_index(vocals_rows);
    let nonvocals_by_time = create_time_index(nonvocals_rows);
    let video_by_time = create_time_index(video_rows);

    // Get all unique timestamps
    let mut all_times: BTreeSet<OrderedFloat<f64>> = BTreeSet::new();
    all_times.extend(vocals_by_time.keys().copied());
    all_times.extend(nonvocals_by_time.keys().copied());
    all_times.extend(video_by_time.keys().copied());

    let mut combined = Vec::new();

    for time_sec in all_times {
        let mut row = HashMap::new();
        
        // Add time_sec to the row
        row.insert("time_sec".to_string(), time_sec.0);

        // Process vocals data for this timestamp
        if let Some(vocal_row) = vocals_by_time.get(&time_sec) {
            for (k, v) in &vocal_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") {
                        row.insert(k.clone(), val);
                    }
                }
            }
        }

        // Process nonvocals data for this timestamp
        if let Some(nonvocal_row) = nonvocals_by_time.get(&time_sec) {
            for (k, v) in &nonvocal_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") {
                        row.insert(k.clone(), val);
                    }
                }
            }
        }

        // Process video data for this timestamp
        if let Some(video_row) = video_by_time.get(&time_sec) {
            for (k, v) in &video_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") {
                        row.insert(k.clone(), val);
                    }
                }
            }
        }

        // Calculate weighted engagement scores
        let vocal_sum: f64 = WEIGHTS
            .iter()
            .filter(|(k, _)| ["cat", "transc", "vocal_rms", "vocal_spectral"].contains(k))
            .map(|(k, w)| {
                get_feature_value_at_time(time_sec.0, k, vocal_features, &vocals_by_time) * w
            })
            .sum();

        let nonvocal_sum: f64 = WEIGHTS
            .iter()
            .filter(|(k, _)| ["nonvocal_rms", "nonvocal_spectral"].contains(k))
            .map(|(k, w)| {
                get_feature_value_at_time(time_sec.0, k, nonvocal_features, &nonvocals_by_time) * w
            })
            .sum();

        let video_sum: f64 = WEIGHTS
            .iter()
            .filter(|(k, _)| *k == "video")
            .map(|(_, w)| {
                get_video_feature_value_at_time(time_sec.0, video_features, &video_by_time) * w
            })
            .sum();

        let total = vocal_sum + nonvocal_sum + video_sum;
        row.insert("totalengagement".to_string(), total);

        // Calculate EMA percentiles based on available data at this timestamp
        let ema_1s = calculate_ema_at_time(time_sec.0, "_ema_1s_pct", &vocals_by_time, &nonvocals_by_time, &video_by_time);
        let ema_10s = calculate_ema_at_time(time_sec.0, "_ema_10s_pct", &vocals_by_time, &nonvocals_by_time, &video_by_time);
        
        row.insert("totalengagement_ema_1s_pct".to_string(), ema_1s);
        row.insert("totalengagement_ema_10s_pct".to_string(), ema_10s);

        combined.push(row);
    }

    combined
}

// Create a time-indexed map from CSV rows
fn create_time_index(rows: &[CsvRow]) -> std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow> {
    let mut time_index = std::collections::BTreeMap::new();
    
    for row in rows {
        if let Some(Some(time_val)) = row.data.get("time_sec") {
            let time_key = OrderedFloat(*time_val);
            time_index.insert(time_key, row);
        }
    }
    
    time_index
}

// Get feature value at specific time
fn get_feature_value_at_time(
    time_sec: f64,
    feature_key: &str,
    features: &HashMap<String, Vec<f64>>,
    time_index: &std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow>
) -> f64 {
    // Find the matching feature key
    if let Some(feature_name) = features.keys().find(|x| x.contains(feature_key)) {
        if let Some(feature_vec) = features.get(feature_name) {
            // Find the index of this timestamp in the original data
            if let Some(row_index) = time_index.keys().position(|t| t.0 == time_sec) {
                return feature_vec.get(row_index).copied().unwrap_or(0.0);
            }
        }
    }
    0.0
}

// Get video feature value at specific time
fn get_video_feature_value_at_time(
    time_sec: f64,
    features: &HashMap<String, Vec<f64>>,
    time_index: &std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow>
) -> f64 {
    if let Some(row_index) = time_index.keys().position(|t| t.0 == time_sec) {
        let sum: f64 = features.values()
            .map(|vec| vec.get(row_index).copied().unwrap_or(0.0))
            .sum();
        sum
    } else {
        0.0
    }
}

// Calculate EMA at specific time
fn calculate_ema_at_time(
    time_sec: f64,
    ema_suffix: &str,
    vocals_index: &std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow>,
    nonvocals_index: &std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow>,
    video_index: &std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow>
) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;
    let time_key = OrderedFloat(time_sec);

    // Check vocals
    if let Some(row) = vocals_index.get(&time_key) {
        for (k, v) in &row.data {
            if k.contains("engage") && k.ends_with(ema_suffix) {
                if let Some(val) = *v {
                    sum += val;
                    count += 1;
                }
            }
        }
    }

    // Check nonvocals
    if let Some(row) = nonvocals_index.get(&time_key) {
        for (k, v) in &row.data {
            if k.contains("engage") && k.ends_with(ema_suffix) {
                if let Some(val) = *v {
                    sum += val;
                    count += 1;
                }
            }
        }
    }

    // Check video
    if let Some(row) = video_index.get(&time_key) {
        for (k, v) in &row.data {
            if k.contains("engage") && k.ends_with(ema_suffix) {
                if let Some(val) = *v {
                    sum += val;
                    count += 1;
                }
            }
        }
    }

    if count > 0 { sum / count as f64 } else { 0.0 }
}