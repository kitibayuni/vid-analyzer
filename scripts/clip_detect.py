#!/usr/bin/env python3
"""
Engagement Clip Detection Script - Streamlined & Optimized
Analyzes CSV data to identify clippable moments based on engagement metrics.
Optimized with automatic parallelization, chunking, and caching.
"""

import pandas as pd
import numpy as np
import argparse
import sys
from pathlib import Path
import matplotlib.pyplot as plt
import warnings
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
import pickle
import hashlib
import time
import gc

warnings.filterwarnings('ignore', category=UserWarning)
plt.switch_backend('Agg')

# Configuration
CACHE_DIR = Path('.engagement_cache')
CACHE_DIR.mkdir(exist_ok=True)
DEFAULT_CHUNK_SIZE = 10000
DEFAULT_N_JOBS = mp.cpu_count()
LARGE_DATASET_THRESHOLD = 500000

class EngagementCache:
    """Simple caching system for expensive computations."""
    
    def __init__(self):
        self.cache_dir = CACHE_DIR
    
    def _get_key(self, df, method, params):
        """Generate cache key."""
        sample_data = df.head(100).to_string() + str(df.shape) + str(params)
        return hashlib.md5(sample_data.encode()).hexdigest()
    
    def get(self, df, method, params):
        """Get cached result."""
        try:
            key = self._get_key(df, method, params)
            cache_file = self.cache_dir / f"{key}.pkl"
            if cache_file.exists():
                with open(cache_file, 'rb') as f:
                    return pickle.load(f)
        except:
            pass
        return None
    
    def set(self, df, method, params, result):
        """Cache result."""
        try:
            key = self._get_key(df, method, params)
            cache_file = self.cache_dir / f"{key}.pkl"
            with open(cache_file, 'wb') as f:
                pickle.dump(result, f)
        except:
            pass

cache = EngagementCache()

def load_csv_efficiently(file_path, chunk_size=None):
    """Load CSV with automatic chunking for large files."""
    required_cols = ['time_sec', 'total_engag_10s_pct', 'total_engag_1s_pct', 
                     'total_engag_30s_pct', 'total_engag_5s_pct', 'total_engag_raw']
    
    # Validate columns
    header = pd.read_csv(file_path, nrows=0)
    missing = [col for col in required_cols if col not in header.columns]
    if missing:
        print(f"Error: Missing columns: {missing}")
        return None
    
    file_size = Path(file_path).stat().st_size
    
    # Auto-chunk large files
    if file_size > 50 * 1024 * 1024:  # >50MB
        print(f"Large file ({file_size/(1024**2):.1f}MB), using chunked loading")
        chunk_size = chunk_size or DEFAULT_CHUNK_SIZE
        
        chunks = []
        for chunk in pd.read_csv(file_path, usecols=required_cols, chunksize=chunk_size):
            chunks.append(chunk)
            if len(chunks) % 10 == 0:
                print(f"  Loaded {len(chunks) * chunk_size:,} rows...")
        
        df = pd.concat(chunks, ignore_index=True)
        del chunks
        gc.collect()
    else:
        df = pd.read_csv(file_path, usecols=required_cols)
    
    # Sort by time
    df = df.sort_values('time_sec').reset_index(drop=True)
    print(f"Loaded {len(df):,} rows")
    return df

def compute_engagement_metrics(df):
    """Compute all engagement-related metrics efficiently."""
    print("Computing engagement metrics...")
    
    # Engagement score (vectorized)
    engagement_scores = (
        df['total_engag_1s_pct'] * 0.1 +
        df['total_engag_5s_pct'] * 0.2 + 
        df['total_engag_10s_pct'] * 0.3 +
        df['total_engag_30s_pct'] * 0.4
    )
    
    # Engagement difference for crossover detection
    eng_diff = df['total_engag_1s_pct'] - df['total_engag_5s_pct']
    
    # Sustained engagement score
    sustained_scores = df['total_engag_10s_pct'] * 0.4 + df['total_engag_30s_pct'] * 0.6
    sustained_smooth = sustained_scores.rolling(window=5, center=True, min_periods=1).mean()
    
    return engagement_scores, eng_diff, sustained_smooth

