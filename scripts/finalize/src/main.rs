use csv::WriterBuilder;
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex};

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

// Weighted contributions for total_engag_raw
const WEIGHTS_RAW: &[(&str, f64)] = &[
    ("emotion_engage", 0.22),
    ("transc", 0.18),
    ("rms_energy_vocal", 0.13),
    ("spectral_vocal", 0.10),
    ("spectral_nonvocal", 0.03),
    ("rms_energy_nonvocal", 0.05),
    ("video", 0.25),
];

// Legacy weights removed - no longer using totalengagement calculation

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

    let mut combined = combine_features_filtered(
        &vocals_rows,
        &vocal_features,
        &nonvocals_rows,
        &nonvocal_features,
        &video_rows,
        &video_features,
    );

    scale_0_1_to_0_100(&mut combined);

    // Apply finalization (interpolation to 30 fps)
    let finalized = finalize_data(combined);

    // Collect all column names
    let mut all_columns_set = BTreeSet::new();
    for row in &finalized {
        for k in row.keys() {
            all_columns_set.insert(k.clone());
        }
    }
    let all_columns: Vec<String> = all_columns_set.into_iter().collect();

    // Write CSV
    let mut wtr = WriterBuilder::new().from_path("total_engagement.csv")?;
    wtr.write_record(&all_columns)?; // headers

    for row in finalized {
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
    let mut raw = extract_features(rows, &[
        "rms_energy_engage_ema_1s_pct",
        "rms_energy_engage_ema_10s_pct",
        "spectral_engage_ema_1s_pct",
        "spectral_engage_ema_10s_pct",
    ]);

    // Add "nonvocal_" prefix to each feature
    let mut renamed = HashMap::new();
    for (k, v) in raw.drain() {
        renamed.insert(format!("nonvocal_{}", k), v);
    }
    renamed
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

// Helper function to get channel-averaged value for a specific pattern
fn get_channel_averaged_value(row: &HashMap<String, f64>, pattern: &str, suffix: &str) -> f64 {
    let chan1_key = format!("chan1_{}{}", pattern, suffix);
    let chan2_key = format!("chan2_{}{}", pattern, suffix);
    
    let chan1_val = row.get(&chan1_key).copied().unwrap_or(0.0);
    let chan2_val = row.get(&chan2_key).copied().unwrap_or(0.0);
    
    // If one channel has 0, use the other
    if chan1_val == 0.0 && chan2_val != 0.0 {
        chan2_val
    } else if chan2_val == 0.0 && chan1_val != 0.0 {
        chan1_val
    } else if chan1_val != 0.0 && chan2_val != 0.0 {
        // Average both channels
        (chan1_val + chan2_val) / 2.0
    } else {
        0.0
    }
}

// Calculate total_engag_raw using weighted 1s percentiles (parallelized)
fn calculate_total_engag_raw(combined: &mut Vec<HashMap<String, f64>>) {
    combined.par_iter_mut().for_each(|row| {
        let mut weighted_sum = 0.0;
        
        // Emotion engage (22%) - using emotion_engage_1s_pct
        if let Some(val) = row.get("emotion_engage_1s_pct") {
            weighted_sum += val * WEIGHTS_RAW[0].1; // 0.22
        }
        
        // Transcription engage (18%) - need to find transc columns with 1s
        let transc_val = row.keys()
            .filter(|k| k.contains("transc") && k.contains("1s"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += transc_val * WEIGHTS_RAW[1].1; // 0.18
        
        // RMS Energy Vocal (13%) - average chan1 and chan2 rms_energy 1s pct
        let rms_vocal = get_channel_averaged_value(row, "rms_energy_engage_ema_", "1s_pct");
        weighted_sum += rms_vocal * WEIGHTS_RAW[2].1; // 0.13
        
        // Spectral Vocal (10%) - average chan1 and chan2 spectral 1s pct
        let spectral_vocal = get_channel_averaged_value(row, "spectral_engage_ema_", "1s_pct");
        weighted_sum += spectral_vocal * WEIGHTS_RAW[3].1; // 0.10
        
        // Spectral Nonvocal (3%) - need nonvocal spectral 1s pct
        let spectral_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("spectral") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += spectral_nonvocal * WEIGHTS_RAW[4].1; // 0.03
        
        // RMS Energy Nonvocal (5%) - need nonvocal rms 1s pct
        let rms_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("rms_energy") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += rms_nonvocal * WEIGHTS_RAW[5].1; // 0.05
        
        // Video (25%) - using visual_engage_ema_1s_pct
        if let Some(val) = row.get("visual_engage_ema_1s_pct") {
            weighted_sum += val * WEIGHTS_RAW[6].1; // 0.25
        }
        
        row.insert("total_engag_raw".to_string(), weighted_sum);
    });
}

// Calculate EMAs for total_engag_raw (1s, 5s, 10s, 30s percentiles)
fn calculate_total_engag_raw_emas(combined: &mut Vec<HashMap<String, f64>>) {
    let raw_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("total_engag_raw").copied().unwrap_or(0.0))
        .collect();
    
    // Debug: Check if we have non-zero raw values
    let non_zero_count = raw_values.iter().filter(|&&x| x != 0.0).count();
    println!("Debug: {} non-zero total_engag_raw values out of {}", non_zero_count, raw_values.len());
    
    // Calculate EMAs first (raw values)
    let ema_1s = calculate_ema_raw(&raw_values, 1.0);
    let ema_5s = calculate_ema_raw(&raw_values, 5.0);
    let ema_10s = calculate_ema_raw(&raw_values, 10.0);
    let ema_30s = calculate_ema_raw(&raw_values, 30.0);
    
    // Convert each EMA series to percentiles (0.0 to 100.0 scale)
    let ema_1s_pct = convert_to_percentiles(&ema_1s);
    let ema_5s_pct = convert_to_percentiles(&ema_5s);
    let ema_10s_pct = convert_to_percentiles(&ema_10s);
    let ema_30s_pct = convert_to_percentiles(&ema_30s);
    
    // Insert the calculated percentile values
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("total_engag_1s_pct".to_string(), ema_1s_pct[i]);
        row.insert("total_engag_5s_pct".to_string(), ema_5s_pct[i]);
        row.insert("total_engag_10s_pct".to_string(), ema_10s_pct[i]);
        row.insert("total_engag_30s_pct".to_string(), ema_30s_pct[i]);
    }
}

// Combine features but only include engage EMA pct columns and attention data
fn combine_features_filtered(
    vocals_rows: &[CsvRow],
    _vocal_features: &HashMap<String, Vec<f64>>,
    nonvocals_rows: &[CsvRow],
    _nonvocal_features: &HashMap<String, Vec<f64>>,
    video_rows: &[CsvRow],
    _video_features: &HashMap<String, Vec<f64>>,
) -> Vec<HashMap<String, f64>> {
    let vocals_by_time = create_time_index(vocals_rows);
    let nonvocals_by_time = create_time_index(nonvocals_rows);
    let video_by_time = create_time_index(video_rows);

    let mut all_times: BTreeSet<OrderedFloat<f64>> = BTreeSet::new();
    all_times.extend(vocals_by_time.keys().copied());
    all_times.extend(nonvocals_by_time.keys().copied());
    all_times.extend(video_by_time.keys().copied());

    let mut combined = Vec::new();

    for time_sec in all_times {
        let mut row = HashMap::new();
        row.insert("time_sec".to_string(), time_sec.0);

        // Process vocals data
        if let Some(vocal_row) = vocals_by_time.get(&time_sec) {
            for (k, v) in &vocal_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") || k.contains("cat_engage_percentile") {
                        row.insert(k.clone(), val);
                    }
                }
            }
        }

        // Process nonvocals data
        if let Some(nonvocal_row) = nonvocals_by_time.get(&time_sec) {
            for (k, v) in &nonvocal_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") || k.contains("cat_engage_percentile") {
                        row.insert(format!("nonvocal_{}", k), val);
                    }
                }
            }
        }

        // Process video data
        if let Some(video_row) = video_by_time.get(&time_sec) {
            for (k, v) in &video_row.data {
                if let Some(val) = *v {
                    if (k.contains("engage_ema") && k.contains("pct")) || k.contains("attention") || k.contains("cat_engage_percentile") {
                        row.insert(k.clone(), val);
                    }
                }
            }
        }

        combined.push(row);
    }

    // Debug: Check what we have after initial combination
    debug_available_columns(&combined);

    // Step 1: Apply interpolation and basic EMAs first
    apply_linear_interpolation_and_emas(&mut combined);
    
    // Debug: Check what we have after EMAs
    println!("Debug: After EMAs, checking for required columns...");
    debug_available_columns(&combined);
    
    // Step 2: Calculate total_engag_raw (depends on existing percentile columns)
    calculate_total_engag_raw(&mut combined);
    
    // Step 3: Calculate total_engag percentiles LAST (depends on total_engag_raw)
    calculate_total_engag_raw_emas(&mut combined);

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

