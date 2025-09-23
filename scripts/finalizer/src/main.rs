use csv::WriterBuilder;
use std::collections::{BTreeSet, HashMap, BTreeMap};
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

// Enhanced CSV row structure with better debugging
#[derive(Debug, Clone)]
struct CsvRow {
    time_sec: f64,
    data: HashMap<String, f64>,
}

// EMA State tracking to maintain continuity across chunks
#[derive(Debug, Clone)]
struct EMAState {
    last_value: f64,
    last_time: f64,
    initialized: bool,
}

impl EMAState {
    fn new() -> Self {
        Self {
            last_value: 0.0,
            last_time: 0.0,
            initialized: false,
        }
    }
}

// Global EMA state manager
struct EMAStateManager {
    states: HashMap<String, EMAState>,
}

impl EMAStateManager {
    fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }
    
    fn get_or_create_state(&mut self, column: &str) -> &mut EMAState {
        self.states.entry(column.to_string()).or_insert_with(EMAState::new)
    }
}

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

// Enhanced memory monitoring with validation
struct MemoryMonitor {
    max_rows_in_memory: usize,
    overlap_buffer: usize,
}

impl MemoryMonitor {
    fn new() -> Self {
        Self {
            max_rows_in_memory: 30_000, // Reduced for safety
            overlap_buffer: 1000, // Buffer for EMA continuity
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

    println!("🔄 Processing engagement data with enhanced OOM-safe approach...");
    
    let memory_monitor = MemoryMonitor::new();
    
    process_engagement_streaming_fixed(
        vocals_file,
        nonvocals_file, 
        video_file,
        output_file,
        &memory_monitor,
    )?;

    println!("✅ CSV exported to {}", output_file);
    Ok(())
}

// Enhanced streaming processing with state continuity
fn process_engagement_streaming_fixed(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    output_file: &str,
    memory_monitor: &MemoryMonitor,
) -> Result<(), Box<dyn Error>> {
    
    // Step 1: Create comprehensive validation of input files
    println!("📊 Validating input files...");
    validate_input_files(vocals_file, nonvocals_file, video_file)?;
    
    println!("📊 Creating time indices...");
    let vocals_times = extract_time_indices(vocals_file)?;
    let nonvocals_times = extract_time_indices(nonvocals_file)?;
    let video_times = extract_time_indices(video_file)?;
    
    if vocals_times.is_empty() || nonvocals_times.is_empty() || video_times.is_empty() {
        return Err("One or more input files contain no valid time data".into());
    }
    
    // Combine all unique timestamps
    let mut all_times: BTreeSet<OrderedFloat<f64>> = BTreeSet::new();
    all_times.extend(vocals_times.into_iter().map(OrderedFloat));
    all_times.extend(nonvocals_times.into_iter().map(OrderedFloat));
    all_times.extend(video_times.into_iter().map(OrderedFloat));
    
    println!("⏱️  Processing {} unique timestamps", all_times.len());
    
    // Step 2: Create overlapping chunks for EMA continuity
    let chunk_size = memory_monitor.max_rows_in_memory;
    let overlap = memory_monitor.overlap_buffer;
    
    let all_times_vec: Vec<f64> = all_times.into_iter().map(|t| t.0).collect();
    let time_chunks = create_overlapping_chunks(&all_times_vec, chunk_size, overlap);
    
    println!("🗂️  Processing {} overlapping chunks", time_chunks.len());
    
    // Step 3: Initialize output and EMA state management
    let output_writer = BufWriter::new(File::create(output_file)?);
    let mut csv_writer = WriterBuilder::new().from_writer(output_writer);
    
    let sample_columns = determine_output_columns(
        vocals_file,
        nonvocals_file,
        video_file,
        &time_chunks[0].main_times[..std::cmp::min(10, time_chunks[0].main_times.len())],
    )?;
    
    csv_writer.write_record(&sample_columns)?;
    
    // Global EMA state manager
    let mut ema_state_manager = EMAStateManager::new();
    
    // Step 4: Process each chunk with state continuity
    for (chunk_idx, time_chunk) in time_chunks.iter().enumerate() {
        println!("🔄 Processing chunk {}/{} ({} timestamps)...", 
                 chunk_idx + 1, time_chunks.len(), time_chunk.main_times.len());
        
        let chunk_data = process_time_chunk_with_state(
            vocals_file,
            nonvocals_file,
            video_file,
            time_chunk,
            &mut ema_state_manager,
        )?;
        
        // Validate chunk data before writing
        if validate_chunk_data(&chunk_data, chunk_idx) {
            // Only write the main portion (exclude overlap for next chunk)
            let main_data = if chunk_idx == time_chunks.len() - 1 {
                // Last chunk - write everything
                chunk_data
            } else {
                // Skip overlap data for next chunk
                let main_count = time_chunk.main_times.len();
                chunk_data.into_iter().take(main_count).collect()
            };
            
            write_chunk_results(&mut csv_writer, &main_data, &sample_columns)?;
        } else {
            println!("⚠️  Warning: Chunk {} contains suspicious data patterns", chunk_idx + 1);
        }
    }
    
    csv_writer.flush()?;
    Ok(())
}

// Enhanced chunk structure with overlap management
struct TimeChunk {
    main_times: Vec<f64>,
    overlap_times: Vec<f64>, // Previous chunk's data for continuity
    all_times: Vec<f64>, // Combined for processing
}

fn create_overlapping_chunks(times: &[f64], chunk_size: usize, overlap: usize) -> Vec<TimeChunk> {
    if times.is_empty() {
        return vec![];
    }
    
    let mut chunks = Vec::new();
    let mut start = 0;
    
    while start < times.len() {
        let end = std::cmp::min(start + chunk_size, times.len());
        let overlap_start = if start == 0 { 0 } else { std::cmp::max(start, overlap) - overlap };
        
        let main_times = times[start..end].to_vec();
        let overlap_times = if start == 0 {
            vec![]
        } else {
            times[overlap_start..start].to_vec()
        };
        
        let mut all_times = overlap_times.clone();
        all_times.extend_from_slice(&main_times);
        
        chunks.push(TimeChunk {
            main_times,
            overlap_times,
            all_times,
        });
        
        start = end;
    }
    
    chunks
}

// Enhanced validation functions
fn validate_input_files(vocals_file: &str, nonvocals_file: &str, video_file: &str) -> Result<(), Box<dyn Error>> {
    for (name, file) in [("vocals", vocals_file), ("nonvocals", nonvocals_file), ("video", video_file)] {
        let file_handle = File::open(file).map_err(|e| format!("Cannot open {} file '{}': {}", name, file, e))?;
        let buf_reader = BufReader::new(file_handle);
        let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
        
        let headers = rdr.headers().map_err(|e| format!("Cannot read headers from {} file: {}", name, e))?;
        
        if !headers.iter().any(|h| h == "time_sec") {
            return Err(format!("{} file missing required 'time_sec' column", name).into());
        }
        
        println!("✅ {} file validated: {} columns", name, headers.len());
    }
    Ok(())
}

fn validate_chunk_data(chunk_data: &[HashMap<String, f64>], chunk_idx: usize) -> bool {
    if chunk_data.is_empty() {
        println!("❌ Chunk {} is empty", chunk_idx);
        return false;
    }
    
    // Check for excessive zeros or frozen values
    let mut zero_counts = HashMap::new();
    let mut value_consistency = HashMap::new();
    let mut value_transitions = HashMap::new();
    
    for (row_idx, row) in chunk_data.iter().enumerate() {
        for (key, &value) in row {
            if key == "time_sec" {
                continue;
            }
            
            *zero_counts.entry(key.clone()).or_insert(0) += if value == 0.0 { 1 } else { 0 };
            
            let values_vec = value_consistency.entry(key.clone()).or_insert(Vec::new());
            values_vec.push(value);
            
            // Track transitions between consecutive values
            if row_idx > 0 {
                let transitions = value_transitions.entry(key.clone()).or_insert(Vec::new());
                if values_vec.len() >= 2 {
                    let prev_val = values_vec[values_vec.len() - 2];
                    transitions.push((prev_val - value).abs());
                }
            }
        }
    }
    
    let mut suspicious = false;
    for (column, zero_count) in zero_counts {
        let zero_ratio = zero_count as f64 / chunk_data.len() as f64;
        
        if zero_ratio > 0.95 && column.contains("engage") {
            println!("   ⚠️  Suspicious: {}% zeros in {} for chunk {}", 
                     (zero_ratio * 100.0) as i32, column, chunk_idx);
            suspicious = true;
        }
        
        // Enhanced stuck value detection
        if let Some(values) = value_consistency.get(&column) {
            if values.len() > 10 {
                let unique_values: BTreeSet<OrderedFloat<f64>> = values.iter().map(|&v| OrderedFloat(v)).collect();
                let variation_ratio = unique_values.len() as f64 / values.len() as f64;
                
                // Check for completely frozen values
                if unique_values.len() == 1 && values[0] != 0.0 {
                    println!("   ⚠️  Completely frozen values in {} for chunk {}: all values = {}", 
                             column, chunk_idx, values[0]);
                    suspicious = true;
                }
                
                // Check for very low variation (seizing behavior)
                else if variation_ratio < 0.02 && column.contains("engage") {
                    println!("   ⚠️  Very low variation in {} for chunk {} ({}% unique values)", 
                             column, chunk_idx, (variation_ratio * 100.0) as i32);
                    suspicious = true;
                }
                
                // Check transition patterns
                if let Some(transitions) = value_transitions.get(&column) {
                    if transitions.len() > 20 {
                        let avg_transition = transitions.iter().sum::<f64>() / transitions.len() as f64;
                        let max_transition = transitions.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                        
                        if avg_transition < 0.001 && max_transition < 0.01 && column.contains("engage") {
                            println!("   ⚠️  Minimal value transitions in {} for chunk {} (avg: {:.6})", 
                                     column, chunk_idx, avg_transition);
                            suspicious = true;
                        }
                    }
                }
                
                // Additional check for end-of-video seizing
                if chunk_idx > 5 && values.len() > 100 { // Later chunks with sufficient data
                    let last_quarter = &values[values.len() * 3 / 4..];
                    let last_quarter_unique: BTreeSet<OrderedFloat<f64>> = 
                        last_quarter.iter().map(|&v| OrderedFloat(v)).collect();
                    
                    if last_quarter_unique.len() == 1 && last_quarter.len() > 10 {
                        println!("   🔒 End-seizing detected in {} for chunk {}: last quarter frozen at {}", 
                                 column, chunk_idx, last_quarter[0]);
                        suspicious = true;
                    }
                }
            }
        }
    }
    
    !suspicious
}

// Enhanced chunk processing with state continuity
fn process_time_chunk_with_state(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    time_chunk: &TimeChunk,
    ema_state_manager: &mut EMAStateManager,
) -> Result<Vec<HashMap<String, f64>>, Box<dyn Error>> {
    
    // Load data for the full time range (including overlap)
    let vocals_rows = load_csv_by_times_safe(vocals_file, &time_chunk.all_times)?;
    let nonvocals_rows = load_csv_by_times_safe(nonvocals_file, &time_chunk.all_times)?;
    let video_rows = load_csv_by_times_safe(video_file, &time_chunk.all_times)?;
    
    // Enhanced logging
    println!("   📊 Loaded: {} vocal, {} nonvocal, {} video rows", 
             vocals_rows.len(), nonvocals_rows.len(), video_rows.len());
    
    let vocals_by_time = create_time_index_from_rows(&vocals_rows);
    let nonvocals_by_time = create_time_index_from_rows(&nonvocals_rows);
    let video_by_time = create_time_index_from_rows(&video_rows);
    
    let mut combined = Vec::with_capacity(time_chunk.all_times.len());
    
    // Combine data with enhanced error checking
    for &time_sec in &time_chunk.all_times {
        let time_key = OrderedFloat(time_sec);
        let mut row = HashMap::new();
        row.insert("time_sec".to_string(), time_sec);
        
        let mut found_any_data = false;
        
        // Process vocals data with validation
        if let Some(vocal_row) = vocals_by_time.get(&time_key) {
            for (k, &v) in &vocal_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                    if v != 0.0 {
                        found_any_data = true;
                    }
                }
            }
        }
        
        // Process nonvocals data
        if let Some(nonvocal_row) = nonvocals_by_time.get(&time_key) {
            for (k, &v) in &nonvocal_row.data {
                if should_include_column(k) {
                    let prefixed_key = if k.starts_with("nonvocal_") {
                        k.clone()
                    } else {
                        format!("nonvocal_{}", k)
                    };
                    row.insert(prefixed_key, v);
                    if v != 0.0 {
                        found_any_data = true;
                    }
                }
            }
        }
        
        // Process video data
        if let Some(video_row) = video_by_time.get(&time_key) {
            for (k, &v) in &video_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                    if v != 0.0 {
                        found_any_data = true;
                    }
                }
            }
        }
        
        // Debug logging for data gaps
        if !found_any_data && combined.len() % 1000 == 0 {
            println!("   ⚠️  No engagement data found at time {:.2}s", time_sec);
        }
        
        combined.push(row);
    }
    
    // Apply processing with state continuity
    apply_interpolation_enhanced(&mut combined);
    calculate_emas_with_state(&mut combined, ema_state_manager);
    calculate_total_engagement_sequential(&mut combined);
    scale_values_safe(&mut combined);
    
    Ok(combined)
}

