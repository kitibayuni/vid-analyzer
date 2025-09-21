#!/usr/bin/env julia
"""
Engagement Clip Detection Script
Analyzes CSV data to identify clippable moments based on engagement metrics.
Enhanced with crossover-based spike detection and integral calculation.
"""

using CSV
using DataFrames
using Statistics
using LinearAlgebra
using Plots
using ArgParse
using Printf

# Performance optimization: Use GR backend for speed
gr()

"""
Load CSV and validate required columns exist.
"""
function load_and_validate_csv(file_path::String)
    try
        df = CSV.read(file_path, DataFrame)
        println("Loaded CSV with $(nrow(df)) rows and $(ncol(df)) columns")
        
        required_cols = [
            "time_sec", "total_engag_10s_pct", "total_engag_1s_pct", 
            "total_engag_30s_pct", "total_engag_5s_pct", "total_engag_raw"
        ]
        
        missing_cols = [col for col in required_cols if !(col in names(df))]
        if !isempty(missing_cols)
            println("Error: Missing required columns: $missing_cols")
            println("Available columns: $(names(df))")
            return nothing
        end
            
        # Sort by time to ensure proper order
        sort!(df, :time_sec)
        println("Data loaded and sorted by time successfully")
        return df
        
    catch e
        println("Error loading CSV: $e")
        return nothing
    end
end

"""
Calculate a weighted engagement score combining all metrics.
"""
function calculate_engagement_score(row::DataFrameRow)
    # Weight longer timeframes more heavily for sustained engagement
    score = (
        row.total_engag_1s_pct * 0.1 +      # Immediate spikes
        row.total_engag_5s_pct * 0.2 +      # Short bursts
        row.total_engag_10s_pct * 0.3 +     # Medium sustained
        row.total_engag_30s_pct * 0.4       # Long sustained (highest weight)
    )
    return score
end

"""
Find points where 1s engagement crosses above and below 5s engagement.
"""
function find_crossover_points(df::DataFrame)
    crossovers = []
    
    # Calculate the difference between 1s and 5s engagement
    df.eng_diff = df.total_engag_1s_pct .- df.total_engag_5s_pct
    
    # Find sign changes in the difference (crossover points)
    for i in 2:nrow(df)
        prev_diff = df.eng_diff[i-1]
        curr_diff = df.eng_diff[i]
        
        # Skip if either value is NaN/missing
        if ismissing(prev_diff) || ismissing(curr_diff) || isnan(prev_diff) || isnan(curr_diff)
            continue
        end
            
        # Check for crossover (sign change)
        if prev_diff <= 0 < curr_diff
            # Crossover up (1s crosses above 5s)
            push!(crossovers, Dict(
                "time" => df.time_sec[i],
                "index" => i,
                "type" => "up",
                "eng_1s" => df.total_engag_1s_pct[i],
                "eng_5s" => df.total_engag_5s_pct[i]
            ))
        elseif prev_diff >= 0 > curr_diff
            # Crossover down (1s crosses below 5s)
            push!(crossovers, Dict(
                "time" => df.time_sec[i],
                "index" => i,
                "type" => "down",
                "eng_1s" => df.total_engag_1s_pct[i],
                "eng_5s" => df.total_engag_5s_pct[i]
            ))
        end
    end
    
    println("Found $(length(crossovers)) crossover points")
    return crossovers
end

"""
Calculate the integral (area) between 1s and 5s engagement curves using trapezoidal rule.
"""
function calculate_integral_between_curves(df::DataFrame, start_idx::Int, end_idx::Int)
    if start_idx >= end_idx || end_idx > nrow(df)
        return 0.0
    end
    
    # Extract the segment of data
    segment = df[start_idx:end_idx, :]
    
    # Remove any NaN/missing values
    segment = dropmissing(segment, [:total_engag_1s_pct, :total_engag_5s_pct, :time_sec])
    
    if nrow(segment) < 2
        return 0.0
    end
    
    # Calculate the difference at each point
    segment.diff = segment.total_engag_1s_pct .- segment.total_engag_5s_pct
    
    # Use trapezoidal integration to calculate area
    time_points = segment.time_sec
    diff_values = segment.diff
    
    # Calculate integral using trapezoidal rule
    integral = 0.0
    for i in 2:length(time_points)
        dt = time_points[i] - time_points[i-1]
        integral += 0.5 * (diff_values[i] + diff_values[i-1]) * dt
    end
    
    return integral
