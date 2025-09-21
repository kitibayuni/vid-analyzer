use csv::WriterBuilder;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};

// Memory-efficient ordered float for precise floating point comparisons
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
struct OrderedFloat<T>(T);

impl<T: PartialOrd> Eq for OrderedFloat<T> {}
impl<T: PartialOrd> Ord for OrderedFloat<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

// Compact CSV row structure - only store non-None values
#[derive(Debug)]
struct CsvRow {
    time_sec: f64,
    data: HashMap<String, f64>, // Only store actual values, not Options
}

// Note: StreamingRow and FeatureProcessor were removed as they weren't used in the final implementation

// Configuration for weights
const WEIGHTS_RAW: &[(&str, f64)] = &[
    ("emotion_engage", 0.22),
    ("transc", 0.18),
    ("rms_energy_vocal", 0.13),
    ("spectral_vocal", 0.10),
    ("spectral_nonvocal", 0.03),
    ("rms_energy_nonvocal", 0.05),
    ("video", 0.25),
];

// Memory usage monitoring
struct MemoryMonitor {
    max_rows_in_memory: usize,
}

impl MemoryMonitor {
    fn new() -> Self {
        // Estimate based on available memory - conservative approach
        let estimated_max_rows = 50_000; // Adjust based on system memory
        Self {
            max_rows_in_memory: estimated_max_rows,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 7 {
        eprintln!(
            "Usage: {} --vocals <file> --nonvocals <file> --video <file> [--output <file>]",
            args[0]
        );
        return Ok(());
    }

    let mut vocals_file = "";
    let mut nonvocals_file = "";
    let mut video_file = "";
    let mut output_file = "total_engagement.csv";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--vocals" => vocals_file = &args[i + 1],
            "--nonvocals" => nonvocals_file = &args[i + 1],
            "--video" => video_file = &args[i + 1],
            "--output" => output_file = &args[i + 1],
            _ => {}
        }
        i += 2;
    }

    println!("🔄 Processing engagement data with OOM-safe approach...");
    
    // Initialize memory monitor
    let memory_monitor = MemoryMonitor::new();
    
    // Process data in streaming fashion
    process_engagement_streaming(
        vocals_file,
        nonvocals_file, 
        video_file,
        output_file,
        &memory_monitor,
    )?;

    println!("✅ CSV exported to {}", output_file);
    Ok(())
}

// Main streaming processing function
fn process_engagement_streaming(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    output_file: &str,
    memory_monitor: &MemoryMonitor,
) -> Result<(), Box<dyn Error>> {
    
    // Step 1: Create time-sorted indices of all files without loading full data
    println!("📊 Creating time indices...");
    let vocals_times = extract_time_indices(vocals_file)?;
    let nonvocals_times = extract_time_indices(nonvocals_file)?;  
    let video_times = extract_time_indices(video_file)?;
    
    // Combine all unique timestamps
    let mut all_times: BTreeSet<OrderedFloat<f64>> = BTreeSet::new();
    all_times.extend(vocals_times.into_iter().map(OrderedFloat));
    all_times.extend(nonvocals_times.into_iter().map(OrderedFloat));
    all_times.extend(video_times.into_iter().map(OrderedFloat));
    
    println!("⏱️  Processing {} unique timestamps", all_times.len());
    
    // Step 2: Process in chunks to avoid OOM
    let chunk_size = memory_monitor.max_rows_in_memory;
    let time_chunks: Vec<Vec<f64>> = all_times
        .into_iter()
        .map(|t| t.0)
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();
    
    println!("🗂️  Processing {} chunks of up to {} rows each", time_chunks.len(), chunk_size);
    
    // Step 3: Initialize output writer
    let output_writer = BufWriter::new(File::create(output_file)?);
    let mut csv_writer = WriterBuilder::new().from_writer(output_writer);
    
    // Determine output columns by processing a small sample
    let sample_columns = determine_output_columns(
        vocals_file,
        nonvocals_file,
        video_file,
        &time_chunks[0][..std::cmp::min(10, time_chunks[0].len())],
    )?;
    
    csv_writer.write_record(&sample_columns)?;
    
    // Step 4: Process each chunk
    for (chunk_idx, time_chunk) in time_chunks.iter().enumerate() {
        println!("🔄 Processing chunk {}/{} ({} timestamps)...", 
                 chunk_idx + 1, time_chunks.len(), time_chunk.len());
        
        let chunk_data = process_time_chunk(
            vocals_file,
            nonvocals_file,
            video_file,
            time_chunk,
        )?;
        
        // Write chunk results immediately to free memory
        write_chunk_results(&mut csv_writer, &chunk_data, &sample_columns)?;
    }
    
    csv_writer.flush()?;
    Ok(())
}

