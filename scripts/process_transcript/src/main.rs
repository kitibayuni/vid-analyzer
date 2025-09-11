use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::Path;

/// Count how many words occur within each 1-second bucket.
fn words_per_second(time_values: &[f64]) -> HashMap<i64, usize> {
    let mut counts = HashMap::new();
    for &t in time_values {
        let sec = t.floor() as i64;
        *counts.entry(sec).or_insert(0) += 1;
    }
    counts
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input.csv>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];

    // Read CSV
    let mut rdr = ReaderBuilder::new().from_path(input_path)?;
    let headers = rdr.headers()?.clone();
    let time_idx = headers.iter().position(|h| h == "time_sec").ok_or("Missing time_sec column")?;
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;

    // Extract times
    let time_values: Vec<f64> = records
        .iter()
        .map(|r| r.get(time_idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0))
        .collect();

    // Compute words per second
    let wps_map = words_per_second(&time_values);

    // Prepare output file
    let input_file = Path::new(input_path);
    let output_path = input_file.with_file_name(format!(
        "{}_processed.csv",
        input_file.file_stem().unwrap().to_string_lossy()
    ));
    let mut wtr = WriterBuilder::new().from_path(output_path)?;

    // Write headers
    let mut new_headers = headers.clone();
    new_headers.push_field("words_per_sec");
    wtr.write_record(&new_headers)?;

    // Write rows
    for rec in &records {
        let mut row: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        let t: f64 = rec.get(time_idx).unwrap().parse().unwrap_or(0.0);
        let sec = t.floor() as i64;
        let wps = wps_map.get(&sec).cloned().unwrap_or(0);
        row.push(wps.to_string());
        wtr.write_record(&row)?;
    }

    wtr.flush()?;
    println!("Processed CSV with words_per_sec saved successfully!");

    Ok(())
}