// Finalize data by interpolating to 30 rows per second (30 fps) with parallelization
fn finalize_data(mut data: Vec<HashMap<String, f64>>) -> Vec<HashMap<String, f64>> {
    if data.is_empty() {
        return data;
    }
    
    // Sort by time_sec to ensure proper ordering
    data.sort_by(|a, b| {
        let time_a = a.get("time_sec").copied().unwrap_or(0.0);
        let time_b = b.get("time_sec").copied().unwrap_or(0.0);
        time_a.partial_cmp(&time_b).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    let first_time = data.first().unwrap().get("time_sec").copied().unwrap_or(0.0);
    let last_time = data.last().unwrap().get("time_sec").copied().unwrap_or(0.0);
    
    // Create 30fps timeline (1/30 second intervals)
    let fps = 30.0;
    let time_step = 1.0 / fps;
    let mut target_times = Vec::new();
    
    let mut current_time = first_time;
    while current_time <= last_time {
        target_times.push(current_time);
        current_time += time_step;
    }
    
    // Get all column names except time_sec
    let mut all_columns = std::collections::HashSet::new();
    for row in &data {
        for key in row.keys() {
            if key != "time_sec" {
                all_columns.insert(key.clone());
            }
        }
    }
    let columns: Vec<String> = all_columns.into_iter().collect();
    
    // Extract original time series once for efficiency
    let original_times: Vec<f64> = data.iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    // Pre-compute column data for parallel access
    let column_data: HashMap<String, Vec<f64>> = columns.par_iter()
        .map(|column| {
            let values: Vec<f64> = data.iter()
                .map(|row| row.get(column).copied().unwrap_or(0.0))
                .collect();
            (column.clone(), values)
        })
        .collect();
    
    // Build interpolated data in parallel
    let interpolated_data: Vec<HashMap<String, f64>> = target_times.par_iter()
        .map(|&target_time| {
            let mut new_row = HashMap::new();
            new_row.insert("time_sec".to_string(), target_time);
            
            // Interpolate each column
            for column in &columns {
                if let Some(values) = column_data.get(column) {
                    let interpolated_value = interpolate_at_time(&original_times, values, target_time);
                    new_row.insert(column.clone(), interpolated_value);
                }
            }
            
            new_row
        })
        .collect();
    
    interpolated_data
}

// Efficiently interpolate a single value at a specific time using linear interpolation
fn interpolate_at_time(times: &[f64], values: &[f64], target_time: f64) -> f64 {
    if times.is_empty() || values.is_empty() || times.len() != values.len() {
        return 0.0;
    }
    
    // Handle edge cases
    if target_time <= times[0] {
        return values[0];
    }
    if target_time >= times[times.len() - 1] {
        return values[values.len() - 1];
    }
    
    // Binary search for the correct interval (more efficient than linear search)
    let mut left = 0;
    let mut right = times.len() - 1;
    
    while left < right - 1 {
        let mid = (left + right) / 2;
        if times[mid] <= target_time {
            left = mid;
        } else {
            right = mid;
        }
    }
    
    // Linear interpolation between times[left] and times[right]
    let t0 = times[left];
    let t1 = times[right];
    let v0 = values[left];
    let v1 = values[right];
    
    if (t1 - t0).abs() < f64::EPSILON {
        // Avoid division by zero
        return v0;
    }
    
    let ratio = (target_time - t0) / (t1 - t0);
    v0 + ratio * (v1 - v0)
}

// Functions get_feature_value_at_time, get_video_feature_value_at_time, and calculate_ema_at_time 
// have been removed as they were only used for the old totalengagement calculation

// Apply linear interpolation to all columns and calculate EMAs (parallelized)
fn apply_linear_interpolation_and_emas(combined: &mut Vec<HashMap<String, f64>>) {
    if combined.is_empty() {
        return;
    }
    
    // Collect all column names except time_sec
    let mut all_columns = std::collections::HashSet::new();
    for row in combined.iter() {
        for key in row.keys() {
            if key != "time_sec" {
                all_columns.insert(key.clone());
            }
        }
    }
    
    // Convert to vector for parallel processing
    let columns: Vec<String> = all_columns.into_iter().collect();
    
    // Apply linear interpolation to each column in parallel
    let combined_arc = Arc::new(Mutex::new(combined));
    
    columns.par_iter().for_each(|column_name| {
        // Extract values for this column
        let values: Vec<Option<f64>> = {
            let combined_guard = combined_arc.lock().unwrap();
            combined_guard
                .iter()
                .map(|row| row.get(column_name).copied())
                .collect()
        };
        
        // Apply linear interpolation
        let mut interpolated_values = values;
        interpolate_column_values(&mut interpolated_values);
        
        // Update the combined data with interpolated values
        let mut combined_guard = combined_arc.lock().unwrap();
        for (i, interpolated_value) in interpolated_values.iter().enumerate() {
            if let Some(val) = interpolated_value {
                combined_guard[i].insert(column_name.clone(), *val);
            }
        }
    });
    
    // Drop the Arc wrapper to get back our mutable reference
    let combined = Arc::try_unwrap(combined_arc).unwrap().into_inner().unwrap();
    
    // Calculate EMAs for attention columns
    calculate_attention_emas(combined);
    
    // Calculate emotion engagement EMAs from cat_engage_percentile
    calculate_emotion_engagement_emas(combined);
}

// Linear interpolation for a single column
fn interpolate_column_values(values: &mut Vec<Option<f64>>) {
    let n = values.len();
    
    // Forward fill from first non-None value
    let mut first_valid = None;
    for i in 0..n {
        if values[i].is_some() {
            first_valid = Some(i);
            break;
        }
    }
    
    if let Some(first_idx) = first_valid {
        // Fill everything before first valid value
        let first_val = values[first_idx].unwrap();
        for i in 0..first_idx {
            values[i] = Some(first_val);
        }
    }
    
    // Backward fill from last non-None value
    let mut last_valid = None;
    for i in (0..n).rev() {
        if values[i].is_some() {
            last_valid = Some(i);
            break;
        }
    }
    
    if let Some(last_idx) = last_valid {
        let last_val = values[last_idx].unwrap();
        for i in (last_idx + 1)..n {
            values[i] = Some(last_val);
        }
    }
    
    // Linear interpolation for gaps in the middle
    for i in 0..n {
        if values[i].is_none() {
            // Find previous and next valid values
            let prev = (0..i).rev().find(|&j| values[j].is_some());
            let next = (i + 1..n).find(|&j| values[j].is_some());
            
            if let (Some(p), Some(nxt)) = (prev, next) {
                let prev_val = values[p].unwrap();
                let next_val = values[nxt].unwrap();
                let ratio = (i - p) as f64 / (nxt - p) as f64;
                values[i] = Some(prev_val + (next_val - prev_val) * ratio);
            }
        }
    }
}

// Calculate EMA percentiles for attention columns
fn calculate_attention_emas(combined: &mut Vec<HashMap<String, f64>>) {
    // Find all attention column names
    let attention_columns: Vec<String> = combined
        .iter()
        .flat_map(|row| row.keys().cloned())
        .filter(|key| key.contains("attention"))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    
    for attention_col in attention_columns {
        let values: Vec<f64> = combined
            .iter()
            .map(|row| row.get(&attention_col).copied().unwrap_or(0.0))
            .collect();
        
        let ema_1s_pct = calculate_ema_percentiles(&values, 1.0);
        let ema_10s_pct = calculate_ema_percentiles(&values, 10.0);
        
        let col_1s = format!("{}_ema_1s_pct", attention_col);
        let col_10s = format!("{}_ema_10s_pct", attention_col);
        
        for (i, row) in combined.iter_mut().enumerate() {
            row.insert(col_1s.clone(), ema_1s_pct[i]);
            row.insert(col_10s.clone(), ema_10s_pct[i]);
        }
    }
}

// Calculate emotion engagement EMAs from cat_engage_percentile
fn calculate_emotion_engagement_emas(combined: &mut Vec<HashMap<String, f64>>) {
    // Find cat_engage_percentile values
    let cat_values: Vec<f64> = combined
        .iter()
        .map(|row| {
            row.keys()
                .find(|key| key.contains("cat_engage_percentile"))
                .and_then(|key| row.get(key))
                .copied()
                .unwrap_or(0.0)
        })
        .collect();
    
    let ema_1s = calculate_ema_raw(&cat_values, 1.0);
    let ema_10s = calculate_ema_raw(&cat_values, 10.0);
    
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("emotion_engage_1s_pct".to_string(), ema_1s[i]);
        row.insert("emotion_engage_10s_pct".to_string(), ema_10s[i]);
    }
}

// Calculate EMA percentiles (for attention columns and total_engag_raw)
fn calculate_ema_percentiles(values: &[f64], window_seconds: f64) -> Vec<f64> {
    let alpha = 2.0 / (window_seconds + 1.0);
    let mut ema_values = vec![0.0; values.len()];
    
    if !values.is_empty() {
        ema_values[0] = values[0];
        for i in 1..values.len() {
            ema_values[i] = alpha * values[i] + (1.0 - alpha) * ema_values[i - 1];
        }
    }
    
    // Use the new percentile conversion function for proper 0.0-100.0 scaling
    convert_to_percentiles(&ema_values)
}

// Calculate raw EMA (for emotion engagement)
fn calculate_ema_raw(values: &[f64], window_seconds: f64) -> Vec<f64> {
    let alpha = 2.0 / (window_seconds + 1.0);
    let mut ema_values = vec![0.0; values.len()];
    
    if !values.is_empty() {
        ema_values[0] = values[0];
        for i in 1..values.len() {
            ema_values[i] = alpha * values[i] + (1.0 - alpha) * ema_values[i - 1];
        }
    }
    
    ema_values
}

fn time_weighted_ema(values: &[f64], times: &[f64], tau: f64) -> Vec<f64> {
    let mut ema = vec![0.0; values.len()];
    if values.is_empty() || times.is_empty() || values.len() != times.len() {
        return ema;
    }
    
    ema[0] = values[0]; // start EMA at first value
    for i in 1..values.len() {
        let dt = times[i] - times[i - 1];
        let alpha = 1.0 - (-dt / tau).exp(); // continuous-time EMA
        ema[i] = alpha * values[i] + (1.0 - alpha) * ema[i - 1];
    }
    ema
}

fn convert_to_percentiles(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    
    // Find min and max values for normalization
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    // Handle edge case where all values are the same
    if (max_val - min_val).abs() < f64::EPSILON {
        return vec![50.0; values.len()]; // Return 50% if no variation
    }
    
    // Scale to 0.0-100.0 range
    values.iter()
        .map(|&val| ((val - min_val) / (max_val - min_val)) * 100.0)
        .collect()
}

fn debug_available_columns(combined: &[HashMap<String, f64>]) {
    if let Some(first_row) = combined.first() {
        let mut columns: Vec<String> = first_row.keys().cloned().collect();
        columns.sort();
        
        println!("Debug: Available columns ({} total):", columns.len());
        for (i, col) in columns.iter().enumerate() {
            if i < 20 { // Show first 20 columns
                let sample_val = first_row.get(col).copied().unwrap_or(0.0);
                println!("  {}: {}", col, sample_val);
            }
        }
        
        // Look for specific engagement patterns
        let engagement_cols: Vec<&String> = columns.iter()
            .filter(|col| col.contains("engage") || col.contains("attention") || col.contains("visual"))
            .collect();
        
        println!("Debug: Engagement-related columns ({}):", engagement_cols.len());
        for col in engagement_cols.iter().take(10) {
            let sample_val = first_row.get(*col).copied().unwrap_or(0.0);
            println!("  {}: {}", col, sample_val);
        }
    }
}

fn scale_0_1_to_0_100(combined: &mut Vec<HashMap<String, f64>>) {
    if combined.is_empty() {
        return;
    }
    
    // Get all column names
    let column_names: Vec<String> = combined[0].keys().cloned().collect();
    
    for column_name in column_names {
        // Skip time_sec and any column containing "attention"
        if column_name == "time_sec" || column_name.contains("attention") {
            continue;
        }
        
        // Extract all values for this column
        let values: Vec<f64> = combined.iter()
            .map(|row| row.get(&column_name).copied().unwrap_or(0.0))
            .collect();
        
        // Check if column is in 0-1 range (allowing small tolerance for floating point errors)
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        
        // If values are in 0-1 range (with small tolerance), scale to 0-100
        if min_val >= -0.001 && max_val <= 1.001 && max_val > 0.001 {
            println!(
                "Scaling column '{}' from 0-1 to 0-100 range (min: {}, max: {})", 
                column_name, min_val, max_val
            );
            
            for row in combined.iter_mut() {
                if let Some(value) = row.get_mut(&column_name) {
                    *value = (*value).clamp(0.0, 1.0) * 100.0;
                }
            }
        }
    }
}
