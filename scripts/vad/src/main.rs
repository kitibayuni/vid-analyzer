use std::env;
use hound::{WavReader, SampleFormat};
use webrtc_vad::{Vad, VadMode, SampleRate};
use std::io::{stdout, Write};

fn print_progress(current: usize, total: usize, width: usize) {
    let percentage = current as f64 / total as f64;
    let filled = (percentage * width as f64).round() as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(width - filled);
    print!("\r[{}] {:>3}% ({}/{})", bar, (percentage * 100.0) as usize, current, total);
    stdout().flush().unwrap();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <input.wav>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];

    // --- Open WAV ---
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

    // --- Read samples as i16 ---
    let samples: Vec<i16> = match spec.sample_format {
        SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap()).collect(),
        SampleFormat::Float => reader.samples::<f32>()
            .map(|s| (s.unwrap().clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect(),
    };

    // --- Initialize VAD ---
    let mut vad = Vad::new_with_rate(SampleRate::Rate48kHz);
    vad.set_mode(VadMode::Aggressive);

    // --- Frame settings ---
    let frame_ms = 30.0;            // frame length in ms
    let hop_ms = 15.0;              // 50% overlap
    let frame_size = (48000.0 * frame_ms / 1000.0) as usize;
    let hop_size = (48000.0 * hop_ms / 1000.0) as usize;

    let total_frames = (samples.len() + hop_size - 1) / hop_size;
    let smooth_window = 3;          // smoothing window in frames
    let mut recent_flags = vec![];
    let mut speech_frames = 0;

    for i in 0..total_frames {
        let start = i * hop_size;
        let end = usize::min(start + frame_size, samples.len());
        if start >= samples.len() { break; }
        let frame = &samples[start..end];
        let is_speech = vad.is_voice_segment(frame).unwrap_or(false);

        recent_flags.push(is_speech);
        if recent_flags.len() > smooth_window {
            recent_flags.remove(0);
        }

        if recent_flags.iter().filter(|&&f| f).count() >= (smooth_window / 2 + 1) {
            speech_frames += 1;
        }

        print_progress(i + 1, total_frames, 40);
    }

    println!(""); // newline

    let speech_prominence = if total_frames > 0 {
        speech_frames as f64 / total_frames as f64
    } else {
        0.0
    };

    println!("Speech prominence: {:.4}", speech_prominence);
}