end

"""
Detect spikes based on crossover analysis and integral calculation.
"""
function detect_crossover_spikes(df::DataFrame; min_integral_threshold=5.0, min_duration=1.0)
    spikes = []
    
    # Find all crossover points
    crossovers = find_crossover_points(df)
    
    if length(crossovers) < 2
        println("Not enough crossover points found for spike detection")
        return spikes
    end
    
    # Pair up crossovers to find spike ranges (up -> down)
    i = 1
    while i < length(crossovers)
        current = crossovers[i]
        
        # Look for an 'up' crossover
        if current["type"] == "up"
            # Find the next 'down' crossover
            for j in (i+1):length(crossovers)
                next_crossover = crossovers[j]
                if next_crossover["type"] == "down"
                    # Found a complete spike range
                    start_idx = current["index"]
                    end_idx = next_crossover["index"]
                    start_time = current["time"]
                    end_time = next_crossover["time"]
                    duration = end_time - start_time
                    
                    # Check minimum duration
                    if duration >= min_duration
                        # Calculate integral (area between curves)
                        integral = calculate_integral_between_curves(df, start_idx, end_idx)
                        
                        # Only include spikes with significant integral
                        if integral >= min_integral_threshold
                            # Find peak point within the range
                            segment = df[start_idx:end_idx, :]
                            peak_idx_local = argmax(segment.eng_diff)
                            peak_idx = start_idx + peak_idx_local - 1
                            peak_time = df.time_sec[peak_idx]
                            peak_eng_1s = df.total_engag_1s_pct[peak_idx]
                            peak_eng_5s = df.total_engag_5s_pct[peak_idx]
                            peak_diff = df.eng_diff[peak_idx]
                            
                            # Calculate engagement score at peak
                            peak_engagement_score = calculate_engagement_score(df[peak_idx, :])
                            
                            push!(spikes, Dict(
                                "start_time" => start_time,
                                "end_time" => end_time,
                                "duration" => duration,
                                "peak_time" => peak_time,
                                "peak_eng_1s" => peak_eng_1s,
                                "peak_eng_5s" => peak_eng_5s,
                                "peak_difference" => peak_diff,
                                "integral" => integral,
                                "engagement_score" => peak_engagement_score,
                                "start_idx" => start_idx,
                                "end_idx" => end_idx,
                                "type" => "crossover_spike"
                            ))
                        end
                    end
                    
                    # Move to the crossover after this 'down' point
                    i = j
                    break
                end
            end
        else
            # Current is 'down', move to next
            i += 1
        end
    end
    
    # Sort spikes by integral value (descending)
    sort!(spikes, by=x -> x["integral"], rev=true)
    
    println("Detected $(length(spikes)) crossover-based spikes")
    
    # Add padding to spike boundaries for better clipping
    for spike in spikes
        spike["padded_start_time"] = max(0, spike["start_time"] - 2)
        spike["padded_end_time"] = spike["end_time"] + 3
    end
    
    return spikes
end

