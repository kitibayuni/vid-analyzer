#!/usr/bin/env python3
"""
Engagement Clip Detection Script - Performance Optimized
Analyzes CSV data to identify clippable moments based on engagement metrics.
Enhanced with crossover-based spike detection and integral calculation.
Optimized for speed while maintaining analysis quality.
"""

import pandas as pd
import numpy as np
import argparse
import sys
from pathlib import Path
import matplotlib.pyplot as plt
from matplotlib.backends.backend_agg import FigureCanvasAgg  # Faster backend
from scipy import integrate
import warnings
warnings.filterwarnings('ignore', category=UserWarning)  # Suppress plotting warnings

# Set matplotlib to use fast backend
plt.switch_backend('Agg')

def load_and_validate_csv(file_path):
    """Load CSV and validate required columns exist."""
    try:
        # Read only required columns for faster loading
        required_cols = [
            'time_sec', 'total_engag_10s_pct', 'total_engag_1s_pct', 
            'total_engag_30s_pct', 'total_engag_5s_pct', 'total_engag_raw'
        ]
        
        # First, check if columns exist by reading just the header
        header_df = pd.read_csv(file_path, nrows=0)
        missing_cols = [col for col in required_cols if col not in header_df.columns]
        if missing_cols:
            print(f"Error: Missing required columns: {missing_cols}")
            print(f"Available columns: {list(header_df.columns)}")
            return None
        
        # Load only required columns
        df = pd.read_csv(file_path, usecols=required_cols)
        print(f"Loaded CSV with {len(df)} rows and {len(df.columns)} columns")
            
        # Sort by time to ensure proper order (use numpy for speed)
        sort_idx = np.argsort(df['time_sec'].values)
        df = df.iloc[sort_idx].reset_index(drop=True)
        print("Data loaded and sorted by time successfully")
        return df
        
    except Exception as e:
        print(f"Error loading CSV: {e}")
        return None

def calculate_engagement_score_vectorized(df):
    """Calculate engagement scores for all rows at once (vectorized)."""
    scores = (
        df['total_engag_1s_pct'].values * 0.1 +      # Immediate spikes
        df['total_engag_5s_pct'].values * 0.2 +      # Short bursts
        df['total_engag_10s_pct'].values * 0.3 +     # Medium sustained
        df['total_engag_30s_pct'].values * 0.4       # Long sustained
    )
    return scores

def find_crossover_points_vectorized(df):
    """Find crossover points using vectorized operations."""
    # Calculate difference between 1s and 5s engagement
    eng_diff = df['total_engag_1s_pct'].values - df['total_engag_5s_pct'].values
    df['eng_diff'] = eng_diff
    
    # Find sign changes using numpy operations
    valid_mask = ~(np.isnan(eng_diff[:-1]) | np.isnan(eng_diff[1:]))
    
    # Detect crossovers where sign changes occur
    sign_changes = np.diff(np.sign(eng_diff))
    crossover_indices = np.where((sign_changes != 0) & valid_mask)[0] + 1
    
    crossovers = []
    for idx in crossover_indices:
        prev_diff = eng_diff[idx-1]
        curr_diff = eng_diff[idx]
        
        if prev_diff <= 0 < curr_diff:
            crossover_type = 'up'
        elif prev_diff >= 0 > curr_diff:
            crossover_type = 'down'
        else:
            continue
            
        crossovers.append({
            'time': df.iloc[idx]['time_sec'],
            'index': idx,
            'type': crossover_type,
            'eng_1s': df.iloc[idx]['total_engag_1s_pct'],
            'eng_5s': df.iloc[idx]['total_engag_5s_pct']
        })
    
    print(f"Found {len(crossovers)} crossover points")
    return crossovers

def calculate_integral_between_curves_fast(df, start_idx, end_idx):
    """Fast integral calculation using numpy."""
    if start_idx >= end_idx or end_idx >= len(df):
        return 0.0
    
    # Extract arrays directly for speed
    time_slice = df['time_sec'].iloc[start_idx:end_idx+1].values
    diff_slice = df['eng_diff'].iloc[start_idx:end_idx+1].values
    
    # Remove NaN values
    valid_mask = ~np.isnan(diff_slice)
    if np.sum(valid_mask) < 2:
        return 0.0
    
    time_clean = time_slice[valid_mask]
    diff_clean = diff_slice[valid_mask]
    
    # Fast trapezoidal integration
    return np.trapz(diff_clean, time_clean)

