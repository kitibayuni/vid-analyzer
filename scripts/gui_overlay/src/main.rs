use opencv::{
    prelude::*,
    videoio,
    core,
    imgproc,
};
use csv::Reader;
use serde::Deserialize;
use anyhow::Result;

#[derive(Debug, Deserialize, Clone)]
struct FrameData {
    time_sec: f64,
    motion_intensity: f64,
    motion_variance: f64,
    motion_change_rate: f64,
    mean_saliency: f64,
    max_saliency: f64,
    saliency_entropy: f64,
    saliency_change_rate: f64,
    attention_center_x: f64,
    attention_center_y: f64,
    attention_concentration: f64,
    attention_shift_rate: f64,
    visual_engagement_score: f64,
    visual_engagement_ema_1s: f64,
    visual_engagement_ema_3s: f64,
    visual_engagement_ema_10s: f64,
    visual_engagement_ema_1s_percentile: f64,
    visual_engagement_ema_3s_percentile: f64,
    visual_engagement_ema_10s_percentile: f64,
    visual_engagement_variance_1s: f64,
    visual_engagement_variance_5s: f64,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_video> <csv_data>", args[0]);
        std::process::exit(1);
    }
    let video_path = &args[1];
    let csv_path = &args[2];

    // Load CSV
    let mut rdr = Reader::from_path(csv_path)?;
    let mut frame_data: Vec<FrameData> = Vec::new();
    for result in rdr.deserialize() {
        let record: FrameData = result?;
        frame_data.push(record);
    }

    // Open video
    let mut cap = videoio::VideoCapture::from_file(video_path, videoio::CAP_ANY)?;
    if !cap.is_opened()? {
        panic!("Failed to open video");
    }

    let fps = cap.get(videoio::CAP_PROP_FPS)? as f64;
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32;

    let top_bar_height = 60;
    let bottom_bar_height = 120;
    let right_bar_width = 300;

    let full_width = width + right_bar_width;
    let full_height = height + top_bar_height + bottom_bar_height;

    // Scale video for top-left display
    let scaled_video_width = width / 2;
    let scaled_video_height = height / 2;

    let fourcc = videoio::VideoWriter::fourcc('a','v','c','1')?;
    let mut writer = videoio::VideoWriter::new(
        "output_overlay.mp4",
        fourcc,
        fps,
        core::Size::new(full_width, full_height),
        true
    )?;

    // Attention smoothing and trail
    let mut last_x = scaled_video_width / 2; // Initialize to center
    let mut last_y = scaled_video_height / 2 + top_bar_height; // Initialize to center + offset
    let movement_amplifier = 8.0; // Increased from 3.0 for more dramatic movement
    let jitter_max = 0.08; // Increased from 0.03 for more jitter
    let trail_length = 15; // Increased from 10 for longer trail
    let mut trail: Vec<(i32, i32)> = Vec::new();

    let mut frame_idx = 0;
    loop {
        let mut frame = Mat::default();
        if !cap.read(&mut frame)? || frame.empty() {
            break;
        }

        let current_time = frame_idx as f64 / fps;
        let data = frame_data.iter()
            .min_by(|a,b| ((a.time_sec - current_time).abs())
                .partial_cmp(&(b.time_sec - current_time).abs()).unwrap())
            .unwrap()
            .clone();

        // Create canvas
        let mut canvas = Mat::zeros(full_height, full_width, frame.typ())?.to_mat()?;

        // --- Resize original frame to top-left ---
        let mut resized_frame = Mat::default();
        imgproc::resize(
            &frame,
            &mut resized_frame,
            core::Size::new(scaled_video_width, scaled_video_height),
            0.0,
            0.0,
            imgproc::INTER_LINEAR
        )?;

        // Copy resized frame to canvas (positioned below top bar)
        let roi = core::Rect::new(0, top_bar_height, scaled_video_width, scaled_video_height);
        let mut roi_mat = core::Mat::roi_mut(&mut canvas, roi)?;
        resized_frame.copy_to(&mut roi_mat)?;

        // --- Draw overlay elements ---
        draw_top_bar(&mut canvas, &data, full_width, top_bar_height)?;
        draw_bottom_bar(&mut canvas, &data, full_width, bottom_bar_height, top_bar_height)?;
        draw_right_bar(&mut canvas, &data, width, top_bar_height)?;

        // --- Calculate attention point coordinates (relative to the resized video area) ---
        // Center the coordinate system and allow full range across the video
        let center_x = scaled_video_width / 2;
        let center_y = scaled_video_height / 2;
        
        // Map attention coordinates from [0,1] to [-0.5, 0.5] for centering, then scale to video dimensions
        let attention_x_centered = (data.attention_center_x - 0.5) * movement_amplifier;
        let attention_y_centered = (data.attention_center_y - 0.5) * movement_amplifier;
        
        let mut x = (center_x as f64 + attention_x_centered * scaled_video_width as f64) as i32;
        let mut y = (center_y as f64 + attention_y_centered * scaled_video_height as f64) as i32;
        
        // Clamp to video bounds
        x = x.clamp(0, scaled_video_width-1);
        y = y.clamp(0, scaled_video_height-1);

        // Add offset for positioning within the canvas (video is offset by top_bar_height)
        y += top_bar_height;

        // Smooth movement (reduced smoothing for more responsive movement)
        x = ((0.4 * last_x as f64 + 0.6 * x as f64) as i32).clamp(0, scaled_video_width-1);
        y = ((0.4 * last_y as f64 + 0.6 * y as f64) as i32).clamp(top_bar_height, top_bar_height + scaled_video_height-1);
        last_x = x;
        last_y = y;

        // Optional: Remove jitter for purely data-driven movement
        // let jitter_x = ((rand::random::<f64>() * 2.0 - 1.0) * scaled_video_width as f64 * jitter_max) as i32;
        // let jitter_y = ((rand::random::<f64>() * 2.0 - 1.0) * scaled_video_height as f64 * jitter_max) as i32;
        // x = (x + jitter_x).clamp(0, scaled_video_width-1);
        // y = (y + jitter_y).clamp(top_bar_height, top_bar_height + scaled_video_height-1);

        // --- Draw attention trail on the canvas ---
        trail.push((x, y));
        if trail.len() > trail_length { trail.remove(0); }
        for (i, &(tx, ty)) in trail.iter().enumerate() {
            let alpha = i as f64 / trail.len() as f64;
            let radius = (5.0 + alpha * 10.0) as i32; // Variable radius for trail
            imgproc::circle(
                &mut canvas,
                core::Point::new(tx, ty),
                radius,
                core::Scalar::new(255.0 * alpha, 100.0 * alpha, 0.0, 0.0), // Orange to red gradient
                1, // Changed from 2 to 1-2 pixel width
                imgproc::LINE_8,
                0
            )?;
        }

        // --- Draw current attention dot (hollow outer ring only) ---
        imgproc::circle(
            &mut canvas,
            core::Point::new(x, y),
            25, // Outer ring radius
            core::Scalar::new(255.0, 255.0, 0.0, 0.0), // Yellow color
            2, // 1-2 pixel width hollow ring
            imgproc::LINE_8,
            0
        )?;

        // CRITICAL: Write the frame to the video file
        writer.write(&canvas)?;
        frame_idx += 1;
    }

    writer.release()?;
    cap.release()?;
    println!("Overlay video written to output_overlay.mp4");
    Ok(())
}

