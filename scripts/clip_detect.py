#!/usr/bin/env python3
"""
Engagement Clip Detection Script
Analyzes CSV data to identify clippable moments based on engagement metrics.
Enhanced with crossover-based spike detection and integral calculation.
"""

import pandas as pd
import numpy as np
import argparse
import sys
from pathlib import Path
import matplotlib.pyplot as plt
from scipy import integrate

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

def find_crossover_points(df):
    """Find points where 1s engagement crosses above and below 5s engagement."""
    crossovers = []
    
    # Calculate the difference between 1s and 5s engagement
    df['eng_diff'] = df['total_engag_1s_pct'] - df['total_engag_5s_pct']
    
    # Find sign changes in the difference (crossover points)
    for i in range(1, len(df)):
        prev_diff = df.iloc[i-1]['eng_diff']
        curr_diff = df.iloc[i]['eng_diff']
        
        # Skip if either value is NaN
        if pd.isna(prev_diff) or pd.isna(curr_diff):
            continue
            
        # Check for crossover (sign change)
        if prev_diff <= 0 < curr_diff:
            # Crossover up (1s crosses above 5s)
            crossovers.append({
                'time': df.iloc[i]['time_sec'],
                'index': i,
                'type': 'up',
                'eng_1s': df.iloc[i]['total_engag_1s_pct'],
                'eng_5s': df.iloc[i]['total_engag_5s_pct']
            })
        elif prev_diff >= 0 > curr_diff:
            # Crossover down (1s crosses below 5s)
            crossovers.append({
                'time': df.iloc[i]['time_sec'],
                'index': i,
                'type': 'down',
                'eng_1s': df.iloc[i]['total_engag_1s_pct'],
                'eng_5s': df.iloc[i]['total_engag_5s_pct']
            })
    
    print(f"Found {len(crossovers)} crossover points")
    return crossovers

def calculate_integral_between_curves(df, start_idx, end_idx):
    """Calculate the integral (area) between 1s and 5s engagement curves."""
    if start_idx >= end_idx or end_idx >= len(df):
        return 0.0
    
    # Extract the segment of data
    segment = df.iloc[start_idx:end_idx+1].copy()
    
    # Remove any NaN values
    segment = segment.dropna(subset=['total_engag_1s_pct', 'total_engag_5s_pct', 'time_sec'])
    
    if len(segment) < 2:
        return 0.0
    
    # Calculate the difference at each point
    segment['diff'] = segment['total_engag_1s_pct'] - segment['total_engag_5s_pct']
    
    # Use trapezoidal integration to calculate area
    time_points = segment['time_sec'].values
    diff_values = segment['diff'].values
    
    # Calculate integral using trapezoidal rule
    integral = np.trapz(diff_values, time_points)
    
    return integral