"""
Detect longer periods of sustained high engagement.
"""
function detect_sustained_moments(df::DataFrame; duration_threshold=15, engagement_threshold_percentile=75)
    sustained_moments = []
    
    # Calculate rolling engagement for sustained periods (focus on 10s and 30s metrics)
    df.sustained_score = df.total_engag_10s_pct .* 0.4 .+ df.total_engag_30s_pct .* 0.6
    
    # Smooth the sustained score to avoid small fluctuations (5-point moving average)
    window_size = 5
    df.sustained_smooth = similar(df.sustained_score, Float64)
    for i in 1:nrow(df)
        start_idx = max(1, i - div(window_size, 2))
        end_idx = min(nrow(df), i + div(window_size, 2))
        df.sustained_smooth[i] = mean(df.sustained_score[start_idx:end_idx])
    end
    
    # Calculate threshold
    valid_scores = filter(!ismissing, df.sustained_smooth)
    threshold = quantile(valid_scores, engagement_threshold_percentile/100)
    
    # Find continuous periods above threshold
    above_threshold = df.sustained_smooth .> threshold
    
    # Group continuous periods
    groups = []
    current_group = Int[]
    
    for (idx, is_above) in enumerate(above_threshold)
        if !ismissing(is_above) && is_above
            push!(current_group, idx)
        else
            if !isempty(current_group)
                push!(groups, copy(current_group))
                empty!(current_group)
            end
        end
    end
    
    # Don't forget the last group
    if !isempty(current_group)
        push!(groups, current_group)
    end
    
    # Filter groups by duration and create clips
    for group in groups
        start_idx, end_idx = group[1], group[end]
        start_time = df.time_sec[start_idx]
        end_time = df.time_sec[end_idx]
        duration = end_time - start_time
        
        if duration >= duration_threshold
            # Extend clip slightly for context
            extended_start = max(0, start_time - 5)
            extended_end = end_time + 5
            
            # Calculate average engagement metrics for this period
            period_data = df[start_idx:end_idx, :]
            avg_10s = mean(period_data.total_engag_10s_pct)
            avg_30s = mean(period_data.total_engag_30s_pct)
            avg_sustained = mean(period_data.sustained_score)
            
            push!(sustained_moments, Dict(
                "start_time" => extended_start,
                "end_time" => extended_end,
                "duration" => duration,
                "avg_engagement_10s" => avg_10s,
                "avg_engagement_30s" => avg_30s,
                "avg_sustained_score" => avg_sustained,
                "type" => "sustained"
            ))
        end
    end
    
    println("Detected $(length(sustained_moments)) sustained engagement periods")
    return sustained_moments
end

"""
Convert seconds to MM:SS format.
"""
function format_time(seconds::Real)
    minutes = floor(Int, seconds / 60)
    secs = floor(Int, seconds % 60)
    return @sprintf("%02d:%02d", minutes, secs)
end