def detect_crossover_spikes_optimized(df, min_integral_threshold=5.0, min_duration=1.0):
    """Optimized spike detection."""
    spikes = []
    
    # Use vectorized crossover detection
    crossovers = find_crossover_points_vectorized(df)
    
    if len(crossovers) < 2:
        print("Not enough crossover points found for spike detection")
        return spikes
    
    # Pre-calculate engagement scores for all rows
    engagement_scores = calculate_engagement_score_vectorized(df)
    
    # Convert crossovers to arrays for faster processing
    crossover_times = np.array([c['time'] for c in crossovers])
    crossover_types = np.array([c['type'] for c in crossovers])
    crossover_indices = np.array([c['index'] for c in crossovers])
    
    # Find up crossovers
    up_indices = np.where(crossover_types == 'up')[0]
    
    for up_idx in up_indices:
        # Find next down crossover
        down_indices = np.where((crossover_types == 'down') & 
                               (np.arange(len(crossovers)) > up_idx))[0]
        
        if len(down_indices) == 0:
            continue
            
        down_idx = down_indices[0]
        
        start_idx = crossover_indices[up_idx]
        end_idx = crossover_indices[down_idx]
        start_time = crossover_times[up_idx]
        end_time = crossover_times[down_idx]
        duration = end_time - start_time
        
        if duration >= min_duration:
            # Fast integral calculation
            integral = calculate_integral_between_curves_fast(df, start_idx, end_idx)
            
            if integral >= min_integral_threshold:
                # Find peak efficiently
                segment_diff = df['eng_diff'].iloc[start_idx:end_idx+1].values
                peak_local_idx = np.argmax(segment_diff)
                peak_idx = start_idx + peak_local_idx
                
                peak_time = df.iloc[peak_idx]['time_sec']
                peak_eng_1s = df.iloc[peak_idx]['total_engag_1s_pct']
                peak_eng_5s = df.iloc[peak_idx]['total_engag_5s_pct']
                peak_diff = df.iloc[peak_idx]['eng_diff']
                peak_engagement_score = engagement_scores[peak_idx]
                
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
    
    # Sort by integral (descending)
    spikes.sort(key=lambda x: x['integral'], reverse=True)
    
    print(f"Detected {len(spikes)} crossover-based spikes")
    
    # Add padding
    for spike in spikes:
        spike['padded_start_time'] = max(0, spike['start_time'] - 2)
        spike['padded_end_time'] = spike['end_time'] + 3
    
    return spikes

def detect_sustained_moments_fast(df, duration_threshold=15, engagement_threshold_percentile=75):
    """Optimized sustained moment detection."""
    sustained_moments = []
    
    # Vectorized sustained score calculation
    sustained_score = (
        df['total_engag_10s_pct'].values * 0.4 + 
        df['total_engag_30s_pct'].values * 0.6
    )
    df['sustained_score'] = sustained_score
    
    # Fast rolling mean using pandas (optimized C implementation)
    df['sustained_smooth'] = df['sustained_score'].rolling(window=5, center=True, min_periods=1).mean()
    
    # Fast percentile calculation
    valid_scores = df['sustained_smooth'].dropna().values
    threshold = np.percentile(valid_scores, engagement_threshold_percentile)
    
    # Vectorized threshold detection
    above_threshold = df['sustained_smooth'].values > threshold
    
    # Find continuous periods using numpy
    diff_above = np.diff(np.concatenate(([False], above_threshold, [False])).astype(int))
    starts = np.where(diff_above == 1)[0]
    ends = np.where(diff_above == -1)[0] - 1
    
    # Process groups
    for start_idx, end_idx in zip(starts, ends):
        if start_idx >= len(df) or end_idx >= len(df):
            continue
            
        start_time = df.iloc[start_idx]['time_sec']
        end_time = df.iloc[end_idx]['time_sec']
        duration = end_time - start_time
        
        if duration >= duration_threshold:
            extended_start = max(0, start_time - 5)
            extended_end = end_time + 5
            
            # Fast mean calculation
            period_slice = slice(start_idx, end_idx + 1)
            avg_10s = df['total_engag_10s_pct'].iloc[period_slice].mean()
            avg_30s = df['total_engag_30s_pct'].iloc[period_slice].mean()
            avg_sustained = df['sustained_score'].iloc[period_slice].mean()
            
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

