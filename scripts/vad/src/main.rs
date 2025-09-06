use std::env;
use std::fs::File;
use hound::{WavReader, SampleFormat};
use webrtc_vad::Vad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get input WAV path from argument
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input.wav>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    // Open WAV file
    let mut reader = WavReader::open(input_path)?;
    let spec = reader.spec();

    // Ensure 48 kHz, mono
    if spec.channels != 1 {
        eprintln!("Only mono audio supported");
        std::process::exit(1);
    }
    if spec.sample_rate != 48000 {
        eprintln!("Only 48 kHz audio supported");
        std::process::exit(1);
    }

    // Convert samples to i16 for webrtc-vad
    let samples: Vec<i16> = match spec.sample_format {
        SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap()).collect(),
        SampleFormat::Float => reader.samples::<f32>()
            .map(|s| {
                let f = s.unwrap();
                (f.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
            })
            .collect(),
    };

    // Initialize VAD (mode 3 = aggressive)
    let mut vad = Vad::new_with_rate(webrtc_vad::SampleRate::Rate48000, webrtc_vad::VadMode::Aggressive)?;

    // Split audio into 30ms frames
    let frame_size = (48000.0 * 0.03) as usize; // 30ms
    let mut speech_detected = false;

    for frame in samples.chunks(frame_size) {
        if frame.len() < frame_size { break; }
        if vad.is_speech(frame, 48000)? {
            speech_detected = true;
            break;
        }
    }

    println!("{}", speech_detected); // true or false
    Ok(())
}