def find_crossovers_vectorized(eng_diff):
    """Find crossover points using vectorized operations."""
    print("Finding crossover points...")
    
    # Convert to numpy array to avoid pandas broadcasting issues
    eng_diff_values = eng_diff.values
    
    # Find sign changes (this creates array of length n-1)
    sign_changes = np.diff(np.sign(eng_diff_values))
    valid_mask = ~(np.isnan(eng_diff_values[:-1]) | np.isnan(eng_diff_values[1:]))
    
    # Find indices where sign changes occur
    indices = np.where((sign_changes != 0) & valid_mask)[0] + 1
    
    crossovers = []
    for idx in indices:
        if idx >= len(eng_diff_values):
            continue
            
        prev_diff = eng_diff_values[idx-1]
        curr_diff = eng_diff_values[idx]
        
        if prev_diff <= 0 < curr_diff:
            crossover_type = 'up'
        elif prev_diff >= 0 > curr_diff:
            crossover_type = 'down'
        else:
            continue
            
        crossovers.append({'index': idx, 'type': crossover_type})
    
    print(f"Found {len(crossovers)} crossover points")
    return crossovers

def detect_spikes_optimized(df, eng_diff, engagement_scores, crossovers, 
                           min_integral=5.0, min_duration=1.0):
    """Detect engagement spikes using crossover analysis."""
    if len(crossovers) < 2:
        return []
    
    print("Detecting engagement spikes...")
    
    # Find spike regions
    up_indices = [i for i, c in enumerate(crossovers) if c['type'] == 'up']
    spike_candidates = []
    
    for up_idx in up_indices:
        # Find next down crossover
        down_candidates = [i for i in range(up_idx + 1, len(crossovers)) 
                          if crossovers[i]['type'] == 'down']
        if not down_candidates:
            continue
            
        down_idx = down_candidates[0]
        start_idx = crossovers[up_idx]['index']
        end_idx = crossovers[down_idx]['index']
        
        start_time = df.iloc[start_idx]['time_sec']
        end_time = df.iloc[end_idx]['time_sec']
        duration = end_time - start_time
        
        if duration >= min_duration:
            spike_candidates.append((start_idx, end_idx, start_time, end_time, duration))
    
    # Calculate integrals and filter
    spikes = []
    eng_diff_values = eng_diff.values  # Convert to numpy for consistency
    
    for start_idx, end_idx, start_time, end_time, duration in spike_candidates:
        # Calculate integral using numpy arrays
        time_slice = df['time_sec'].iloc[start_idx:end_idx+1].values
        diff_slice = eng_diff_values[start_idx:end_idx+1]
        
        valid_mask = ~np.isnan(diff_slice)
        if valid_mask.sum() >= 2:
            integral = np.trapz(diff_slice[valid_mask], time_slice[valid_mask])
            
            if integral >= min_integral:
                # Find peak
                peak_local_idx = np.argmax(diff_slice)
                peak_idx = start_idx + peak_local_idx
                
                spikes.append({
                    'start_time': start_time,
                    'end_time': end_time,
                    'duration': duration,
                    'peak_time': df.iloc[peak_idx]['time_sec'],
                    'peak_eng_1s': df.iloc[peak_idx]['total_engag_1s_pct'],
                    'peak_eng_5s': df.iloc[peak_idx]['total_engag_5s_pct'],
                    'peak_difference': diff_slice[peak_local_idx],
                    'integral': integral,
                    'engagement_score': engagement_scores.iloc[peak_idx],
                    'padded_start_time': max(0, start_time - 2),
                    'padded_end_time': end_time + 3,
                    'type': 'crossover_spike'
                })
    
    # Sort by integral
    spikes.sort(key=lambda x: x['integral'], reverse=True)
    print(f"Detected {len(spikes)} spikes")
    return spikes

