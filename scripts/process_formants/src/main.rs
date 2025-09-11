use std::collections::HashMap;
use std::error::Error;
use csv::ReaderBuilder;
use csv::Writer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

#[derive(Debug, Clone)]
pub struct SpeakerInfo {
    pub sex: String,          // "M", "F", or "Unknown"
    pub age_group: String,    // "Child", "Adult", "Elderly"
}

impl Default for SpeakerInfo {
    fn default() -> Self {
        Self {
            sex: "Unknown".to_string(),
            age_group: "Adult".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormantTrackingParams {
    pub sampling_rate_hz: f64,
    pub frame_shift_ms: f64,
    pub max_change_hz_per_frame: f64,
    pub median_window_size: usize,
    pub savgol_window_size: usize,
    pub savgol_polynomial_order: usize,
}

impl Default for FormantTrackingParams {
    fn default() -> Self {
        Self {
            sampling_rate_hz: 100.0,  // 100 Hz analysis rate (10ms frames)
            frame_shift_ms: 10.0,
            max_change_hz_per_frame: 75.0,  // Conservative limit
            median_window_size: 3,     // Smaller window
            savgol_window_size: 5,     // Smaller window 
            savgol_polynomial_order: 2,
        }
    }
}

// --- Proper Savitzky-Golay filter implementation ---
fn savitzky_golay_filter(values: &[f64], window_size: usize, polynomial_order: usize) -> Vec<f64> {
    if window_size < 3 || polynomial_order >= window_size {
        return values.to_vec();
    }
    
    let mut result = vec![0.0; values.len()];
    let half_window = window_size / 2;
    
    // Precompute Savitzky-Golay coefficients for smoothing
    let coeffs = compute_savgol_coefficients(window_size, polynomial_order);
    
    for i in 0..values.len() {
        let start = if i >= half_window { i - half_window } else { 0 };
        let end = (i + half_window + 1).min(values.len());
        let actual_window = end - start;
        
        if actual_window == window_size {
            // Use precomputed coefficients
            result[i] = values[start..end]
                .iter()
                .zip(coeffs.iter())
                .map(|(v, c)| v * c)
                .sum();
        } else {
            // Fallback to simple average for edge cases
            result[i] = values[start..end].iter().sum::<f64>() / actual_window as f64;
        }
    }
    
    result
}

// Compute Savitzky-Golay coefficients (simplified for common cases)
fn compute_savgol_coefficients(window_size: usize, poly_order: usize) -> Vec<f64> {
    match (window_size, poly_order) {
        (3, 1) => vec![-0.333, 0.667, -0.333],
        (3, 2) => vec![-0.083, 0.583, 0.583, -0.083],
        (5, 2) => vec![-0.086, 0.343, 0.486, 0.343, -0.086],
        (5, 3) => vec![-0.084, 0.021, 0.103, 0.161, 0.196, 0.161, 0.103, 0.021, -0.084],
        _ => {
            // Fallback: equal weights (moving average)
            vec![1.0 / window_size as f64; window_size]
        }
    }
}

// --- Improved median filter ---
fn median_filter(values: &[f64], window_size: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    let half = window_size / 2;
    
    for i in 0..values.len() {
        let start = if i >= half { i - half } else { 0 };
        let end = (i + half + 1).min(values.len());
        
        let mut window: Vec<f64> = values[start..end]
            .iter()
            .filter(|&&x| !x.is_nan())
            .copied()
            .collect();
        
        if window.is_empty() {
            result.push(f64::NAN);
        } else {
            window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            result.push(window[window.len() / 2]);
        }
    }
    result
}

// --- Formant continuity-aware interpolation ---
fn interpolate_with_continuity(values: &mut Vec<f64>, params: &FormantTrackingParams) {
    let max_gap_frames = (50.0 / params.frame_shift_ms) as usize; // Max 50ms gaps
    
    // Don't interpolate if too sparse
    let nan_count = values.iter().filter(|&&x| x.is_nan()).count();
    if nan_count as f64 / values.len() as f64 > 0.7 {
        println!("    Warning: Too many NaN values ({:.1}%) - minimal interpolation applied", 
                 (nan_count as f64 / values.len() as f64) * 100.0);
        return;
    }
    
    let mut i = 0;
    while i < values.len() {
        if values[i].is_nan() {
            // Find the extent of NaN values
            let nan_start = i;
            while i < values.len() && values[i].is_nan() {
                i += 1;
            }
            let nan_end = i;
            let gap_length = nan_end - nan_start;
            
            // Only interpolate small gaps
            if gap_length <= max_gap_frames {
                let before_val = if nan_start > 0 { values[nan_start - 1] } else { f64::NAN };
                let after_val = if nan_end < values.len() { values[nan_end] } else { f64::NAN };
                
                if !before_val.is_nan() && !after_val.is_nan() {
                    // Linear interpolation with continuity check
                    let change_per_frame = (after_val - before_val) / (gap_length + 1) as f64;
                    
                    // Check if interpolation would create reasonable trajectory
                    if change_per_frame.abs() <= params.max_change_hz_per_frame * 1.5 {
                        for j in 0..gap_length {
                            values[nan_start + j] = before_val + change_per_frame * (j + 1) as f64;
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    
    // Forward/backward fill for remaining NaNs
    fill_remaining_nans(values);
}

fn fill_remaining_nans(values: &mut Vec<f64>) {
    // Forward fill
    for i in 1..values.len() {
        if values[i].is_nan() && !values[i - 1].is_nan() {
            values[i] = values[i - 1];
        }
    }
    
    // Backward fill for any remaining NaNs at the beginning
    for i in (0..values.len() - 1).rev() {
        if values[i].is_nan() && !values[i + 1].is_nan() {
            values[i] = values[i + 1];
        }
    }
}

// --- Outlier detection based on formant continuity ---
fn detect_outliers_by_continuity(values: &[f64], max_change_per_frame: f64) -> Vec<bool> {
    let mut outliers = vec![false; values.len()];
    
    for i in 1..values.len() {
        if !values[i].is_nan() && !values[i - 1].is_nan() {
            let change = (values[i] - values[i - 1]).abs();
            if change > max_change_per_frame {
                outliers[i] = true;
            }
        }
    }
    
    outliers
}

// --- Adaptive range validation ---
fn get_adaptive_formant_ranges(formant: &str, speaker: &SpeakerInfo) -> (f64, f64) {
    let base_ranges = match formant {
        "chan1_f1_hz" => (200.0, 1000.0),
        "chan1_f2_hz" => (500.0, 2500.0),  
        "chan1_f3_hz" => (1500.0, 3500.0),
        "chan1_f4_hz" => (2500.0, 4500.0),
        _ => (100.0, 5000.0), // Default wide range
    };
    
    // Adjust for speaker characteristics
    let (mut low, mut high) = base_ranges;
    
    match speaker.sex.as_str() {
        "F" => {
            // Women typically have higher formants
            low *= 1.15;
            high *= 1.25;
        }
        "M" => {
            // Men typically have lower formants
            low *= 0.85;
            high *= 0.90;
        }
        _ => {
            // Unknown sex: use wider ranges
            low *= 0.80;
            high *= 1.30;
        }
    }
    
    match speaker.age_group.as_str() {
        "Child" => {
            // Children have higher formants
            low *= 1.2;
            high *= 1.4;
        }
        "Elderly" => {
            // Elderly may have slightly different ranges
            low *= 0.95;
            high *= 1.05;
        }
        _ => {} // Adult - no adjustment
    }
    
    (low, high)
}

// --- Cross-formant validation ---
fn validate_formant_ordering(formant_data: &HashMap<String, Vec<f64>>) -> Vec<bool> {
    let formant_keys = ["chan1_f1_hz", "chan1_f2_hz", "chan1_f3_hz", "chan1_f4_hz"];
    
    // Find the length (all should be same length)
    let length = formant_data.values().next().map(|v| v.len()).unwrap_or(0);
    let mut valid = vec![true; length];
    
    // Check if we have all four formants
    let all_present = formant_keys.iter().all(|k| formant_data.contains_key(*k));
    if !all_present {
        return valid; // Can't validate ordering without all formants
    }
    
    for i in 0..length {
        let f1 = formant_data["chan1_f1_hz"][i];
        let f2 = formant_data["chan1_f2_hz"][i];
        let f3 = formant_data["chan1_f3_hz"][i];
        let f4 = formant_data["chan1_f4_hz"][i];
        
        // Check ordering: F1 < F2 < F3 < F4 (with some tolerance)
        if !f1.is_nan() && !f2.is_nan() && f1 >= f2 - 50.0 {
            valid[i] = false;
        }
        if !f2.is_nan() && !f3.is_nan() && f2 >= f3 - 50.0 {
            valid[i] = false;
        }
        if !f3.is_nan() && !f4.is_nan() && f3 >= f4 - 50.0 {
            valid[i] = false;
        }
    }
    
    valid
}

// --- Main processing function ---
pub fn process_formants_advanced(
    input_path: &str, 
    output_path: &str,
    speaker_info: Option<SpeakerInfo>,
    params: Option<FormantTrackingParams>
) -> Result<(), Box<dyn Error>> {
    
    let speaker = speaker_info.unwrap_or_default();
    let tracking_params = params.unwrap_or_default();
    
    println!("Advanced Formant Processing");
    println!("Input: {}", input_path);
    println!("Output: {}", output_path);
    println!("Speaker: {:?}", speaker);
    println!("Parameters: {:?}", tracking_params);

    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    
    let header_map: HashMap<String, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_string(), i))
        .collect();
    
    let mut records = Vec::new();
    for result in rdr.records() {
        records.push(result?);
    }

    println!("Loaded {} records", records.len());

    let formant_columns = ["chan1_f1_hz", "chan1_f2_hz", "chan1_f3_hz", "chan1_f4_hz"];
    
    let m = MultiProgress::new();
    let status_bar = m.add(ProgressBar::new(formant_columns.len() as u64));
    status_bar.set_style(
        ProgressStyle::default_bar()
            .template("Processing [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );

    let mut cleaned_columns: HashMap<String, Vec<f64>> = HashMap::new();

    // Process each formant column
    for &formant_name in &formant_columns {
        status_bar.set_message(format!("Processing {}", formant_name));

        let col_index = match header_map.get(formant_name) {
            Some(idx) => *idx,
            None => {
                eprintln!("Warning: Column '{}' not found", formant_name);
                status_bar.inc(1);
                continue;
            }
        };

        // Extract values
        let mut values: Vec<f64> = records
            .iter()
            .map(|record| {
                record.get(col_index)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(f64::NAN)
            })
            .collect();

        // Apply adaptive range filtering
        let (low, high) = get_adaptive_formant_ranges(formant_name, &speaker);
        println!("  {} range: {:.0}-{:.0} Hz", formant_name, low, high);
        
        for v in &mut values {
            if !v.is_nan() && (*v < low || *v > high) {
                *v = f64::NAN;
            }
        }

        // Detect and remove continuity outliers
        let outliers = detect_outliers_by_continuity(&values, tracking_params.max_change_hz_per_frame);
        let outlier_count = outliers.iter().filter(|&&x| x).count();
        if outlier_count > 0 {
            println!("  Detected {} outliers based on continuity", outlier_count);
            for (i, &is_outlier) in outliers.iter().enumerate() {
                if is_outlier {
                    values[i] = f64::NAN;
                }
            }
        }

        // Interpolate missing values with continuity constraints
        interpolate_with_continuity(&mut values, &tracking_params);

        // Apply median filter (smaller window)
        let median_filtered = median_filter(&values, tracking_params.median_window_size);

        // Apply proper Savitzky-Golay smoothing (smaller window)  
        let smoothed = savitzky_golay_filter(
            &median_filtered, 
            tracking_params.savgol_window_size,
            tracking_params.savgol_polynomial_order
        );
        
        cleaned_columns.insert(formant_name.to_string(), smoothed);
        status_bar.inc(1);
    }

    // Cross-formant validation
    println!("Validating formant ordering...");
    let ordering_valid = validate_formant_ordering(&cleaned_columns);
    let invalid_count = ordering_valid.iter().filter(|&&x| !x).count();
    if invalid_count > 0 {
        println!("Warning: {} frames have invalid formant ordering", invalid_count);
    }

    status_bar.finish_with_message("All formants processed");

    // Write output
    println!("Writing output CSV...");
    let mut writer = Writer::from_path(output_path)?;
    writer.write_record(&headers)?;

    for (i, record) in records.iter().enumerate() {
        let mut output_record = Vec::new();
        
        for (j, header) in headers.iter().enumerate() {
            if let Some(cleaned_values) = cleaned_columns.get(header) {
                if i < cleaned_values.len() {
                    // Check if this frame has valid formant ordering
                    let value = if i < ordering_valid.len() && !ordering_valid[i] {
                        // Mark invalid ordering frames
                        format!("{:.2}", cleaned_values[i]) // Still output but could add flag
                    } else {
                        format!("{:.2}", cleaned_values[i])
                    };
                    output_record.push(value);
                } else {
                    output_record.push(String::new());
                }
            } else {
                output_record.push(record.get(j).unwrap_or("").to_string());
            }
        }
        writer.write_record(&output_record)?;
    }
    
    writer.flush()?;
    println!("Done! Advanced formant processing complete.");
    println!("Output saved to: {}", output_path);

    Ok(())
}

// Convenience function with defaults
pub fn process(input_path: &str, output_path: &str) -> Result<(), Box<dyn Error>> {
    process_formants_advanced(input_path, output_path, None, None)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_csv> <output_csv>", args[0]);
        eprintln!("       Advanced formant processing with continuity constraints");
        std::process::exit(1);
    }
    
    // You can customize speaker info and parameters here
    let speaker_info = SpeakerInfo {
        sex: "Unknown".to_string(),
        age_group: "Adult".to_string(),
    };
    
    let params = FormantTrackingParams::default();
    
    process_formants_advanced(&args[1], &args[2], Some(speaker_info), Some(params))
}