def detect_crossover_spikes(df, min_integral_threshold=5.0, min_duration=1.0):
    """Detect spikes based on crossover analysis and integral calculation."""
    spikes = []
    
    # Find all crossover points
    crossovers = find_crossover_points(df)
    
    if len(crossovers) < 2:
        print("Not enough crossover points found for spike detection")
        return spikes
    
    # Pair up crossovers to find spike ranges (up -> down)
    i = 0
    while i < len(crossovers) - 1:
        current = crossovers[i]
        
        # Look for an 'up' crossover
        if current['type'] == 'up':
            # Find the next 'down' crossover
            for j in range(i + 1, len(crossovers)):
                next_crossover = crossovers[j]
                if next_crossover['type'] == 'down':
                    # Found a complete spike range
                    start_idx = current['index']
                    end_idx = next_crossover['index']
                    start_time = current['time']
                    end_time = next_crossover['time']
                    duration = end_time - start_time
                    
                    # Check minimum duration
                    if duration >= min_duration:
                        # Calculate integral (area between curves)
                        integral = calculate_integral_between_curves(df, start_idx, end_idx)
                        
                        # Only include spikes with significant integral
                        if integral >= min_integral_threshold:
                            # Find peak point within the range
                            segment = df.iloc[start_idx:end_idx+1]
                            peak_idx = segment['eng_diff'].idxmax()
                            peak_time = df.iloc[peak_idx]['time_sec']
                            peak_eng_1s = df.iloc[peak_idx]['total_engag_1s_pct']
                            peak_eng_5s = df.iloc[peak_idx]['total_engag_5s_pct']
                            peak_diff = df.iloc[peak_idx]['eng_diff']
                            
                            # Calculate engagement score at peak
                            peak_engagement_score = calculate_engagement_score(df.iloc[peak_idx])
                            
                            spikes.append({
                                'start_time': start_time,
                                'end_time': end_time,
                                'duration': duration,
                                'peak_time': peak_time,
                                'peak_eng_1s': peak_eng_1s,
                                'peak_eng_5s': peak_eng_5s,
                                'peak_difference': peak_diff,
                                'integral': integral,
                                'engagement_score': peak_engagement_score,
                                'start_idx': start_idx,
                                'end_idx': end_idx,
                                'type': 'crossover_spike'
                            })
                    
                    # Move to the crossover after this 'down' point
                    i = j
                    break
            else:
                # No matching 'down' found, move to next crossover
                i += 1
        else:
            # Current is 'down', move to next
            i += 1
    
    # Sort spikes by integral value (descending)
    spikes.sort(key=lambda x: x['integral'], reverse=True)
    
    print(f"Detected {len(spikes)} crossover-based spikes")
    
    # Add padding to spike boundaries for better clipping
    for spike in spikes:
        spike['padded_start_time'] = max(0, spike['start_time'] - 2)
        spike['padded_end_time'] = spike['end_time'] + 3
    
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
        f.write("CROSSOVER-BASED ENGAGEMENT SPIKES (Ranked by Integral)\n")
        f.write("-" * 55 + "\n")
        if not spikes:
            f.write("No significant crossover spikes detected.\n\n")
        else:
            for i, spike in enumerate(spikes, 1):
                f.write(f"Spike #{i} (Rank by Integral)\n")
                f.write(f"  Time Range: {format_time(spike['start_time'])} - {format_time(spike['end_time'])}\n")
                f.write(f"  Padded Range: {format_time(spike['padded_start_time'])} - {format_time(spike['padded_end_time'])}\n")
                f.write(f"  Duration: {spike['duration']:.1f} seconds\n")
                f.write(f"  Peak at: {format_time(spike['peak_time'])}\n")
                f.write(f"  Peak 1s Engagement: {spike['peak_eng_1s']:.1f}%\n")
                f.write(f"  Peak 5s Engagement: {spike['peak_eng_5s']:.1f}%\n")
                f.write(f"  Peak Difference: {spike['peak_difference']:.1f}%\n")
                f.write(f"  Integral (Area): {spike['integral']:.2f}\n")
                f.write(f"  Engagement Score: {spike['engagement_score']:.2f}\n")
                f.write(f"  Detection Method: crossover_integral\n\n")
        
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
    for i, spike in enumerate(spikes, 1):
        all_clips.append({
            'rank': i,
            'type': 'crossover_spike',
            'start_time': spike['padded_start_time'],
            'end_time': spike['padded_end_time'],
            'duration': spike['padded_end_time'] - spike['padded_start_time'],
            'start_formatted': format_time(spike['padded_start_time']),
            'end_formatted': format_time(spike['padded_end_time']),
            'original_start': spike['start_time'],
            'original_end': spike['end_time'],
            'original_duration': spike['duration'],
            'peak_time': spike['peak_time'],
            'peak_eng_1s': spike['peak_eng_1s'],
            'peak_eng_5s': spike['peak_eng_5s'],
            'peak_difference': spike['peak_difference'],
            'integral': spike['integral'],
            'engagement_score': spike['engagement_score'],
            'avg_engagement_10s_pct': '',
            'avg_engagement_30s_pct': '',
            'sustained_score': '',
            'detection_method': 'crossover_integral'
        })
    
    # Add sustained moments to clips list
    for i, moment in enumerate(sustained_moments, 1):
        all_clips.append({
            'rank': len(spikes) + i,
            'type': 'sustained',
            'start_time': moment['start_time'],
            'end_time': moment['end_time'],
            'duration': moment['duration'],
            'start_formatted': format_time(moment['start_time']),
            'end_formatted': format_time(moment['end_time']),
            'original_start': moment['start_time'],
            'original_end': moment['end_time'],
            'original_duration': moment['duration'],
            'peak_time': '',
            'peak_eng_1s': '',
            'peak_eng_5s': '',
            'peak_difference': '',
            'integral': '',
            'engagement_score': '',
            'avg_engagement_10s_pct': moment['avg_engagement_10s'],
            'avg_engagement_30s_pct': moment['avg_engagement_30s'],
            'sustained_score': moment['avg_sustained_score'],
            'detection_method': 'sustained_period'
        })
    
    # Sort clips by rank (spikes are already ranked by integral)
    all_clips.sort(key=lambda x: x['rank'])
    
    # Save to CSV
    if all_clips:
        clips_df = pd.DataFrame(all_clips)
        clips_df.to_csv(csv_path, index=False)
        print(f"Created CSV file: {csv_path}")
    else:
        # Create empty CSV with headers
        pd.DataFrame(columns=[
            'rank', 'type', 'start_time', 'end_time', 'duration', 'start_formatted', 'end_formatted',
            'original_start', 'original_end', 'original_duration', 'peak_time', 'peak_eng_1s', 
            'peak_eng_5s', 'peak_difference', 'integral', 'engagement_score',
            'avg_engagement_10s_pct', 'avg_engagement_30s_pct', 'sustained_score', 'detection_method'
        ]).to_csv(csv_path, index=False)
        print(f"Created empty CSV file: {csv_path}")
    
    print(f"Created text file: {txt_path}")
    return txt_path, csv_path

