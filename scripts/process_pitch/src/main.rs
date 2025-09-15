use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

const FEATURES: &[&str] = &[
    "pitch_norm", "pitch_variation", "pitch_range", "pitch_modulation",
    "jitter_local", "jitter_ppq5", "shimmer_local", "shimmer_apq5",
    "hnr", "vfeats_engage_score",
    "vfeats_engage_ema_1s", "vfeats_engage_ema_5s", "vfeats_engage_ema_10s",
    "vfeats_engage_ema_1s_pct", "vfeats_engage_ema_5s_pct", "vfeats_engage_ema_10s_pct"
];

#[derive(Default)]
struct WindowFeatures {
    pitch_norm: f64,
    pitch_variation: f64,
    pitch_range: f64,
    pitch_modulation: f64,
    jitter_local: f64,
    jitter_ppq5: f64,
    shimmer_local: f64,
    shimmer_apq5: f64,
    hnr: f64,
    engagement_score: f64,
}

impl WindowFeatures {
    fn to_vec(&self) -> Vec<f64> {
        vec![
            self.pitch_norm, self.pitch_variation, self.pitch_range, self.pitch_modulation,
            self.jitter_local, self.jitter_ppq5, self.shimmer_local, self.shimmer_apq5,
            self.hnr, self.engagement_score
        ]
    }
}

// --- Utility Functions ---
fn safe_compare(a: &f64, b: &f64) -> std::cmp::Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
    }
}

fn parse_numeric_value(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("nan") || trimmed.eq_ignore_ascii_case("none") {
        0.0
    } else {
        trimmed.parse::<f64>().unwrap_or(0.0)
    }
}

fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    let mut prev = *values.get(0).unwrap_or(&0.0);
    result.push(prev);
    for &v in &values[1..] {
        let next = alpha * v + (1.0 - alpha) * prev;
        result.push(next);
        prev = next;
    }
    result
}

fn percentile_rank(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(safe_compare);
    values.iter().map(|&v| {
        let count = sorted.iter().filter(|&&x| !x.is_nan() && x <= v).count();
        count as f64 / values.len() as f64
    }).collect()
}

fn rolling_variance(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>()/n;
    values.iter().map(|x| (x-mean).powi(2)).sum::<f64>() / n
}

fn rolling_range(values: &[f64]) -> f64 {
    let min = values.iter().filter(|v| v.is_finite()).fold(f64::INFINITY, |a,&b| a.min(b));
    let max = values.iter().filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, |a,&b| a.max(b));
    if min.is_infinite() || max.is_infinite() { 0.0 } else { max-min }
}

fn calculate_pitch_modulation(values: &[f64]) -> f64 {
    values.windows(2).map(|w|(w[1]-w[0]).abs()).sum()
}

fn calculate_periods(f0_hz: &[f64]) -> Vec<f64> {
    f0_hz.iter().filter_map(|&f| if f>0.0 && f.is_finite() { Some(1.0/f) } else { None }).collect()
}

fn calculate_jitter_local(periods: &[f64]) -> f64 {
    if periods.len()<2 { return 0.0; }
    let mean = periods.iter().sum::<f64>()/periods.len() as f64;
    let diff = periods.windows(2).map(|w|(w[1]-w[0]).abs()).sum::<f64>() / (periods.len()-1) as f64;
    (diff/mean*100.0).min(100.0)
}

fn calculate_jitter_ppq5(periods: &[f64]) -> f64 {
    if periods.len()<6 { return 0.0; }
    let mean = periods.iter().sum::<f64>()/periods.len() as f64;
    let diff = periods.windows(6).map(|w|(w[5]-w[0]).abs()).sum::<f64>() / (periods.len()-5) as f64;
    (diff/mean*100.0).min(100.0)
}

fn calculate_shimmer_local(amplitudes: &[f64]) -> f64 {
    if amplitudes.len()<2 { return 0.0; }
    let mean = amplitudes.iter().sum::<f64>()/amplitudes.len() as f64;
    let diff = amplitudes.windows(2).map(|w|(w[1]-w[0]).abs()).sum::<f64>() / (amplitudes.len()-1) as f64;
    (diff/mean*100.0).min(100.0)
}