"""
Create text and CSV output files.
"""
function create_output_files(spikes::Vector, sustained_moments::Vector, output_prefix::String)
    
    # Create text file with readable format
    txt_path = "$(output_prefix)_clips.txt"
    open(txt_path, "w") do f
        println(f, "ENGAGEMENT-BASED CLIPPABLE MOMENTS")
        println(f, "=" ^ 50)
        println(f)
        
        # Spikes section
        println(f, "CROSSOVER-BASED ENGAGEMENT SPIKES (Ranked by Integral)")
        println(f, "-" ^ 55)
        if isempty(spikes)
            println(f, "No significant crossover spikes detected.")
            println(f)
        else
            for (i, spike) in enumerate(spikes)
                println(f, "Spike #$i (Rank by Integral)")
                println(f, "  Time Range: $(format_time(spike["start_time"])) - $(format_time(spike["end_time"]))")
                println(f, "  Padded Range: $(format_time(spike["padded_start_time"])) - $(format_time(spike["padded_end_time"]))")
                println(f, "  Duration: $(round(spike["duration"], digits=1)) seconds")
                println(f, "  Peak at: $(format_time(spike["peak_time"]))")
                println(f, "  Peak 1s Engagement: $(round(spike["peak_eng_1s"], digits=1))%")
                println(f, "  Peak 5s Engagement: $(round(spike["peak_eng_5s"], digits=1))%")
                println(f, "  Peak Difference: $(round(spike["peak_difference"], digits=1))%")
                println(f, "  Integral (Area): $(round(spike["integral"], digits=2))")
                println(f, "  Engagement Score: $(round(spike["engagement_score"], digits=2))")
                println(f, "  Detection Method: crossover_integral")
                println(f)
            end
        end
        
        # Sustained moments section
        println(f, "SUSTAINED HIGH ENGAGEMENT PERIODS")
        println(f, "-" ^ 35)
        if isempty(sustained_moments)
            println(f, "No sustained high engagement periods detected.")
        else
            for (i, moment) in enumerate(sustained_moments)
                println(f, "Period #$i")
                println(f, "  Time Range: $(format_time(moment["start_time"])) - $(format_time(moment["end_time"]))")
                println(f, "  Duration: $(round(moment["duration"], digits=1)) seconds")
                println(f, "  Avg 10s Engagement: $(round(moment["avg_engagement_10s"], digits=1))%")
                println(f, "  Avg 30s Engagement: $(round(moment["avg_engagement_30s"], digits=1))%")
                println(f, "  Sustained Score: $(round(moment["avg_sustained_score"], digits=2))")
                println(f)
            end
        end
    end
    
    # Create CSV file with all clips
    csv_path = "$(output_prefix)_clips.csv"
    all_clips = []
    
    # Add spikes to clips list
    for (i, spike) in enumerate(spikes)
        push!(all_clips, Dict(
            "rank" => i,
            "type" => "crossover_spike",
            "start_time" => spike["padded_start_time"],
            "end_time" => spike["padded_end_time"],
            "duration" => spike["padded_end_time"] - spike["padded_start_time"],
            "start_formatted" => format_time(spike["padded_start_time"]),
            "end_formatted" => format_time(spike["padded_end_time"]),
            "original_start" => spike["start_time"],
            "original_end" => spike["end_time"],
            "original_duration" => spike["duration"],
            "peak_time" => spike["peak_time"],
            "peak_eng_1s" => spike["peak_eng_1s"],
            "peak_eng_5s" => spike["peak_eng_5s"],
            "peak_difference" => spike["peak_difference"],
            "integral" => spike["integral"],
            "engagement_score" => spike["engagement_score"],
            "avg_engagement_10s_pct" => missing,
            "avg_engagement_30s_pct" => missing,
            "sustained_score" => missing,
            "detection_method" => "crossover_integral"
        ))
    end
    
    # Add sustained moments to clips list
    for (i, moment) in enumerate(sustained_moments)
        push!(all_clips, Dict(
            "rank" => length(spikes) + i,
            "type" => "sustained",
            "start_time" => moment["start_time"],
            "end_time" => moment["end_time"],
            "duration" => moment["duration"],
            "start_formatted" => format_time(moment["start_time"]),
            "end_formatted" => format_time(moment["end_time"]),
            "original_start" => moment["start_time"],
            "original_end" => moment["end_time"],
            "original_duration" => moment["duration"],
            "peak_time" => missing,
            "peak_eng_1s" => missing,
            "peak_eng_5s" => missing,
            "peak_difference" => missing,
            "integral" => missing,
            "engagement_score" => missing,
            "avg_engagement_10s_pct" => moment["avg_engagement_10s"],
            "avg_engagement_30s_pct" => moment["avg_engagement_30s"],
            "sustained_score" => moment["avg_sustained_score"],
            "detection_method" => "sustained_period"
        ))
    end
    
    # Sort clips by rank (spikes are already ranked by integral)
    sort!(all_clips, by=x -> x["rank"])
    
    # Save to CSV
    if !isempty(all_clips)
        clips_df = DataFrame(all_clips)
        CSV.write(csv_path, clips_df)
        println("Created CSV file: $csv_path")
    else
        # Create empty CSV with headers
        empty_df = DataFrame(
            rank=Int[], type=String[], start_time=Float64[], end_time=Float64[], 
            duration=Float64[], start_formatted=String[], end_formatted=String[],
            original_start=Float64[], original_end=Float64[], original_duration=Float64[], 
            peak_time=Union{Float64,Missing}[], peak_eng_1s=Union{Float64,Missing}[], 
            peak_eng_5s=Union{Float64,Missing}[], peak_difference=Union{Float64,Missing}[], 
            integral=Union{Float64,Missing}[], engagement_score=Union{Float64,Missing}[],
            avg_engagement_10s_pct=Union{Float64,Missing}[], avg_engagement_30s_pct=Union{Float64,Missing}[], 
            sustained_score=Union{Float64,Missing}[], detection_method=String[]
        )
        CSV.write(csv_path, empty_df)
        println("Created empty CSV file: $csv_path")
    end
    
    println("Created text file: $txt_path")
    return txt_path, csv_path
