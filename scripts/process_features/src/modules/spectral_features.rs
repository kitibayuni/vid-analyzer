use hound;
use csv::Writer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Arc;

pub fn process(input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input WAV file: {}", input_path);
    println!("Output CSV file: {}", output_path);

    // --- OPEN WAV ---
    let reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();
    let samplerate = spec.sample_rate as usize;
    let channels = spec.channels as usize;
    println!("Sample rate: {} Hz, {} channel(s)", samplerate, channels);

    // --- FRAME PARAMETERS ---
    let frame_len = (0.025 * samplerate as f64) as usize; // 25ms frames
    let fft_size = frame_len.next_power_of_two();
    println!(
        "Calculating spectral features using {}-sample frames (~25ms), FFT size: {}",
        frame_len, fft_size
    );

    // --- MULTIPROGRESS ---
    let m = MultiProgress::new();
    let status_bar = m.add(ProgressBar::new(1));
    status_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());

    // --- LOAD SAMPLES INTO CHANNEL BUFFERS ---
    status_bar.set_message("[== SLICING DATA INTO CHANNEL BUFFERS ==]");
    let mut channel_buffers: Vec<Vec<f64>> = vec![Vec::new(); channels];

    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = 2f64.powi(spec.bits_per_sample as i32 - 1) - 1.0;
            for (i, sample) in reader.into_samples::<i32>().enumerate() {
                let s = sample? as f64 / max_val;
                let chan = i % channels;
                channel_buffers[chan].push(s);
            }
        }
        hound::SampleFormat::Float => {
            for (i, sample) in reader.into_samples::<f32>().enumerate() {
                let s = sample? as f64;
                let chan = i % channels;
                channel_buffers[chan].push(s);
            }
        }
    }

    // --- FFT SETUP ---
    let mut planner = FftPlanner::new();
    let fft: Arc<dyn rustfft::Fft<f64>> = planner.plan_fft_forward(fft_size);

    // --- CSV SETUP ---
    let mut writer = Writer::from_path(output_path)?;
    let mut headers = vec!["time_sec".to_string()];
    for c in 0..channels {
        headers.extend([
            format!("chan{}_spectral_centroid", c + 1),
            format!("chan{}_spectral_rolloff", c + 1),
            format!("chan{}_spectral_bandwidth", c + 1),
            format!("chan{}_spectral_flatness", c + 1),
            format!("chan{}_spectral_flux", c + 1),
            format!("chan{}_zero_crossing_rate", c + 1),
        ]);
    }
    writer.write_record(&headers)?;

    // --- PROCESS FRAMES ---
    let max_len = channel_buffers.iter().map(|v| v.len()).max().unwrap_or(0);
    let frame_hop = frame_len; // non-overlapping
    let mut prev_magnitude_spectra: Vec<Option<Vec<f64>>> = vec![None; channels];

    for start in (0..max_len).step_by(frame_hop) {
        let time_sec = start as f64 / samplerate as f64;
        let mut row: Vec<String> = vec![format!("{:.4}", time_sec)];

        for (chan_idx, chan) in channel_buffers.iter().enumerate() {
            if start >= chan.len() {
                row.extend(vec!["".to_string(); 6]);
                continue;
            }

            let end = (start + frame_len).min(chan.len());
            let frame = &chan[start..end];

            // FFT
            let magnitude_spectrum = compute_fft_spectrum(frame, fft_size, &*fft);

            // Spectral features
            let spectral_centroid = calculate_spectral_centroid(&magnitude_spectrum, samplerate);
            let spectral_rolloff = calculate_spectral_rolloff(&magnitude_spectrum, samplerate, 0.85);
            let spectral_bandwidth = calculate_spectral_bandwidth(&magnitude_spectrum, samplerate, spectral_centroid);
            let spectral_flatness = calculate_spectral_flatness(&magnitude_spectrum);
            let spectral_flux = calculate_spectral_flux(&magnitude_spectrum, &prev_magnitude_spectra[chan_idx]);
            let zero_crossing_rate = calculate_zero_crossing_rate(frame);

            row.push(format!("{:.2}", spectral_centroid));
            row.push(format!("{:.2}", spectral_rolloff));
            row.push(format!("{:.2}", spectral_bandwidth));
            row.push(format!("{:.6}", spectral_flatness));
            row.push(format!("{:.6}", spectral_flux));
            row.push(format!("{:.6}", zero_crossing_rate));

            prev_magnitude_spectra[chan_idx] = Some(magnitude_spectrum);
        }

        writer.write_record(&row)?;
    }

    writer.flush()?;
    status_bar.finish_with_message("[== SPECTRAL FEATURES CSV COMPLETE ==]");
    println!("Done. Output saved to {}", output_path);
    Ok(())
}