def create_output_files_fast(spikes, sustained_moments, output_prefix):
    """Optimized output file creation."""
    
    # Create text file
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
    
    # Create CSV using efficient DataFrame construction
    csv_path = f"{output_prefix}_clips.csv"
    
    # Pre-allocate data structure
    all_clips_data = {
        'rank': [], 'type': [], 'start_time': [], 'end_time': [], 'duration': [],
        'start_formatted': [], 'end_formatted': [], 'original_start': [], 'original_end': [],
        'original_duration': [], 'peak_time': [], 'peak_eng_1s': [], 'peak_eng_5s': [],
        'peak_difference': [], 'integral': [], 'engagement_score': [],
        'avg_engagement_10s_pct': [], 'avg_engagement_30s_pct': [], 'sustained_score': [],
        'detection_method': []
    }
    
    # Add spikes
    for i, spike in enumerate(spikes, 1):
        all_clips_data['rank'].append(i)
        all_clips_data['type'].append('crossover_spike')
        all_clips_data['start_time'].append(spike['padded_start_time'])
        all_clips_data['end_time'].append(spike['padded_end_time'])
        all_clips_data['duration'].append(spike['padded_end_time'] - spike['padded_start_time'])
        all_clips_data['start_formatted'].append(format_time(spike['padded_start_time']))
        all_clips_data['end_formatted'].append(format_time(spike['padded_end_time']))
        all_clips_data['original_start'].append(spike['start_time'])
        all_clips_data['original_end'].append(spike['end_time'])
        all_clips_data['original_duration'].append(spike['duration'])
        all_clips_data['peak_time'].append(spike['peak_time'])
        all_clips_data['peak_eng_1s'].append(spike['peak_eng_1s'])
        all_clips_data['peak_eng_5s'].append(spike['peak_eng_5s'])
        all_clips_data['peak_difference'].append(spike['peak_difference'])
        all_clips_data['integral'].append(spike['integral'])
        all_clips_data['engagement_score'].append(spike['engagement_score'])
        all_clips_data['avg_engagement_10s_pct'].append('')
        all_clips_data['avg_engagement_30s_pct'].append('')
        all_clips_data['sustained_score'].append('')
        all_clips_data['detection_method'].append('crossover_integral')
    
    # Add sustained moments
    for i, moment in enumerate(sustained_moments, 1):
        rank = len(spikes) + i
        all_clips_data['rank'].append(rank)
        all_clips_data['type'].append('sustained')
        all_clips_data['start_time'].append(moment['start_time'])
        all_clips_data['end_time'].append(moment['end_time'])
        all_clips_data['duration'].append(moment['duration'])
        all_clips_data['start_formatted'].append(format_time(moment['start_time']))
        all_clips_data['end_formatted'].append(format_time(moment['end_time']))
        all_clips_data['original_start'].append(moment['start_time'])
        all_clips_data['original_end'].append(moment['end_time'])
        all_clips_data['original_duration'].append(moment['duration'])
        all_clips_data['peak_time'].append('')
        all_clips_data['peak_eng_1s'].append('')
        all_clips_data['peak_eng_5s'].append('')
        all_clips_data['peak_difference'].append('')
        all_clips_data['integral'].append('')
        all_clips_data['engagement_score'].append('')
        all_clips_data['avg_engagement_10s_pct'].append(moment['avg_engagement_10s'])
        all_clips_data['avg_engagement_30s_pct'].append(moment['avg_engagement_30s'])
        all_clips_data['sustained_score'].append(moment['avg_sustained_score'])
        all_clips_data['detection_method'].append('sustained_period')
    
    # Create DataFrame efficiently
    if all_clips_data['rank']:
        clips_df = pd.DataFrame(all_clips_data)
        clips_df.to_csv(csv_path, index=False)
        print(f"Created CSV file: {csv_path}")
    else:
        # Create empty CSV
        pd.DataFrame(columns=list(all_clips_data.keys())).to_csv(csv_path, index=False)
        print(f"Created empty CSV file: {csv_path}")
    
    print(f"Created text file: {txt_path}")
    return txt_path, csv_path

