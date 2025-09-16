#!/usr/bin/env julia

using CSV, DataFrames, Statistics, ArgParse

# -----------------------------
# Argument parsing
# -----------------------------
function parse_arguments()
    parser = ArgParseSettings()
    @add_arg_table parser begin
        "--vocals"
            help = "Vocals CSV input file"
            arg_type = String
            required = true
        "--nonvocals"
            help = "Nonvocals CSV input file"
            arg_type = String
            required = true
        "--video"
            help = "Video CSV input file"
            arg_type = String
            required = true
    end
    return parse_args(parser)
end

# -----------------------------
# Helper functions
# -----------------------------

# Calculate percentiles from EMA values
function calculate_percentiles(ema_1s, ema_5s, ema_10s)
    # Convert to regular vectors to handle SentinelArrays
    vec_1s = Vector{Union{Missing, Float64}}(ema_1s)
    vec_5s = Vector{Union{Missing, Float64}}(ema_5s)
    vec_10s = Vector{Union{Missing, Float64}}(ema_10s)
    
    n = length(vec_1s)
    pct_1s = Vector{Union{Missing, Float64}}(undef, n)
    pct_5s = Vector{Union{Missing, Float64}}(undef, n)
    pct_10s = Vector{Union{Missing, Float64}}(undef, n)
    
    # Calculate percentiles for each timepoint
    for i in 1:n
        if !ismissing(vec_1s[i]) && !ismissing(vec_5s[i]) && !ismissing(vec_10s[i])
            # Use all available data up to current point for percentile calculation
            data_1s = filter(!ismissing, vec_1s[1:i])
            data_5s = filter(!ismissing, vec_5s[1:i])
            data_10s = filter(!ismissing, vec_10s[1:i])
            
            if !isempty(data_1s)
                pct_1s[i] = sum(data_1s .<= vec_1s[i]) / length(data_1s) * 100
            else
                pct_1s[i] = missing
            end
            
            if !isempty(data_5s)
                pct_5s[i] = sum(data_5s .<= vec_5s[i]) / length(data_5s) * 100
            else
                pct_5s[i] = missing
            end
            
            if !isempty(data_10s)
                pct_10s[i] = sum(data_10s .<= vec_10s[i]) / length(data_10s) * 100
            else
                pct_10s[i] = missing
            end
        else
            pct_1s[i] = missing
            pct_5s[i] = missing
            pct_10s[i] = missing
        end
    end
    
    return pct_1s, pct_5s, pct_10s
end

# Average multiple channels, excluding non-values and zeros
function average_valid_channels(df::DataFrame, feature_pattern::String)
    # Find all columns matching the pattern
    matching_cols = filter(name -> occursin(feature_pattern, name), names(df))
    
    if isempty(matching_cols)
        return fill(missing, nrow(df))
    end
    
    # Filter out columns that are all missing, zero, or invalid
    valid_cols = String[]
    for col in matching_cols
        col_data = Vector{Union{Missing, Float64}}(df[!, col])  # Convert to regular vector
        valid_values = filter(x -> !ismissing(x) && x != 0 && !isnan(x), col_data)
        if !isempty(valid_values)
            push!(valid_cols, col)
        end
    end
    
    if isempty(valid_cols)
        return fill(missing, nrow(df))
    end
    
    # Average across valid columns
    result = Vector{Union{Missing, Float64}}(undef, nrow(df))
    for i in 1:nrow(df)
        valid_row_values = Float64[]
        for col in valid_cols
            val = df[i, col]
            if !ismissing(val) && val != 0 && !isnan(val)
                push!(valid_row_values, Float64(val))
            end
        end
        
        if !isempty(valid_row_values)
            result[i] = mean(valid_row_values)
        else
            result[i] = missing
        end
    end
    
    return result
end

# Extract specific EMA percentile columns
function extract_ema_percentiles(df::DataFrame, prefix::String)
    result_df = DataFrame()
    
    # Look for EMA percentile columns
    pct_cols = filter(name -> startswith(name, prefix) && occursin("_pct", name), names(df))
    
    for col in pct_cols
        result_df[!, col] = df[!, col]
    end
    
    return result_df
end