// Enhanced interpolation with better gap handling
fn apply_interpolation_enhanced(combined: &mut [HashMap<String, f64>]) {
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
    
    for column_name in all_columns {
        let mut values: Vec<Option<f64>> = combined
            .iter()
            .map(|row| row.get(&column_name).copied())
            .collect();
        
        // Enhanced interpolation with gap detection
        interpolate_column_values_enhanced(&mut values, &column_name);
        
        // Update values back into combined data
        for (i, interpolated_value) in values.iter().enumerate() {
            if let Some(val) = interpolated_value {
                combined[i].insert(column_name.clone(), *val);
            }
        }
    }
}

// Enhanced interpolation with gap size limits
fn interpolate_column_values_enhanced(values: &mut Vec<Option<f64>>, column_name: &str) {
    let n = values.len();
    let max_gap_size = 300; // Maximum gap to interpolate (10 seconds at 30fps)
    
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
    
    // Linear interpolation for gaps with size limit
    for i in 0..n {
        if values[i].is_none() {
            let prev = (0..i).rev().find(|&j| values[j].is_some());
            let next = (i + 1..n).find(|&j| values[j].is_some());
            
            if let (Some(p), Some(nxt)) = (prev, next) {
                let gap_size = nxt - p;
                
                if gap_size <= max_gap_size {
                    let prev_val = values[p].unwrap();
                    let next_val = values[nxt].unwrap();
                    let ratio = (i - p) as f64 / (nxt - p) as f64;
                    values[i] = Some(prev_val + (next_val - prev_val) * ratio);
                } else {
                    // Gap too large - use zero or last known value
                    values[i] = Some(values[p].unwrap_or(0.0));
                    if column_name.contains("engage") && i % 100 == 0 {
                        println!("   ⚠️  Large gap ({} samples) in {} around index {}", gap_size, column_name, i);
                    }
                }
            }
        }
    }
}