end

"""
Fast plotting function for very large datasets - sacrifices some visual fidelity for speed.
"""
function plot_engagement_fast(df::DataFrame, spikes::Vector, sustained_moments::Vector, output_prefix::String)
    
    # Aggressive downsampling for speed
    max_points = 1000
    downsample_factor = max(1, div(nrow(df), max_points))
    
    println("Using fast plotting mode with $downsample_factor downsampling factor")
    plot_indices = 1:downsample_factor:nrow(df)
    df_plot = df[plot_indices, :]
    
    # Simplified color scheme
    spike_color = :red
    sustained_color = :yellow
    
    # Basic engagement plot - only show 1s and 5s for clarity
    p1 = plot(df_plot.time_sec, df_plot.total_engag_1s_pct, 
              label="1s", color=:orange, linewidth=1,
              title="Engagement Overview (Fast Mode)",
              size=(800, 250), dpi=72, legend=:topright)
    
    plot!(p1, df_plot.time_sec, df_plot.total_engag_5s_pct, 
          label="5s", color=:blue, linewidth=1)

    # Only highlight top 3 spikes
    top_spikes = spikes[1:min(3, length(spikes))]
    for (i, spike) in enumerate(top_spikes)
        vspan!(p1, [spike["start_time"], spike["end_time"]], 
               color=spike_color, alpha=0.2, 
               label=i==1 ? "Top Spikes" : "")
    end

    # Simple difference plot
    p2 = plot(df_plot.time_sec, df_plot.eng_diff, 
              label="1s-5s", color=:black, linewidth=1,
              title="Engagement Difference",
              size=(800, 250), dpi=72, legend=:topright)
    
    hline!(p2, [0], color=:gray, alpha=0.5, linewidth=1, label="")
    
    # Mark only top spike peaks
    for (i, spike) in enumerate(top_spikes)
        scatter!(p2, [spike["peak_time"]], [spike["peak_difference"]], 
                color=spike_color, markersize=3, markerstrokewidth=0,
                label=i==1 ? "Peaks" : "")
    end

    # Combine and save
    plot_combined = plot(p1, p2, layout=(2,1), size=(800, 500))
    
    plot_path = "$(output_prefix)_fast_analysis.png"
    savefig(plot_combined, plot_path)
    println("Created fast visualization: $plot_path")
    return plot_path
end

