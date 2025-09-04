// main.rs
use std::env;

mod modules {
    pub mod rms_energy;
    pub mod pitch;
}

use modules::{rms_energy, pitch};

fn print_usage() {
    let prog_name = env::args().nth(0).unwrap_or_default();
    eprintln!("Usage:");
    eprintln!("  {} --rms-in <input.flac> --rms-out <output.csv>", prog_name);
    eprintln!("  {} --pitch-in <input.flac> --pitch-out <output.csv>", prog_name);
    eprintln!("  {} --rms-in <rms_input.flac> --rms-out <rms_output.csv> --pitch-in <pitch_input.flac> --pitch-out <pitch_output.csv>", prog_name);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        print_usage();
        std::process::exit(1);
    }

    let mut rms_input: Option<String> = None;
    let mut rms_output: Option<String> = None;
    let mut pitch_input: Option<String> = None;
    let mut pitch_output: Option<String> = None;

    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rms-in" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --rms-in requires a file path");
                    std::process::exit(1);
                }
                rms_input = Some(args[i + 1].clone());
                i += 2;
            }
            "--rms-out" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --rms-out requires a file path");
                    std::process::exit(1);
                }
                rms_output = Some(args[i + 1].clone());
                i += 2;
            }
            "--pitch-in" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --pitch-in requires a file path");
                    std::process::exit(1);
                }
                pitch_input = Some(args[i + 1].clone());
                i += 2;
            }
            "--pitch-out" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --pitch-out requires a file path");
                    std::process::exit(1);
                }
                pitch_output = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    // Validate argument combinations
    let run_rms = rms_input.is_some() || rms_output.is_some();
    let run_pitch = pitch_input.is_some() || pitch_output.is_some();

    if run_rms && (rms_input.is_none() || rms_output.is_none()) {
        eprintln!("Error: Both --rms-in and --rms-out are required for RMS processing");
        std::process::exit(1);
    }

    if run_pitch && (pitch_input.is_none() || pitch_output.is_none()) {
        eprintln!("Error: Both --pitch-in and --pitch-out are required for pitch processing");
        std::process::exit(1);
    }

    if !run_rms && !run_pitch {
        eprintln!("Error: No processing specified");
        print_usage();
        std::process::exit(1);
    }

    // Run processing
    if run_rms {
        println!("=== Running RMS Energy Analysis ===");
        rms_energy::process(&rms_input.unwrap(), &rms_output.unwrap())?;
    }

    if run_pitch {
        println!("=== Running Pitch Analysis ===");
        pitch::process(&pitch_input.unwrap(), &pitch_output.unwrap())?;
    }

    println!("=== All processing complete ===");
    Ok(())
}