fn calculate_shimmer_apq5(amplitudes: &[f64]) -> f64 {
    if amplitudes.len()<6 { return 0.0; }
    let mean = amplitudes.iter().sum::<f64>()/amplitudes.len() as f64;
    let diff = amplitudes.windows(6).map(|w| {
        let avg5 = w[0..5].iter().sum::<f64>()/5.0;
        (w[5]-avg5).abs()
    }).sum::<f64>() / (amplitudes.len()-5) as f64;
    (diff/mean*100.0).min(100.0)
}

fn calculate_hnr(f0_hz: &[f64]) -> f64 {
    let finite: Vec<f64> = f0_hz.iter().filter(|&&f| f>0.0 && f.is_finite()).copied().collect();
    if finite.is_empty() { return 0.0; }
    let mean_power = finite.iter().map(|f| f*f).sum::<f64>() / finite.len() as f64;
    10.0*(mean_power/1e-6).log10()
}

fn normalize_01(values: &[f64]) -> Vec<f64> {
    let finite: Vec<f64> = values.iter().filter(|v| v.is_finite()).copied().collect();
    if finite.is_empty() { return vec![0.0; values.len()]; }
    let min = finite.iter().fold(f64::INFINITY, |a,&b| a.min(b));
    let max = finite.iter().fold(f64::NEG_INFINITY, |a,&b| a.max(b));
    if (max-min).abs()<f64::EPSILON { return vec![0.0; values.len()]; }
    values.iter().map(|&v| if v.is_finite() {(v-min)/(max-min)} else {0.0}).collect()
}