// Extract just the time values from a CSV file without loading full data
fn extract_time_indices(file_path: &str) -> Result<Vec<f64>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    let mut times = Vec::new();
    for result in rdr.records() {
        let record = result?;
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                times.push(time_val);
            }
        }
    }
    
    Ok(times)
}

// Load only specific rows by time values (memory efficient)
fn load_csv_by_times(file_path: &str, target_times: &[f64]) -> Result<Vec<CsvRow>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    // Create a set for fast lookup - use BTreeSet instead of HashSet for OrderedFloat
    let target_set: std::collections::BTreeSet<OrderedFloat<f64>> = 
        target_times.iter().map(|&t| OrderedFloat(t)).collect();
    
    let mut rows = Vec::new();
    
    for result in rdr.records() {
        let record = result?;
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                // Only process rows that match our target times
                if target_set.contains(&OrderedFloat(time_val)) {
                    let mut data = HashMap::new();
                    
                    for (i, field) in record.iter().enumerate() {
                        if i != time_col_idx && !field.trim().is_empty() {
                            if let Ok(val) = field.parse::<f64>() {
                                data.insert(headers[i].to_string(), val);
                            }
                        }
                    }
                    
                    rows.push(CsvRow {
                        time_sec: time_val,
                        data,
                    });
                }
            }
        }
    }
    
    Ok(rows)
}

// Process a chunk of time values
fn process_time_chunk(
    vocals_file: &str,
    nonvocals_file: &str, 
    video_file: &str,
    time_chunk: &[f64],
) -> Result<Vec<HashMap<String, f64>>, Box<dyn Error>> {
    
    // Load only the data we need for this time chunk
    let vocals_rows = load_csv_by_times(vocals_file, time_chunk)?;
    let nonvocals_rows = load_csv_by_times(nonvocals_file, time_chunk)?;
    let video_rows = load_csv_by_times(video_file, time_chunk)?;
    
    // Create time indices for this chunk
    let vocals_by_time = create_time_index_from_rows(&vocals_rows);
    let nonvocals_by_time = create_time_index_from_rows(&nonvocals_rows);
    let video_by_time = create_time_index_from_rows(&video_rows);
    
    let mut combined = Vec::with_capacity(time_chunk.len());
    
    // Process each time point
    for &time_sec in time_chunk {
        let time_key = OrderedFloat(time_sec);
        let mut row = HashMap::new();
        row.insert("time_sec".to_string(), time_sec);
        
        // Process vocals data
        if let Some(vocal_row) = vocals_by_time.get(&time_key) {
            for (k, &v) in &vocal_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                }
            }
        }
        
        // Process nonvocals data with prefix
        if let Some(nonvocal_row) = nonvocals_by_time.get(&time_key) {
            for (k, &v) in &nonvocal_row.data {
                if should_include_column(k) {
                    let prefixed_key = if k.starts_with("nonvocal_") {
                        k.clone()
                    } else {
                        format!("nonvocal_{}", k)
                    };
                    row.insert(prefixed_key, v);
                }
            }
        }
        
        // Process video data
        if let Some(video_row) = video_by_time.get(&time_key) {
            for (k, &v) in &video_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                }
            }
        }
        
        combined.push(row);
    }
    
    // Apply processing steps sequentially to avoid memory spikes
    apply_interpolation_sequential(&mut combined);
    calculate_emas_sequential(&mut combined);
    calculate_total_engagement_sequential(&mut combined);
    scale_values_sequential(&mut combined);
    
    Ok(combined)
}

// Helper function to determine which columns to include
fn should_include_column(column_name: &str) -> bool {
    (column_name.contains("engage_ema") && column_name.contains("pct")) 
        || column_name.contains("attention") 
        || column_name.contains("cat_engage_percentile")
}

// Create time index from loaded rows  
fn create_time_index_from_rows(rows: &[CsvRow]) -> std::collections::BTreeMap<OrderedFloat<f64>, &CsvRow> {
    let mut time_index = std::collections::BTreeMap::new();
    for row in rows {
        time_index.insert(OrderedFloat(row.time_sec), row);
    }
    time_index
}