// --- FFT computation ---
fn compute_fft_spectrum(frame: &[f64], fft_size: usize, fft: &dyn rustfft::Fft<f64>) -> Vec<f64> {
    let mut buffer: Vec<Complex<f64>> = frame.iter().map(|&x| Complex::new(x, 0.0)).collect();
    buffer.resize(fft_size, Complex::new(0.0, 0.0));
    apply_hamming_window(&mut buffer);
    fft.process(&mut buffer);
    buffer[0..fft_size / 2].iter().map(|c| c.norm()).collect()
}

// --- WINDOW ---
fn apply_hamming_window(samples: &mut [Complex<f64>]) {
    let n = samples.len();
    for (i, sample) in samples.iter_mut().enumerate() {
        let w = 0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
        *sample *= w;
    }
}

// --- SPECTRAL FEATURES ---
fn calculate_spectral_centroid(mag: &[f64], sr: usize) -> f64 {
    let mut weighted_sum = 0.0;
    let mut mag_sum = 0.0;
    for (i, &m) in mag.iter().enumerate() {
        let freq = i as f64 * sr as f64 / (2.0 * mag.len() as f64);
        weighted_sum += freq * m;
        mag_sum += m;
    }
    if mag_sum > 0.0 { weighted_sum / mag_sum } else { 0.0 }
}

fn calculate_spectral_rolloff(mag: &[f64], sr: usize, rolloff_percent: f64) -> f64 {
    let total_energy: f64 = mag.iter().map(|&x| x * x).sum();
    let threshold = total_energy * rolloff_percent;
    let mut cum_energy = 0.0;
    for (i, &m) in mag.iter().enumerate() {
        cum_energy += m * m;
        if cum_energy >= threshold {
            return i as f64 * sr as f64 / (2.0 * mag.len() as f64);
        }
    }
    sr as f64 / 2.0
}

fn calculate_spectral_bandwidth(mag: &[f64], sr: usize, centroid: f64) -> f64 {
    let mut weighted_var = 0.0;
    let mut mag_sum = 0.0;
    for (i, &m) in mag.iter().enumerate() {
        let freq = i as f64 * sr as f64 / (2.0 * mag.len() as f64);
        let diff = freq - centroid;
        weighted_var += diff * diff * m;
        mag_sum += m;
    }
    if mag_sum > 0.0 { (weighted_var / mag_sum).sqrt() } else { 0.0 }
}

fn calculate_spectral_flatness(mag: &[f64]) -> f64 {
    let geometric_mean = mag.iter().filter(|&&x| x > 0.0).map(|&x| x.ln()).sum::<f64>() / mag.len() as f64;
    let arithmetic_mean = mag.iter().sum::<f64>() / mag.len() as f64;
    if arithmetic_mean > 0.0 { geometric_mean.exp() / arithmetic_mean } else { 0.0 }
}

fn calculate_spectral_flux(current: &[f64], previous: &Option<Vec<f64>>) -> f64 {
    match previous {
        Some(prev) => {
            if prev.len() != current.len() { return 0.0; }
            current.iter().zip(prev.iter()).map(|(c,p)| (c-p).powi(2)).sum::<f64>().sqrt()
        }
        None => 0.0,
    }
}

fn calculate_zero_crossing_rate(frame: &[f64]) -> f64 {
    if frame.len() < 2 { return 0.0; }
    let mut crossings = 0;
    for i in 1..frame.len() {
        if (frame[i] >= 0.0) != (frame[i-1] >= 0.0) { crossings += 1; }
    }
    crossings as f64 / (frame.len() - 1) as f64
}
