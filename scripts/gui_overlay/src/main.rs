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
    mean_saliency: Option<f64>,
    max_saliency: Option<f64>,
}

fn main() -> Result<()> {
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

    let fps = cap.get(videoio::CAP_PROP_FPS)? as f64;
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32;

    let fourcc = videoio::VideoWriter::fourcc('a','v','c','1')?;
    let mut writer = videoio::VideoWriter::new(
        "output_overlay.mp4",
        fourcc,
        fps,
        core::Size::new(width, height),
        true
    )?;

    // --- EMA smoothing ---
    let mut ema_x = 0.5;
    let mut ema_y = 0.5;
    let ema_alpha = 0.2;

    let movement_amplifier = 4.0;
    let jitter_max = 0.03;

    // --- Store past points for trail ---
    let mut trail: Vec<(i32, i32)> = Vec::new();
    let trail_max_len = 20; // last 20 frames

    let mut frame_idx = 0;
    loop {
        let mut frame = Mat::default();
        if !cap.read(&mut frame)? || frame.empty() {
            break;
        }

        let current_time = frame_idx as f64 / fps;

        let data = frame_data.iter()
            .min_by(|a, b| ((a.time_sec - current_time).abs())
            .partial_cmp(&(b.time_sec - current_time).abs()).unwrap())
            .unwrap();

        // --- Smooth attention ---
        ema_x = ema_alpha * data.attention_center_x + (1.0 - ema_alpha) * ema_x;
        ema_y = ema_alpha * data.attention_center_y + (1.0 - ema_alpha) * ema_y;

        let mut x = ((ema_x - 0.5) * movement_amplifier + 0.5) * width as f64;
        let mut y = ((ema_y - 0.5) * movement_amplifier + 0.5) * height as f64;

        let jitter_x = ((rand::random::<f64>() * 2.0 - 1.0) * width as f64 * jitter_max) as f64;
        let jitter_y = ((rand::random::<f64>() * 2.0 - 1.0) * height as f64 * jitter_max) as f64;

        x = (x + jitter_x).clamp(0.0, width as f64 - 1.0);
        y = (y + jitter_y).clamp(0.0, height as f64 - 1.0);

        // --- Add to trail ---
        trail.push((x as i32, y as i32));
        if trail.len() > trail_max_len {
            trail.remove(0);
        }

        // --- Draw trail ---
        for (i, &(tx, ty)) in trail.iter().enumerate() {
            let alpha = (i as f64 / trail.len() as f64 * 0.5).clamp(0.1, 0.5); // semi-transparent
            imgproc::circle(
                &mut frame,
                core::Point::new(tx, ty),
                10,
                core::Scalar::new(0.0, 0.0, 255.0 * alpha, 0.0),
                -1,
                imgproc::LINE_8,
                0
            )?;
        }

        // --- Draw current attention circle ---
        let color_factor = (data.visual_engagement_score.clamp(0.0,1.0) * 255.0) as f64;
        imgproc::circle(
            &mut frame,
            core::Point::new(x as i32, y as i32),
            25,
            core::Scalar::new(0.0, 0.0, 255.0, 0.0),
            -1,
            imgproc::LINE_8,
            0
        )?;

        // --- Draw saliency points ---
        if let Some(mean) = data.mean_saliency {
            let radius = (mean * 50.0) as i32;
            imgproc::circle(
                &mut frame,
                core::Point::new(x as i32, y as i32),
                radius,
                core::Scalar::new(255.0, 0.0, 0.0, 0.0),
                2,
                imgproc::LINE_8,
                0
            )?;
        }

        if let Some(max) = data.max_saliency {
            let radius = (max * 80.0) as i32;
            imgproc::circle(
                &mut frame,
                core::Point::new(x as i32, y as i32),
                radius,
                core::Scalar::new(255.0, 255.0, 0.0, 0.0),
                2,
                imgproc::LINE_8,
                0
            )?;
        }

        // --- Draw engagement graph at bottom ---
        let graph_height = 100;
        let graph_value = (data.visual_engagement_ema_1s * graph_height as f64) as i32;
        imgproc::rectangle(
            &mut frame,
            core::Rect::new(50 + frame_idx as i32 % width, height - graph_height, 5, graph_value),
            core::Scalar::new(0.0, 255.0, 0.0, 0.0),
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
