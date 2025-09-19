#!/usr/bin/env python3
"""
Engagement Clip Detection Script
Analyzes CSV data to identify clippable moments based on engagement metrics.
"""

import pandas as pd
import numpy as np
import argparse
import sys
from pathlib import Path
import matplotlib.pyplot as plt

def load_and_validate_csv(file_path):
    """Load CSV and validate required columns exist."""
    try:
        df = pd.read_csv(file_path)
        print(f"Loaded CSV with {len(df)} rows and {len(df.columns)} columns")
        
        required_cols = [
            'time_sec', 'total_engag_10s_pct', 'total_engag_1s_pct', 
            'total_engag_30s_pct', 'total_engag_5s_pct', 'total_engag_raw'
        ]
        
        missing_cols = [col for col in required_cols if col not in df.columns]
        if missing_cols:
            print(f"Error: Missing required columns: {missing_cols}")
            print(f"Available columns: {list(df.columns)}")
            return None
            
        # Sort by time to ensure proper order
        df = df.sort_values('time_sec').reset_index(drop=True)
        print("Data loaded and sorted by time successfully")
        return df
        
    except Exception as e:
        print(f"Error loading CSV: {e}")
        return None

def calculate_engagement_score(row):
    """Calculate a weighted engagement score combining all metrics."""
    # Weight longer timeframes more heavily for sustained engagement
    score = (
        row['total_engag_1s_pct'] * 0.1 +      # Immediate spikes
        row['total_engag_5s_pct'] * 0.2 +      # Short bursts
        row['total_engag_10s_pct'] * 0.3 +     # Medium sustained
        row['total_engag_30s_pct'] * 0.4       # Long sustained (highest weight)
    )
    return score

def detect_spikes(df, spike_threshold_percentile=95, min_spike_gap=5):
    """Detect sudden engagement spikes (jumpscares/yells)."""
    spikes = []
    
    # Calculate engagement score and its rolling statistics
    df['engagement_score'] = df.apply(calculate_engagement_score, axis=1)
    df['score_rolling_mean'] = df['engagement_score'].rolling(window=10, center=True).mean()
    df['score_std'] = df['engagement_score'].rolling(window=20, center=True).std()
    
    # Find spikes based on 1s and 5s engagement (more reactive to sudden changes)
    spike_threshold = np.percentile(df['total_engag_1s_pct'].dropna(), spike_threshold_percentile)
    
    potential_spikes = df[
        (df['total_engag_1s_pct'] > spike_threshold) |
        (df['total_engag_5s_pct'] > np.percentile(df['total_engag_5s_pct'].dropna(), spike_threshold_percentile))
    ].copy()
    
    if len(potential_spikes) == 0:
        return spikes
    
    # Group nearby spikes to avoid duplicates
    last_spike_time = -float('inf')
    
    for idx, row in potential_spikes.iterrows():
        if row['time_sec'] - last_spike_time >= min_spike_gap:
            spikes.append({
                'start_time': max(0, row['time_sec'] - 2),  # 2 seconds before spike
                'end_time': row['time_sec'] + 3,            # 3 seconds after spike
                'peak_time': row['time_sec'],
                'engagement_1s': row['total_engag_1s_pct'],
                'engagement_5s': row['total_engag_5s_pct'],
                'engagement_score': row['engagement_score'],
                'type': 'spike'
            })
            last_spike_time = row['time_sec']
    
    print(f"Detected {len(spikes)} engagement spikes")
    return spikes

def detect_sustained_moments(df, duration_threshold=15, engagement_threshold_percentile=75):
    """Detect longer periods of sustained high engagement."""
    sustained_moments = []
    
    # Calculate rolling engagement for sustained periods (focus on 10s and 30s metrics)
    df['sustained_score'] = (
        df['total_engag_10s_pct'] * 0.4 + 
        df['total_engag_30s_pct'] * 0.6
    )
    
    # Smooth the sustained score to avoid small fluctuations
    df['sustained_smooth'] = df['sustained_score'].rolling(window=5, center=True).mean()
    
    threshold = np.percentile(df['sustained_smooth'].dropna(), engagement_threshold_percentile)
    
    # Find continuous periods above threshold
    above_threshold = df['sustained_smooth'] > threshold
    
    # Group continuous periods
    groups = []
    current_group = []
    
    for idx, is_above in enumerate(above_threshold):
        if is_above:
            current_group.append(idx)
        else:
            if current_group:
                groups.append(current_group)
                current_group = []
    
    # Don't forget the last group
    if current_group:
        groups.append(current_group)
    
    # Filter groups by duration and create clips
    for group in groups:
        start_idx, end_idx = group[0], group[-1]
        start_time = df.iloc[start_idx]['time_sec']
        end_time = df.iloc[end_idx]['time_sec']
        duration = end_time - start_time
        
        if duration >= duration_threshold:
            # Extend clip slightly for context
            extended_start = max(0, start_time - 5)
            extended_end = end_time + 5
            
            # Calculate average engagement metrics for this period
            period_data = df.iloc[start_idx:end_idx+1]
            avg_10s = period_data['total_engag_10s_pct'].mean()
            avg_30s = period_data['total_engag_30s_pct'].mean()
            avg_sustained = period_data['sustained_score'].mean()
            
            sustained_moments.append({
                'start_time': extended_start,
                'end_time': extended_end,
                'duration': duration,
                'avg_engagement_10s': avg_10s,
                'avg_engagement_30s': avg_30s,
                'avg_sustained_score': avg_sustained,
                'type': 'sustained'
            })
    
    print(f"Detected {len(sustained_moments)} sustained engagement periods")
    return sustained_moments

