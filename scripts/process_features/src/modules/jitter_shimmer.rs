use hound::{SampleFormat, WavReader};
use csv::{ReaderBuilder, Writer};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;

/// Load pitch CSV into a map: channel_idx -> Vec<Option<f64>>
fn load_pitch_csv(path: &str) -> Result<HashMap<usize, Vec<Option<f64>>>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut pitch_map: HashMap<usize, Vec<Option<f64>>> = HashMap::new();

    for (i, header) in headers.iter().enumerate() {
        if header.contains("_f0_hz") {
            pitch_map.insert(i, Vec::new());
        }
    }

    for result in rdr.records() {
        let record = result?;
        for (i, &(_, col_idx)) in headers
            .iter()
            .enumerate()
            .filter(|(_, h)| h.contains("_f0_hz"))
            .enumerate()
        {
            let val = &record[col_idx];
            let pitch = val.parse::<f64>().ok();
            pitch_map.get_mut(&col_idx).unwrap().push(pitch);
        }
    }

    Ok(pitch_map)
}

/// Process a WAV file and calculate jitter, shimmer, and HNR using precomputed pitch
pub fn process(
    input_path: &str,
    pitch_csv_path: &str,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input WAV file: {}", input_path);
    println!("Pitch CSV file: {}", pitch_csv_path);
    println!("Output CSV file: {}", output_path);

    // --- OPEN WAV FILE ---
    let reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();
    let samplerate = spec.sample_rate as usize;
    let channels = spec.channels as usize;

    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- LOAD PITCH CSV ---
    let pitch_map = load_pitch_csv(pitch_csv_path)?;
    
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
    let mut channel_buffers: Vec<Vec<f64>> = vec![Vec::new(); channels];

    match spec.sample_format {
        SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => {
                    for (i, sample) in reader.into_samples::<i16>().enumerate() {
                        let s = sample?;
                        let chan = i % channels;
                        channel_buffers[chan].push(s as f64 / i16::MAX as f64);
                    }
                }
                24 => {
                    for (i, sample) in reader.into_samples::<i32>().enumerate() {
                        let s = sample?;
                        let s24 = (s >> 8) as f64;
                        let chan = i % channels;
                        channel_buffers[chan].push(s24 / 8_388_607.0);
                    }
                }
                32 => {
                    for (i, sample) in reader.into_samples::<i32>().enumerate() {
                        let s = sample?;
                        let chan = i % channels;
                        channel_buffers[chan].push(s as f64 / i32::MAX as f64);
                    }
                }
                other => {
                    return Err(format!("Unsupported integer bit depth: {} bits", other).into());
                }
            }
        }
        SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(format!("Unsupported float bit depth: {} bits", spec.bits_per_sample).into());
            }
            for (i, sample) in reader.into_samples::<f32>().enumerate() {
                let s = sample?;
                let chan = i % channels;
                channel_buffers[chan].push(s as f64);
            }
        }
    }

    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.push(format!("chan{}_f0_hz", c + 1));
        headers.push(format!("chan{}_jitter_local_percent", c + 1));
        headers.push(format!("chan{}_jitter_ppq5_percent", c + 1));
        headers.push(format!("chan{}_shimmer_local_percent", c + 1));
        headers.push(format!("chan{}_shimmer_apq5_percent", c + 1));
        headers.push(format!("chan{}_hnr_db", c + 1));
    }
    writer.write_record(&headers)?;

    // --- PROCESS EACH CHANNEL ---
    let window_len = (0.050 * samplerate as f64) as usize; // 50 ms
    let hop_len = (0.010 * samplerate as f64) as usize; // 10 ms

    let mut all_results: Vec<Vec<JitterShimmerResult>> = Vec::new();

    for (chan_idx, samples) in channel_buffers.iter().enumerate() {
        let has_audio = samples.iter().any(|&v| v.abs() > 1e-6);
        if !has_audio {
            println!("Channel {} has no data, skipping.", chan_idx + 1);
            all_results.push(Vec::new());
            channel_bar.inc(1);
            continue;
        }

        let pitch_vec = pitch_map.get(&chan_idx).cloned().unwrap_or_default();

        status_bar.set_message(format!("[ == PROCESSING CHANNEL {} == ]", chan_idx + 1));

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

        let results: Vec<JitterShimmerResult> = window_indices
            .par_iter()
            .enumerate()
            .map(|(win_idx, &start)| {
                let end = start + window_len;
                let window = &samples[start..end];
                let time_sec = start as f64 / samplerate as f64;

                // Use pitch from CSV
                let f0_hz = pitch_vec.get(win_idx).cloned().unwrap_or(None);

                let result = if let Some(f0) = f0_hz {
                    if f0 > 0.0 {
                        analyze_window_with_pitch(window, samplerate, time_sec, f0)
                    } else {
                        JitterShimmerResult::empty(time_sec)
                    }
                } else {
                    JitterShimmerResult::empty(time_sec)
                };

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

impl JitterShimmerResult {
    fn empty(time_sec: f64) -> Self {
        JitterShimmerResult {
            time_sec,
            f0_hz: None,
            jitter_local: None,
            jitter_ppq5: None,
            shimmer_local: None,
            shimmer_apq5: None,
            hnr_db: None,
        }
    }
}

/// Analyze a window using **precomputed pitch**
fn analyze_window_with_pitch(
    window: &[f64],
    sample_rate: usize,
    time_sec: f64,
    f0_hz: f64,
) -> JitterShimmerResult {
    let windowed: Vec<f64> = window
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (window.len() - 1) as f64).cos());
            x * w
        })
        .collect();

    let period_samples = sample_rate as f64 / f0_hz;
    let periods = extract_periods(&windowed, period_samples);

    let (jitter_local, jitter_ppq5) = if periods.len() >= 5 {
        (calculate_jitter_local(&periods, sample_rate), calculate_jitter_ppq5(&periods, sample_rate))
    } else {
        (None, None)
    };

    let amplitudes: Vec<f64> = periods.iter().map(|p| calculate_amplitude(p)).collect();
    let (shimmer_local, shimmer_apq5) = if amplitudes.len() >= 5 {
        (calculate_shimmer_local(&amplitudes), calculate_shimmer_apq5(&amplitudes))
    } else {
        (None, None)
    };

    let hnr_db = calculate_hnr(&windowed, f0_hz, sample_rate);

    JitterShimmerResult {
        time_sec,
        f0_hz: Some(f0_hz),
        jitter_local,
        jitter_ppq5,
        shimmer_local,
        shimmer_apq5,
        hnr_db,
    }
}

