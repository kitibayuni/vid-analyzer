use std::fs::File;
use std::io::BufReader;
use claxon::FlacReader;
use csv::Writer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

pub fn process(input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input FLAC file: {}", input_path);
    println!("Output CSV file: {}", output_path);
    
    // --- OPEN FLAC ---
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;
    let samplerate = flac.streaminfo().sample_rate as usize;
    let channels = flac.streaminfo().channels as usize;
    
    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);
    
    // --- ANALYSIS PARAMETERS ---
    let window_len = (0.050 * samplerate as f64) as usize; // 50ms analysis windows
    let hop_len = (0.010 * samplerate as f64) as usize;    // 10ms hop for overlap
    let min_f0 = 50.0;  // Minimum expected F0 (Hz)
    let max_f0 = 500.0; // Maximum expected F0 (Hz)
    
    println!(
        "Analysis parameters: {:.1}ms windows, {:.1}ms hop, F0 range: {:.0}-{:.0} Hz",
        window_len as f64 / samplerate as f64 * 1000.0,
        hop_len as f64 / samplerate as f64 * 1000.0,
        min_f0, max_f0
    );
    
    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
    
    let channel_bar = m.add(ProgressBar::new(channels as u64));
    channel_bar.set_style(
        ProgressStyle::default_bar()
            .template("Channels [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("|  "),
    );
    
    // --- LOAD SAMPLES INTO CHANNEL BUFFERS ---
    status_bar.set_message("[ == SLICING DATA INTO CHANNEL BUFFERS == ]");
    let total_samples = flac.streaminfo().samples.unwrap_or(0) as usize;
    let mut channel_buffers: Vec<Vec<f64>> =
        vec![Vec::with_capacity(total_samples / channels.max(1)); channels];
    
    for (i, sample) in flac.samples().enumerate() {
        let s = sample?;
        let chan = i % channels;
        channel_buffers[chan].push(s as f64 / i32::MAX as f64);
    }
    
    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.push(format!("chan{}_f0_hz", c + 1));
        headers.push(format!("chan{}_jitter_local_percent", c + 1));
        headers.push(format!("chan{}_jitter_ppq5_percent", c + 1)); // PPQ5: 5-point period perturbation quotient
        headers.push(format!("chan{}_shimmer_local_percent", c + 1));
        headers.push(format!("chan{}_shimmer_apq5_percent", c + 1)); // APQ5: 5-point amplitude perturbation quotient
        headers.push(format!("chan{}_hnr_db", c + 1)); // Harmonics-to-Noise Ratio
    }
    writer.write_record(&headers)?;
    
    let mut all_results: Vec<Vec<JitterShimmerResult>> = Vec::new();
    
    // --- PROCESS EACH CHANNEL ---
    for (chan_idx, samples) in channel_buffers.iter().enumerate() {
        let has_audio = samples.iter().any(|&v| v.abs() > 1e-6);
        if !has_audio {
            println!("Channel {} has no data, skipping.", chan_idx + 1);
            all_results.push(Vec::new());
            channel_bar.inc(1);
            continue;
        }
        
        status_bar.set_message(format!("[ == PROCESSING CHANNEL {} == ]", chan_idx + 1));
        
        // Create analysis windows
        let num_windows = (samples.len().saturating_sub(window_len)) / hop_len + 1;
        let window_bar = m.add(ProgressBar::new(num_windows as u64));
        window_bar.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "[{{elapsed_precise}}] Ch.{} Windows [{{wide_bar}}] {{pos}}/{{len}} ({{eta}})",
                    chan_idx + 1
                ))
                .unwrap()
                .progress_chars("|  "),
        );
        
        let window_indices: Vec<usize> = (0..samples.len())
            .step_by(hop_len)
            .take_while(|&i| i + window_len <= samples.len())
            .collect();
        
        // Parallel processing of windows
        let results: Vec<JitterShimmerResult> = window_indices
            .par_iter()
            .map(|&start| {
                let end = start + window_len;
                let window = &samples[start..end];
                let time_sec = start as f64 / samplerate as f64;
                
                let result = analyze_window(window, samplerate, time_sec, min_f0, max_f0);
                window_bar.inc(1);
                result
            })
            .collect();
        
        all_results.push(results);
        window_bar.finish_with_message(format!("Channel {} complete", chan_idx + 1));
        channel_bar.inc(1);
    }
    
    channel_bar.finish_with_message("All channels complete");
    status_bar.set_message("[ == WRITING CSV == ]");
    
    // --- WRITE CSV ---
    let max_len = all_results.iter().map(|v| v.len()).max().unwrap_or(0);
    for i in 0..max_len {
        let mut row: Vec<String> = Vec::with_capacity(channels * 6 + 1);
        
        // Get time from first available channel
        let time_sec = all_results
            .iter()
            .find_map(|chan| chan.get(i).map(|r| r.time_sec));
        row.push(time_sec.map(|t| format!("{:.4}", t)).unwrap_or_default());
        
        for chan_results in &all_results {
            if let Some(result) = chan_results.get(i) {
                row.push(result.f0_hz.map(|f| format!("{:.2}", f)).unwrap_or_default());
                row.push(result.jitter_local.map(|j| format!("{:.4}", j)).unwrap_or_default());
                row.push(result.jitter_ppq5.map(|j| format!("{:.4}", j)).unwrap_or_default());
                row.push(result.shimmer_local.map(|s| format!("{:.4}", s)).unwrap_or_default());
                row.push(result.shimmer_apq5.map(|s| format!("{:.4}", s)).unwrap_or_default());
                row.push(result.hnr_db.map(|h| format!("{:.2}", h)).unwrap_or_default());
            } else {
                for _ in 0..6 {
                    row.push("".to_string());
                }
            }
        }
        writer.write_record(&row)?;
    }
    
    writer.flush()?;
    status_bar.finish_with_message("[ == JITTER/SHIMMER ANALYSIS COMPLETE == ]");
    println!("Done. Output saved to {}", output_path);
    Ok(())
}