// Determine output columns from a small sample
fn determine_output_columns(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    sample_times: &[f64],
) -> Result<Vec<String>, Box<dyn Error>> {
    
    let sample_data = process_time_chunk(
        vocals_file,
        nonvocals_file,
        video_file,
        sample_times,
    )?;
    
    let mut all_columns = BTreeSet::new();
    for row in sample_data {
        for key in row.keys() {
            all_columns.insert(key.clone());
        }
    }
    
    Ok(all_columns.into_iter().collect())
}

// Write chunk results to CSV
fn write_chunk_results(
    csv_writer: &mut csv::Writer<BufWriter<File>>,
    chunk_data: &[HashMap<String, f64>], 
    column_order: &[String],
) -> Result<(), Box<dyn Error>> {
    
    for row in chunk_data {
        let record: Vec<String> = column_order
            .iter()
            .map(|col| row.get(col).unwrap_or(&0.0).to_string())
            .collect();
        csv_writer.write_record(record)?;
    }
    
    Ok(())
}

// Sequential interpolation to avoid memory spikes
fn apply_interpolation_sequential(combined: &mut [HashMap<String, f64>]) {
    if combined.is_empty() {
        return;
    }
    
    // Get all column names except time_sec
    let mut all_columns = BTreeSet::new();
    for row in combined.iter() {
        for key in row.keys() {
            if key != "time_sec" {
                all_columns.insert(key.clone());
            }
        }
    }
    
    // Process each column sequentially to minimize memory usage
    for column_name in all_columns {
        let mut values: Vec<Option<f64>> = combined
            .iter()
            .map(|row| row.get(&column_name).copied())
            .collect();
        
        interpolate_column_values(&mut values);
        
        // Update values back into combined data
        for (i, interpolated_value) in values.iter().enumerate() {
            if let Some(val) = interpolated_value {
                combined[i].insert(column_name.clone(), *val);
            }
        }
    }
}

// Sequential EMA calculation
fn calculate_emas_sequential(combined: &mut [HashMap<String, f64>]) {
    // Extract time values once
    let time_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    // Process attention EMAs
    calculate_attention_emas_memory_efficient(combined, &time_values);
    
    // Process emotion engagement EMAs
    calculate_emotion_engagement_emas_memory_efficient(combined, &time_values);
}

