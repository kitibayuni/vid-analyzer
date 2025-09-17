use opencv::{
    prelude::*,
    videoio,
    core,
    imgproc,
};
use csv::ReaderBuilder;
use serde::Deserialize;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
struct FrameData {
    time_sec: f64,
    cat_engage_ema_1s_pct: f64,
    cat_engage_ema_5s_pct: f64,
    cat_engage_ema_10s_pct: f64,
    transc_engage_ema_1s_pct: f64,
    transc_engage_ema_5s_pct: f64,
    transc_engage_ema_10s_pct: f64,
    vocals_rms_energy_ema_1s: f64,
    vocals_rms_energy_ema_5s: f64,
    vocals_rms_energy_ema_10s: f64,
    vocals_rms_energy_ema_1s_pct: f64,
    vocals_rms_energy_ema_5s_pct: f64,
    vocals_rms_energy_ema_10s_pct: f64,
    vocals_spectral_ema_1s: f64,
    vocals_spectral_ema_5s: f64,
    vocals_spectral_ema_10s: f64,
    vocals_spectral_ema_1s_pct: f64,
    vocals_spectral_ema_5s_pct: f64,
    vocals_spectral_ema_10s_pct: f64,
    nonvocals_rms_energy_ema_1s: f64,
    nonvocals_rms_energy_ema_5s: f64,
    nonvocals_rms_energy_ema_10s: f64,
    nonvocals_rms_energy_ema_1s_pct: f64,
    nonvocals_rms_energy_ema_5s_pct: f64,
    nonvocals_rms_energy_ema_10s_pct: f64,
    nonvocals_spectral_ema_1s: f64,
    nonvocals_spectral_ema_5s: f64,
    nonvocals_spectral_ema_10s: f64,
    nonvocals_spectral_ema_1s_pct: f64,
    nonvocals_spectral_ema_5s_pct: f64,
    nonvocals_spectral_ema_10s_pct: f64,
    attention_center_x: f64,
    attention_center_y: f64,
    attention_concentration: f64,
    attention_shift_rate: f64,
    visual_engage_ema_1s_pct: f64,
    visual_engage_ema_3s_pct: f64,
    visual_engage_ema_10s_pct: f64,
    visual_engage_ema_1s: f64,
    visual_engage_ema_3s: f64,
    visual_engage_ema_10s: f64,
    total_engagement: f64,
    total_engagement_pct: f64,
}