"""
Plot engagement metrics and highlight detected clips with crossover analysis.
Optimized for performance while maintaining good fidelity.
"""
function plot_engagement(df::DataFrame, spikes::Vector, sustained_moments::Vector, output_prefix::String)
    
    # Performance optimization: Downsample data if too large
    max_points = 5000  # Adjust based on needs
    downsample_factor = max(1, div(nrow(df), max_points))
    
    if downsample_factor > 1
        println("Downsampling data by factor of $downsample_factor for plotting performance")
        plot_indices = 1:downsample_factor:nrow(df)
        df_plot = df[plot_indices, :]
    else
        df_plot = df
    end
    
    # Pre-calculate colors and ranks to avoid repeated computation
    rank_colors = [:darkred, :crimson, :red, :indianred, :lightcoral]
    num_ranks = length(rank_colors)
    
    # Determine integral bins for ranks
    if !isempty(spikes)
        integrals = [spike["integral"] for spike in spikes]
        rank_edges = quantile(integrals, [0.0, 0.2, 0.4, 0.6, 0.8, 1.0])  # 5 bins
        spike_ranks = [min(searchsortedfirst(rank_edges, spike["integral"]), num_ranks) for spike in spikes]
    else
        rank_edges = [0.0]
        spike_ranks = Int[]
    end
    
    # Create main engagement plot with optimized settings
    p1 = plot(df_plot.time_sec, df_plot.total_engag_1s_pct, 
              label="1s", color=:orange, linewidth=1.5,
              xlabel="", ylabel="Engagement (%)", 
              title="Engagement Metrics with Crossover Spikes",
              legend=:topright, grid=true, gridwidth=1, gridcolor=:gray, gridalpha=0.3,
              size=(1000, 300), dpi=100)  # Reduced DPI for speed
    
    plot!(p1, df_plot.time_sec, df_plot.total_engag_5s_pct, 
          label="5s", color=:blue, linewidth=1.5)
    
    plot!(p1, df_plot.time_sec, df_plot.total_engag_10s_pct, 
          label="10s", color=:green, linewidth=1)
    
    plot!(p1, df_plot.time_sec, df_plot.total_engag_30s_pct, 
          label="30s", color=:purple, linewidth=1)

    # Batch process spikes for better performance
    legend_added = falses(num_ranks)
    
    # Group spikes by rank for batch processing
    for rank in 1:num_ranks
        rank_spikes = [spikes[i] for (i, r) in enumerate(spike_ranks) if r == rank]
        if !isempty(rank_spikes)
            color = rank_colors[rank]
            
            # Batch add vertical spans (more efficient than individual calls)
            for spike in rank_spikes
                vspan!(p1, [spike["padded_start_time"], spike["padded_end_time"]], 
                       color=color, alpha=0.25, 
                       label=legend_added[rank] ? "" : "Rank $rank")
                legend_added[rank] = true
            end
            
            # Add start/end lines only for top 3 spikes to reduce clutter
            for spike in rank_spikes[1:min(3, length(rank_spikes))]
                vline!(p1, [spike["start_time"]], color=color, linestyle=:dash, 
                       alpha=0.6, linewidth=1, label="")
                vline!(p1, [spike["end_time"]], color=color, linestyle=:dash, 
                       alpha=0.6, linewidth=1, label="")
            end
        end
    end

    # Batch process sustained periods
    if !isempty(sustained_moments)
        for moment in sustained_moments
            vspan!(p1, [moment["start_time"], moment["end_time"]], 
                   color=:yellow, alpha=0.15, label="Sustained")
            break  # Only add label once
        end
        # Add remaining without labels
        for moment in sustained_moments[2:end]
            vspan!(p1, [moment["start_time"], moment["end_time"]], 
                   color=:yellow, alpha=0.15, label="")
        end
    end

    # Create difference plot with performance optimizations
    p2 = plot(df_plot.time_sec, df_plot.eng_diff, 
              label="1s - 5s Diff", color=:black, linewidth=1.5,
              xlabel="Time (seconds)", ylabel="Difference (%)",
              title="Engagement Difference Analysis",
              legend=:topright, grid=true, gridwidth=1, gridcolor=:gray, gridalpha=0.3,
              size=(1000, 300), dpi=100)
    
    hline!(p2, [0], color=:gray, linestyle=:solid, alpha=0.5, linewidth=1, label="")
    
    # Simplified cumulative integral calculation (skip if data is too large)
    if nrow(df_plot) < 2000  # Only calculate for smaller datasets
        y = coalesce.(df_plot.eng_diff, 0.0)
        x = df_plot.time_sec
        cumulative_integral = zeros(length(y))
        for i in 2:length(y)
            dt = x[i] - x[i-1]
            cumulative_integral[i] = cumulative_integral[i-1] + 0.5 * (y[i] + y[i-1]) * dt
        end
        
        plot!(p2, df_plot.time_sec, cumulative_integral,
              label="Cumulative ∫", color=:blue, linewidth=1.5, alpha=0.7)
    end
    
    # Simplified fill areas - use fewer segments for performance
    positive_indices = findall(x -> !ismissing(x) && x > 0, df_plot.eng_diff)
    negative_indices = findall(x -> !ismissing(x) && x < 0, df_plot.eng_diff)
    
    if !isempty(positive_indices)
        plot!(p2, df_plot.time_sec[positive_indices], df_plot.eng_diff[positive_indices], 
              fillrange=0, alpha=0.2, color=:green, label="Positive", seriestype=:line)
    end
    
    if !isempty(negative_indices)
        plot!(p2, df_plot.time_sec[negative_indices], df_plot.eng_diff[negative_indices], 
              fillrange=0, alpha=0.2, color=:red, label="Negative", seriestype=:line)
    end

    # Mark only top 3 spikes to reduce visual clutter and improve performance
    top_spikes = spikes[1:min(3, length(spikes))]
    for (i, spike) in enumerate(top_spikes)
        color = rank_colors[min(spike_ranks[i], length(rank_colors))]
        
        # Simplified spike highlighting
        vspan!(p2, [spike["start_time"], spike["end_time"]], 
               color=color, alpha=0.15, label="")
        
        # Mark peak point with larger marker for visibility
        scatter!(p2, [spike["peak_time"]], [spike["integral"]], 
                color=color, markersize=4, markerstrokewidth=0,
                label=i==1 ? "Top Spikes" : "")
    end

    # Combine plots with optimized layout
    plot_combined = plot(p1, p2, layout=(2,1), size=(1000, 600), 
                        margin=5Plots.mm, dpi=150)  # Balanced DPI
    
    # Save with optimized format
    plot_path = "$(output_prefix)_crossover_analysis.png"
    savefig(plot_combined, plot_path)
    println("Created crossover analysis visualization: $plot_path")
    return plot_path