// --- Same helper functions as before ---
fn extract_periods(signal: &[f64], period_samples: f64) -> Vec<Vec<f64>> {
    let mut periods = Vec::new();
    let period_len = period_samples as usize;
    if period_len == 0 || period_len >= signal.len() {
        return periods;
    }
    let mut start = 0;
    while start + period_len < signal.len() {
        periods.push(signal[start..start + period_len].to_vec());
        start += period_len;
    }
    periods
}

fn calculate_amplitude(period: &[f64]) -> f64 {
    period.iter().map(|&x| x.abs()).fold(0.0, f64::max)
}

fn calculate_jitter_local(periods: &[Vec<f64>], sample_rate: usize) -> Option<f64> {
    if periods.len() < 2 { return None; }
    let durations: Vec<f64> = periods.iter().map(|p| p.len() as f64 / sample_rate as f64).collect();
    let diffs: Vec<f64> = durations.windows(2).map(|w| (w[1]-w[0]).abs()).collect();
    let mean_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let mean_period: f64 = durations.iter().sum::<f64>() / durations.len() as f64;
    if mean_period > 0.0 { Some((mean_diff/mean_period)*100.0) } else { None }
}

fn calculate_jitter_ppq5(periods: &[Vec<f64>], sample_rate: usize) -> Option<f64> {
    if periods.len() < 5 { return None; }
    let durations: Vec<f64> = periods.iter().map(|p| p.len() as f64 / sample_rate as f64).collect();
    let ppq5_values: Vec<f64> = (2..durations.len()-2).map(|i| {
        let mean5 = durations[i-2..=i+2].iter().sum::<f64>()/5.0;
        (durations[i]-mean5).abs()
    }).collect();
    let mean_ppq5 = ppq5_values.iter().sum::<f64>() / ppq5_values.len() as f64;
    let mean_period: f64 = durations.iter().sum::<f64>() / durations.len() as f64;
    if mean_period > 0.0 { Some((mean_ppq5/mean_period)*100.0) } else { None }
}

fn calculate_shimmer_local(amplitudes: &[f64]) -> Option<f64> {
    if amplitudes.len() < 2 { return None; }
    let diffs: Vec<f64> = amplitudes.windows(2).map(|w| (w[1]-w[0]).abs()).collect();
    let mean_diff: f64 = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let mean_amp: f64 = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
    if mean_amp > 0.0 { Some((mean_diff/mean_amp)*100.0) } else { None }
}

fn calculate_shimmer_apq5(amplitudes: &[f64]) -> Option<f64> {
    if amplitudes.len() < 5 { return None; }
    let apq5_values: Vec<f64> = (2..amplitudes.len()-2).map(|i| {
        let mean5 = amplitudes[i-2..=i+2].iter().sum::<f64>() / 5.0;
        (amplitudes[i]-mean5).abs()
    }).collect();
    let mean_apq5 = apq5_values.iter().sum::<f64>() / apq5_values.len() as f64;
    let mean_amp: f64 = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
    if mean_amp > 0.0 { Some((mean_apq5/mean_amp)*100.0) } else { None }
}

fn calculate_hnr(signal: &[f64], f0: f64, _sample_rate: usize) -> Option<f64> {
    if f0 <= 0.0 { return None; }
    let mut harmonic = 0.0;
    let mut noise = 0.0;
    for i in 0..signal.len().saturating_sub(1) {
        let current = signal[i];
        let delayed = signal[i + 1];
        let corr = current * delayed;
        if corr > 0.0 { harmonic += corr; } else { noise += corr.abs(); }
    }
    if noise > 0.0 && harmonic > 0.0 { Some(10.0 * (harmonic/noise).log10()) }
    else if harmonic > 0.0 { Some(40.0) }
    else { Some(0.0) }
}