fn safe_parse_f64(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

fn load_csv_data(csv_path: &str) -> Result<Vec<FrameData>> {
    let mut frame_data: Vec<FrameData> = Vec::new();
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(csv_path)?;

    // Get headers to understand the CSV structure
    let headers = rdr.headers()?.clone();
    println!("CSV Headers found:");
    for (i, header) in headers.iter().enumerate() {
        println!("  {}: {}", i, header);
    }

    // Process each record manually
    for (row_num, result) in rdr.records().enumerate() {
        match result {
            Ok(record) => {
                let mut data = FrameData::default();
                
                // Map columns by name, handling variations
                for (i, header) in headers.iter().enumerate() {
                    if let Some(value) = record.get(i) {
                        let val = safe_parse_f64(value);
                        
                        match header {
                            // Time
                            "time_sec" => data.time_sec = val,
                            
                            // Cat engagement - look for variations
                            h if h.contains("cat") && h.contains("engage_ema_1s_pct") => data.cat_engage_ema_1s_pct = val,
                            h if h.contains("cat") && h.contains("engage_ema_5s_pct") => data.cat_engage_ema_5s_pct = val,
                            h if h.contains("cat") && h.contains("engage_ema_10s_pct") => data.cat_engage_ema_10s_pct = val,
                            
                            // Transcription engagement
                            h if h.contains("transc") && h.contains("engage_ema_1s_pct") => data.transc_engage_ema_1s_pct = val,
                            h if h.contains("transc") && h.contains("engage_ema_5s_pct") => data.transc_engage_ema_5s_pct = val,
                            h if h.contains("transc") && h.contains("engage_ema_10s_pct") => data.transc_engage_ema_10s_pct = val,
                            
                            // Vocals RMS Energy
                            h if h.contains("vocals") && h.contains("rms_energy_ema_1s") && !h.contains("pct") => data.vocals_rms_energy_ema_1s = val,
                            h if h.contains("vocals") && h.contains("rms_energy_ema_5s") && !h.contains("pct") => data.vocals_rms_energy_ema_5s = val,
                            h if h.contains("vocals") && h.contains("rms_energy_ema_10s") && !h.contains("pct") => data.vocals_rms_energy_ema_10s = val,
                            h if h.contains("vocals") && h.contains("rms_energy_ema_1s_pct") => data.vocals_rms_energy_ema_1s_pct = val,
                            h if h.contains("vocals") && h.contains("rms_energy_ema_5s_pct") => data.vocals_rms_energy_ema_5s_pct = val,
                            h if h.contains("vocals") && h.contains("rms_energy_ema_10s_pct") => data.vocals_rms_energy_ema_10s_pct = val,
                            
                            // Vocals Spectral
                            h if h.contains("vocals") && h.contains("spectral_ema_1s") && !h.contains("pct") => data.vocals_spectral_ema_1s = val,
                            h if h.contains("vocals") && h.contains("spectral_ema_5s") && !h.contains("pct") => data.vocals_spectral_ema_5s = val,
                            h if h.contains("vocals") && h.contains("spectral_ema_10s") && !h.contains("pct") => data.vocals_spectral_ema_10s = val,
                            h if h.contains("vocals") && h.contains("spectral_ema_1s_pct") => data.vocals_spectral_ema_1s_pct = val,
                            h if h.contains("vocals") && h.contains("spectral_ema_5s_pct") => data.vocals_spectral_ema_5s_pct = val,
                            h if h.contains("vocals") && h.contains("spectral_ema_10s_pct") => data.vocals_spectral_ema_10s_pct = val,
                            
                            // Nonvocals RMS Energy
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_1s") && !h.contains("pct") => data.nonvocals_rms_energy_ema_1s = val,
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_5s") && !h.contains("pct") => data.nonvocals_rms_energy_ema_5s = val,
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_10s") && !h.contains("pct") => data.nonvocals_rms_energy_ema_10s = val,
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_1s_pct") => data.nonvocals_rms_energy_ema_1s_pct = val,
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_5s_pct") => data.nonvocals_rms_energy_ema_5s_pct = val,
                            h if h.contains("nonvocals") && h.contains("rms_energy_ema_10s_pct") => data.nonvocals_rms_energy_ema_10s_pct = val,
                            
                            // Nonvocals Spectral
                            h if h.contains("nonvocals") && h.contains("spectral_ema_1s") && !h.contains("pct") => data.nonvocals_spectral_ema_1s = val,
                            h if h.contains("nonvocals") && h.contains("spectral_ema_5s") && !h.contains("pct") => data.nonvocals_spectral_ema_5s = val,
                            h if h.contains("nonvocals") && h.contains("spectral_ema_10s") && !h.contains("pct") => data.nonvocals_spectral_ema_10s = val,
                            h if h.contains("nonvocals") && h.contains("spectral_ema_1s_pct") => data.nonvocals_spectral_ema_1s_pct = val,
                            h if h.contains("nonvocals") && h.contains("spectral_ema_5s_pct") => data.nonvocals_spectral_ema_5s_pct = val,
                            h if h.contains("nonvocals") && h.contains("spectral_ema_10s_pct") => data.nonvocals_spectral_ema_10s_pct = val,
                            
                            // Attention
                            h if h.contains("attention_center_x") => data.attention_center_x = val,
                            h if h.contains("attention_center_y") => data.attention_center_y = val,
                            h if h.contains("attention_concentration") => data.attention_concentration = val,
                            h if h.contains("attention_shift_rate") => data.attention_shift_rate = val,
                            
                            // Visual engagement
                            h if h.contains("visual_engage_ema_1s_pct") => data.visual_engage_ema_1s_pct = val,
                            h if h.contains("visual_engage_ema_3s_pct") => data.visual_engage_ema_3s_pct = val,
                            h if h.contains("visual_engage_ema_10s_pct") => data.visual_engage_ema_10s_pct = val,
                            h if h.contains("visual_engage_ema_1s") && !h.contains("pct") => data.visual_engage_ema_1s = val,
                            h if h.contains("visual_engage_ema_3s") && !h.contains("pct") => data.visual_engage_ema_3s = val,
                            h if h.contains("visual_engage_ema_10s") && !h.contains("pct") => data.visual_engage_ema_10s = val,
                            
                            // Total engagement
                            h if h.contains("total_engagement_score") => data.total_engagement = val,
                            h if h.contains("total_engagement_percentile") => data.total_engagement_pct = val,
                            h if h.contains("total_engagement") && !h.contains("score") && !h.contains("percentile") => data.total_engagement = val,
                            
                            _ => {} // Ignore unrecognized columns
                        }
                    }
                }
                
                // Debug output for first few rows
                if row_num < 3 {
                    println!("Row {}: cat_1s={}, cat_5s={}, cat_10s={}, total={}", 
                        row_num, data.cat_engage_ema_1s_pct, data.cat_engage_ema_5s_pct, 
                        data.cat_engage_ema_10s_pct, data.total_engagement);
                }
                
                frame_data.push(data);
            }
            Err(e) => {
                eprintln!("Warning: skipping malformed row {}: {}", row_num, e);
            }
        }
    }

    println!("Successfully loaded {} rows of data", frame_data.len());
    Ok(frame_data)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_video> <csv_data>", args[0]);
        std::process::exit(1);
    }
    let video_path = &args[1];
    let csv_path = &args[2];

    // Load CSV with flexible parsing
    let frame_data = load_csv_data(csv_path)?;

    // Check if any data loaded
    if frame_data.is_empty() {
        eprintln!("Error: No valid rows found in CSV. Check CSV format.");
        std::process::exit(1);
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
    let bottom_bar_height = 300; // Increased for more engagement bars
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
    let mut last_x = scaled_video_width / 2;
    let mut last_y = scaled_video_height / 2 + top_bar_height;
    let movement_amplifier = 20.0;
    let trail_length = 15;
    let mut trail: Vec<(i32, i32)> = Vec::new();

    // Dynamic center calculation for 5-second windows
    let window_duration = 5.0;

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

        // Calculate dynamic center based on 5-second window average
        let window_start_time = (current_time / window_duration).floor() * window_duration;
        let window_end_time = window_start_time + window_duration;
        
        // Find all data points in current 5-second window
        let window_data: Vec<&FrameData> = frame_data.iter()
            .filter(|d| d.time_sec >= window_start_time && d.time_sec < window_end_time)
            .collect();
        
        // Calculate average attention position for this window
        let (avg_attention_x, avg_attention_y) = if !window_data.is_empty() {
            let sum_x: f64 = window_data.iter().map(|d| d.attention_center_x).sum();
            let sum_y: f64 = window_data.iter().map(|d| d.attention_center_y).sum();
            (sum_x / window_data.len() as f64, sum_y / window_data.len() as f64)
        } else {
            (0.5, 0.5)
        };

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
        draw_bottom_engagement_bars(&mut canvas, &data, full_width, bottom_bar_height, full_height)?;
        draw_right_vertical_bars(&mut canvas, &data, width, top_bar_height, scaled_video_height)?;

        // --- Calculate attention point coordinates with dynamic centering ---
        let center_x = scaled_video_width / 2;
        let center_y = scaled_video_height / 2;
        
        let attention_x_offset = (data.attention_center_x - avg_attention_x) * movement_amplifier;
        let attention_y_offset = (data.attention_center_y - avg_attention_y) * movement_amplifier;
        
        let mut x = (center_x as f64 + attention_x_offset * scaled_video_width as f64) as i32;
        let mut y = (center_y as f64 + attention_y_offset * scaled_video_height as f64) as i32;
        
        x = x.clamp(0, scaled_video_width-1);
        y = y.clamp(0, scaled_video_height-1);
        y += top_bar_height;

        // Smooth movement
        x = ((0.4 * last_x as f64 + 0.6 * x as f64) as i32).clamp(0, scaled_video_width-1);
        y = ((0.4 * last_y as f64 + 0.6 * y as f64) as i32).clamp(top_bar_height, top_bar_height + scaled_video_height-1);
        last_x = x;
        last_y = y;

        // --- Draw attention trail ---
        trail.push((x, y));
        if trail.len() > trail_length { trail.remove(0); }
        for (i, &(tx, ty)) in trail.iter().enumerate() {
            let alpha = i as f64 / trail.len() as f64;
            let radius = (5.0 + alpha * 10.0) as i32;
            imgproc::circle(
                &mut canvas,
                core::Point::new(tx, ty),
                radius,
                core::Scalar::new(255.0 * alpha, 100.0 * alpha, 0.0, 0.0),
                1,
                imgproc::LINE_8,
                0
            )?;
        }

        // --- Draw yellow hollow outer ring ---
        imgproc::circle(
            &mut canvas,
            core::Point::new(x, y),
            25,
            core::Scalar::new(0.0, 255.0, 255.0, 0.0),
            2,
            imgproc::LINE_8,
            0
        )?;

        writer.write(&canvas)?;
        frame_idx += 1;
    }

    writer.release()?;
    cap.release()?;
    println!("Overlay video written to output_overlay.mp4");
    Ok(())
}