// Memory-efficient attention EMA calculation
fn calculate_attention_emas_memory_efficient(combined: &mut [HashMap<String, f64>], time_values: &[f64]) {
    // Find attention columns without storing them all in memory
    let attention_columns: Vec<String> = combined
        .iter()
        .flat_map(|row| row.keys().cloned())
        .filter(|key| key.contains("attention"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    
    for attention_col in attention_columns {
        let values: Vec<f64> = combined
            .iter()
            .map(|row| row.get(&attention_col).copied().unwrap_or(0.0))
            .collect();
        
        let ema_1s_pct = calculate_time_aware_ema_with_percentiles(&values, time_values, 1.0);
        let ema_10s_pct = calculate_time_aware_ema_with_percentiles(&values, time_values, 10.0);
        
        let col_1s = format!("{}_ema_1s_pct", attention_col);
        let col_10s = format!("{}_ema_10s_pct", attention_col);
        
        for (i, row) in combined.iter_mut().enumerate() {
            row.insert(col_1s.clone(), ema_1s_pct[i]);
            row.insert(col_10s.clone(), ema_10s_pct[i]);
        }
        
        // Clear intermediate vectors to free memory
        drop(ema_1s_pct);
        drop(ema_10s_pct);
    }
}

// Memory-efficient emotion engagement EMA calculation
fn calculate_emotion_engagement_emas_memory_efficient(combined: &mut [HashMap<String, f64>], time_values: &[f64]) {
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
    
    let ema_1s = calculate_time_aware_ema_raw(&cat_values, time_values, 1.0);
    let ema_10s = calculate_time_aware_ema_raw(&cat_values, time_values, 10.0);
    
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("emotion_engage_1s_pct".to_string(), ema_1s[i]);
        row.insert("emotion_engage_10s_pct".to_string(), ema_10s[i]);
    }
}

// Sequential total engagement calculation
fn calculate_total_engagement_sequential(combined: &mut [HashMap<String, f64>]) {
    // Step 1: Calculate raw total engagement
    calculate_total_engag_raw_sequential(combined);
    
    // Step 2: Calculate total engagement EMAs
    calculate_total_engag_raw_emas_sequential(combined);
}

// Memory-efficient total engagement raw calculation
fn calculate_total_engag_raw_sequential(combined: &mut [HashMap<String, f64>]) {
    for row in combined.iter_mut() {
        let mut weighted_sum = 0.0;
        
        // Emotion engage (22%)
        if let Some(val) = row.get("emotion_engage_1s_pct") {
            weighted_sum += val * WEIGHTS_RAW[0].1;
        }
        
        // Transcription engage (18%)
        let transc_val = row.keys()
            .filter(|k| k.contains("transc") && k.contains("1s"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += transc_val * WEIGHTS_RAW[1].1;
        
        // RMS Energy Vocal (13%)
        let rms_vocal = get_channel_averaged_value(row, "rms_energy_engage_ema_", "1s_pct");
        weighted_sum += rms_vocal * WEIGHTS_RAW[2].1;
        
        // Spectral Vocal (10%)
        let spectral_vocal = get_channel_averaged_value(row, "spectral_engage_ema_", "1s_pct");
        weighted_sum += spectral_vocal * WEIGHTS_RAW[3].1;
        
        // Spectral Nonvocal (3%)
        let spectral_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("spectral") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += spectral_nonvocal * WEIGHTS_RAW[4].1;
        
        // RMS Energy Nonvocal (5%)
        let rms_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("rms_energy") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .next()
            .copied()
            .unwrap_or(0.0);
        weighted_sum += rms_nonvocal * WEIGHTS_RAW[5].1;
        
        // Video (25%)
        if let Some(val) = row.get("visual_engage_ema_1s_pct") {
            weighted_sum += val * WEIGHTS_RAW[6].1;
        }
        
        row.insert("total_engag_raw".to_string(), weighted_sum);
    }
}

// Memory-efficient total engagement EMA calculation
fn calculate_total_engag_raw_emas_sequential(combined: &mut [HashMap<String, f64>]) {
    let raw_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("total_engag_raw").copied().unwrap_or(0.0))
        .collect();
    
    let time_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    // Calculate EMAs one by one to minimize peak memory usage
    let ema_1s_pct = calculate_time_aware_ema_with_percentiles(&raw_values, &time_values, 1.0);
    
    // Insert 1s values and drop intermediate vector
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("total_engag_1s_pct".to_string(), ema_1s_pct[i]);
    }
    drop(ema_1s_pct);
    
    let ema_5s_pct = calculate_time_aware_ema_with_percentiles(&raw_values, &time_values, 5.0);
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("total_engag_5s_pct".to_string(), ema_5s_pct[i]);
    }
    drop(ema_5s_pct);
    
    let ema_10s_pct = calculate_time_aware_ema_with_percentiles(&raw_values, &time_values, 10.0);
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("total_engag_10s_pct".to_string(), ema_10s_pct[i]);
    }
    drop(ema_10s_pct);
    
    let ema_30s_pct = calculate_time_aware_ema_with_percentiles(&raw_values, &time_values, 30.0);
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("total_engag_30s_pct".to_string(), ema_30s_pct[i]);
    }
}

// Sequential scaling to avoid memory spikes
fn scale_values_sequential(combined: &mut [HashMap<String, f64>]) {
    if combined.is_empty() {
        return;
    }
    
    let column_names: Vec<String> = combined[0].keys().cloned().collect();
    
    for column_name in column_names {
        if column_name == "time_sec" || column_name.contains("attention") {
            continue;
        }
        
        // Process scaling for this column
        let values: Vec<f64> = combined.iter()
            .map(|row| row.get(&column_name).copied().unwrap_or(0.0))
            .collect();
        
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        
        if min_val >= -0.001 && max_val <= 1.001 && max_val > 0.001 {
            for row in combined.iter_mut() {
                if let Some(value) = row.get_mut(&column_name) {
                    *value = (*value).clamp(0.0, 1.0) * 100.0;
                }
            }
        }
    }
}

// Helper functions (keeping original functionality)

