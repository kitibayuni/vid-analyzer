use std::env;

mod modules {
    pub mod rms_energy;
    pub mod pitch;
    pub mod spectral_features;
    pub mod formant_analysis;
    pub mod zcr;
}

use modules::{rms_energy, pitch, spectral_features, formant_analysis, zcr};

fn print_usage() {
    let prog_name = env::args().nth(0).unwrap_or_default();
    eprintln!("Usage:");
    eprintln!("  {} --rms-in <input.wav> --rms-out <output.csv>", prog_name);
    eprintln!("  {} --pitch-in <input.wav> --pitch-out <output.csv>", prog_name);
    eprintln!("  {} --spectral-in <input.wav> --spectral-out <output.csv>", prog_name);
    eprintln!("  {} --zcr-in <input.wav> --zcr-out <output.csv>", prog_name);
    eprintln!("  {} --formant-in <input.wav> --formant-out <output.csv>", prog_name);
    eprintln!("  {} can combine multiple feature sets in one command", prog_name);
    eprintln!();
    eprintln!("Features:");
    eprintln!("  --rms-*       : RMS energy and total energy analysis");
    eprintln!("  --pitch-*     : Pitch detection and analysis");
    eprintln!("  --spectral-*  : Spectral features (centroid, rolloff, bandwidth, flatness, flux)");
    eprintln!("  --zcr-*       : Zero-crossing rate analysis");
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
    let mut zcr_input: Option<String> = None;
    let mut zcr_output: Option<String> = None;
    let mut formant_input: Option<String> = None;
    let mut formant_output: Option<String> = None;

    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rms-in" => { rms_input = args.get(i + 1).cloned(); i += 2; }
            "--rms-out" => { rms_output = args.get(i + 1).cloned(); i += 2; }
            "--pitch-in" => { pitch_input = args.get(i + 1).cloned(); i += 2; }
            "--pitch-out" => { pitch_output = args.get(i + 1).cloned(); i += 2; }
            "--spectral-in" => { spectral_input = args.get(i + 1).cloned(); i += 2; }
            "--spectral-out" => { spectral_output = args.get(i + 1).cloned(); i += 2; }
            "--zcr-in" => { zcr_input = args.get(i + 1).cloned(); i += 2; }
            "--zcr-out" => { zcr_output = args.get(i + 1).cloned(); i += 2; }
            "--formant-in" => { formant_input = args.get(i + 1).cloned(); i += 2; }
            "--formant-out" => { formant_output = args.get(i + 1).cloned(); i += 2; }
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
    let run_zcr = zcr_input.is_some() || zcr_output.is_some();
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
    if run_zcr && (zcr_input.is_none() || zcr_output.is_none()) {
        eprintln!("Error: Both --zcr-in and --zcr-out are required for ZCR processing");
        std::process::exit(1);
    }
    if run_formant && (formant_input.is_none() || formant_output.is_none()) {
        eprintln!("Error: Both --formant-in and --formant-out are required for formant processing");
        std::process::exit(1);
    }

    if !run_rms && !run_pitch && !run_spectral && !run_zcr && !run_formant {
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
    if run_zcr {
        println!("=== Running Zero-Crossing Rate Analysis ===");
        zcr::process(&zcr_input.unwrap(), &zcr_output.unwrap())?;
    }
    if run_formant {
        println!("=== Running Formant Analysis ===");
        formant_analysis::process(&formant_input.unwrap(), &formant_output.unwrap())?;
    }

    println!("=== All processing complete ===");
    Ok(())
}