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
    // Attention metrics
    attention_center_x: f64,
    attention_center_x_ema_10s_pct: f64,
    attention_center_x_ema_1s_pct: f64,
    attention_center_y: f64,
    attention_center_y_ema_10s_pct: f64,
    attention_center_y_ema_1s_pct: f64,
    attention_concentration: f64,
    attention_concentration_ema_10s_pct: f64,
    attention_concentration_ema_1s_pct: f64,
    attention_shift_rate: f64,
    attention_shift_rate_ema_10s_pct: f64,
    attention_shift_rate_ema_1s_pct: f64,
    
    // Engagement metrics
    cat_engage_percentile: f64,
    
    // Channel 1 audio features
    chan1_rms_energy_engage_ema_10s_pct: f64,
    chan1_rms_energy_engage_ema_1s_pct: f64,
    chan1_rms_energy_engage_ema_5s_pct: f64,
    chan1_spectral_engage_ema_10s_pct: f64,
    chan1_spectral_engage_ema_1s_pct: f64,
    chan1_spectral_engage_ema_5s_pct: f64,
    chan1_vfeats_engage_ema_10s_pct: f64,
    chan1_vfeats_engage_ema_1s_pct: f64,
    chan1_vfeats_engage_ema_5s_pct: f64,
    
    // Channel 2 audio features
    chan2_rms_energy_engage_ema_10s_pct: f64,
    chan2_rms_energy_engage_ema_1s_pct: f64,
    chan2_rms_energy_engage_ema_5s_pct: f64,
    chan2_spectral_engage_ema_10s_pct: f64,
    chan2_spectral_engage_ema_1s_pct: f64,
    chan2_spectral_engage_ema_5s_pct: f64,
    chan2_vfeats_engage_ema_10s_pct: f64,
    chan2_vfeats_engage_ema_1s_pct: f64,
    chan2_vfeats_engage_ema_5s_pct: f64,
    
    // Emotion and other metrics
    emotion_engage_10s_pct: f64,
    emotion_engage_1s_pct: f64,
    
    // Non-vocal channel 1 features
    nonvocal_chan1_rms_energy_engage_ema_10s_pct: f64,
    nonvocal_chan1_rms_energy_engage_ema_1s_pct: f64,
    nonvocal_chan1_rms_energy_engage_ema_5s_pct: f64,
    nonvocal_chan1_spectral_engage_ema_10s_pct: f64,
    nonvocal_chan1_spectral_engage_ema_1s_pct: f64,
    nonvocal_chan1_spectral_engage_ema_5s_pct: f64,
    
    // Updated total engagement fields
    total_engag_raw: f64,
    total_engag_10s_pct: f64,
    total_engag_1s_pct: f64,
    total_engag_30s_pct: f64,
    total_engag_5s_pct: f64,
    
    // Visual engagement
    visual_engage_ema_10s_pct: f64,
    visual_engage_ema_1s_pct: f64,
    visual_engage_ema_3s_pct: f64,
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
                
                // Map columns by exact header name
                for (i, header) in headers.iter().enumerate() {
                    if let Some(value) = record.get(i) {
                        let val = safe_parse_f64(value);
                        
                        match header {
                            // Time
                            "time_sec" => data.time_sec = val,
                            
                            // Attention metrics
                            "attention_center_x" => data.attention_center_x = val,
                            "attention_center_x_ema_10s_pct" => data.attention_center_x_ema_10s_pct = val,
                            "attention_center_x_ema_1s_pct" => data.attention_center_x_ema_1s_pct = val,
                            "attention_center_y" => data.attention_center_y = val,
                            "attention_center_y_ema_10s_pct" => data.attention_center_y_ema_10s_pct = val,
                            "attention_center_y_ema_1s_pct" => data.attention_center_y_ema_1s_pct = val,
                            "attention_concentration" => data.attention_concentration = val,
                            "attention_concentration_ema_10s_pct" => data.attention_concentration_ema_10s_pct = val,
                            "attention_concentration_ema_1s_pct" => data.attention_concentration_ema_1s_pct = val,
                            "attention_shift_rate" => data.attention_shift_rate = val,
                            "attention_shift_rate_ema_10s_pct" => data.attention_shift_rate_ema_10s_pct = val,
                            "attention_shift_rate_ema_1s_pct" => data.attention_shift_rate_ema_1s_pct = val,
                            
                            // Engagement
                            "cat_engage_percentile" => data.cat_engage_percentile = val,
                            
                            // Channel 1 audio features
                            "chan1_rms_energy_engage_ema_10s_pct" => data.chan1_rms_energy_engage_ema_10s_pct = val,
                            "chan1_rms_energy_engage_ema_1s_pct" => data.chan1_rms_energy_engage_ema_1s_pct = val,
                            "chan1_rms_energy_engage_ema_5s_pct" => data.chan1_rms_energy_engage_ema_5s_pct = val,
                            "chan1_spectral_engage_ema_10s_pct" => data.chan1_spectral_engage_ema_10s_pct = val,
                            "chan1_spectral_engage_ema_1s_pct" => data.chan1_spectral_engage_ema_1s_pct = val,
                            "chan1_spectral_engage_ema_5s_pct" => data.chan1_spectral_engage_ema_5s_pct = val,
                            "chan1_vfeats_engage_ema_10s_pct" => data.chan1_vfeats_engage_ema_10s_pct = val,
                            "chan1_vfeats_engage_ema_1s_pct" => data.chan1_vfeats_engage_ema_1s_pct = val,
                            "chan1_vfeats_engage_ema_5s_pct" => data.chan1_vfeats_engage_ema_5s_pct = val,
                            
                            // Channel 2 audio features
                            "chan2_rms_energy_engage_ema_10s_pct" => data.chan2_rms_energy_engage_ema_10s_pct = val,
                            "chan2_rms_energy_engage_ema_1s_pct" => data.chan2_rms_energy_engage_ema_1s_pct = val,
                            "chan2_rms_energy_engage_ema_5s_pct" => data.chan2_rms_energy_engage_ema_5s_pct = val,
                            "chan2_spectral_engage_ema_10s_pct" => data.chan2_spectral_engage_ema_10s_pct = val,
                            "chan2_spectral_engage_ema_1s_pct" => data.chan2_spectral_engage_ema_1s_pct = val,
                            "chan2_spectral_engage_ema_5s_pct" => data.chan2_spectral_engage_ema_5s_pct = val,
                            "chan2_vfeats_engage_ema_10s_pct" => data.chan2_vfeats_engage_ema_10s_pct = val,
                            "chan2_vfeats_engage_ema_1s_pct" => data.chan2_vfeats_engage_ema_1s_pct = val,
                            "chan2_vfeats_engage_ema_5s_pct" => data.chan2_vfeats_engage_ema_5s_pct = val,
                            
                            // Emotion
                            "emotion_engage_10s_pct" => data.emotion_engage_10s_pct = val,
                            "emotion_engage_1s_pct" => data.emotion_engage_1s_pct = val,
                            
                            // Non-vocal channel 1
                            "nonvocal_chan1_rms_energy_engage_ema_10s_pct" => data.nonvocal_chan1_rms_energy_engage_ema_10s_pct = val,
                            "nonvocal_chan1_rms_energy_engage_ema_1s_pct" => data.nonvocal_chan1_rms_energy_engage_ema_1s_pct = val,
                            "nonvocal_chan1_rms_energy_engage_ema_5s_pct" => data.nonvocal_chan1_rms_energy_engage_ema_5s_pct = val,
                            "nonvocal_chan1_spectral_engage_ema_10s_pct" => data.nonvocal_chan1_spectral_engage_ema_10s_pct = val,
                            "nonvocal_chan1_spectral_engage_ema_1s_pct" => data.nonvocal_chan1_spectral_engage_ema_1s_pct = val,
                            "nonvocal_chan1_spectral_engage_ema_5s_pct" => data.nonvocal_chan1_spectral_engage_ema_5s_pct = val,
                            
                            // Updated total engagement column names
                            "total_engag_raw" => data.total_engag_raw = val,
                            "total_engag_10s_pct" => data.total_engag_10s_pct = val,
                            "total_engag_1s_pct" => data.total_engag_1s_pct = val,
                            "total_engag_30s_pct" => data.total_engag_30s_pct = val,
                            "total_engag_5s_pct" => data.total_engag_5s_pct = val,
                            
                            // Visual engagement
                            "visual_engage_ema_10s_pct" => data.visual_engage_ema_10s_pct = val,
                            "visual_engage_ema_1s_pct" => data.visual_engage_ema_1s_pct = val,
                            "visual_engage_ema_3s_pct" => data.visual_engage_ema_3s_pct = val,
                            
                            _ => {} // Ignore unrecognized columns
                        }
                    }
                }
                
                // Debug output for first few rows
                if row_num < 3 {
                    println!("Row {}: time={:.3}, cat_engage={:.2}, total_1s={:.2}, visual_1s={:.2}", 
                        row_num, data.time_sec, data.cat_engage_percentile, 
                        data.total_engag_1s_pct, data.visual_engage_ema_1s_pct);
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

    // Removed top_bar_height since we're not using top bars anymore
    let bottom_bar_height = 350; // Increased for more engagement bars
    let right_bar_width = 350; // Increased for more metrics

    let full_width = width + right_bar_width;
    let full_height = height + bottom_bar_height; // No top bar anymore

    // Scale video to fill more of the available space
    let scaled_video_width = width;
    let scaled_video_height = height;

    let fourcc = videoio::VideoWriter::fourcc('a','v','c','1')?;
    let mut writer = videoio::VideoWriter::new(
        "output_overlay.mp4",
        fourcc,
        fps,
        core::Size::new(full_width, full_height),
        true
    )?;

    // Attention smoothing and trail - adjusted for full-size video
    let mut last_x = scaled_video_width / 2;
    let mut last_y = scaled_video_height / 2;
    let movement_amplifier = 30.0; // Increased since video is now larger
    let trail_length = 20;
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
        
        // With 30 data points per second, find the closest data point
        let data = frame_data.iter()
            .min_by(|a, b| ((a.time_sec - current_time).abs())
                .partial_cmp(&(b.time_sec - current_time).abs()).unwrap())
            .unwrap_or(&FrameData::default())
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

        // --- Resize original frame to fill left side (no scaling down) ---
        let mut resized_frame = Mat::default();
        imgproc::resize(
            &frame,
            &mut resized_frame,
            core::Size::new(scaled_video_width, scaled_video_height),
            0.0,
            0.0,
            imgproc::INTER_LINEAR
        )?;

        // Copy resized frame to canvas (positioned at left side, full height)
        let roi = core::Rect::new(0, 0, scaled_video_width, scaled_video_height);
        let mut roi_mat = core::Mat::roi_mut(&mut canvas, roi)?;
        resized_frame.copy_to(&mut roi_mat)?;

        // --- Draw overlay elements ---
        // Removed draw_top_bar call
        draw_bottom_engagement_bars(&mut canvas, &data, full_width, bottom_bar_height, full_height)?;
        draw_right_vertical_bars(&mut canvas, &data, width, 0, scaled_video_height, right_bar_width)?;

        // --- Calculate attention point coordinates with dynamic centering ---
        let center_x = scaled_video_width / 2;
        let center_y = scaled_video_height / 2;
        
        let attention_x_offset = (data.attention_center_x - avg_attention_x) * movement_amplifier;
        let attention_y_offset = (data.attention_center_y - avg_attention_y) * movement_amplifier;
        
        let mut x = (center_x as f64 + attention_x_offset * scaled_video_width as f64) as i32;
        let mut y = (center_y as f64 + attention_y_offset * scaled_video_height as f64) as i32;
        
        x = x.clamp(0, scaled_video_width-1);
        y = y.clamp(0, scaled_video_height-1);

        // Smooth movement (less smoothing due to higher data rate)
        x = ((0.2 * last_x as f64 + 0.8 * x as f64) as i32).clamp(0, scaled_video_width-1);
        y = ((0.2 * last_y as f64 + 0.8 * y as f64) as i32).clamp(0, scaled_video_height-1);
        last_x = x;
        last_y = y;

        // --- Draw attention trail with larger radius for bigger video ---
        trail.push((x, y));
        if trail.len() > trail_length { trail.remove(0); }
        for (i, &(tx, ty)) in trail.iter().enumerate() {
            let alpha = i as f64 / trail.len() as f64;
            let radius = (5.0 + alpha * 12.0) as i32; // Larger trail points
            imgproc::circle(
                &mut canvas,
                core::Point::new(tx, ty),
                radius,
                core::Scalar::new(255.0 * alpha, 100.0 * alpha, 0.0, 0.0),
                2, // Thicker trail lines
                imgproc::LINE_8,
                0
            )?;
        }

        // --- Draw larger yellow hollow outer ring ---
        imgproc::circle(
            &mut canvas,
            core::Point::new(x, y),
            40, // Larger circle for bigger video
            core::Scalar::new(0.0, 255.0, 255.0, 0.0),
            3, // Thicker line
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

// --- Bottom Engagement Bars (Total engagement moved to top) ---
fn draw_bottom_engagement_bars(frame: &mut Mat, data: &FrameData, width: i32, bottom_height: i32, full_height: i32) -> Result<()> {
    let y_start = full_height - bottom_height;
    let bar_height = 18;
    let spacing = 6;
    let max_bar_width = width / 2 - 100; // Space for labels and values

    // Updated engagement categories with Total at the top and new column names
    let engagement_data = [
        ("Total", data.total_engag_1s_pct / 100.0, data.total_engag_10s_pct / 100.0, core::Scalar::new(255.0, 255.0, 255.0, 0.0)), // White - moved to top
        ("Cat", data.cat_engage_percentile / 100.0, data.cat_engage_percentile / 100.0, core::Scalar::new(0.0, 255.0, 0.0, 0.0)), // Green
        ("Visual", data.visual_engage_ema_1s_pct / 100.0, data.visual_engage_ema_10s_pct / 100.0, core::Scalar::new(0.0, 255.0, 255.0, 0.0)), // Yellow
        ("Ch1 RMS", data.chan1_rms_energy_engage_ema_1s_pct / 100.0, data.chan1_rms_energy_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 255.0, 0.0)), // Magenta
        ("Ch1 Spec", data.chan1_spectral_engage_ema_1s_pct / 100.0, data.chan1_spectral_engage_ema_10s_pct / 100.0, core::Scalar::new(128.0, 0.0, 255.0, 0.0)), // Purple
        ("Ch1 VFeat", data.chan1_vfeats_engage_ema_1s_pct / 100.0, data.chan1_vfeats_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 128.0, 0.0, 0.0)), // Orange
        ("Ch2 RMS", data.chan2_rms_energy_engage_ema_1s_pct / 100.0, data.chan2_rms_energy_engage_ema_10s_pct / 100.0, core::Scalar::new(0.0, 128.0, 255.0, 0.0)), // Light Blue
        ("Ch2 Spec", data.chan2_spectral_engage_ema_1s_pct / 100.0, data.chan2_spectral_engage_ema_10s_pct / 100.0, core::Scalar::new(128.0, 255.0, 0.0, 0.0)), // Light Green
        ("Ch2 VFeat", data.chan2_vfeats_engage_ema_1s_pct / 100.0, data.chan2_vfeats_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 128.0, 0.0)), // Pink
        ("NonVoc RMS", data.nonvocal_chan1_rms_energy_engage_ema_1s_pct / 100.0, data.nonvocal_chan1_rms_energy_engage_ema_10s_pct / 100.0, core::Scalar::new(128.0, 128.0, 255.0, 0.0)), // Light Purple
        ("NonVoc Spec", data.nonvocal_chan1_spectral_engage_ema_1s_pct / 100.0, data.nonvocal_chan1_spectral_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 128.0, 128.0, 0.0)), // Light Red
        ("Emotion", data.emotion_engage_1s_pct / 100.0, data.emotion_engage_10s_pct / 100.0, core::Scalar::new(128.0, 255.0, 128.0, 0.0)), // Light Green
    ];

    for (i, &(label, val_1s, val_10s, color)) in engagement_data.iter().enumerate() {
        let y = y_start + 10 + i as i32 * (bar_height + spacing);

        // Left side - 1s EMA percentile
        let bar_width_1s = (val_1s * max_bar_width as f64) as i32;
        imgproc::rectangle(
            frame,
            core::Rect::new(90, y, bar_width_1s, bar_height),
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
            0.35,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value for 1s
        imgproc::put_text(
            frame,
            &format!("{:.1}", val_1s * 100.0),
            core::Point::new(90 + bar_width_1s + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.35,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Right side - 10s EMA percentile
        let x_start_10s = width / 2 + 90;
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
            0.35,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value for 10s
        imgproc::put_text(
            frame,
            &format!("{:.1}", val_10s * 100.0),
            core::Point::new(x_start_10s + bar_width_10s + 5, y + bar_height - 4),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.35,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    Ok(())
}

// --- Right Vertical Bars ---
fn draw_right_vertical_bars(frame: &mut Mat, data: &FrameData, width: i32, top_offset: i32, video_height: i32, right_bar_width: i32) -> Result<()> {
    let x_start = width + 15;
    let bar_width = 25;
    let spacing = 30;
    let max_bar_height = video_height - 40; // Leave some padding

    // Vertical bars for key metrics using updated column names
    let vertical_bars = [
        ("AttnCon", data.attention_concentration_ema_1s_pct / 100.0, core::Scalar::new(0.0, 255.0, 0.0, 0.0)), // Green - Attention Concentration
        ("AttnShf", data.attention_shift_rate_ema_1s_pct / 100.0, core::Scalar::new(255.0, 0.0, 0.0, 0.0)), // Blue - Attention Shift
        ("TotEng", data.total_engag_1s_pct / 100.0, core::Scalar::new(255.0, 255.0, 255.0, 0.0)), // White - Total Engagement
        ("Vis1s", data.visual_engage_ema_1s_pct / 100.0, core::Scalar::new(0.0, 255.0, 255.0, 0.0)), // Yellow - Visual 1s
        ("Vis3s", data.visual_engage_ema_3s_pct / 100.0, core::Scalar::new(255.0, 255.0, 0.0, 0.0)), // Cyan - Visual 3s
        ("Vis10s", data.visual_engage_ema_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 255.0, 0.0)), // Magenta - Visual 10s
        ("Cat", data.cat_engage_percentile / 100.0, core::Scalar::new(128.0, 255.0, 0.0, 0.0)), // Light Green - Cat Engagement
        ("Emot1s", data.emotion_engage_1s_pct / 100.0, core::Scalar::new(255.0, 128.0, 0.0, 0.0)), // Orange - Emotion 1s
        ("Emot10s", data.emotion_engage_10s_pct / 100.0, core::Scalar::new(255.0, 0.0, 128.0, 0.0)), // Pink - Emotion 10s
        ("Tot30s", data.total_engag_30s_pct / 100.0, core::Scalar::new(128.0, 128.0, 255.0, 0.0)), // Light Purple - Total 30s
    ];

    for (i, &(label, value, color)) in vertical_bars.iter().enumerate() {
        let x = x_start + i as i32 * spacing;
        let bar_height = (value * max_bar_height as f64) as i32;
        let y_bottom = top_offset + video_height - 20;
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

        // Label at bottom (rotated text would be ideal, but using abbreviated labels)
        imgproc::put_text(
            frame,
            label,
            core::Point::new(x - 8, y_bottom + 15),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.3,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;

        // Value at top
        imgproc::put_text(
            frame,
            &format!("{:.1}", value * 100.0),
            core::Point::new(x - 8, y_top - 5),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.25,
            core::Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_8,
            false
        )?;
    }

    // Add attention center coordinates display
    let coord_y_start = top_offset + 20;
    imgproc::put_text(
        frame,
        &format!("Attn X: {:.3}", data.attention_center_x),
        core::Point::new(width + 10, coord_y_start),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.4,
        core::Scalar::new(255.0, 255.0, 255.0, 0.0),
        1,
        imgproc::LINE_8,
        false
    )?;

    imgproc::put_text(
        frame,
        &format!("Attn Y: {:.3}", data.attention_center_y),
        core::Point::new(width + 10, coord_y_start + 20),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.4,
        core::Scalar::new(255.0, 255.0, 255.0, 0.0),
        1,
        imgproc::LINE_8,
        false
    )?;

    imgproc::put_text(
        frame,
        &format!("Time: {:.2}s", data.time_sec),
        core::Point::new(width + 10, coord_y_start + 40),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.4,
        core::Scalar::new(255.0, 255.0, 255.0, 0.0),
        1,
        imgproc::LINE_8,
        false
    )?;

    Ok(())
}