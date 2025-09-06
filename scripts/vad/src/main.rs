use std::env;
use hound::{WavReader, SampleFormat};
use webrtc_vad::{Vad, VadMode, SampleRate};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input.wav>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    let mut reader = WavReader::open(input_path).expect("Failed to open WAV");
    let spec = reader.spec();

    if spec.channels != 1 {
        eprintln!("Only mono audio supported");
        std::process::exit(1);
    }
    if spec.sample_rate != 48000 {
        eprintln!("Only 48 kHz audio supported");
        std::process::exit(1);
    }

    let samples: Vec<i16> = match spec.sample_format {
        SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap()).collect(),
        SampleFormat::Float => reader.samples::<f32>()
            .map(|s| (s.unwrap().clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect(),
    };

    let mut vad = Vad::new_with_rate(SampleRate::Rate48kHz);
    vad.set_mode(VadMode::Aggressive);

    let frame_size = (48000.0 * 0.03) as usize; // 30 ms
    let mut speech_detected = false;

    for frame in samples.chunks(frame_size) {
        if frame.len() < frame_size { break; }

        // unwrap Result<bool, ()>, panic if error
        if vad.is_voice_segment(frame).expect("VAD failed on this frame") {
            speech_detected = true;
            break;
        }
    }


    println!("{}", speech_detected);
}