def plot_engagement_fast(df, spikes, sustained_moments, output_prefix):
    """Fast plotting optimized for large datasets."""
    
    # Downsample data if too large
    max_points = 5000
    if len(df) > max_points:
        step = len(df) // max_points
        df_plot = df.iloc[::step].copy()
        print(f"Downsampling data by factor of {step} for plotting performance")
    else:
        df_plot = df
    
    # Use fast figure creation
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12, 8), sharex=True, dpi=100)
    
    # Top subplot: Main engagement metrics with reduced styling
    time_vals = df_plot['time_sec'].values
    ax1.plot(time_vals, df_plot['total_engag_1s_pct'].values, 
             label='1s', color='orange', linewidth=1.5, alpha=0.9)
    ax1.plot(time_vals, df_plot['total_engag_5s_pct'].values, 
             label='5s', color='blue', linewidth=1.5, alpha=0.9)
    ax1.plot(time_vals, df_plot['total_engag_10s_pct'].values, 
             label='10s', color='green', linewidth=1, alpha=0.8)
    ax1.plot(time_vals, df_plot['total_engag_30s_pct'].values, 
             label='30s', color='purple', linewidth=1, alpha=0.8)

    # Simplified spike visualization - only show top 5 spikes
    rank_colors = ['darkred', 'red', 'indianred', 'lightcoral', 'mistyrose']
    
    # Only process top spikes for speed
    top_spikes = spikes[:min(5, len(spikes))]
    
    for i, spike in enumerate(top_spikes):
        color = rank_colors[min(i, len(rank_colors)-1)]
        
        # Simplified spike highlighting
        ax1.axvspan(spike['padded_start_time'], spike['padded_end_time'],
                    color=color, alpha=0.25, 
                    label=f'Rank {i+1}' if i < 3 else None)  # Only label top 3
        
        # Mark spike boundaries
        if i < 3:  # Only for top 3 spikes to reduce clutter
            ax1.axvline(spike['start_time'], color=color, linestyle='--', alpha=0.6, linewidth=1)
            ax1.axvline(spike['end_time'], color=color, linestyle='--', alpha=0.6, linewidth=1)

    # Simplified sustained periods
    for i, moment in enumerate(sustained_moments):
        ax1.axvspan(moment['start_time'], moment['end_time'], 
                    color='yellow', alpha=0.15, 
                    label='Sustained' if i == 0 else None)

    ax1.set_ylabel("Engagement (%)")
    ax1.set_title("Engagement Metrics with Top Crossover Spikes")
    ax1.legend(loc="upper right")
    ax1.grid(True, alpha=0.3)

    # Bottom subplot: Simplified difference plot
    if 'eng_diff' in df_plot.columns:
        ax2.plot(time_vals, df_plot['eng_diff'].values, 
                 label='1s - 5s Diff', color='black', linewidth=1.5)
        ax2.axhline(y=0, color='gray', linestyle='-', alpha=0.5, linewidth=1)

        # Simplified cumulative integral (only if data is reasonable size)
        if len(df_plot) < 2000:
            y = np.nan_to_num(df_plot['eng_diff'].values)
            x = time_vals
            cumulative_integral = np.zeros_like(y)
            for i in range(1, len(y)):
                cumulative_integral[i] = np.trapz(y[:i+1], x[:i+1])

            ax2.plot(time_vals, cumulative_integral,
                     label='Cumulative ∫', color='blue', linewidth=1.5, alpha=0.7)

        # Simplified fill areas
        positive_mask = df_plot['eng_diff'] > 0
        negative_mask = df_plot['eng_diff'] < 0
        
        ax2.fill_between(time_vals, df_plot['eng_diff'], 0, 
                        where=positive_mask, 
                        alpha=0.2, color='green', label='Positive')
        ax2.fill_between(time_vals, df_plot['eng_diff'], 0, 
                        where=negative_mask, 
                        alpha=0.2, color='red', label='Negative')

    # Mark only top 3 spike peaks
    for i, spike in enumerate(top_spikes[:3]):
        color = rank_colors[i]
        ax2.scatter(spike['peak_time'], spike['integral'],
                    color=color, s=40, zorder=5,
                    label=f"Top Integrals" if i == 0 else None)

    ax2.set_xlabel("Time (seconds)")
    ax2.set_ylabel("Difference (%)")
    ax2.set_title("Engagement Difference Analysis")
    ax2.legend(loc="upper right")
    ax2.grid(True, alpha=0.3)

    plt.tight_layout()

    # Save with moderate quality for speed
    plot_path = f"{output_prefix}_crossover_analysis.png"
    plt.savefig(plot_path, dpi=150, bbox_inches='tight', facecolor='white')
    plt.close(fig)  # Explicitly close to free memory
    print(f"Created fast visualization: {plot_path}")
    return plot_path