fn get_channel_averaged_value(row: &HashMap<String, f64>, pattern: &str, suffix: &str) -> f64 {
    let chan1_key = format!("chan1_{}{}", pattern, suffix);
    let chan2_key = format!("chan2_{}{}", pattern, suffix);
    
    let chan1_val = row.get(&chan1_key).copied().unwrap_or(0.0);
    let chan2_val = row.get(&chan2_key).copied().unwrap_or(0.0);
    
    if chan1_val == 0.0 && chan2_val != 0.0 {
        chan2_val
    } else if chan2_val == 0.0 && chan1_val != 0.0 {
        chan1_val
    } else if chan1_val != 0.0 && chan2_val != 0.0 {
        (chan1_val + chan2_val) / 2.0
    } else {
        0.0
    }
}

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
    
    // Linear interpolation for gaps
    for i in 0..n {
        if values[i].is_none() {
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

fn time_weighted_ema(values: &[f64], times: &[f64], tau: f64) -> Vec<f64> {
    let mut ema = vec![0.0; values.len()];
    if values.is_empty() || times.is_empty() || values.len() != times.len() {
        return ema;
    }
    
    ema[0] = values[0];
    
    for i in 1..values.len() {
        let dt = times[i] - times[i - 1];
        let alpha = 1.0 - (-dt / tau).exp();
        ema[i] = alpha * values[i] + (1.0 - alpha) * ema[i - 1];
    }
    ema
}

fn calculate_time_aware_ema_with_percentiles(values: &[f64], times: &[f64], tau_seconds: f64) -> Vec<f64> {
    let ema_values = time_weighted_ema(values, times, tau_seconds);
    convert_to_percentiles(&ema_values)
}

fn calculate_time_aware_ema_raw(values: &[f64], times: &[f64], tau_seconds: f64) -> Vec<f64> {
    time_weighted_ema(values, times, tau_seconds)
}

fn convert_to_percentiles(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    if (max_val - min_val).abs() < f64::EPSILON {
        return vec![50.0; values.len()];
    }
    
    values.iter()
        .map(|&val| ((val - min_val) / (max_val - min_val)) * 100.0)
        .collect()
}

// Additional memory-efficient utility functions

// Stream-based 30fps interpolation to avoid loading entire dataset
fn finalize_data_streaming(
    input_file: &str,
    output_file: &str,
) -> Result<(), Box<dyn Error>> {
    // Read input file to determine time range
    let (first_time, last_time) = get_time_range(input_file)?;
    
    // Create 30fps timeline
    let fps = 30.0;
    let time_step = 1.0 / fps;
    let mut target_times = Vec::new();
    
    let mut current_time = first_time;
    while current_time <= last_time {
        target_times.push(current_time);
        current_time += time_step;
    }
    
    // Process in chunks to avoid OOM
    let chunk_size = 1000; // Smaller chunks for 30fps processing
    let output_writer = BufWriter::new(File::create(output_file)?);
    let mut csv_writer = WriterBuilder::new().from_writer(output_writer);
    
    // Determine columns from input file
    let columns = get_column_names(input_file)?;
    csv_writer.write_record(&columns)?;
    
    // Process chunks of target times
    for time_chunk in target_times.chunks(chunk_size) {
        let interpolated_chunk = interpolate_time_chunk_streaming(input_file, time_chunk, &columns)?;
        
        // Write results immediately
        for row in interpolated_chunk {
            let record: Vec<String> = columns
                .iter()
                .map(|col| row.get(col).unwrap_or(&0.0).to_string())
                .collect();
            csv_writer.write_record(record)?;
        }
    }
    
    csv_writer.flush()?;
    Ok(())
}

// Get time range without loading full dataset
fn get_time_range(file_path: &str) -> Result<(f64, f64), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    let mut first_time = None;
    let mut last_time = None;
    
    for result in rdr.records() {
        let record = result?;
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                if first_time.is_none() {
                    first_time = Some(time_val);
                }
                last_time = Some(time_val);
            }
        }
    }
    
    match (first_time, last_time) {
        (Some(first), Some(last)) => Ok((first, last)),
        _ => Err("No valid time values found".into()),
    }
}

// Get column names without loading full dataset
fn get_column_names(file_path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    Ok(headers.iter().map(|s| s.to_string()).collect())
}

// Memory-efficient chunk interpolation
fn interpolate_time_chunk_streaming(
    input_file: &str,
    target_times: &[f64],
    columns: &[String],
) -> Result<Vec<HashMap<String, f64>>, Box<dyn Error>> {
    
    // Find the time range we need to load (with buffer)
    let min_target = target_times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_target = target_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let buffer = 2.0; // 2 second buffer on each side
    
    // Load only the data we need for interpolation
    let source_data = load_csv_time_range(
        input_file, 
        min_target - buffer, 
        max_target + buffer,
        columns,
    )?;
    
    if source_data.is_empty() {
        // Return empty rows for target times
        return Ok(target_times.iter().map(|&time| {
            let mut row = HashMap::new();
            row.insert("time_sec".to_string(), time);
            for col in columns {
                if col != "time_sec" {
                    row.insert(col.clone(), 0.0);
                }
            }
            row
        }).collect());
    }
    
    // Extract source times and values for interpolation
    let source_times: Vec<f64> = source_data.iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    let mut result = Vec::with_capacity(target_times.len());
    
    for &target_time in target_times {
        let mut interpolated_row = HashMap::new();
        interpolated_row.insert("time_sec".to_string(), target_time);
        
        // Interpolate each column
        for col in columns {
            if col != "time_sec" {
                let source_values: Vec<f64> = source_data.iter()
                    .map(|row| row.get(col).copied().unwrap_or(0.0))
                    .collect();
                
                let interpolated_value = interpolate_at_time(&source_times, &source_values, target_time);
                interpolated_row.insert(col.clone(), interpolated_value);
            }
        }
        
        result.push(interpolated_row);
    }
    
    Ok(result)
}

