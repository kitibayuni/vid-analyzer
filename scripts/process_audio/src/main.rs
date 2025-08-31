use std::env;
use std::fs::File;
use std::io::BufReader;

use claxon::FlacReader;
use ndarray::prelude::*;
use pyin::{PYINExecutor, Framing, PadMode};
use csv::Writer;



fn main() -> Result<(), Box<dyn std:error::Error>> {
    // cli argument handling
    let args: Vec<String> = env::args().collect();

    // checking # of args
    if args.len() != 2 {
        eprintln!("Usage: {} input_audio.flac>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];
    
    // --- PROCESS .FLAC --- //

    let file = File.open(input_path)?;
    let reader = BufReader::new(file);
    let mut flac = FlacReader::new(reader)?;

    // take the flac's stream info's sample rate
    let samplerate = flac.streaminfo().sample_rate as usize;

    // --- INITIATE PYIN --- //
}
