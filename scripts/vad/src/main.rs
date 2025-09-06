use std::env;
use std::fs::File;
use std::io::BufReader;
use hound::{WavReader};
use webrtc_vad::Vad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input.wav>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let file = File::open(input_path)?;
    let mut reader = WavReader::new(BufReader::new(file))?;
    let spec = reader.spec();

    // Check WAV properties
    if spec.sample_rate != 48000 {
        eprintln!("Warning: WAV sample rate is {}, expected 48000 Hz", spec.sample_rate);
    }
    if spec.channels != 1 {
        eprintln!("Warning: WAV has {} channels, expected mono", spec.channels);
    }
    if spec.bits_per_sample != 16 {
        eprintln!("Warning: WAV bits per sample is {}, expected 16-bit PCM", spec.bits_per_sample);
    }

    // Initialize VAD
    let mut vad = Vad::new_with_rate_and_mode(48000, webrtc_vad::VadMode::Aggressive)?;

    // Read samples into i16
    let samples: Vec<i16> = reader.samples::<i16>().filter_map(Result::ok).collect();

    let frame_ms = 30; // frame length in milliseconds
    let frame_size = (48000 / 1000) * frame_ms as usize; // samples per frame
    let mut speech_frames = 0;
    let mut total_frames = 0;

    for chunk in samples.chunks(frame_size) {
        if chunk.len() < frame_size {
            break;
        }
        total_frames += 1;

        // WebRTC VAD expects &[i16]
        if vad.is_speech(chunk) {
            speech_frames += 1;
        }
    }

    let speech_fraction = speech_frames as f64 / total_frames as f64;
    // Consider track "active" if >10% of frames contain speech
    let is_speech = speech_fraction > 0.1;

    println!("{}", is_speech); // prints "true" or "false" for Bash

    Ok(())
}