# Merge dataframes by time with nearest neighbor approach
function merge_by_time(base_df::DataFrame, merge_df::DataFrame, time_col::Symbol = :time_sec)
    if !hasproperty(base_df, time_col) || !hasproperty(merge_df, time_col)
        error("Time column $time_col not found in dataframes")
    end
    
    # Sort both dataframes by time
    base_sorted = sort(base_df, time_col)
    merge_sorted = sort(merge_df, time_col)
    
    # Initialize result with base dataframe
    result = copy(base_sorted)
    
    # Add columns from merge dataframe (excluding time column)
    merge_cols = filter(x -> x != time_col, names(merge_sorted))
    
    for col in merge_cols
        result[!, col] = Vector{Union{Missing, eltype(merge_sorted[!, col])}}(undef, nrow(result))
        
        # Find nearest time match for each row in base
        for i in 1:nrow(result)
            target_time = result[i, time_col]
            
            # Find closest time in merge dataframe
            time_diffs = abs.(merge_sorted[!, time_col] .- target_time)
            min_idx = argmin(time_diffs)
            
            result[i, col] = merge_sorted[min_idx, col]
        end
    end
    
    return result
end

# -----------------------------
# Main processing function
# -----------------------------
function main()
    args = parse_arguments()
    
    println("📂 Loading CSV files...")
    
    # Load dataframes
    vocals = CSV.read(args["vocals"], DataFrame)
    nonvocals = CSV.read(args["nonvocals"], DataFrame)
    video = CSV.read(args["video"], DataFrame)
    
    println("✅ Files loaded successfully")
    println("   - Vocals: $(nrow(vocals)) rows, $(ncol(vocals)) columns")
    println("   - Nonvocals: $(nrow(nonvocals)) rows, $(ncol(nonvocals)) columns")
    println("   - Video: $(nrow(video)) rows, $(ncol(video)) columns")
    
    # Initialize result dataframe with time from vocals
    result = DataFrame(time_sec = vocals.time_sec)
    
    # -----------------------------
    # Process vocals data
    # -----------------------------
    println("🎤 Processing vocals data...")
    
    # Process cat_ engagement
    if hasproperty(vocals, :cat_engage_ema_1s) && hasproperty(vocals, :cat_engage_ema_5s) && hasproperty(vocals, :cat_engage_ema_10s)
        pct_1s, pct_5s, pct_10s = calculate_percentiles(
            vocals.cat_engage_ema_1s, 
            vocals.cat_engage_ema_5s, 
            vocals.cat_engage_ema_10s
        )
        result[!, :cat_engage_ema_1s_pct] = pct_1s
        result[!, :cat_engage_ema_5s_pct] = pct_5s
        result[!, :cat_engage_ema_10s_pct] = pct_10s
    end
    
    # Process transc_ engagement
    if hasproperty(vocals, :transc_engage_ema_1s) && hasproperty(vocals, :transc_engage_ema_5s) && hasproperty(vocals, :transc_engage_ema_10s)
        pct_1s, pct_5s, pct_10s = calculate_percentiles(
            vocals.transc_engage_ema_1s, 
            vocals.transc_engage_ema_5s, 
            vocals.transc_engage_ema_10s
        )
        result[!, :transc_engage_ema_1s_pct] = pct_1s
        result[!, :transc_engage_ema_5s_pct] = pct_5s
        result[!, :transc_engage_ema_10s_pct] = pct_10s
    end
    
    # Process vocal RMS energy (average channels)
    rms_avg_1s = average_valid_channels(vocals, "rms_energy_engage_ema_1s")
    rms_avg_5s = average_valid_channels(vocals, "rms_energy_engage_ema_5s")
    rms_avg_10s = average_valid_channels(vocals, "rms_energy_engage_ema_10s")
    
    if !all(ismissing.(rms_avg_1s))
        result[!, :vocals_rms_energy_ema_1s] = rms_avg_1s
        result[!, :vocals_rms_energy_ema_5s] = rms_avg_5s
        result[!, :vocals_rms_energy_ema_10s] = rms_avg_10s
        
        pct_1s, pct_5s, pct_10s = calculate_percentiles(rms_avg_1s, rms_avg_5s, rms_avg_10s)
        result[!, :vocals_rms_energy_ema_1s_pct] = pct_1s
        result[!, :vocals_rms_energy_ema_5s_pct] = pct_5s
        result[!, :vocals_rms_energy_ema_10s_pct] = pct_10s
    end
    
    # Process vocal spectral (average channels)
    spectral_avg_1s = average_valid_channels(vocals, "spectral_engage_ema_1s")
    spectral_avg_5s = average_valid_channels(vocals, "spectral_engage_ema_5s")
    spectral_avg_10s = average_valid_channels(vocals, "spectral_engage_ema_10s")
    
    if !all(ismissing.(spectral_avg_1s))
        result[!, :vocals_spectral_ema_1s] = spectral_avg_1s
        result[!, :vocals_spectral_ema_5s] = spectral_avg_5s
        result[!, :vocals_spectral_ema_10s] = spectral_avg_10s
        
        pct_1s, pct_5s, pct_10s = calculate_percentiles(spectral_avg_1s, spectral_avg_5s, spectral_avg_10s)
        result[!, :vocals_spectral_ema_1s_pct] = pct_1s
        result[!, :vocals_spectral_ema_5s_pct] = pct_5s
        result[!, :vocals_spectral_ema_10s_pct] = pct_10s
    end
    
    # -----------------------------
    # Process nonvocals data
    # -----------------------------
    println("🔊 Processing nonvocals data...")
    
    # Create nonvocals dataframe for merging
    nonvocals_df = DataFrame(time_sec = nonvocals.time_sec)
    
    # Process nonvocal RMS energy
    nv_rms_avg_1s = average_valid_channels(nonvocals, "rms_energy_engage_ema_1s")
    nv_rms_avg_5s = average_valid_channels(nonvocals, "rms_energy_engage_ema_5s")
    nv_rms_avg_10s = average_valid_channels(nonvocals, "rms_energy_engage_ema_10s")
    
    if !all(ismissing.(nv_rms_avg_1s))
        nonvocals_df[!, :nonvocals_rms_energy_ema_1s] = nv_rms_avg_1s
        nonvocals_df[!, :nonvocals_rms_energy_ema_5s] = nv_rms_avg_5s
        nonvocals_df[!, :nonvocals_rms_energy_ema_10s] = nv_rms_avg_10s
        
        pct_1s, pct_5s, pct_10s = calculate_percentiles(nv_rms_avg_1s, nv_rms_avg_5s, nv_rms_avg_10s)
        nonvocals_df[!, :nonvocals_rms_energy_ema_1s_pct] = pct_1s
        nonvocals_df[!, :nonvocals_rms_energy_ema_5s_pct] = pct_5s
        nonvocals_df[!, :nonvocals_rms_energy_ema_10s_pct] = pct_10s
    end
    
    # Process nonvocal spectral
    nv_spectral_avg_1s = average_valid_channels(nonvocals, "spectral_engage_ema_1s")
    nv_spectral_avg_5s = average_valid_channels(nonvocals, "spectral_engage_ema_5s")
    nv_spectral_avg_10s = average_valid_channels(nonvocals, "spectral_engage_ema_10s")
    
    if !all(ismissing.(nv_spectral_avg_1s))
        nonvocals_df[!, :nonvocals_spectral_ema_1s] = nv_spectral_avg_1s
        nonvocals_df[!, :nonvocals_spectral_ema_5s] = nv_spectral_avg_5s
        nonvocals_df[!, :nonvocals_spectral_ema_10s] = nv_spectral_avg_10s
        
        pct_1s, pct_5s, pct_10s = calculate_percentiles(nv_spectral_avg_1s, nv_spectral_avg_5s, nv_spectral_avg_10s)
        nonvocals_df[!, :nonvocals_spectral_ema_1s_pct] = pct_1s
        nonvocals_df[!, :nonvocals_spectral_ema_5s_pct] = pct_5s
        nonvocals_df[!, :nonvocals_spectral_ema_10s_pct] = pct_10s
    end
    
    # Merge nonvocals data
    result = merge_by_time(result, nonvocals_df)
    
    # -----------------------------
    # Process video data
    # -----------------------------
    println("📹 Processing video data...")
    
    # Create video dataframe with attention columns and EMA percentiles
    video_df = DataFrame(time_sec = video.time_sec)
    
    # Keep all attention columns
    attention_cols = filter(name -> startswith(name, "attention_"), names(video))
    for col in attention_cols
        video_df[!, col] = video[!, col]
    end
    
    # Keep all visual engagement EMA percentiles
    visual_ema_cols = filter(name -> startswith(name, "visual_engage_ema_") && occursin("_pct", name), names(video))
    for col in visual_ema_cols
        video_df[!, col] = video[!, col]
    end
    
    # Also keep the base EMA values for calculation
    visual_base_cols = filter(name -> startswith(name, "visual_engage_ema_") && !occursin("_pct", name), names(video))
    for col in visual_base_cols
        video_df[!, col] = video[!, col]
    end
    
    # Merge video data
    result = merge_by_time(result, video_df)
    
    # -----------------------------
    # Calculate total engagement score
    # -----------------------------
    println("⚡ Calculating total engagement score...")
    
    # Initialize total engagement
    result[!, :total_engagement] = zeros(nrow(result))
    
    # Weights: cat 22%, transc 18%, rms_energy(vocal) 13%, spectral(vocal) 10%, 
    #         spectral(nonvocals) 3%, rms_energy(nonvocals) 5%, video 25%
    
    weights = Dict(
        "cat" => 0.22,
        "transc" => 0.18,
        "vocals_rms" => 0.13,
        "vocals_spectral" => 0.10,
        "nonvocals_spectral" => 0.03,
        "nonvocals_rms" => 0.05,
        "video" => 0.25
    )
    
    # Apply weights to percentile columns
    for (component, weight) in weights
        pct_cols = String[]
        
        if component == "cat"
            pct_cols = filter(name -> startswith(name, "cat_engage_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "transc"
            pct_cols = filter(name -> startswith(name, "transc_engage_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "vocals_rms"
            pct_cols = filter(name -> startswith(name, "vocals_rms_energy_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "vocals_spectral"
            pct_cols = filter(name -> startswith(name, "vocals_spectral_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "nonvocals_spectral"
            pct_cols = filter(name -> startswith(name, "nonvocals_spectral_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "nonvocals_rms"
            pct_cols = filter(name -> startswith(name, "nonvocals_rms_energy_ema_") && endswith(name, "_pct"), names(result))
        elseif component == "video"
            pct_cols = filter(name -> startswith(name, "visual_engage_ema_") && endswith(name, "_pct"), names(result))
        end
        
        if !isempty(pct_cols)
            # Average the percentile columns for this component
            component_avg = Vector{Float64}(undef, nrow(result))
            for i in 1:nrow(result)
                valid_values = Float64[]
                for col in pct_cols
                    val = result[i, col]
                    if !ismissing(val) && !isnan(val)
                        push!(valid_values, val)
                    end
                end
                
                if !isempty(valid_values)
                    component_avg[i] = mean(valid_values)
                else
                    component_avg[i] = 0.0
                end
            end
            
            # Add weighted component to total
            result[!, :total_engagement] .+= component_avg .* weight
        end
    end
    
    # Calculate total engagement EMA percentiles
    if hasproperty(result, :total_engagement)
        # Use the total engagement as 1s, 5s, and 10s for percentile calculation
        # (This is a simplification - you might want to implement actual EMAs)
        total_values = Vector{Union{Missing, Float64}}(result.total_engagement)
        pct_values = Vector{Float64}(undef, length(total_values))
        
        for i in 1:length(total_values)
            if !ismissing(total_values[i])
                data_subset = filter(x -> !ismissing(x), total_values[1:i])
                if !isempty(data_subset)
                    pct_values[i] = sum(data_subset .<= total_values[i]) / length(data_subset) * 100
                else
                    pct_values[i] = 50.0  # Default to median
                end
            else
                pct_values[i] = 50.0
            end
        end
        
        result[!, :total_engagement_pct] = pct_values
    end
    
    # -----------------------------
    # Export results
    # -----------------------------
    output_file = "combined_engagement.csv"
    CSV.write(output_file, result)
    
    println("✅ Analysis complete!")
    println("   - Output file: $output_file")
    println("   - Total rows: $(nrow(result))")
    println("   - Total columns: $(ncol(result))")
    
    # Show summary of key columns
    println("\n📊 Key engagement components:")
    key_cols = filter(name -> occursin("_pct", name) || name == "total_engagement", names(result))
    for col in key_cols[1:min(10, length(key_cols))]
        non_missing = sum(.!ismissing.(result[!, col]))
        println("   - $col: $non_missing non-missing values")
    end
    
    if length(key_cols) > 10
        println("   ... and $(length(key_cols) - 10) more columns")
    end
    
    return result
end

# Run the main function
if abspath(PROGRAM_FILE) == @__FILE__
    main()
end