fn process_window(pitch_window: &[f64]) -> WindowFeatures {
    let periods = calculate_periods(pitch_window);
    let amplitudes: Vec<f64> = pitch_window.iter().map(|v| v.abs()).collect();
    let pitch_norm = pitch_window.iter().filter(|v| v.is_finite()).sum::<f64>()/pitch_window.len() as f64;
    let pitch_variation = rolling_variance(pitch_window);
    let pitch_range = rolling_range(pitch_window);
    let pitch_modulation = calculate_pitch_modulation(pitch_window);
    let jitter_local = calculate_jitter_local(&periods);
    let jitter_ppq5 = calculate_jitter_ppq5(&periods);
    let shimmer_local = calculate_shimmer_local(&amplitudes);
    let shimmer_apq5 = calculate_shimmer_apq5(&amplitudes);
    let hnr = calculate_hnr(pitch_window);
    let engagement_score = (0.25*pitch_variation + 0.25*pitch_modulation + 0.15*jitter_local.min(100.0)
        + 0.15*shimmer_local.min(100.0) + 0.20*hnr.max(0.0)).max(0.0);
    WindowFeatures {pitch_norm,pitch_variation,pitch_range,pitch_modulation,jitter_local,jitter_ppq5,shimmer_local,shimmer_apq5,hnr,engagement_score}
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len()<2 { eprintln!("Usage: {} <input.csv> [--time_col TIME_COLUMN]", args[0]); std::process::exit(1); }
    let input_path = &args[1];
    let mut time_col = "time_sec".to_string();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--time_col" => { if i+1<args.len() { time_col=args[i+1].clone(); i+=2; } else { eprintln!("--time_col requires a value"); std::process::exit(1); } }
            _ => { eprintln!("Unknown argument: {}", args[i]); std::process::exit(1); }
        }
    }

    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_,_>>()?;

    let channels: Vec<String> = headers.iter()
        .filter_map(|h| h.strip_suffix("_pitch_hz").map(|s| s.to_string()))
        .collect();
    if channels.is_empty() { eprintln!("No pitch columns found"); std::process::exit(1); }

    let time_idx = headers.iter().position(|h| h==&time_col)
        .ok_or(format!("Time column '{}' not found", time_col))?;
    let time_values: Vec<f64> = records.iter().map(|r| parse_numeric_value(r.get(time_idx).unwrap_or("0"))).collect();

    let avg_delta = time_values.windows(2).map(|w| w[1]-w[0]).sum::<f64>()/(time_values.len()-1) as f64;
    let rows_per_sec = (1.0/avg_delta).round() as usize;
    let window_sec=1.0; let hop_sec=0.1;
    let window_len = ((window_sec/avg_delta).round() as usize).max(1);
    let hop_len = ((hop_sec/avg_delta).round() as usize).max(1);

    let alpha_1s = 2.0/(1.0*rows_per_sec as f64+1.0);
    let alpha_5s = 2.0/(5.0*rows_per_sec as f64+1.0);
    let alpha_10s = 2.0/(10.0*rows_per_sec as f64+1.0);

    let mut all_results: HashMap<String, Vec<WindowFeatures>> = HashMap::new();
    let mut window_times = Vec::new();

    for ch in &channels {
        let pitch_idx = headers.iter().position(|h| h==&format!("{}_pitch_hz", ch)).unwrap();
        let pitch_vals: Vec<f64> = records.iter().map(|r| parse_numeric_value(r.get(pitch_idx).unwrap_or("0"))).collect();
        let mut channel_results = Vec::new();
        let mut idx = 0;
        while idx+window_len <= pitch_vals.len() {
            if ch==&channels[0] { window_times.push(time_values[idx]); }
            let window = &pitch_vals[idx..idx+window_len];
            channel_results.push(process_window(window));
            idx += hop_len;
        }
        all_results.insert(ch.clone(), channel_results);
    }

    let mut final_results: HashMap<String, HashMap<String, Vec<f64>>> = HashMap::new();

    for (ch, features) in &all_results {
        let raw: Vec<Vec<f64>> = (0..10).map(|i| features.iter().map(|f| f.to_vec()[i]).collect()).collect();
        let mut channel_data: HashMap<String, Vec<f64>> = FEATURES[0..10].iter()
            .enumerate()
            .map(|(i,f)| {
                let key = if f.contains("vfeats_engage") { f.to_string() } else { f.to_string() };
                (key, normalize_01(&raw[i]))
            }).collect();

        let engagement = channel_data.get("vfeats_engage_score").unwrap();
        let ema_1s = ema(engagement, alpha_1s);
        let ema_5s = ema(engagement, alpha_5s);
        let ema_10s = ema(engagement, alpha_10s);

        channel_data.insert("vfeats_engage_ema_1s".to_string(), ema_1s.clone());
        channel_data.insert("vfeats_engage_ema_5s".to_string(), ema_5s.clone());
        channel_data.insert("vfeats_engage_ema_10s".to_string(), ema_10s.clone());

        channel_data.insert("vfeats_engage_ema_1s_pct".to_string(), percentile_rank(&ema_1s));
        channel_data.insert("vfeats_engage_ema_5s_pct".to_string(), percentile_rank(&ema_5s));
        channel_data.insert("vfeats_engage_ema_10s_pct".to_string(), percentile_rank(&ema_10s));
        final_results.insert(ch.clone(), channel_data);
    }

    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!("{}_processed.csv", input_file.file_stem().unwrap().to_string_lossy()));
    let mut wtr = WriterBuilder::new().from_path(&output_path)?;

    let mut out_headers = vec!["time_sec".to_string()];
    for ch in &channels {
        for f in FEATURES {
            out_headers.push(format!("{}_{}", ch, f));
        }
    }


    wtr.write_record(&out_headers)?;  // <-- this line is missing in your code

    for i in 0..window_times.len() {
        let mut row = vec![format!("{:.6}", window_times[i])];
        for ch in &channels {
            if let Some(cd) = final_results.get(ch) {
                for f in FEATURES {
                    let val = cd.get(*f).and_then(|v| v.get(i)).copied().unwrap_or(0.0);
                    row.push(format!("{:.6}", val));
                }
            } else {
                for _ in FEATURES {
                    row.push("0.0".to_string());
                }
            }
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Output written to: {}", output_path.display());
    Ok(())
}