end

"""
Parse command line arguments.
"""
function parse_commandline()
    s = ArgParseSettings(description="Detect clippable moments using crossover-based integral analysis")
    
    @add_arg_table! s begin
        "input_csv"
            help = "Input CSV file path"
            required = true
        "--output", "-o"
            help = "Output file prefix (default: same as input filename)"
            default = nothing
        "--min-integral"
            help = "Minimum integral threshold for spike detection (default: 5.0)"
            arg_type = Float64
            default = 5.0
        "--min-duration"
            help = "Minimum duration for spike in seconds (default: 1.0)"
            arg_type = Float64
            default = 1.0
        "--sustained-threshold"
            help = "Percentile threshold for sustained engagement (default: 75)"
            arg_type = Float64
            default = 75.0
        "--sustained-min-duration"
            help = "Minimum duration for sustained moments in seconds (default: 15)"
            arg_type = Float64
            default = 15.0
        "--fast-plot"
            help = "Force fast plotting mode for large datasets"
            action = :store_true
    end
    
    return parse_args(s)
end

"""
Main function.
"""
function main()
    args = parse_commandline()
    
    # Validate input file
    input_path = args["input_csv"]
    if !isfile(input_path)
        println("Error: Input file '$input_path' not found")
        exit(1)
    end
    
    # Set output prefix
    output_prefix = if args["output"] !== nothing
        args["output"]
    else
        splitext(basename(input_path))[1]
    end
    
    # Load and process data
    df = load_and_validate_csv(input_path)
    if df === nothing
        exit(1)
    end
    
    println("\nAnalyzing engagement data with crossover-based integral method...")
    
    # Detect clips using crossover analysis
    spikes = detect_crossover_spikes(df, 
                                   min_integral_threshold=args["min-integral"],
                                   min_duration=args["min-duration"])
    
    sustained_moments = detect_sustained_moments(
        df, 
        duration_threshold=args["sustained-min-duration"],
        engagement_threshold_percentile=args["sustained-threshold"]
    )
    
    # Create output files
    txt_file, csv_file = create_output_files(spikes, sustained_moments, output_prefix)
    
    # Create visualization (choose based on data size or user preference)
    if args["fast-plot"] || nrow(df) > 10000
        println("Using fast plotting mode...")
        plot_file = plot_engagement_fast(df, spikes, sustained_moments, output_prefix)
    else
        plot_file = plot_engagement(df, spikes, sustained_moments, output_prefix)
    end

    println("\nSummary:")
    println("  Crossover spikes detected: $(length(spikes)) (ranked by integral)")
    if !isempty(spikes)
        println("    Top spike integral: $(round(spikes[1]["integral"], digits=2))")
        println("    Lowest spike integral: $(round(spikes[end]["integral"], digits=2))")
    end
    println("  Sustained moments detected: $(length(sustained_moments))")
    println("  Total clips: $(length(spikes) + length(sustained_moments))")
    println("\nOutput files created:")
    println("  $txt_file")
    println("  $csv_file")
    println("  $plot_file")
end

# Run main function if script is executed directly
if abspath(PROGRAM_FILE) == @__FILE__
    main()
end