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

    let fps = cap.get(videoio::CAP_PROP_FPS)?;
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32;

    // Overlay sizes
    let top_bar_height = 60;
    let bottom_bar_height = 120;
    let right_bar_width = 300;

    let full_width = width;
    let full_height = height;

    // Video writer
    let fourcc = videoio::VideoWriter::fourcc('a','v','c','1')?;
    let mut writer = videoio::VideoWriter::new(
        "output_overlay.mp4",
        fourcc,
        fps,
        core::Size::new(full_width, full_height),
        true
    )?;

    // Attention smoothing
    let mut last_x = width / 2;
    let mut last_y = height / 2;
    let movement_amplifier = 3.0;
    let jitter_max = 0.03;
    let trail_length = 10;
    let mut trail: Vec<(i32, i32)> = Vec::new();

    // --- Scaling for video in top-left ---
    let scaled_video_width = (width as f64 * 0.5) as i32;  // example scale factor
    let scaled_video_height = (height as f64 * 0.5) as i32;

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

        // --- Create canvas with original resolution ---
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

        let roi = core::Rect::new(0, 0, scaled_video_width, scaled_video_height);
        let mut roi_mat = core::Mat::roi_mut(&mut canvas, roi)?;
        resized_frame.copy_to(&mut roi_mat)?;

        // --- Draw top bar ---
        draw_top_bar(&mut canvas, &data, full_width, top_bar_height)?;

        // --- Draw bottom bar ---
        draw_bottom_bar(&mut canvas, &frame_data, frame_idx, full_width, full_height, bottom_bar_height)?;

        // --- Draw right-side metrics ---
        draw_right_bar(&mut canvas, &data, full_width, top_bar_height)?;

        // --- Attention point ---
        let mut x = ((data.attention_center_x * scaled_video_width as f64 * movement_amplifier) as i32)
            .clamp(scaled_video_width, full_width-1);
        let mut y = ((data.attention_center_y * scaled_video_height as f64 * movement_amplifier) as i32)
            .clamp(0, scaled_video_height-1);

        x = ((0.7 * last_x as f64 + 0.3 * x as f64) as i32).clamp(0, full_width-1);
        y = ((0.7 * last_y as f64 + 0.3 * y as f64) as i32).clamp(0, full_height-1);
        last_x = x;
        last_y = y;

        let jitter_x = ((rand::random::<f64>() * 2.0 - 1.0) * full_width as f64 * jitter_max) as i32;
        let jitter_y = ((rand::random::<f64>() * 2.0 - 1.0) * full_height as f64 * jitter_max) as i32;
        x = (x + jitter_x).clamp(0, full_width-1);
        y = (y + jitter_y).clamp(0, full_height-1);

        // --- Draw attention trail ---
        trail.push((x, y));
        if trail.len() > trail_length { trail.remove(0); }
        for (i, &(tx, ty)) in trail.iter().enumerate() {
            let alpha = i as f64 / trail.len() as f64;
            imgproc::circle(&mut canvas, core::Point::new(tx, ty), 10,
                            core::Scalar::new(255.0 * alpha, 0.0, 0.0, 0.0),
                            -1, imgproc::LINE_8, 0)?;
        }

        // --- Draw current attention dot ---
        imgproc::circle(&mut canvas, core::Point::new(x, y), 15,
                        core::Scalar::new(0.0,0.0,255.0,0.0),
                        -1, imgproc::LINE_8, 0)?;

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

// --- Bottom Bar (Stacked bars for EMA and Percentiles) ---
fn draw_bottom_bar(frame: &mut Mat, frame_data: &Vec<FrameData>, frame_idx: usize,
                   width: i32, bottom_height: i32, top_bar_height: i32) -> Result<()> {
    let y_start = frame.rows() - bottom_height; // remove the `?` here
    let bar_width = 6; // width per bar
    let spacing = 2;   // spacing between bars

    let data = &frame_data[frame_idx.min(frame_data.len()-1)];

    // Values to visualize (scaled 0..1)
    let values = [
        data.visual_engagement_ema_1s_percentile,
        data.visual_engagement_ema_3s_percentile,
        data.visual_engagement_ema_10s_percentile,
    ];

    let colors = [
        core::Scalar::new(0.0, 255.0, 0.0, 0.0),   // EMA1s - Green
        core::Scalar::new(0.0, 255.0, 255.0, 0.0), // EMA3s - Yellow
        core::Scalar::new(255.0, 0.0, 0.0, 0.0),   // EMA10s - Red
    ];

    for (i, &val) in values.iter().enumerate() {
        let x = (frame_idx as i32 * (bar_width + spacing)) % width + i as i32 * bar_width;
        let bar_h = (val * bottom_height as f64) as i32;

        imgproc::rectangle(
            frame,
            core::Rect::new(
                x,
                y_start + bottom_height - bar_h,
                bar_width,
                bar_h
            ),
            colors[i],
            -1,
            imgproc::LINE_8,
            0
        )?;
    }

    Ok(())
}



// --- Right Bar ---
fn draw_right_bar(frame: &mut Mat, data: &FrameData,
                  width: i32, top_bar_height: i32) -> Result<()> {
    let x_start = width - 300;
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
        format!("EMA 1s: {:.2}", data.visual_engagement_ema_1s),
        format!("EMA 3s: {:.2}", data.visual_engagement_ema_3s),
        format!("EMA 10s: {:.2}", data.visual_engagement_ema_10s),
        format!("EMA 1s Percentile: {:.2}", data.visual_engagement_ema_1s_percentile),
        format!("EMA 3s Percentile: {:.2}", data.visual_engagement_ema_3s_percentile),
        format!("EMA 10s Percentile: {:.2}", data.visual_engagement_ema_10s_percentile),
        format!("Variance 1s: {:.2}", data.visual_engagement_variance_1s),
        format!("Variance 5s: {:.2}", data.visual_engagement_variance_5s),
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