def plot_engagement_ultra_fast(df, spikes, sustained_moments, output_prefix):
    """Ultra-fast plotting for very large datasets."""
    
    # Aggressive downsampling
    max_points = 1000
    step = max(1, len(df) // max_points)
    df_plot = df.iloc[::step].copy()
    print(f"Using ultra-fast mode with {step}x downsampling")
    
    # Minimal plot
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 6), dpi=80)
    
    time_vals = df_plot['time_sec'].values
    
    # Only plot 1s and 5s engagement
    ax1.plot(time_vals, df_plot['total_engag_1s_pct'].values, 
             label='1s', color='orange', linewidth=1)
    ax1.plot(time_vals, df_plot['total_engag_5s_pct'].values, 
             label='5s', color='blue', linewidth=1)
    
    # Only highlight top 3 spikes
    for i, spike in enumerate(spikes[:3]):
        ax1.axvspan(spike['start_time'], spike['end_time'], 
                    color='red', alpha=0.2, 
                    label='Top Spikes' if i == 0 else None)
    
    ax1.set_title("Engagement Overview (Ultra-Fast)")
    ax1.legend()
    
    # Simple difference plot
    ax2.plot(time_vals, df_plot['eng_diff'].values, 'k-', linewidth=1)
    ax2.axhline(0, color='gray', alpha=0.5)
    ax2.set_title("Difference Analysis")
    ax2.set_xlabel("Time (seconds)")
    
    plt.tight_layout()
    
    plot_path = f"{output_prefix}_ultra_fast_analysis.png"
    plt.savefig(plot_path, dpi=100, facecolor='white')
    plt.close(fig)
    print(f"Created ultra-fast visualization: {plot_path}")
    return plot_path

def main():
    parser = argparse.ArgumentParser(description='Detect clippable moments using optimized crossover-based integral analysis')
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
    parser.add_argument('--fast-plot', action='store_true',
                       help='Use fast plotting mode')
    parser.add_argument('--ultra-fast', action='store_true',
                       help='Use ultra-fast mode for very large datasets')
    
    args = parser.parse_args()
    
    # Validate input file
    input_path = Path(args.input_csv)
    if not input_path.exists():
        print(f"Error: Input file '{args.input_csv}' not found")
        sys.exit(1)
    
    # Set output prefix
    output_prefix = args.output or input_path.stem
    
    # Load and process data
    print("Loading data...")
    df = load_and_validate_csv(args.input_csv)
    if df is None:
        sys.exit(1)
    
    print("\nAnalyzing engagement data with optimized crossover-based integral method...")
    
    # Detect clips using optimized analysis
    spikes = detect_crossover_spikes_optimized(df, 
                                               min_integral_threshold=args.min_integral,
                                               min_duration=args.min_duration)
    
    sustained_moments = detect_sustained_moments_fast(
        df, 
        duration_threshold=args.sustained_min_duration,
        engagement_threshold_percentile=args.sustained_threshold
    )
    
    # Create output files
    print("Creating output files...")
    txt_file, csv_file = create_output_files_fast(spikes, sustained_moments, output_prefix)
    
    # Create visualization based on mode
    print("Creating visualization...")
    if args.ultra_fast or len(df) > 50000:
        plot_file = plot_engagement_ultra_fast(df, spikes, sustained_moments, output_prefix)
    elif args.fast_plot or len(df) > 10000:
        plot_file = plot_engagement_fast(df, spikes, sustained_moments, output_prefix)
    else:
        # Use original plotting function for smaller datasets
        from scipy import integrate
        plot_file = plot_engagement_fast(df, spikes, sustained_moments, output_prefix)

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