#[derive(Debug, Clone)]
struct JitterShimmerResult {
    time_sec: f64,
    f0_hz: Option<f64>,
    jitter_local: Option<f64>,
    jitter_ppq5: Option<f64>,
    shimmer_local: Option<f64>,
    shimmer_apq5: Option<f64>,
    hnr_db: Option<f64>,
}

fn analyze_window(
    window: &[f64],
    sample_rate: usize,
    time_sec: f64,
    min_f0: f64,
    max_f0: f64,
) -> JitterShimmerResult {
    // Apply window function (Hann window)
    let windowed: Vec<f64> = window
        .iter()
        .enumerate()
        .map(|(i, &sample)| {
            let window_val = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (window.len() - 1) as f64).cos());
            sample * window_val
        })
        .collect();
    
    // Estimate F0 using autocorrelation
    let f0_hz = estimate_f0_autocorr(&windowed, sample_rate, min_f0, max_f0);
    
    if f0_hz.is_none() {
        return JitterShimmerResult {
            time_sec,
            f0_hz: None,
            jitter_local: None,
            jitter_ppq5: None,
            shimmer_local: None,
            shimmer_apq5: None,
            hnr_db: None,
        };
    }
    
    let f0 = f0_hz.unwrap();
    let period_samples = sample_rate as f64 / f0;
    
    // Extract periods for jitter/shimmer analysis
    let periods = extract_periods(&windowed, period_samples, sample_rate);
    
    if periods.len() < 5 {
        return JitterShimmerResult {
            time_sec,
            f0_hz: Some(f0),
            jitter_local: None,
            jitter_ppq5: None,
            shimmer_local: None,
            shimmer_apq5: None,
            hnr_db: None,
        };
    }
    
    // Calculate jitter measures
    let jitter_local = calculate_jitter_local(&periods, sample_rate);
    let jitter_ppq5 = calculate_jitter_ppq5(&periods, sample_rate);
    
    // Calculate shimmer measures
    let amplitudes: Vec<f64> = periods.iter().map(|p| calculate_amplitude(p)).collect();
    let shimmer_local = calculate_shimmer_local(&amplitudes);
    let shimmer_apq5 = calculate_shimmer_apq5(&amplitudes);
    
    // Calculate HNR (simplified approximation)
    let hnr_db = calculate_hnr(&windowed, f0, sample_rate);
    
    JitterShimmerResult {
        time_sec,
        f0_hz: Some(f0),
        jitter_local,
        jitter_ppq5,
        shimmer_local,
        shimmer_apq5,
        hnr_db,
    }
}

fn estimate_f0_autocorr(signal: &[f64], sample_rate: usize, min_f0: f64, max_f0: f64) -> Option<f64> {
    let min_lag = (sample_rate as f64 / max_f0) as usize;
    let max_lag = (sample_rate as f64 / min_f0) as usize;
    
    if max_lag >= signal.len() || min_lag >= max_lag {
        return None;
    }
    
    let mut max_corr = 0.0;
    let mut best_lag = 0;
    
    for lag in min_lag..=max_lag.min(signal.len() - 1) {
        let mut correlation = 0.0;
        let mut norm1 = 0.0;
        let mut norm2 = 0.0;
        
        let len = signal.len() - lag;
        for i in 0..len {
            correlation += signal[i] * signal[i + lag];
            norm1 += signal[i] * signal[i];
            norm2 += signal[i + lag] * signal[i + lag];
        }
        
        if norm1 > 0.0 && norm2 > 0.0 {
            let normalized_corr = correlation / (norm1 * norm2).sqrt();
            if normalized_corr > max_corr {
                max_corr = normalized_corr;
                best_lag = lag;
            }
        }
    }
    
    if max_corr > 0.3 && best_lag > 0 {
        Some(sample_rate as f64 / best_lag as f64)
    } else {
        None
    }
}

