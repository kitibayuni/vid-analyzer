use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::path::Path;

/// Exponential Moving Average
fn ema(values: &[f64], alpha: f64) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
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

/// Count words per second
fn words_per_second(time_values: &[f64]) -> HashMap<i64, usize> {
    let mut counts = HashMap::new();
    for &t in time_values {
        let sec = t.floor() as i64;
        *counts.entry(sec).or_insert(0) += 1;
    }
    counts
}

/// Lexicon entry for MWEs or unigrams
#[derive(Debug, Clone)]
struct LexiconEntry {
    words: Vec<String>,
    features: HashMap<String, f64>,
}

/// Load a CSV lexicon (assumes first column is term, rest numeric features)
fn load_lexicon(path: &str) -> Result<HashMap<String, Vec<LexiconEntry>>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers = rdr.headers()?.clone();
    let mut lex_map: HashMap<String, Vec<LexiconEntry>> = HashMap::new();
    for result in rdr.records() {
        let rec = result?;
        let term = rec.get(0).unwrap_or("").to_lowercase();
        let words: Vec<String> = term.split_whitespace().map(|w| w.to_string()).collect();
        let mut features = HashMap::new();
        for (i, h) in headers.iter().enumerate().skip(1) {
            if let Ok(val) = rec.get(i).unwrap_or("0").parse::<f64>() {
                features.insert(h.to_string(), val);
            }
        }
        lex_map.entry(words[0].clone()).or_default().push(LexiconEntry { words, features });
    }
    Ok(lex_map)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 {
        eprintln!("Usage: {} <transcript.csv> <emo.csv> <vad.csv> <wcst.csv> <worry.csv>", args[0]);
        std::process::exit(1);
    }

    let transcript_path = &args[1];
    let emo_path = &args[2];
    let vad_path = &args[3];
    let wcst_path = &args[4];
    let worry_path = &args[5];

    // Load lexicons
    let emo_lex = load_lexicon(emo_path)?;
    let vad_lex = load_lexicon(vad_path)?;
    let wcst_lex = load_lexicon(wcst_path)?;
    let worry_lex = load_lexicon(worry_path)?;

    // Read transcript
    let mut rdr = ReaderBuilder::new().from_path(transcript_path)?;
    let headers = rdr.headers()?.clone();
    let time_idx = headers.iter().position(|h| h == "time_sec").ok_or("Missing time_sec")?;
    let word_idx = headers.iter().position(|h| h == "word").ok_or("Missing word")?;

    let mut words: Vec<(f64, String)> = Vec::new();
    for result in rdr.records() {
        let rec = result?;
        let t: f64 = rec.get(time_idx).unwrap_or("0").parse().unwrap_or(0.0);
        let w = rec.get(word_idx).unwrap_or("").to_lowercase();
        words.push((t, w));
    }

    // Compute word counts per second
    let times: Vec<f64> = words.iter().map(|(t, _)| *t).collect();
    let wps_map = words_per_second(&times);

    // Build 0.2s timeline
    let start = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let end = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut timeline = Vec::new();
    let mut t = start;
    while t <= end + 1e-6 {
        timeline.push((t * 5.0).round() / 5.0); // multiples of 0.2
        t += 0.2;
    }

    // Map words to timeline slots
    let mut word_map: HashMap<f64, String> = HashMap::new();
    for (time, w) in &words {
        let slot = (time * 5.0).round() / 5.0;
        word_map.insert(slot, w.clone());
    }

    // Initialize lexicon feature map per timeline slot
    let mut lex_features: Vec<HashMap<String, f64>> = vec![HashMap::new(); timeline.len()];

    for (i, &tt) in timeline.iter().enumerate() {
        if let Some(word) = word_map.get(&tt) {
            let word_lower = word.to_lowercase();

            // Check all lexicons
            for lex in &[&emo_lex, &vad_lex, &wcst_lex, &worry_lex] {
                if let Some(entries) = lex.get(&word_lower) {
                    // Find the longest MWE match
                    let mut best_entry = None;
                    for entry in entries {
                        let mut match_ok = true;
                        for (offset, w_check) in entry.words.iter().enumerate().skip(1) {
                            let future_idx = timeline.get(i + offset);
                            let future_word = future_idx.and_then(|fut_t| word_map.get(fut_t));
                            if future_word.map(|fw| fw != w_check).unwrap_or(true) {
                                match_ok = false;
                                break;
                            }
                        }
                        if match_ok {
                            if best_entry.as_ref().map_or(true, |be: &LexiconEntry| entry.words.len() > be.words.len()) {
                                best_entry = Some(entry.clone());
                            }
                        }
                    }
                    if let Some(be) = best_entry {
                        for (k, v) in &be.features {
                            lex_features[i].insert(k.clone(), *v);
                        }
                    }
                }
            }
        }
    }

    // Build words per second series
    let wps_series: Vec<f64> = timeline
        .iter()
        .map(|&tt| {
            let sec = tt.floor() as i64;
            wps_map.get(&sec).cloned().unwrap_or(0) as f64
        })
        .collect();

    // Compute 5s EMA for words per second
    let alpha_5s = 2.0 / (5.0 + 1.0);
    let wps_ema_5s = ema(&wps_series, alpha_5s);

    // Write output CSV
    let input_file = Path::new(transcript_path);
    let output_path = input_file.with_file_name(format!(
        "{}_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Headers
    let mut out_headers = vec!["time_sec", "word", "words_per_sec", "words_per_sec_ema_5s"];
    // Add lexicon feature headers
    let mut all_features: Vec<String> = lex_features.iter()
        .flat_map(|fmap| fmap.keys().cloned())
        .collect();
    all_features.sort();
    all_features.dedup();
    out_headers.extend(all_features.iter().map(|s| s.as_str()));
    wtr.write_record(&out_headers)?;

    // Write rows
    for (i, &tt) in timeline.iter().enumerate() {
        let mut row: Vec<String> = vec![
            format!("{:.1}", tt),
            word_map.get(&tt).cloned().unwrap_or_default(),
            wps_series[i].to_string(),
            wps_ema_5s[i].to_string(),
        ];
        let fmap = &lex_features[i];
        for feat in &all_features {
            row.push(fmap.get(feat).cloned().unwrap_or(0.0).to_string());
        }
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV saved with lexicon features, MWEs, words per second, and 5s EMA!");
    Ok(())
}