def detect_sustained_moments(df, sustained_smooth, duration_threshold=15, percentile=75):
    """Detect sustained high engagement periods."""
    print("Detecting sustained moments...")
    
    threshold = np.percentile(sustained_smooth.dropna(), percentile)
    above_threshold = sustained_smooth > threshold
    
    # Find continuous periods
    diff = np.diff(np.concatenate(([False], above_threshold, [False])).astype(int))
    starts = np.where(diff == 1)[0]
    ends = np.where(diff == -1)[0] - 1
    
    moments = []
    for start_idx, end_idx in zip(starts, ends):
        start_time = df.iloc[start_idx]['time_sec']
        end_time = df.iloc[end_idx]['time_sec'] 
        duration = end_time - start_time
        
        if duration >= duration_threshold:
            moments.append({
                'start_time': max(0, start_time - 5),
                'end_time': end_time + 5,
                'duration': duration,
                'avg_engagement_10s': df['total_engag_10s_pct'].iloc[start_idx:end_idx+1].mean(),
                'avg_engagement_30s': df['total_engag_30s_pct'].iloc[start_idx:end_idx+1].mean(),
                'avg_sustained_score': sustained_smooth.iloc[start_idx:end_idx+1].mean(),
                'type': 'sustained'
            })
    
    print(f"Detected {len(moments)} sustained periods")
    return moments

def create_outputs(spikes, sustained_moments, output_prefix):
    """Create text and CSV output files."""
    # Text file
    txt_path = f"{output_prefix}_clips.txt"
    with open(txt_path, 'w') as f:
        f.write("ENGAGEMENT-BASED CLIPPABLE MOMENTS\n" + "="*50 + "\n\n")
        
        f.write("CROSSOVER-BASED SPIKES (Ranked by Integral)\n" + "-"*45 + "\n")
        for i, spike in enumerate(spikes, 1):
            f.write(f"Spike #{i}\n")
            f.write(f"  Range: {spike['start_time']//60:02.0f}:{spike['start_time']%60:02.0f} - {spike['end_time']//60:02.0f}:{spike['end_time']%60:02.0f}\n")
            f.write(f"  Padded: {spike['padded_start_time']//60:02.0f}:{spike['padded_start_time']%60:02.0f} - {spike['padded_end_time']//60:02.0f}:{spike['padded_end_time']%60:02.0f}\n")
            f.write(f"  Duration: {spike['duration']:.1f}s, Integral: {spike['integral']:.2f}\n")
            f.write(f"  Peak: 1s={spike['peak_eng_1s']:.1f}%, 5s={spike['peak_eng_5s']:.1f}%\n\n")
        
        f.write("\nSUSTAINED HIGH ENGAGEMENT\n" + "-"*25 + "\n")
        for i, moment in enumerate(sustained_moments, 1):
            f.write(f"Period #{i}\n")
            f.write(f"  Range: {moment['start_time']//60:02.0f}:{moment['start_time']%60:02.0f} - {moment['end_time']//60:02.0f}:{moment['end_time']%60:02.0f}\n")
            f.write(f"  Duration: {moment['duration']:.1f}s\n")
            f.write(f"  Avg: 10s={moment['avg_engagement_10s']:.1f}%, 30s={moment['avg_engagement_30s']:.1f}%\n\n")
    
    # CSV file
    csv_path = f"{output_prefix}_clips.csv"
    all_clips = []
    
    for i, spike in enumerate(spikes, 1):
        all_clips.append({
            'rank': i, 'type': 'spike', 'start_time': spike['padded_start_time'],
            'end_time': spike['padded_end_time'], 'duration': spike['padded_end_time'] - spike['padded_start_time'],
            'integral': spike['integral'], 'peak_eng_1s': spike['peak_eng_1s'],
            'peak_eng_5s': spike['peak_eng_5s'], 'engagement_score': spike['engagement_score']
        })
    
    for i, moment in enumerate(sustained_moments, 1):
        all_clips.append({
            'rank': len(spikes) + i, 'type': 'sustained', 'start_time': moment['start_time'],
            'end_time': moment['end_time'], 'duration': moment['duration'],
            'avg_engagement_10s': moment['avg_engagement_10s'], 'avg_engagement_30s': moment['avg_engagement_30s'],
            'sustained_score': moment['avg_sustained_score']
        })
    
    pd.DataFrame(all_clips).to_csv(csv_path, index=False)
    return txt_path, csv_path