// Enhanced EMA calculation with state management
fn calculate_emas_with_state(combined: &mut [HashMap<String, f64>], ema_state_manager: &mut EMAStateManager) {
    let time_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    calculate_attention_emas_with_state(combined, &time_values, ema_state_manager);
    calculate_emotion_engagement_emas_with_state(combined, &time_values, ema_state_manager);
}

fn calculate_attention_emas_with_state(
    combined: &mut [HashMap<String, f64>], 
    time_values: &[f64],
    ema_state_manager: &mut EMAStateManager,
) {
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
        
        // Calculate EMAs with state continuity
        let ema_1s_key = format!("{}_ema_1s", attention_col);
        let ema_10s_key = format!("{}_ema_10s", attention_col);
        
        let ema_1s_pct = calculate_ema_with_state(
            &values, time_values, 1.0, 
            ema_state_manager.get_or_create_state(&ema_1s_key)
        );
        let ema_1s_pct = convert_to_percentiles(&ema_1s_pct);
        
        let ema_10s_pct = calculate_ema_with_state(
            &values, time_values, 10.0,
            ema_state_manager.get_or_create_state(&ema_10s_key)
        );
        let ema_10s_pct = convert_to_percentiles(&ema_10s_pct);
        
        let col_1s = format!("{}_ema_1s_pct", attention_col);
        let col_10s = format!("{}_ema_10s_pct", attention_col);
        
        for (i, row) in combined.iter_mut().enumerate() {
            row.insert(col_1s.clone(), ema_1s_pct[i]);
            row.insert(col_10s.clone(), ema_10s_pct[i]);
        }
    }
}