def format_time(seconds):
    """Convert seconds to MM:SS format."""
    minutes = int(seconds // 60)
    secs = int(seconds % 60)
    return f"{minutes:02d}:{secs:02d}"

def create_output_files(spikes, sustained_moments, output_prefix):
    """Create text and CSV output files."""
    
    # Create text file with readable format
    txt_path = f"{output_prefix}_clips.txt"
    with open(txt_path, 'w') as f:
        f.write("ENGAGEMENT-BASED CLIPPABLE MOMENTS\n")
        f.write("=" * 50 + "\n\n")
        
        # Spikes section
        f.write("ENGAGEMENT SPIKES (Jumpscares/Yells)\n")
        f.write("-" * 35 + "\n")
        if not spikes:
            f.write("No significant spikes detected.\n\n")
        else:
            for i, spike in enumerate(spikes, 1):
                f.write(f"Spike #{i}\n")
                f.write(f"  Time Range: {format_time(spike['start_time'])} - {format_time(spike['end_time'])}\n")
                f.write(f"  Peak at: {format_time(spike['peak_time'])}\n")
                f.write(f"  1s Engagement: {spike['engagement_1s']:.1f}%\n")
                f.write(f"  5s Engagement: {spike['engagement_5s']:.1f}%\n")
                f.write(f"  Engagement Score: {spike['engagement_score']:.2f}\n\n")
        
        # Sustained moments section
        f.write("\nSUSTAINED HIGH ENGAGEMENT PERIODS\n")
        f.write("-" * 35 + "\n")
        if not sustained_moments:
            f.write("No sustained high engagement periods detected.\n")
        else:
            for i, moment in enumerate(sustained_moments, 1):
                f.write(f"Period #{i}\n")
                f.write(f"  Time Range: {format_time(moment['start_time'])} - {format_time(moment['end_time'])}\n")
                f.write(f"  Duration: {moment['duration']:.1f} seconds\n")
                f.write(f"  Avg 10s Engagement: {moment['avg_engagement_10s']:.1f}%\n")
                f.write(f"  Avg 30s Engagement: {moment['avg_engagement_30s']:.1f}%\n")
                f.write(f"  Sustained Score: {moment['avg_sustained_score']:.2f}\n\n")
    
    # Create CSV file with all clips
    csv_path = f"{output_prefix}_clips.csv"
    all_clips = []
    
    # Add spikes to clips list
    for spike in spikes:
        all_clips.append({
            'type': 'spike',
            'start_time': spike['start_time'],
            'end_time': spike['end_time'],
            'duration': spike['end_time'] - spike['start_time'],
            'start_formatted': format_time(spike['start_time']),
            'end_formatted': format_time(spike['end_time']),
            'peak_time': spike.get('peak_time', ''),
            'engagement_1s_pct': spike.get('engagement_1s', ''),
            'engagement_5s_pct': spike.get('engagement_5s', ''),
            'avg_engagement_10s_pct': '',
            'avg_engagement_30s_pct': '',
            'engagement_score': spike.get('engagement_score', ''),
            'sustained_score': ''
        })
    
    # Add sustained moments to clips list
    for moment in sustained_moments:
        all_clips.append({
            'type': 'sustained',
            'start_time': moment['start_time'],
            'end_time': moment['end_time'],
            'duration': moment['duration'],
            'start_formatted': format_time(moment['start_time']),
            'end_formatted': format_time(moment['end_time']),
            'peak_time': '',
            'engagement_1s_pct': '',
            'engagement_5s_pct': '',
            'avg_engagement_10s_pct': moment['avg_engagement_10s'],
            'avg_engagement_30s_pct': moment['avg_engagement_30s'],
            'engagement_score': '',
            'sustained_score': moment['avg_sustained_score']
        })
    
    # Sort clips by start time
    all_clips.sort(key=lambda x: x['start_time'])
    
    # Save to CSV
    if all_clips:
        clips_df = pd.DataFrame(all_clips)
        clips_df.to_csv(csv_path, index=False)
        print(f"Created CSV file: {csv_path}")
    else:
        # Create empty CSV with headers
        pd.DataFrame(columns=[
            'type', 'start_time', 'end_time', 'duration', 'start_formatted', 'end_formatted',
            'peak_time', 'engagement_1s_pct', 'engagement_5s_pct', 'avg_engagement_10s_pct',
            'avg_engagement_30s_pct', 'engagement_score', 'sustained_score'
        ]).to_csv(csv_path, index=False)
        print(f"Created empty CSV file: {csv_path}")
    
    print(f"Created text file: {txt_path}")
    return txt_path, csv_path

def plot_engagement(df, spikes, sustained_moments, output_prefix):
    """Plot engagement metrics and highlight detected clips."""
    plt.figure(figsize=(14, 7))

    # Plot the key engagement metrics
    plt.plot(df['time_sec'], df['total_engag_1s_pct'], 
             label='1s Engagement', alpha=0.6, color='orange')
    plt.plot(df['time_sec'], df['total_engag_5s_pct'], 
             label='5s Engagement', alpha=0.6, color='blue')
    plt.plot(df['time_sec'], df['total_engag_10s_pct'], 
             label='10s Engagement', alpha=0.7, color='green')
    plt.plot(df['time_sec'], df['total_engag_30s_pct'], 
             label='30s Engagement', alpha=0.7, color='purple')

    # Engagement score
    if 'engagement_score' in df.columns:
        plt.plot(df['time_sec'], df['engagement_score'], 
                 label='Weighted Score', color='black', linewidth=2)

    # Highlight spikes
    for spike in spikes:
        plt.axvspan(spike['start_time'], spike['end_time'], 
                    color='red', alpha=0.3, label='Spike' if 'Spike' not in plt.gca().get_legend_handles_labels()[1] else "")

    # Highlight sustained periods
    for moment in sustained_moments:
        plt.axvspan(moment['start_time'], moment['end_time'], 
                    color='yellow', alpha=0.2, label='Sustained' if 'Sustained' not in plt.gca().get_legend_handles_labels()[1] else "")

    plt.xlabel("Time (seconds)")
    plt.ylabel("Engagement (%)")
    plt.title("Engagement Metrics Over Time")
    plt.legend(loc="upper right")
    plt.tight_layout()

    # Save to file
    plot_path = f"{output_prefix}_engagement_plot.png"
    plt.savefig(plot_path, dpi=200)
    plt.close()
    print(f"Created engagement visualization: {plot_path}")
    return plot_path

def main():
    parser = argparse.ArgumentParser(description='Detect clippable moments from engagement CSV data')
    parser.add_argument('input_csv', help='Input CSV file path')
    parser.add_argument('-o', '--output', help='Output file prefix (default: same as input filename)')
    parser.add_argument('--spike-threshold', type=float, default=95, 
                       help='Percentile threshold for spike detection (default: 95)')
    parser.add_argument('--sustained-threshold', type=float, default=75,
                       help='Percentile threshold for sustained engagement (default: 75)')
    parser.add_argument('--min-duration', type=float, default=15,
                       help='Minimum duration for sustained moments in seconds (default: 15)')
    
    args = parser.parse_args()
    
    # Validate input file
    input_path = Path(args.input_csv)
    if not input_path.exists():
        print(f"Error: Input file '{args.input_csv}' not found")
        sys.exit(1)
    
    # Set output prefix
    output_prefix = args.output or input_path.stem
    
    # Load and process data
    df = load_and_validate_csv(args.input_csv)
    if df is None:
        sys.exit(1)
    
    print("\nAnalyzing engagement data...")
    
    # Detect clips
    spikes = detect_spikes(df, spike_threshold_percentile=args.spike_threshold)
    sustained_moments = detect_sustained_moments(
        df, 
        duration_threshold=args.min_duration,
        engagement_threshold_percentile=args.sustained_threshold
    )
    
    # Create output files
    txt_file, csv_file = create_output_files(spikes, sustained_moments, output_prefix)
    
    plot_file = plot_engagement(df, spikes, sustained_moments, output_prefix)
    print(f"  {plot_file}")

    print(f"\nSummary:")
    print(f"  Spikes detected: {len(spikes)}")
    print(f"  Sustained moments detected: {len(sustained_moments)}")
    print(f"  Total clips: {len(spikes) + len(sustained_moments)}")
    print(f"\nOutput files created:")
    print(f"  {txt_file}")
    print(f"  {csv_file}")

if __name__ == "__main__":
    main()