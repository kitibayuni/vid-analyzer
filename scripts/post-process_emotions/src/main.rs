use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

/// Exponential Moving Average
fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
    let mut result = Vec::with_capacity(values.len());
    let mut prev = values[0];
    result.push(prev);
    for &val in &values[1..] {
        let next = alpha * val + (1.0 - alpha) * prev;
        result.push(next);
        prev = next;
    }
    result
}

/// Rolling variance
fn rolling_variance(values: &[f64], window: usize) -> Vec<f64> {
    let mut var = vec![0.0; values.len()];
    for i in 0..values.len() {
        let start = if i >= window { i + 1 - window } else { 0 };
        let window_slice = &values[start..=i];
        let mean = window_slice.iter().sum::<f64>() / window_slice.len() as f64;
        let v = window_slice
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / window_slice.len() as f64;
        var[i] = v;
    }
    var
}

/// Normalize to 0–1
fn normalize_01(values: &[f64]) -> Vec<f64> {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < 1e-8 {
        return vec![0.0; values.len()];
    }
    values.iter().map(|v| (v - min) / (max - min)).collect()
}

/// Percentile rank
fn percentile_rank(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values.iter().map(|v| {
        let count = sorted.iter().filter(|&&x| x <= *v).count();
        count as f64 / values.len() as f64
    }).collect()
}

/// Engagement score from VAD + categorical emotions
fn calculate_engagement(
    vad: (&[f64], &[f64], &[f64]), // valence, arousal, dominance
    cats: (&[f64], &[f64], &[f64], &[f64]), // neu, hap, ang, sad
    confidence: &[f64],
) -> Vec<f64> {
    let (valence, arousal, dominance) = vad;
    let (neu, hap, ang, sad) = cats;

    let mut engagement = Vec::with_capacity(valence.len());
    for i in 0..valence.len() {
        // VAD component
        let vad_component = 0.4 * arousal[i] + 0.3 * valence[i].abs() + 0.3 * dominance[i];

        // Categorical component: treat happiness + anger as more engaging than neutral/sad
        let cat_component = 0.4 * hap[i] + 0.3 * ang[i] + 0.2 * sad[i] + 0.1 * (1.0 - neu[i]);

        // Confidence weighting
        let conf_component = confidence[i];

        let total = 0.6 * vad_component + 0.3 * cat_component + 0.1 * conf_component;
        engagement.push(total.max(0.0));
    }
    engagement
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_emotions.csv>", args[0]);
        eprintln!("Expected columns: chunk_index,time_sec,end_sec,valence,arousal,dominance,cat_neu,cat_hap,cat_ang,cat_sad,predicted_emotion,confidence");
        std::process::exit(1);
    }

    let input_path = &args[1];

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    println!("[INFO] Processing {} rows from emotion CSV", records.len());

    // Required columns
    let required_cols = [
        ("valence", "valence"),
        ("arousal", "arousal"),
        ("dominance", "dominance"),
        ("cat_neu", "cat_neu"),
        ("cat_hap", "cat_hap"),
        ("cat_ang", "cat_ang"),
        ("cat_sad", "cat_sad"),
        ("confidence", "confidence"),
    ];

    let mut col_indices: HashMap<&str, usize> = HashMap::new();
    for (key, col_name) in &required_cols {
        let idx = headers.iter().position(|h| h == *col_name)
            .ok_or_else(|| format!("Required column '{}' not found", col_name))?;
        col_indices.insert(*key, idx);
    }

    // Extract columns
    let valence: Vec<f64> = records.iter().map(|r| r.get(col_indices["valence"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let arousal: Vec<f64> = records.iter().map(|r| r.get(col_indices["arousal"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let dominance: Vec<f64> = records.iter().map(|r| r.get(col_indices["dominance"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let neu: Vec<f64> = records.iter().map(|r| r.get(col_indices["cat_neu"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let hap: Vec<f64> = records.iter().map(|r| r.get(col_indices["cat_hap"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let ang: Vec<f64> = records.iter().map(|r| r.get(col_indices["cat_ang"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let sad: Vec<f64> = records.iter().map(|r| r.get(col_indices["cat_sad"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();
    let confidence: Vec<f64> = records.iter().map(|r| r.get(col_indices["confidence"]).unwrap().parse::<f64>().unwrap_or(0.0)).collect();

    // Compute engagement
    let engagement_raw = calculate_engagement((&valence, &arousal, &dominance), (&neu, &hap, &ang, &sad), &confidence);
    let engagement_norm = normalize_01(&engagement_raw);

    // EMAs
    let rows_per_sec = 1; // assuming chunk_sec is constant in preprocessing
    let alpha_1s = 2.0 / ((1.0 * rows_per_sec as f64) + 1.0);
    let alpha_5s = 2.0 / ((5.0 * rows_per_sec as f64) + 1.0);
    let alpha_10s = 2.0 / ((10.0 * rows_per_sec as f64) + 1.0);

    let engagement_ema_1s = ema(&engagement_norm, alpha_1s);
    let engagement_ema_5s = ema(&engagement_norm, alpha_5s);
    let engagement_ema_10s = ema(&engagement_norm, alpha_10s);

    let engagement_percentile = percentile_rank(&engagement_norm);
    let engagement_variance_5s = rolling_variance(&engagement_norm, 5 * rows_per_sec);

    // Write output
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_emo_engage.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(&output_path)?;

    // Create new headers
    let mut new_headers = csv::StringRecord::new();
    for h in headers.iter() {
        let renamed = match &h[..] {
            "valence" | "arousal" | "dominance" => format!("emo_{}", h),
            _ => h.to_string(),
        };
        new_headers.push_field(&renamed);
    }

    // Append engagement feature columns
    let engagement_features = [
        "emotion_engage_score",
        "emotion_engage_ema_1s",
        "emotion_engage_ema_5s",
        "emotion_engage_ema_10s",
        "emotion_engage_percentile",
        "emotion_engage_variance_5s",
    ];
    for feat in &engagement_features {
        new_headers.push_field(feat);
    }

    wtr.write_record(&new_headers)?;


    for i in 0..records.len() {
        let mut row: Vec<String> = records[i].iter().map(|s| s.to_string()).collect();
        row.push(format!("{:.6}", engagement_norm[i]));
        row.push(format!("{:.6}", engagement_ema_1s[i]));
        row.push(format!("{:.6}", engagement_ema_5s[i]));
        row.push(format!("{:.6}", engagement_ema_10s[i]));
        row.push(format!("{:.6}", engagement_percentile[i]));
        row.push(format!("{:.6}", engagement_variance_5s[i]));
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("✅ Emotion engagement analysis saved to: {}", output_path.display());

    Ok(())
}