// --- Top Bar ---
fn draw_top_bar(frame: &mut Mat, data: &FrameData, width: i32, height: i32) -> Result<()> {
    let bar_width = (width / 6) as i32;
    let spacing = 20;
    let bars = [
        (data.motion_intensity, core::Scalar::new(0.0, 255.0, 0.0, 0.0)),
        (data.motion_variance, core::Scalar::new(0.0, 0.0, 255.0, 0.0)),
        (data.motion_change_rate, core::Scalar::new(255.0, 0.0, 0.0, 0.0))
    ];
    for (i, &(value, color)) in bars.iter().enumerate() {
        let h = (value * height as f64) as i32;
        imgproc::rectangle(frame,
            core::Rect::new(i as i32*(bar_width+spacing), 0, bar_width, h),
            color,
            -1,
            imgproc::LINE_8,
            0
        )?;
    }
    Ok(())
}

fn draw_bottom_bar(frame: &mut Mat, data: &FrameData, width: i32, bottom_height: i32, _top_bar_height: i32) -> Result<()> {
    let y_start = frame.rows() - bottom_height;
    let bar_height = 20;
    let spacing = 10;

    // Max width per half of the video
    let max_bar_width = width / 2 - 50; // 50px padding for label/numbers

    // Left half values
    let left_values = [
        data.visual_engagement_ema_1s_percentile,
        data.visual_engagement_ema_3s_percentile,
    ];
    let left_colors = [
        core::Scalar::new(0.0, 255.0, 0.0, 0.0),   // EMA1s - Green
        core::Scalar::new(0.0, 255.0, 255.0, 0.0), // EMA3s - Yellow
    ];
    let left_labels = ["EMA1s", "EMA3s"];

    for (i, &val) in left_values.iter().enumerate() {
        let bar_width = (val * max_bar_width as f64) as i32;
        let y = y_start + i as i32 * (bar_height + spacing);

        // Draw horizontal bar
        imgproc::rectangle(
            frame,
            core::Rect::new(50, y, bar_width, bar_height),
            left_colors[i],
            -1,
            imgproc::LINE_8,
            0
        )?;

        // Draw label
        imgproc::put_text(
            frame,
            left_labels[i],
            core::Point::new(5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Draw numeric value
        imgproc::put_text(
            frame,
            &format!("{:.2}", val),
            core::Point::new(50 + bar_width + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    // Right half values
    let right_values = [
        data.visual_engagement_ema_10s_percentile,
    ];
    let right_colors = [
        core::Scalar::new(255.0, 0.0, 0.0, 0.0), // EMA10s - Red
    ];
    let right_labels = ["EMA10s"];

    for (i, &val) in right_values.iter().enumerate() {
        let bar_width = (val * max_bar_width as f64) as i32;
        let y = y_start + (i + left_values.len()) as i32 * (bar_height + spacing);

        // Draw horizontal bar on right half
        let x_start = width / 2 + 50;
        imgproc::rectangle(
            frame,
            core::Rect::new(x_start, y, bar_width, bar_height),
            right_colors[i],
            -1,
            imgproc::LINE_8,
            0
        )?;

        // Draw label
        imgproc::put_text(
            frame,
            right_labels[i],
            core::Point::new(x_start - 45, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Draw numeric value
        imgproc::put_text(
            frame,
            &format!("{:.2}", val),
            core::Point::new(x_start + bar_width + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    Ok(())
}

// --- Right Metrics Panel ---
fn draw_right_bar(frame: &mut Mat, data: &FrameData, width: i32, top_bar_height: i32) -> Result<()> {
    let x_start = width;
    let mut y = top_bar_height;
    let line_height = 20;

    let metrics = [
        format!("Time: {:.2}", data.time_sec),
        format!("Motion Intensity: {:.2}", data.motion_intensity),
        format!("Motion Variance: {:.2}", data.motion_variance),
        format!("Motion Change Rate: {:.2}", data.motion_change_rate),
        format!("Mean Saliency: {:.2}", data.mean_saliency),
        format!("Max Saliency: {:.2}", data.max_saliency),
        format!("Saliency Entropy: {:.2}", data.saliency_entropy),
        format!("Saliency Change Rate: {:.2}", data.saliency_change_rate),
        format!("Attention X/Y: {:.2}/{:.2}", data.attention_center_x, data.attention_center_y),
        format!("Attention Concentration: {:.2}", data.attention_concentration),
        format!("Attention Shift Rate: {:.2}", data.attention_shift_rate),
        format!("Visual Engagement Score: {:.2}", data.visual_engagement_score),
        format!("VE EMA1s: {:.2}", data.visual_engagement_ema_1s),
        format!("VE EMA3s: {:.2}", data.visual_engagement_ema_3s),
        format!("VE EMA10s: {:.2}", data.visual_engagement_ema_10s),
        format!("VE Variance 1s: {:.2}", data.visual_engagement_variance_1s),
        format!("VE Variance 5s: {:.2}", data.visual_engagement_variance_5s),
    ];

    for text in &metrics {
        imgproc::put_text(
            frame,
            text,
            core::Point::new(x_start + 5, y),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
        y += line_height;
    }

    Ok(())
}