fn calculate_emotion_engagement_emas_with_state(
    combined: &mut [HashMap<String, f64>], 
    time_values: &[f64],
    ema_state_manager: &mut EMAStateManager,
) {
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
    
    let ema_1s = calculate_ema_with_state(
        &cat_values, time_values, 1.0,
        ema_state_manager.get_or_create_state("emotion_engage_1s")
    );
    
    let ema_10s = calculate_ema_with_state(
        &cat_values, time_values, 10.0,
        ema_state_manager.get_or_create_state("emotion_engage_10s")
    );
    
    for (i, row) in combined.iter_mut().enumerate() {
        row.insert("emotion_engage_1s_pct".to_string(), ema_1s[i]);
        row.insert("emotion_engage_10s_pct".to_string(), ema_10s[i]);
    }
}

// Simplified but more aggressive EMA calculation to prevent seizing
fn calculate_ema_with_state(
    values: &[f64], 
    times: &[f64], 
    tau: f64, 
    state: &mut EMAState
) -> Vec<f64> {
    let mut ema = vec![0.0; values.len()];
    if values.is_empty() || times.is_empty() || values.len() != times.len() {
        return ema;
    }
    
    // For long tau values, reduce the effective time constant to prevent seizing
    let effective_tau = if tau > 10.0 {
        tau * 0.5  // Make long-term EMAs more responsive
    } else {
        tau
    };
    
    // Initialize with state from previous chunk
    if state.initialized {
        let dt = if !times.is_empty() { times[0] - state.last_time } else { 0.0 };
        
        if dt > 0.0 && dt < 60.0 {
            let alpha = 1.0 - (-dt / effective_tau).exp();
            // Force minimum alpha to prevent complete stagnation
            let min_alpha = if tau > 10.0 { 0.02 } else { 0.01 };
            let adjusted_alpha = alpha.max(min_alpha);
            ema[0] = adjusted_alpha * values[0] + (1.0 - adjusted_alpha) * state.last_value;
        } else {
            ema[0] = values[0]; // Reset on large gaps
        }
    } else {
        ema[0] = values[0];
        state.initialized = true;
    }
    
    // Calculate EMA with minimum responsiveness guarantee
    for i in 1..values.len() {
        let dt = times[i] - times[i - 1];
        let base_alpha = if dt > 0.0 { 1.0 - (-dt / effective_tau).exp() } else { 1.0 };
        
        // Enforce minimum alpha based on tau to prevent complete stagnation
        let min_alpha = match () {
            _ if tau >= 30.0 => 0.005,  // 30s EMA: minimum 0.5% new data influence
            _ if tau >= 10.0 => 0.01,   // 10s EMA: minimum 1% new data influence  
            _ if tau >= 5.0 => 0.02,    // 5s EMA: minimum 2% new data influence
            _ => 0.05,                  // 1s EMA: minimum 5% new data influence
        };
        
        let alpha = base_alpha.max(min_alpha);
        ema[i] = alpha * values[i] + (1.0 - alpha) * ema[i - 1];
        
        // Sanity check
        if !ema[i].is_finite() {
            ema[i] = values[i];
        }
    }
    
    // Update state for next chunk
    if !ema.is_empty() && !times.is_empty() {
        state.last_value = ema[ema.len() - 1];
        state.last_time = times[times.len() - 1];
    }
    
    ema
}