def create_visualization(df, eng_diff, spikes, sustained_moments, output_prefix):
    """Create adaptive visualization based on dataset size."""
    data_size = len(df)
    
    # Adaptive downsampling
    if data_size > 50000:
        step = data_size // 3000
        df_plot = df.iloc[::step].copy()
        eng_diff_plot = eng_diff.iloc[::step]
        print(f"Downsampling by {step}x for visualization")
    else:
        df_plot = df
        eng_diff_plot = eng_diff
    
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12, 8), sharex=True)
    
    time_vals = df_plot['time_sec']
    
    # Main engagement plot
    ax1.plot(time_vals, df_plot['total_engag_1s_pct'], label='1s', color='orange', alpha=0.8)
    ax1.plot(time_vals, df_plot['total_engag_5s_pct'], label='5s', color='blue', alpha=0.8)
    ax1.plot(time_vals, df_plot['total_engag_10s_pct'], label='10s', color='green', alpha=0.7)
    ax1.plot(time_vals, df_plot['total_engag_30s_pct'], label='30s', color='purple', alpha=0.7)
    
    # Highlight spikes
    colors = ['darkred', 'red', 'indianred', 'lightcoral', 'mistyrose']
    for i, spike in enumerate(spikes[:5]):
        color = colors[min(i, len(colors)-1)]
        ax1.axvspan(spike['start_time'], spike['end_time'], alpha=0.25, color=color,
                   label=f'Spike {i+1}' if i < 3 else None)
    
    # Highlight sustained periods
    for i, moment in enumerate(sustained_moments):
        ax1.axvspan(moment['start_time'], moment['end_time'], alpha=0.15, color='yellow',
                   label='Sustained' if i == 0 else None)
    
    ax1.set_ylabel("Engagement (%)")
    ax1.set_title("Engagement Analysis with Detected Clips")
    ax1.legend()
    ax1.grid(True, alpha=0.3)
    
    # Difference plot
    ax2.plot(time_vals, eng_diff_plot, color='black', linewidth=1.5, label='1s - 5s Diff')
    ax2.axhline(0, color='gray', alpha=0.5)
    ax2.fill_between(time_vals, eng_diff_plot, 0, where=(eng_diff_plot > 0), 
                    alpha=0.2, color='green', label='Positive')
    ax2.fill_between(time_vals, eng_diff_plot, 0, where=(eng_diff_plot < 0), 
                    alpha=0.2, color='red', label='Negative')
    
    # Mark spike peaks
    for i, spike in enumerate(spikes[:3]):
        ax2.scatter(spike['peak_time'], spike['peak_difference'], 
                   color=colors[i], s=50, zorder=5)
    
    ax2.set_xlabel("Time (seconds)")
    ax2.set_ylabel("Difference (%)")
    ax2.set_title("Engagement Difference Analysis")
    ax2.legend()
    ax2.grid(True, alpha=0.3)
    
    plt.tight_layout()
    
    plot_path = f"{output_prefix}_analysis.png"
    plt.savefig(plot_path, dpi=150, bbox_inches='tight')
    plt.close()
    
    return plot_path

