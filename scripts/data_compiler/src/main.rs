use csv::{ReaderBuilder, WriterBuilder};
use ordered_float::OrderedFloat;
use rayon::prelude::*;
use std::collections::{HashMap, BTreeSet};
use std::env;
use std::error::Error;

/// Struct to store CSV data
#[derive(Debug)]
struct CsvData {
    headers: Vec<String>,
    rows: Vec<HashMap<String, f64>>,
}

/// Read CSV into structured format
fn read_csv(path: &str, time_col: &str) -> Result<CsvData, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers: Vec<String> = rdr.headers()?.iter().map(|h| h.to_string()).collect();

    if !headers.contains(&time_col.to_string()) {
        return Err(format!("Time column '{}' not found in {}", time_col, path).into());
    }

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result?;
        let mut row_map = HashMap::new();
        for (h, v) in headers.iter().zip(record.iter()) {
            if let Ok(val) = v.parse::<f64>() {
                row_map.insert(h.clone(), val);
            }
        }
        rows.push(row_map);
    }

    Ok(CsvData { headers, rows })
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <time_column> <output.csv> <input1.csv> <input2.csv> ...", args[0]);
        std::process::exit(1);
    }

    let time_col = &args[1];
    let output_path = &args[2];
    let input_files: Vec<_> = args[3..].to_vec();

    // Read all CSVs
    let datasets: Vec<CsvData> = input_files
        .iter()
        .map(|f| read_csv(f, time_col))
        .collect::<Result<_, _>>()?;

    // Collect all unique times across all CSVs
    let mut all_times = BTreeSet::new();
    for d in &datasets {
        for r in &d.rows {
            if let Some(&t) = r.get(time_col) {
                all_times.insert(OrderedFloat(t));
            }
        }
    }

    // Detect duplicates across CSVs
    let mut col_counts: HashMap<String, usize> = HashMap::new();
    for d in &datasets {
        for h in &d.headers {
            if h != time_col {
                *col_counts.entry(h.clone()).or_default() += 1;
            }
        }
    }

    // Prepare global header
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

    // Pre-index CSVs by time for fast lookup
    let indexed: Vec<HashMap<OrderedFloat<f64>, &HashMap<String,f64>>> = datasets
        .iter()
        .map(|d| {
            d.rows
                .iter()
                .filter_map(|r| r.get(time_col).map(|&t| (OrderedFloat(t), r)))
                .collect()
        })
        .collect();

    // Parallel assembly of rows
    let rows: Vec<Vec<String>> = all_times
        .par_iter()
        .map(|&OrderedFloat(time_val)| {
            let mut row: Vec<String> = Vec::new();
            row.push(format!("{:.6}", time_val));

            for (i, d) in datasets.iter().enumerate() {
                for h in &d.headers {
                    if h == time_col { continue; }
                    let val_str = if let Some(r) = indexed[i].get(&OrderedFloat(time_val)) {
                        if let Some(val) = r.get(h) {
                            format!("{:.6}", val)
                        } else {
                            "".to_string()
                        }
                    } else {
                        "".to_string()
                    };
                    row.push(val_str);
                }
            }

            row
        })
        .collect();

    // Write output CSV
    let mut wtr = WriterBuilder::new().from_path(output_path)?;
    wtr.write_record(&all_headers)?;
    for r in rows {
        wtr.write_record(&r)?;
    }
    wtr.flush()?;

    println!("Output written to {}", output_path);
    Ok(())
}