def plot_engagement(df, spikes, sustained_moments, output_prefix):
    """Plot engagement metrics and highlight detected clips with crossover analysis."""
    plt.figure(figsize=(16, 10))
    
    # Create subplots
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(16, 10), sharex=True)
    
    # Top subplot: Main engagement metrics
    ax1.plot(df['time_sec'], df['total_engag_1s_pct'], 
             label='1s Engagement', alpha=0.8, color='orange', linewidth=2)
    ax1.plot(df['time_sec'], df['total_engag_5s_pct'], 
             label='5s Engagement', alpha=0.8, color='blue', linewidth=2)
    ax1.plot(df['time_sec'], df['total_engag_10s_pct'], 
             label='10s Engagement', alpha=0.7, color='green')
    ax1.plot(df['time_sec'], df['total_engag_30s_pct'], 
             label='30s Engagement', alpha=0.7, color='purple')

    # Define colors for 5 ranks (deep red → light red)
    rank_colors = ['darkred', 'crimson', 'red', 'indianred', 'lightcoral']
    num_ranks = len(rank_colors)

    # Determine integral bins for ranks
    integrals = [spike['integral'] for spike in spikes]
    rank_edges = np.percentile(integrals, [0, 20, 40, 60, 80, 100])  # 5 bins

    # Track which rank labels are already used for the legend
    legend_added = [False] * num_ranks

    for spike in spikes:
        # Determine spike rank (0 = lowest, 4 = highest)
        rank = np.searchsorted(rank_edges, spike['integral'], side='right') - 1
        rank = min(rank, num_ranks-1)  # safety

        color = rank_colors[rank]

        # Add label only once per rank
        label = f"Rank {rank+1}" if not legend_added[rank] else None
        legend_added[rank] = True

        # Plot spike duration on top subplot
        ax1.axvspan(spike['padded_start_time'], spike['padded_end_time'],
                    color=color, alpha=0.3, label=label)

        # Optional: vertical lines
        ax1.axvline(spike['start_time'], color=color, linestyle='--', alpha=0.7)
        ax1.axvline(spike['end_time'], color=color, linestyle='--', alpha=0.7)

        # Also mark on difference subplot
        ax2.axvspan(spike['start_time'], spike['end_time'], color=color, alpha=0.2)
        ax2.scatter(spike['peak_time'], spike['integral'], color=color, s=50, zorder=5)


    # Highlight sustained periods
    sustained_labeled = False
    for moment in sustained_moments:
        label = 'Sustained' if not sustained_labeled else ""
        ax1.axvspan(moment['start_time'], moment['end_time'], 
                    color='yellow', alpha=0.2, label=label)
        sustained_labeled = True

    ax1.set_ylabel("Engagement (%)")
    ax1.set_title("Engagement Metrics with Crossover-Based Spike Detection (Ranked by Integral)")
    ax1.legend(loc="upper right")
    ax1.grid(True, alpha=0.3)

    # Bottom subplot: Difference and integral analysis
    if 'eng_diff' in df.columns:
        ax2.plot(df['time_sec'], df['eng_diff'], 
                 label='1s - 5s Difference', color='black', linewidth=1.5)
        ax2.axhline(y=0, color='gray', linestyle='-', alpha=0.5)

        y = df['eng_diff'].fillna(0).values
        x = df['time_sec'].values
        cumulative_integral = np.zeros_like(y)
        for i in range(1, len(y)):
            cumulative_integral[i] = np.trapz(y[:i+1], x[:i+1])


        ax2.plot(df['time_sec'], cumulative_integral,
                 label='Cumulative Integral (∫ diff dt)', color='blue', linewidth=2, alpha=0.7)

        ax2.fill_between(df['time_sec'], df['eng_diff'], 0, 
                        where=(df['eng_diff'] > 0), 
                        alpha=0.3, color='green', label='1s > 5s (Positive Area)')
        ax2.fill_between(df['time_sec'], df['eng_diff'], 0, 
                        where=(df['eng_diff'] < 0), 
                        alpha=0.3, color='red', label='1s < 5s (Negative Area)')

    # Mark crossover spikes on difference plot
    for i, spike in enumerate(spikes[:5]):
        color = rank_colors[min(i, len(rank_colors)-1)]
        # highlight spike duration on diff plot
        ax2.axvspan(spike['start_time'], spike['end_time'], color=color, alpha=0.2)

        # mark the integral peak point
        ax2.scatter(spike['peak_time'], spike['integral'],
                    color=color, s=50, zorder=5,
                    label=f"Spike #{i+1} Integral={spike['integral']:.1f}" if i == 0 else "")


    ax2.set_xlabel("Time (seconds)")
    ax2.set_ylabel("Engagement Difference (%)")
    ax2.set_title("Engagement Difference (1s - 5s) with Crossover Points and Integrals")
    ax2.legend(loc="upper right")
    ax2.grid(True, alpha=0.3)

    plt.tight_layout()

    # Save to file
    plot_path = f"{output_prefix}_crossover_analysis.png"
    plt.savefig(plot_path, dpi=200, bbox_inches='tight')
    plt.close()
    print(f"Created crossover analysis visualization: {plot_path}")
    return plot_path