// --- Top Bar (keeping original audio-related bars) ---
fn draw_top_bar(frame: &mut Mat, data: &FrameData, width: i32, height: i32) -> Result<()> {
    let bar_width = (width / 6) as i32;
    let spacing = 20;
    let bars = [
        (data.vocals_rms_energy_ema_1s_pct / 100.0, core::Scalar::new(0.0, 255.0, 0.0, 0.0)),
        (data.vocals_spectral_ema_1s_pct / 100.0, core::Scalar::new(0.0, 0.0, 255.0, 0.0)),
        (data.nonvocals_rms_energy_ema_1s_pct / 100.0, core::Scalar::new(255.0, 0.0, 0.0, 0.0))
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

// --- Bottom Engagement Bars ---
fn draw_bottom_engagement_bars(frame: &mut Mat, data: &FrameData, width: i32, bottom_height: i32, full_height: i32) -> Result<()> {
    let y_start = full_height - bottom_height;
    let bar_height = 20;
    let spacing = 8;
    let max_bar_width = width / 2 - 80; // Space for labels and values

    // Engagement categories with their 1s and 10s percentile values
    let engagement_data = [
        ("Cat", data.cat_engage_ema_1s_pct / 100.0, data.cat_engage_ema_10s_pct / 100.0, core::Scalar::new(0.0, 255.0, 0.0, 0.0)), // Green
        ("Transc", data.transc_engage_ema_1s_pct / 100.0, data.transc_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 255.0, 0.0, 0.0)), // Cyan
        ("Visual", data.visual_engage_ema_1s_pct / 100.0, data.visual_engage_ema_10s_pct / 100.0, core::Scalar::new(0.0, 255.0, 255.0, 0.0)), // Yellow
        ("Vocals RMS", data.vocals_rms_energy_ema_1s_pct / 100.0, data.vocals_rms_energy_ema_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 255.0, 0.0)), // Magenta
        ("Vocals Spec", data.vocals_spectral_ema_1s_pct / 100.0, data.vocals_spectral_ema_10s_pct / 100.0, core::Scalar::new(128.0, 0.0, 255.0, 0.0)), // Purple
        ("NonVoc RMS", data.nonvocals_rms_energy_ema_1s_pct / 100.0, data.nonvocals_rms_energy_ema_10s_pct / 100.0, core::Scalar::new(255.0, 128.0, 0.0, 0.0)), // Orange
        ("NonVoc Spec", data.nonvocals_spectral_ema_1s_pct / 100.0, data.nonvocals_spectral_ema_10s_pct / 100.0, core::Scalar::new(0.0, 128.0, 255.0, 0.0)), // Light Blue
        ("Total", data.total_engagement_pct / 100.0, data.total_engagement_pct / 100.0, core::Scalar::new(255.0, 255.0, 255.0, 0.0)), // White
    ];

    for (i, &(label, val_1s, val_10s, color)) in engagement_data.iter().enumerate() {
        let y = y_start + 10 + i as i32 * (bar_height + spacing);

        // Left side - 1s EMA percentile
        let bar_width_1s = (val_1s * max_bar_width as f64) as i32;
        imgproc::rectangle(
            frame,
            core::Rect::new(70, y, bar_width_1s, bar_height),
            color,
            -1,
            imgproc::LINE_8,
            0
        )?;

        // Label for category
        imgproc::put_text(
            frame,
            &format!("{}1s", label),
            core::Point::new(5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.4,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value for 1s
        imgproc::put_text(
            frame,
            &format!("{:.2}", val_1s * 100.0),
            core::Point::new(70 + bar_width_1s + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.4,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Right side - 10s EMA percentile
        let x_start_10s = width / 2 + 70;
        let bar_width_10s = (val_10s * max_bar_width as f64) as i32;
        imgproc::rectangle(
            frame,
            core::Rect::new(x_start_10s, y, bar_width_10s, bar_height),
            color,
            -1,
            imgproc::LINE_8,
            0
        )?;

        // Label for 10s
        imgproc::put_text(
            frame,
            &format!("{}10s", label),
            core::Point::new(width / 2 + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.4,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value for 10s
        imgproc::put_text(
            frame,
            &format!("{:.2}", val_10s * 100.0),
            core::Point::new(x_start_10s + bar_width_10s + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.4,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    Ok(())
}

// --- Right Vertical Bars ---
fn draw_right_vertical_bars(frame: &mut Mat, data: &FrameData, width: i32, top_bar_height: i32, video_height: i32) -> Result<()> {
    let x_start = width + 10;
    let bar_width = 30;
    let spacing = 35;
    let max_bar_height = video_height - 40; // Leave some padding

    // Vertical bars for key metrics
    let vertical_bars = [
        ("Attn", data.attention_concentration / 100.0, core::Scalar::new(0.0, 255.0, 0.0, 0.0)), // Green
        ("Shift", data.attention_shift_rate / 100.0, core::Scalar::new(255.0, 0.0, 0.0, 0.0)), // Red
        ("TotEng", data.total_engagement_pct / 100.0, core::Scalar::new(255.0, 255.0, 255.0, 0.0)), // White
        ("V1s", data.visual_engage_ema_1s_pct / 100.0, core::Scalar::new(0.0, 255.0, 255.0, 0.0)), // Yellow
        ("V3s", data.visual_engage_ema_3s_pct / 100.0, core::Scalar::new(255.0, 255.0, 0.0, 0.0)), // Cyan
        ("V10s", data.visual_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 255.0, 0.0)), // Magenta
    ];

    for (i, &(label, value, color)) in vertical_bars.iter().enumerate() {
        let x = x_start + i as i32 * spacing;
        let bar_height = (value * max_bar_height as f64) as i32;
        let y_bottom = top_bar_height + video_height - 20;
        let y_top = y_bottom - bar_height;

        // Draw vertical bar
        imgproc::rectangle(
            frame,
            core::Rect::new(x, y_top, bar_width, bar_height),
            color,
            -1,
            imgproc::LINE_8,
            0
        )?;

        // Label at bottom
        imgproc::put_text(
            frame,
            label,
            core::Point::new(x - 5, y_bottom + 15),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.4,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value at top
        imgproc::put_text(
            frame,
            &format!("{:.2}", value * 100.0),
            core::Point::new(x - 10, y_top - 5),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.3,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    Ok(())
}