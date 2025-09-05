use std::env;

mod modules {
    pub mod rms_energy;
    pub mod pitch;
    pub mod spectral_features;
    pub mod jitter_shimmer;
    pub mod formant_analysis;
}

use modules::{rms_energy, pitch, spectral_features, jitter_shimmer, formant_analysis};

fn print_usage() {
    let prog_name = env::args().nth(0).unwrap_or_default();
    eprintln!("Usage:");
    eprintln!("  {} --rms-in <input.flac> --rms-out <output.csv>", prog_name);
    eprintln!("  {} --pitch-in <input.flac> --pitch-out <output.csv>", prog_name);
    eprintln!("  {} --spectral-in <input.flac> --spectral-out <output.csv>", prog_name);
    eprintln!("  {} --jitter-in <input.flac> --jitter-out <output.csv>", prog_name);
    eprintln!("  {} --formant-in <input.flac> --formant-out <output.csv>", prog_name);
    eprintln!("  {} --rms-in <rms_input.flac> --rms-out <rms_output.csv> --pitch-in <pitch_input.flac> --pitch-out <pitch_output.csv>", prog_name);
    eprintln!("  {} --spectral-in <input.flac> --spectral-out <output.csv> --jitter-in <input.flac> --jitter-out <output.csv>", prog_name);
    eprintln!("  {} --formant-in <input.flac> --formant-out <output.csv> --pitch-in <input.flac> --pitch-out <output.csv>", prog_name);
    eprintln!("");
    eprintln!("Features:");
    eprintln!("  --rms-*       : RMS energy and total energy analysis");
    eprintln!("  --pitch-*     : Pitch detection and analysis");
    eprintln!("  --spectral-*  : Spectral features (centroid, rolloff, bandwidth, flatness, flux, zero-crossing rate)");
    eprintln!("  --jitter-*    : Jitter, shimmer, and harmonics-to-noise ratio analysis");
    eprintln!("  --formant-*   : Formant frequency analysis (F1, F2, F3, F4)");
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
    let mut spectral_input: Option<String> = None;
    let mut spectral_output: Option<String> = None;
    let mut jitter_input: Option<String> = None;
    let mut jitter_output: Option<String> = None;
    let mut formant_input: Option<String> = None;
    let mut formant_output: Option<String> = None;

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
            "--spectral-in" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --spectral-in requires a file path");
                    std::process::exit(1);
                }
                spectral_input = Some(args[i + 1].clone());
                i += 2;
            }
            "--spectral-out" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --spectral-out requires a file path");
                    std::process::exit(1);
                }
                spectral_output = Some(args[i + 1].clone());
                i += 2;
            }
            "--jitter-in" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --jitter-in requires a file path");
                    std::process::exit(1);
                }
                jitter_input = Some(args[i + 1].clone());
                i += 2;
            }
            "--jitter-out" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --jitter-out requires a file path");
                    std::process::exit(1);
                }
                jitter_output = Some(args[i + 1].clone());
                i += 2;
            }
            "--formant-in" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --formant-in requires a file path");
                    std::process::exit(1);
                }
                formant_input = Some(args[i + 1].clone());
                i += 2;
            }
            "--formant-out" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --formant-out requires a file path");
                    std::process::exit(1);
                }
                formant_output = Some(args[i + 1].clone());
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
    let run_spectral = spectral_input.is_some() || spectral_output.is_some();
    let run_jitter = jitter_input.is_some() || jitter_output.is_some();
    let run_formant = formant_input.is_some() || formant_output.is_some();

    if run_rms && (rms_input.is_none() || rms_output.is_none()) {
        eprintln!("Error: Both --rms-in and --rms-out are required for RMS processing");
        std::process::exit(1);
    }

    if run_pitch && (pitch_input.is_none() || pitch_output.is_none()) {
        eprintln!("Error: Both --pitch-in and --pitch-out are required for pitch processing");
        std::process::exit(1);
    }

    if run_spectral && (spectral_input.is_none() || spectral_output.is_none()) {
        eprintln!("Error: Both --spectral-in and --spectral-out are required for spectral processing");
        std::process::exit(1);
    }

    if run_jitter && (jitter_input.is_none() || jitter_output.is_none()) {
        eprintln!("Error: Both --jitter-in and --jitter-out are required for jitter/shimmer processing");
        std::process::exit(1);
    }

    if run_formant && (formant_input.is_none() || formant_output.is_none()) {
        eprintln!("Error: Both --formant-in and --formant-out are required for formant processing");
        std::process::exit(1);
    }

    if !run_rms && !run_pitch && !run_spectral && !run_jitter && !run_formant {
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

    if run_spectral {
        println!("=== Running Spectral Features Analysis ===");
        spectral_features::process(&spectral_input.unwrap(), &spectral_output.unwrap())?;
    }

    if run_jitter {
        println!("=== Running Jitter/Shimmer Analysis ===");
        jitter_shimmer::process(&jitter_input.unwrap(), &jitter_output.unwrap())?;
    }

    if run_formant {
        println!("=== Running Formant Analysis ===");
        formant_analysis::process(&formant_input.unwrap(), &formant_output.unwrap())?;
    }

    println!("=== All processing complete ===");
    Ok(())
}