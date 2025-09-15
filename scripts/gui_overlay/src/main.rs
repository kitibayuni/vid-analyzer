use opencv::{
    prelude::*,
    videoio,
    core,
    imgproc,
    types::*,
};

use std::path::Path;
use csv::Reader;
use serde::Deserialize;
use anyhow::Result;

#[derive(Debug, Deserialize)]
struct FrameData {
    time_sec: f64,
    visual_engagement_score: f64,
    visual_engagement_ema_1s: f64,
    attention_center_x: f64,
    attention_center_y: f64,
}

fn main() -> Result<()> {
    // --- CLI args ---
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_video> <csv_data>", args[0]);
        std::process::exit(1);
    }
    let video_path = &args[1];
    let csv_path = &args[2];

    // --- Load CSV ---
    let mut rdr = Reader::from_path(csv_path)?;
    let mut frame_data = Vec::new();
    for result in rdr.deserialize() {
        let record: FrameData = result?;
        frame_data.push(record);
    }

    // --- Open Video ---
    let mut cap = videoio::VideoCapture::from_file(video_path, videoio::CAP_ANY)?;
    if !cap.is_opened()? {
        panic!("Failed to open video");
    }

    let fps = cap.get(videoio::CAP_PROP_FPS)?;
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32;

    // --- Scale factors for attention center ---
    let scale_x = width as f64 / 224.0;
    let scale_y = height as f64 / 224.0;

    // --- Amplification & jitter parameters ---
    let movement_amplifier = 2.0; // exaggerate movement 2x
    let jitter_max = 0.03; // add up to ±3% of frame width/height

    let fourcc = videoio::VideoWriter::fourcc('a','v','c','1')?;
    let mut writer = videoio::VideoWriter::new(
        "output_overlay.mp4",
        fourcc,
        fps,
        core::Size::new(width, height),
        true
    )?;

    let mut frame_idx = 0;
    loop {
        let mut frame = Mat::default();
        if !cap.read(&mut frame)? || frame.empty() {
            break;
        }

        let current_time = frame_idx as f64 / fps;
        // Find nearest CSV record
        let data = frame_data.iter()
            .min_by(|a,b| ((a.time_sec - current_time).abs())
                .partial_cmp(&(b.time_sec - current_time).abs()).unwrap())
            .unwrap();

        // --- Exaggerate and jitter attention point ---
        let mut x = (data.attention_center_x * 224.0 * scale_x * movement_amplifier) as i32;
        let mut y = (data.attention_center_y * 224.0 * scale_y * movement_amplifier) as i32;

        // Add small random jitter to simulate micro-saccades
        let jitter_x = ((rand::random::<f64>() * 2.0 - 1.0) * width as f64 * jitter_max) as i32;
        let jitter_y = ((rand::random::<f64>() * 2.0 - 1.0) * height as f64 * jitter_max) as i32;

        x = (x + jitter_x).clamp(0, width - 1);
        y = (y + jitter_y).clamp(0, height - 1);

        // --- Draw attention circle ---
        imgproc::circle(&mut frame, core::Point::new(x, y), 15,
                        core::Scalar::new(0.0, 0.0, 255.0, 0.0),
                        -1, imgproc::LINE_8, 0)?;

        // --- Draw engagement graph on bottom 100px ---
        let graph_height = 100;
        let graph_color = core::Scalar::new(0.0, 255.0, 0.0, 0.0);
        let graph_value = (data.visual_engagement_ema_1s * graph_height as f64) as i32;
        imgproc::rectangle(
            &mut frame,
            core::Rect::new(50 + frame_idx as i32 % width, height - graph_height, 5, graph_value),
            graph_color,
            -1,
            imgproc::LINE_8,
            0
        )?;

        writer.write(&frame)?;
        frame_idx += 1;
    }

    writer.release()?;
    cap.release()?;

    println!("Overlay video written to output_overlay.mp4");
    Ok(())
}
