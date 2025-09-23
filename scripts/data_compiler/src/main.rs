use csv::{ReaderBuilder, WriterBuilder, StringRecord};
use ordered_float::OrderedFloat;
use rayon::prelude::*;
use std::collections::{HashMap, BTreeSet};
use std::env;
use std::error::Error;

/// Struct to store CSV data with original string values
#[derive(Debug)]
struct CsvData {
    headers: Vec<String>,
    rows: Vec<StringRecord>,
    time_index: usize,
}

/// Read CSV into structured format preserving original strings
fn read_csv(path: &str, time_col: &str) -> Result<CsvData, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers_record = rdr.headers()?.clone();
    let headers: Vec<String> = headers_record.iter().map(|h| h.to_string()).collect();

    let time_index = headers.iter().position(|h| h == time_col)
        .ok_or_else(|| format!("Time column '{}' not found in {}", time_col, path))?;

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result?;
        rows.push(record);
    }

    println!("[INFO] Read {} rows from {}", rows.len(), path);
    Ok(CsvData { headers, rows, time_index })
}

/// Parse time value from string, handling potential errors gracefully
fn parse_time(time_str: &str) -> Option<f64> {
    time_str.trim().parse::<f64>().ok()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <time_column> <output.csv> <input1.csv> <input2.csv> ...", args[0]);
        eprintln!("This script merges multiple CSVs on a time column while preserving data fidelity");
        std::process::exit(1);
    }

    let time_col = &args[1];
    let output_path = &args[2];
    let input_files: Vec<_> = args[3..].to_vec();

    println!("[INFO] Merging {} CSV files on time column '{}'", input_files.len(), time_col);

    // Read all CSVs
    let datasets: Vec<CsvData> = input_files
        .iter()
        .map(|f| {
            println!("[INFO] Reading {}", f);
            read_csv(f, time_col)
        })
        .collect::<Result<_, _>>()?;

    // Collect all unique times across all CSVs
    let mut all_times = BTreeSet::new();
    for d in &datasets {
        for row in &d.rows {
            if let Some(time_str) = row.get(d.time_index) {
                if let Some(time_val) = parse_time(time_str) {
                    all_times.insert(OrderedFloat(time_val));
                }
            }
        }
    }

    println!("[INFO] Found {} unique time points", all_times.len());

    // Detect duplicate column names across CSVs (excluding time column)
    let mut col_counts: HashMap<String, usize> = HashMap::new();
    for d in &datasets {
        for h in &d.headers {
            if h != time_col {
                *col_counts.entry(h.clone()).or_default() += 1;
            }
        }
    }

    // Build global header with duplicate handling
    let mut all_headers = vec![time_col.to_string()];
    let mut dup_counters: HashMap<String, usize> = HashMap::new();

    for d in &datasets {
        for h in &d.headers {
            if h == time_col { continue; }

            let header_name = if let Some(&count) = col_counts.get(h) {
                if count > 1 {
                    let counter = dup_counters.entry(h.clone()).or_insert(1);
                    let name = format!("DUP{}_{}", counter, h);
                    *counter += 1;
                    name
                } else {
                    h.clone()
                }
            } else {
                h.clone()
            };

            all_headers.push(header_name);
        }
    }

    // Pre-index CSVs by time for fast lookup, preserving original strings
    let indexed: Vec<HashMap<OrderedFloat<f64>, &StringRecord>> = datasets
        .iter()
        .map(|d| {
            d.rows
                .iter()
                .filter_map(|row| {
                    if let Some(time_str) = row.get(d.time_index) {
                        if let Some(time_val) = parse_time(time_str) {
                            return Some((OrderedFloat(time_val), row));
                        }
                    }
                    None
                })
                .collect()
        })
        .collect();

    // Parallel assembly of rows - preserving original string values
    let rows: Vec<StringRecord> = all_times
        .par_iter()
        .map(|&ordered_time| {
            let time_val = ordered_time.0;
            let mut row_fields: Vec<String> = Vec::new();
            
            // Add time value - format consistently to avoid precision issues
            row_fields.push(format!("{:.6}", time_val));

            // Add data from each dataset
            for (dataset_idx, d) in datasets.iter().enumerate() {
                for (col_idx, h) in d.headers.iter().enumerate() {
                    if h == time_col { continue; }

                    // Look up the row for this time point
                    let field_value = if let Some(source_row) = indexed[dataset_idx].get(&ordered_time) {
                        // Preserve original string value
                        source_row.get(col_idx)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| String::new())
                    } else {
                        // No data at this time point
                        String::new()
                    };

                    row_fields.push(field_value);
                }
            }

            // Create StringRecord from fields
            let mut record = StringRecord::new();
            for field in row_fields {
                record.push_field(&field);
            }
            record
        })
        .collect();

    println!("[INFO] Assembled {} merged rows", rows.len());

    // Write output CSV
    let mut wtr = WriterBuilder::new().from_path(output_path)?;
    
    // Write header
    let mut header_record = StringRecord::new();
    for header in &all_headers {
        header_record.push_field(header);
    }
    wtr.write_record(&header_record)?;

    // Write data rows
    for row in &rows {
        wtr.write_record(row)?;
    }
    
    wtr.flush()?;

    println!("✅ Merged CSV written to: {}", output_path);
    println!("[INFO] Final dimensions: {} columns × {} rows", all_headers.len(), rows.len());
    
    // Print summary of duplicates handled
    if !dup_counters.is_empty() {
        println!("[INFO] Duplicate columns renamed:");
        for (orig, count) in &dup_counters {
            println!("  {} appeared in {} files", orig, count);
        }
    }

    Ok(())
}