def analyze_engagement(df, min_integral=5.0, min_duration=1.0, sustained_duration=15, sustained_percentile=75):
    """Main analysis pipeline with caching."""
    
    # Check cache
    cache_params = {
        'min_integral': min_integral, 'min_duration': min_duration,
        'sustained_duration': sustained_duration, 'sustained_percentile': sustained_percentile
    }
    
    if cache:
        cached = cache.get(df, 'full_analysis', cache_params)
        if cached:
            print("Using cached analysis results")
            return cached
    
    # Compute metrics
    engagement_scores, eng_diff, sustained_smooth = compute_engagement_metrics(df)
    df['eng_diff'] = eng_diff
    
    # Find crossovers and detect spikes
    crossovers = find_crossovers_vectorized(eng_diff)
    spikes = detect_spikes_optimized(df, eng_diff, engagement_scores, crossovers, 
                                   min_integral, min_duration)
    
    # Detect sustained moments
    sustained_moments = detect_sustained_moments(df, sustained_smooth, 
                                                sustained_duration, sustained_percentile)
    
    result = {'spikes': spikes, 'sustained_moments': sustained_moments, 'eng_diff': eng_diff}
    
    # Cache result
    if cache:
        cache.set(df, 'full_analysis', cache_params, result)
    
    return result

def main():
    parser = argparse.ArgumentParser(description='Detect clippable engagement moments')
    parser.add_argument('input_csv', help='Input CSV file')
    parser.add_argument('-o', '--output', help='Output prefix')
    parser.add_argument('--min-integral', type=float, default=5.0, help='Min integral threshold')
    parser.add_argument('--min-duration', type=float, default=1.0, help='Min spike duration')
    parser.add_argument('--sustained-threshold', type=float, default=75, help='Sustained percentile')
    parser.add_argument('--sustained-min-duration', type=float, default=15, help='Min sustained duration')
    parser.add_argument('--n-jobs', type=int, default=DEFAULT_N_JOBS, help='Parallel jobs')
    parser.add_argument('--chunk-size', type=int, default=DEFAULT_CHUNK_SIZE, help='Chunk size')
    parser.add_argument('--disable-cache', action='store_true', help='Disable cache')
    parser.add_argument('--clear-cache', action='store_true', help='Clear cache')
    
    args = parser.parse_args()
    
    # Handle cache
    global cache
    if args.clear_cache and CACHE_DIR.exists():
        import shutil
        shutil.rmtree(CACHE_DIR)
        CACHE_DIR.mkdir()
        print("Cache cleared")
    if args.disable_cache:
        cache = None
    
    # Validate input
    if not Path(args.input_csv).exists():
        print(f"Error: File '{args.input_csv}' not found")
        sys.exit(1)
    
    output_prefix = args.output or Path(args.input_csv).stem
    start_time = time.time()
    
    print(f"Starting analysis with {args.n_jobs} cores...")
    
    # Load data
    df = load_csv_efficiently(args.input_csv, args.chunk_size)
    if df is None:
        sys.exit(1)
    
    load_time = time.time()
    print(f"Loading: {load_time - start_time:.2f}s")
    
    # Analyze
    result = analyze_engagement(df, args.min_integral, args.min_duration, 
                              args.sustained_min_duration, args.sustained_threshold)
    
    analysis_time = time.time()
    print(f"Analysis: {analysis_time - load_time:.2f}s")
    
    # Create outputs
    txt_file, csv_file = create_outputs(result['spikes'], result['sustained_moments'], output_prefix)
    plot_file = create_visualization(df, result['eng_diff'], result['spikes'], 
                                   result['sustained_moments'], output_prefix)
    
    total_time = time.time() - start_time
    
    # Summary
    print(f"\n{'='*50}")
    print(f"Dataset: {len(df):,} rows processed in {total_time:.2f}s")
    print(f"Rate: {len(df)/total_time:,.0f} rows/second")
    print(f"Spikes: {len(result['spikes'])}")
    print(f"Sustained: {len(result['sustained_moments'])}")
    print(f"Files: {txt_file}, {csv_file}, {plot_file}")
    
    del df
    gc.collect()

if __name__ == "__main__":
    main()