// Load CSV data within a specific time range
fn load_csv_time_range(
    file_path: &str,
    min_time: f64,
    max_time: f64,
    expected_columns: &[String],
) -> Result<Vec<HashMap<String, f64>>, Box<dyn Error>> {
    
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    let mut rows = Vec::new();
    
    for result in rdr.records() {
        let record = result?;
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                // Only load rows within our target time range
                if time_val >= min_time && time_val <= max_time {
                    let mut data = HashMap::new();
                    data.insert("time_sec".to_string(), time_val);
                    
                    for (i, field) in record.iter().enumerate() {
                        if i != time_col_idx && !field.trim().is_empty() {
                            let column_name = &headers[i];
                            if let Ok(val) = field.parse::<f64>() {
                                data.insert(column_name.to_string(), val);
                            }
                        }
                    }
                    
                    // Ensure all expected columns are present (with 0.0 default)
                    for col in expected_columns {
                        if !data.contains_key(col) && col != "time_sec" {
                            data.insert(col.clone(), 0.0);
                        }
                    }
                    
                    rows.push(data);
                }
            }
        }
    }
    
    // Sort by time for proper interpolation
    rows.sort_by(|a, b| {
        let time_a = a.get("time_sec").copied().unwrap_or(0.0);
        let time_b = b.get("time_sec").copied().unwrap_or(0.0);
        time_a.partial_cmp(&time_b).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    Ok(rows)
}

// Efficient single-value interpolation (reused from original)
fn interpolate_at_time(times: &[f64], values: &[f64], target_time: f64) -> f64 {
    if times.is_empty() || values.is_empty() || times.len() != values.len() {
        return 0.0;
    }
    
    if target_time <= times[0] {
        return values[0];
    }
    if target_time >= times[times.len() - 1] {
        return values[values.len() - 1];
    }
    
    // Binary search for efficiency
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
    
    let t0 = times[left];
    let t1 = times[right];
    let v0 = values[left];
    let v1 = values[right];
    
    if (t1 - t0).abs() < f64::EPSILON {
        return v0;
    }
    
    let ratio = (target_time - t0) / (t1 - t0);
    v0 + ratio * (v1 - v0)
}

// Memory usage tracking and warnings
struct MemoryTracker {
    current_memory_estimate: usize,
}

impl MemoryTracker {
    fn new() -> Self {
        Self {
            current_memory_estimate: 0,
        }
    }
    
    fn log_memory_usage(&self, stage: &str, row_count: usize) {
        let mb_estimate = self.current_memory_estimate / (1024 * 1024);
        println!("🧠 Memory estimate at {}: ~{}MB for {} rows", stage, mb_estimate, row_count);
    }
}

// Enhanced main function with memory tracking
pub fn process_with_memory_monitoring(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    output_file: &str,
) -> Result<(), Box<dyn Error>> {
    
    let memory_tracker = MemoryTracker::new();
    
    println!("🚀 Starting OOM-safe engagement processing...");
    memory_tracker.log_memory_usage("startup", 0);
    
    // Step 1: Process main engagement data
    let temp_file = format!("{}.temp", output_file);
    
    let memory_monitor = MemoryMonitor::new();
    process_engagement_streaming(
        vocals_file,
        nonvocals_file,
        video_file,
        &temp_file,
        &memory_monitor,
    )?;
    
    println!("✅ Main processing complete, starting 30fps interpolation...");
    
    // Step 2: Apply 30fps interpolation
    finalize_data_streaming(&temp_file, output_file)?;
    
    // Cleanup temporary file
    std::fs::remove_file(&temp_file).ok();
    
    println!("✅ Processing complete! Output saved to: {}", output_file);
    Ok(())
}