def main():
    parser = argparse.ArgumentParser(description='Detect clippable moments using crossover-based integral analysis')
    parser.add_argument('input_csv', help='Input CSV file path')
    parser.add_argument('-o', '--output', help='Output file prefix (default: same as input filename)')
    parser.add_argument('--min-integral', type=float, default=5.0, 
                       help='Minimum integral threshold for spike detection (default: 5.0)')
    parser.add_argument('--min-duration', type=float, default=1.0,
                       help='Minimum duration for spike in seconds (default: 1.0)')
    parser.add_argument('--sustained-threshold', type=float, default=75,
                       help='Percentile threshold for sustained engagement (default: 75)')
    parser.add_argument('--sustained-min-duration', type=float, default=15,
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
    
    print("\nAnalyzing engagement data with crossover-based integral method...")
    
    # Detect clips using crossover analysis
    spikes = detect_crossover_spikes(df, 
                                   min_integral_threshold=args.min_integral,
                                   min_duration=args.min_duration)
    
    sustained_moments = detect_sustained_moments(
        df, 
        duration_threshold=args.sustained_min_duration,
        engagement_threshold_percentile=args.sustained_threshold
    )
    
    # Create output files
    txt_file, csv_file = create_output_files(spikes, sustained_moments, output_prefix)
    
    # Create visualization
    plot_file = plot_engagement(df, spikes, sustained_moments, output_prefix)

    print(f"\nSummary:")
    print(f"  Crossover spikes detected: {len(spikes)} (ranked by integral)")
    if spikes:
        print(f"    Top spike integral: {spikes[0]['integral']:.2f}")
        print(f"    Lowest spike integral: {spikes[-1]['integral']:.2f}")
    print(f"  Sustained moments detected: {len(sustained_moments)}")
    print(f"  Total clips: {len(spikes) + len(sustained_moments)}")
    print(f"\nOutput files created:")
    print(f"  {txt_file}")
    print(f"  {csv_file}")
    print(f"  {plot_file}")

if __name__ == "__main__":
    main()