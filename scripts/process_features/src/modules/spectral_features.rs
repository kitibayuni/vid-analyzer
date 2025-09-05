use std::fs::File;
use std::io::BufReader;
use claxon::FlacReader;
use csv::Writer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rustfft::{FftPlanner, num_complex::Complex};

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
    
    // --- FRAME PARAMETERS ---
    let frame_len = (0.025 * samplerate as f64) as usize; // 25ms frames
    let fft_size = frame_len.next_power_of_two(); // FFT size (power of 2)
    println!("Calculating spectral features using {}-sample frames (~25ms), FFT size: {}", frame_len, fft_size);
    
    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
    
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
    
    // --- FFT SETUP ---
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    
    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.push(format!("chan{}_spectral_centroid", c + 1));
        headers.push(format!("chan{}_spectral_rolloff", c + 1));
        headers.push(format!("chan{}_spectral_bandwidth", c + 1));
        headers.push(format!("chan{}_spectral_flatness", c + 1));
        headers.push(format!("chan{}_zero_crossing_rate", c + 1));
    }
    writer.write_record(&headers)?;
    
    // --- CALCULATE SPECTRAL FEATURES PER CHANNEL ---
    status_bar.set_message("[ == CALCULATING SPECTRAL FEATURES == ]");
    let max_len = channel_buffers.iter().map(|v| v.len()).max().unwrap_or(0);
    let frame_hop = frame_len; // non-overlapping frames
    
    for start in (0..max_len).step_by(frame_hop) {
        let time_sec = start as f64 / samplerate as f64;
        let mut row: Vec<String> = vec![format!("{:.4}", time_sec)];
        
        for chan in &channel_buffers {
            if start >= chan.len() {
                // Add empty values for all spectral features
                for _ in 0..5 {
                    row.push("".to_string());
                }
                continue;
            }
            
            let end = (start + frame_len).min(chan.len());
            let frame = &chan[start..end];
            
            // Prepare FFT input (pad with zeros if necessary)
            let mut fft_input: Vec<Complex<f64>> = frame.iter()
                .map(|&x| Complex::new(x, 0.0))
                .collect();
            fft_input.resize(fft_size, Complex::new(0.0, 0.0));
            
            // Apply window (Hamming)
            apply_hamming_window(&mut fft_input);
            
            // Perform FFT
            fft.process(&mut fft_input);
            
            // Calculate magnitude spectrum
            let magnitude_spectrum: Vec<f64> = fft_input[0..fft_size/2]
                .iter()
                .map(|c| c.norm())
                .collect();
            
            // Calculate spectral features
            let spectral_centroid = calculate_spectral_centroid(&magnitude_spectrum, samplerate);
            let spectral_rolloff = calculate_spectral_rolloff(&magnitude_spectrum, samplerate, 0.85);
            let spectral_bandwidth = calculate_spectral_bandwidth(&magnitude_spectrum, samplerate, spectral_centroid);
            let spectral_flatness = calculate_spectral_flatness(&magnitude_spectrum);
            let zero_crossing_rate = calculate_zero_crossing_rate(frame);
            
            row.push(format!("{:.2}", spectral_centroid));
            row.push(format!("{:.2}", spectral_rolloff));
            row.push(format!("{:.2}", spectral_bandwidth));
            row.push(format!("{:.6}", spectral_flatness));
            row.push(format!("{:.6}", zero_crossing_rate));
        }
        
        writer.write_record(&row)?;
    }
    
    writer.flush()?;
    status_bar.finish_with_message("[ == SPECTRAL FEATURES CSV COMPLETE == ]");
    println!("Done. Output saved to {}", output_path);
    Ok(())
}

fn apply_hamming_window(samples: &mut [Complex<f64>]) {
    let n = samples.len();
    for (i, sample) in samples.iter_mut().enumerate() {
        let window_val = 0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
        *sample *= window_val;
    }
}

fn calculate_spectral_centroid(magnitude_spectrum: &[f64], sample_rate: usize) -> f64 {
    let mut weighted_sum = 0.0;
    let mut magnitude_sum = 0.0;
    
    for (i, &magnitude) in magnitude_spectrum.iter().enumerate() {
        let freq = i as f64 * sample_rate as f64 / (2.0 * magnitude_spectrum.len() as f64);
        weighted_sum += freq * magnitude;
        magnitude_sum += magnitude;
    }
    
    if magnitude_sum > 0.0 {
        weighted_sum / magnitude_sum
    } else {
        0.0
    }
}

fn calculate_spectral_rolloff(magnitude_spectrum: &[f64], sample_rate: usize, rolloff_percent: f64) -> f64 {
    let total_energy: f64 = magnitude_spectrum.iter().map(|&x| x * x).sum();
    let threshold = total_energy * rolloff_percent;
    
    let mut cumulative_energy = 0.0;
    for (i, &magnitude) in magnitude_spectrum.iter().enumerate() {
        cumulative_energy += magnitude * magnitude;
        if cumulative_energy >= threshold {
            return i as f64 * sample_rate as f64 / (2.0 * magnitude_spectrum.len() as f64);
        }
    }
    
    sample_rate as f64 / 2.0 // Nyquist frequency
}

fn calculate_spectral_bandwidth(magnitude_spectrum: &[f64], sample_rate: usize, centroid: f64) -> f64 {
    let mut weighted_variance = 0.0;
    let mut magnitude_sum = 0.0;
    
    for (i, &magnitude) in magnitude_spectrum.iter().enumerate() {
        let freq = i as f64 * sample_rate as f64 / (2.0 * magnitude_spectrum.len() as f64);
        let diff = freq - centroid;
        weighted_variance += diff * diff * magnitude;
        magnitude_sum += magnitude;
    }
    
    if magnitude_sum > 0.0 {
        (weighted_variance / magnitude_sum).sqrt()
    } else {
        0.0
    }
}

fn calculate_spectral_flatness(magnitude_spectrum: &[f64]) -> f64 {
    let geometric_mean = magnitude_spectrum.iter()
        .filter(|&&x| x > 0.0)
        .map(|&x| x.ln())
        .sum::<f64>() / magnitude_spectrum.len() as f64;
    
    let arithmetic_mean = magnitude_spectrum.iter().sum::<f64>() / magnitude_spectrum.len() as f64;
    
    if arithmetic_mean > 0.0 {
        geometric_mean.exp() / arithmetic_mean
    } else {
        0.0
    }
}

fn calculate_zero_crossing_rate(frame: &[f64]) -> f64 {
    if frame.len() < 2 {
        return 0.0;
    }
    
    let mut crossings = 0;
    for i in 1..frame.len() {
        if (frame[i] >= 0.0) != (frame[i - 1] >= 0.0) {
            crossings += 1;
        }
    }
    
    crossings as f64 / (frame.len() - 1) as f64
}