fn extract_periods(signal: &[f64], period_samples: f64, sample_rate: usize) -> Vec<Vec<f64>> {
    let mut periods = Vec::new();
    let period_len = period_samples as usize;
    
    if period_len == 0 || period_len >= signal.len() {
        return periods;
    }
    
    let mut start = 0;
    while start + period_len < signal.len() {
        let period = signal[start..start + period_len].to_vec();
        periods.push(period);
        start += period_len;
    }
    
    periods
}

fn calculate_amplitude(period: &[f64]) -> f64 {
    period.iter().map(|&x| x.abs()).fold(0.0, f64::max)
}

fn calculate_jitter_local(periods: &[Vec<f64>], sample_rate: usize) -> Option<f64> {
    if periods.len() < 2 {
        return None;
    }
    
    let period_durations: Vec<f64> = periods.iter()
        .map(|p| p.len() as f64 / sample_rate as f64)
        .collect();
    
    let mut abs_diffs = Vec::new();
    for i in 1..period_durations.len() {
        abs_diffs.push((period_durations[i] - period_durations[i-1]).abs());
    }
    
    let mean_abs_diff: f64 = abs_diffs.iter().sum::<f64>() / abs_diffs.len() as f64;
    let mean_period: f64 = period_durations.iter().sum::<f64>() / period_durations.len() as f64;
    
    if mean_period > 0.0 {
        Some((mean_abs_diff / mean_period) * 100.0) // Convert to percentage
    } else {
        None
    }
}

fn calculate_jitter_ppq5(periods: &[Vec<f64>], sample_rate: usize) -> Option<f64> {
    if periods.len() < 5 {
        return None;
    }
    
    let period_durations: Vec<f64> = periods.iter()
        .map(|p| p.len() as f64 / sample_rate as f64)
        .collect();
    
    let mut ppq5_values = Vec::new();
    for i in 2..period_durations.len()-2 {
        let mean_5 = (period_durations[i-2] + period_durations[i-1] + period_durations[i] + 
                     period_durations[i+1] + period_durations[i+2]) / 5.0;
        ppq5_values.push((period_durations[i] - mean_5).abs());
    }
    
    let mean_ppq5: f64 = ppq5_values.iter().sum::<f64>() / ppq5_values.len() as f64;
    let mean_period: f64 = period_durations.iter().sum::<f64>() / period_durations.len() as f64;
    
    if mean_period > 0.0 {
        Some((mean_ppq5 / mean_period) * 100.0) // Convert to percentage
    } else {
        None
    }
}

fn calculate_shimmer_local(amplitudes: &[f64]) -> Option<f64> {
    if amplitudes.len() < 2 {
        return None;
    }
    
    let mut abs_diffs = Vec::new();
    for i in 1..amplitudes.len() {
        abs_diffs.push((amplitudes[i] - amplitudes[i-1]).abs());
    }
    
    let mean_abs_diff: f64 = abs_diffs.iter().sum::<f64>() / abs_diffs.len() as f64;
    let mean_amplitude: f64 = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
    
    if mean_amplitude > 0.0 {
        Some((mean_abs_diff / mean_amplitude) * 100.0) // Convert to percentage
    } else {
        None
    }
}

fn calculate_shimmer_apq5(amplitudes: &[f64]) -> Option<f64> {
    if amplitudes.len() < 5 {
        return None;
    }
    
    let mut apq5_values = Vec::new();
    for i in 2..amplitudes.len()-2 {
        let mean_5 = (amplitudes[i-2] + amplitudes[i-1] + amplitudes[i] + 
                     amplitudes[i+1] + amplitudes[i+2]) / 5.0;
        apq5_values.push((amplitudes[i] - mean_5).abs());
    }
    
    let mean_apq5: f64 = apq5_values.iter().sum::<f64>() / apq5_values.len() as f64;
    let mean_amplitude: f64 = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
    
    if mean_amplitude > 0.0 {
        Some((mean_apq5 / mean_amplitude) * 100.0) // Convert to percentage
    } else {
        None
    }
}

fn calculate_hnr(signal: &[f64], f0: f64, sample_rate: usize) -> Option<f64> {
    if f0 <= 0.0 {
        return None;
    }
    
    let period_samples = sample_rate as f64 / f0;
    let num_periods = (signal.len() as f64 / period_samples).floor() as usize;
    
    if num_periods < 2 {
        return None;
    }
    
    let period_len = period_samples as usize;
    let mut harmonic_power = 0.0;
    let mut noise_power = 0.0;
    let mut total_power = 0.0;
    
    // Simple HNR estimation: compare periodic vs non-periodic energy
    for i in 0..signal.len().saturating_sub(period_len) {
        let current = signal[i];
        let delayed = signal[i + period_len];
        
        total_power += current * current;
        let correlation = current * delayed;
        
        if correlation > 0.0 {
            harmonic_power += correlation;
        } else {
            noise_power += correlation.abs();
        }
    }
    
    if noise_power > 0.0 && harmonic_power > 0.0 {
        Some(10.0 * (harmonic_power / noise_power).log10())
    } else if harmonic_power > 0.0 {
        Some(40.0) // High HNR when little noise detected
    } else {
        Some(0.0) // Low HNR when no harmonic content
    }
}