// Safe scaling to prevent value corruption
fn scale_values_safe(combined: &mut [HashMap<String, f64>]) {
    if combined.is_empty() {
        return;
    }
    
    let column_names: Vec<String> = combined[0].keys().cloned().collect();
    
    for column_name in column_names {
        if column_name == "time_sec" || column_name.contains("attention") {
            continue;
        }
        
        let values: Vec<f64> = combined.iter()
            .map(|row| row.get(&column_name).copied().unwrap_or(0.0))
            .collect();
        
        // Enhanced validation before scaling
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let valid_count = values.iter().filter(|&&v| v != 0.0).count();
        
        // Only scale if we have reasonable data distribution
        if min_val >= -0.001 && max_val <= 1.001 && max_val > 0.001 && valid_count > 0 {
            for row in combined.iter_mut() {
                if let Some(value) = row.get_mut(&column_name) {
                    *value = (*value).clamp(0.0, 1.0) * 100.0;
                }
            }
        } else if column_name.contains("engage") {
            println!("   ⚠️  Skipping scaling for {} (range: {:.3} to {:.3}, {} valid values)", 
                     column_name, min_val, max_val, valid_count);
        }
    }
}

// Enhanced CSV loading with better error handling
fn load_csv_by_times_safe(file_path: &str, target_times: &[f64]) -> Result<Vec<CsvRow>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    let target_set: BTreeSet<OrderedFloat<f64>> = 
        target_times.iter().map(|&t| OrderedFloat(t)).collect();
    
    let mut rows = Vec::new();
    let mut processed_count = 0;
    let mut matched_count = 0;
    
    for result in rdr.records() {
        let record = result?;
        processed_count += 1;
        
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                if target_set.contains(&OrderedFloat(time_val)) {
                    matched_count += 1;
                    let mut data = HashMap::new();
                    
                    for (i, field) in record.iter().enumerate() {
                        if i != time_col_idx && !field.trim().is_empty() {
                            if let Ok(val) = field.parse::<f64>() {
                                // Validate numeric values
                                if val.is_finite() {
                                    data.insert(headers[i].to_string(), val);
                                }
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
    
    println!("   📊 Matched {} of {} target times from {} total records in {}", 
             matched_count, target_times.len(), processed_count, file_path);
    
    Ok(rows)
}

// Rest of the helper functions with enhanced error handling...

fn extract_time_indices(file_path: &str) -> Result<Vec<f64>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let buf_reader = BufReader::new(file);
    let mut rdr = csv::ReaderBuilder::new().from_reader(buf_reader);
    
    let headers = rdr.headers()?.clone();
    let time_col_idx = headers.iter()
        .position(|h| h == "time_sec")
        .ok_or("time_sec column not found")?;
    
    let mut times = Vec::new();
    let mut invalid_count = 0;
    
    for result in rdr.records() {
        let record = result?;
        if let Some(time_field) = record.get(time_col_idx) {
            if let Ok(time_val) = time_field.parse::<f64>() {
                if time_val.is_finite() && time_val >= 0.0 {
                    times.push(time_val);
                } else {
                    invalid_count += 1;
                }
            }
        }
    }
    
    if invalid_count > 0 {
        println!("   ⚠️  Skipped {} invalid time values in {}", invalid_count, file_path);
    }
    
    Ok(times)
}

fn should_include_column(column_name: &str) -> bool {
    (column_name.contains("engage_ema") && column_name.contains("pct")) 
        || column_name.contains("attention") 
        || column_name.contains("cat_engage_percentile")
}

fn create_time_index_from_rows(rows: &[CsvRow]) -> BTreeMap<OrderedFloat<f64>, &CsvRow> {
    let mut time_index = BTreeMap::new();
    for row in rows {
        time_index.insert(OrderedFloat(row.time_sec), row);
    }
    time_index
}

fn determine_output_columns(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    sample_times: &[f64],
) -> Result<Vec<String>, Box<dyn Error>> {
    
    let sample_data = process_time_chunk_sample(
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

// Simplified sample processing without state management
fn process_time_chunk_sample(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    sample_times: &[f64],
) -> Result<Vec<HashMap<String, f64>>, Box<dyn Error>> {
    
    let vocals_rows = load_csv_by_times_safe(vocals_file, sample_times)?;
    let nonvocals_rows = load_csv_by_times_safe(nonvocals_file, sample_times)?;
    let video_rows = load_csv_by_times_safe(video_file, sample_times)?;
    
    let vocals_by_time = create_time_index_from_rows(&vocals_rows);
    let nonvocals_by_time = create_time_index_from_rows(&nonvocals_rows);
    let video_by_time = create_time_index_from_rows(&video_rows);
    
    let mut combined = Vec::with_capacity(sample_times.len());
    
    for &time_sec in sample_times {
        let time_key = OrderedFloat(time_sec);
        let mut row = HashMap::new();
        row.insert("time_sec".to_string(), time_sec);
        
        if let Some(vocal_row) = vocals_by_time.get(&time_key) {
            for (k, &v) in &vocal_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                }
            }
        }
        
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
        
        if let Some(video_row) = video_by_time.get(&time_key) {
            for (k, &v) in &video_row.data {
                if should_include_column(k) {
                    row.insert(k.clone(), v);
                }
            }
        }
        
        combined.push(row);
    }
    
    // Basic processing for column determination
    apply_interpolation_enhanced(&mut combined);
    let mut dummy_state_manager = EMAStateManager::new();
    calculate_emas_with_state(&mut combined, &mut dummy_state_manager);
    calculate_total_engagement_sequential(&mut combined);
    
    Ok(combined)
}

fn write_chunk_results(
    csv_writer: &mut csv::Writer<BufWriter<File>>,
    chunk_data: &[HashMap<String, f64>], 
    column_order: &[String],
) -> Result<(), Box<dyn Error>> {
    
    for row in chunk_data {
        let record: Vec<String> = column_order
            .iter()
            .map(|col| {
                let value = row.get(col).unwrap_or(&0.0);
                // Validate output values
                if value.is_finite() {
                    format!("{:.6}", value)
                } else {
                    "0.0".to_string()
                }
            })
            .collect();
        csv_writer.write_record(record)?;
    }
    
    Ok(())
}

fn calculate_total_engagement_sequential(combined: &mut [HashMap<String, f64>]) {
    calculate_total_engag_raw_sequential(combined);
    calculate_total_engag_raw_emas_sequential(combined);
}

fn calculate_total_engag_raw_sequential(combined: &mut [HashMap<String, f64>]) {
    for row in combined.iter_mut() {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        
        // Emotion engage (22%)
        if let Some(val) = row.get("emotion_engage_1s_pct") {
            if val.is_finite() && *val >= 0.0 {
                weighted_sum += val * WEIGHTS_RAW[0].1;
                total_weight += WEIGHTS_RAW[0].1;
            }
        }
        
        // Transcription engage (18%)
        let transc_val = row.keys()
            .filter(|k| k.contains("transc") && k.contains("1s"))
            .filter_map(|k| row.get(k))
            .find(|&&v| v.is_finite() && v >= 0.0)
            .copied()
            .unwrap_or(0.0);
        if transc_val > 0.0 {
            weighted_sum += transc_val * WEIGHTS_RAW[1].1;
            total_weight += WEIGHTS_RAW[1].1;
        }
        
        // RMS Energy Vocal (13%)
        let rms_vocal = get_channel_averaged_value_safe(row, "rms_energy_engage_ema_", "1s_pct");
        if rms_vocal > 0.0 {
            weighted_sum += rms_vocal * WEIGHTS_RAW[2].1;
            total_weight += WEIGHTS_RAW[2].1;
        }
        
        // Spectral Vocal (10%)
        let spectral_vocal = get_channel_averaged_value_safe(row, "spectral_engage_ema_", "1s_pct");
        if spectral_vocal > 0.0 {
            weighted_sum += spectral_vocal * WEIGHTS_RAW[3].1;
            total_weight += WEIGHTS_RAW[3].1;
        }
        
        // Spectral Nonvocal (3%)
        let spectral_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("spectral") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .find(|&&v| v.is_finite() && v >= 0.0)
            .copied()
            .unwrap_or(0.0);
        if spectral_nonvocal > 0.0 {
            weighted_sum += spectral_nonvocal * WEIGHTS_RAW[4].1;
            total_weight += WEIGHTS_RAW[4].1;
        }
        
        // RMS Energy Nonvocal (5%)
        let rms_nonvocal = row.keys()
            .filter(|k| k.contains("nonvocal") && k.contains("rms_energy") && k.contains("1s_pct"))
            .filter_map(|k| row.get(k))
            .find(|&&v| v.is_finite() && v >= 0.0)
            .copied()
            .unwrap_or(0.0);
        if rms_nonvocal > 0.0 {
            weighted_sum += rms_nonvocal * WEIGHTS_RAW[5].1;
            total_weight += WEIGHTS_RAW[5].1;
        }
        
        // Video (25%)
        if let Some(val) = row.get("visual_engage_ema_1s_pct") {
            if val.is_finite() && *val >= 0.0 {
                weighted_sum += val * WEIGHTS_RAW[6].1;
                total_weight += WEIGHTS_RAW[6].1;
            }
        }
        
        // Normalize by actual weight used (handles missing components)
        let final_value = if total_weight > 0.0 {
            (weighted_sum / total_weight) * WEIGHTS_RAW.iter().map(|(_, w)| w).sum::<f64>()
        } else {
            0.0
        };
        
        row.insert("total_engag_raw".to_string(), final_value);
    }
}

fn calculate_total_engag_raw_emas_sequential(combined: &mut [HashMap<String, f64>]) {
    let raw_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("total_engag_raw").copied().unwrap_or(0.0))
        .collect();
    
    let time_values: Vec<f64> = combined
        .iter()
        .map(|row| row.get("time_sec").copied().unwrap_or(0.0))
        .collect();
    
    // Create temporary states for total engagement EMAs
    // Use fresh states to avoid cross-contamination between processing stages
    let mut temp_state_manager = EMAStateManager::new();
    
    // Process each EMA independently with validation
    let time_constants = [(1.0, "1s"), (5.0, "5s"), (10.0, "10s"), (30.0, "30s")];
    
    for (tau, suffix) in time_constants.iter() {
        let ema_raw = calculate_ema_with_state(
            &raw_values, &time_values, *tau,
            temp_state_manager.get_or_create_state(&format!("total_{}", suffix))
        );
        
        // Validate EMA before percentile conversion
        let valid_ema_count = ema_raw.iter().filter(|&&v| v.is_finite() && v >= 0.0).count();
        
        if valid_ema_count == 0 {
            println!("   ⚠️  All EMA values invalid for tau={}s, using raw values", tau);
            // Fallback to raw values if EMA calculation failed
            let ema_pct = convert_to_percentiles(&raw_values);
            for (i, row) in combined.iter_mut().enumerate() {
                row.insert(format!("total_engag_{}_pct", suffix), ema_pct[i]);
            }
        } else if (valid_ema_count as f64 / ema_raw.len() as f64) < 0.5 {
            println!("   ⚠️  Many invalid EMA values for tau={}s ({}/{})", 
                     tau, valid_ema_count, ema_raw.len());
            // Convert to percentiles but log the issue
            let ema_pct = convert_to_percentiles(&ema_raw);
            for (i, row) in combined.iter_mut().enumerate() {
                row.insert(format!("total_engag_{}_pct", suffix), ema_pct[i]);
            }
        } else {
            // Normal case
            let ema_pct = convert_to_percentiles(&ema_raw);
            
            // Additional validation for seizing detection
            if ema_pct.len() > 100 {
                let last_50_values = &ema_pct[ema_pct.len() - 50..];
                let last_50_unique: BTreeSet<OrderedFloat<f64>> = 
                    last_50_values.iter().map(|&v| OrderedFloat(v)).collect();
                
                if last_50_unique.len() == 1 {
                    println!("   🔒 Potential seizing in total_engag_{}s_pct: last 50 values = {:.3}", 
                             suffix, last_50_values[0]);
                }
            }
            
            for (i, row) in combined.iter_mut().enumerate() {
                row.insert(format!("total_engag_{}_pct", suffix), ema_pct[i]);
            }
        }
    }
}

fn get_channel_averaged_value_safe(row: &HashMap<String, f64>, pattern: &str, suffix: &str) -> f64 {
    let chan1_key = format!("chan1_{}{}", pattern, suffix);
    let chan2_key = format!("chan2_{}{}", pattern, suffix);
    
    let chan1_val = row.get(&chan1_key).copied().unwrap_or(0.0);
    let chan2_val = row.get(&chan2_key).copied().unwrap_or(0.0);
    
    // Validate values before averaging
    let chan1_valid = chan1_val.is_finite() && chan1_val >= 0.0;
    let chan2_valid = chan2_val.is_finite() && chan2_val >= 0.0;
    
    match (chan1_valid, chan2_valid) {
        (true, true) => (chan1_val + chan2_val) / 2.0,
        (true, false) => chan1_val,
        (false, true) => chan2_val,
        (false, false) => 0.0,
    }
}

fn convert_to_percentiles(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    
    // Filter out invalid values for min/max calculation
    let valid_values: Vec<f64> = values.iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    
    if valid_values.is_empty() {
        return vec![50.0; values.len()];
    }
    
    // Check for extremely low variation (stuck values)
    let min_val = valid_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = valid_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max_val - min_val;
    
    // Enhanced detection of stuck scenarios
    let unique_values: BTreeSet<OrderedFloat<f64>> = valid_values.iter().map(|&v| OrderedFloat(v)).collect();
    let variation_ratio = unique_values.len() as f64 / valid_values.len() as f64;
    
    if range < f64::EPSILON || (range < 0.01 && variation_ratio < 0.1) {
        // Values are essentially identical or very low variation
        println!("   ⚠️  Low variation detected in percentile conversion (range: {:.6}, variation: {:.3})", 
                 range, variation_ratio);
        
        // Instead of flat 50%, use the actual value scaled appropriately
        let base_percentile = if min_val > 0.0 {
            // If values are positive, scale them reasonably
            (min_val * 10.0).min(90.0).max(10.0)
        } else {
            50.0
        };
        
        return vec![base_percentile; values.len()];
    }
    
    // Add small epsilon to prevent edge cases in percentile calculation
    let safe_range = range.max(0.001);
    let percentiles: Vec<f64> = values.iter()
        .map(|&val| {
            if val.is_finite() {
                let percentile = ((val - min_val) / safe_range) * 100.0;
                percentile.clamp(0.0, 100.0)
            } else {
                0.0
            }
        })
        .collect();
    
    // Additional check for percentile distribution issues
    let percentile_range = percentiles.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - 
                          percentiles.iter().cloned().fold(f64::INFINITY, f64::min);
    
    if percentile_range < 0.1 && percentiles.len() > 100 {
        println!("   ⚠️  Suspicious percentile distribution detected (range: {:.3})", percentile_range);
    }
    
    percentiles
}

// Enhanced main processing function with better error recovery
pub fn process_with_memory_monitoring(
    vocals_file: &str,
    nonvocals_file: &str,
    video_file: &str,
    output_file: &str,
) -> Result<(), Box<dyn Error>> {
    
    println!("🚀 Starting enhanced OOM-safe engagement processing...");
    
    // Validate inputs first
    validate_input_files(vocals_file, nonvocals_file, video_file)?;
    
    let memory_monitor = MemoryMonitor::new();
    process_engagement_streaming_fixed(
        vocals_file,
        nonvocals_file,
        video_file,
        output_file,
        &memory_monitor,
    )?;
    
    println!("✅ Processing complete! Output saved to: {}", output_file);
    Ok(())
}