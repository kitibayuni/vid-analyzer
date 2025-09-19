#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"
SEGMENT_TIME=600
OVERLAP=2
STEMS=("vocals" "no_vocals")
APPLY_CROSSFADE=false
CROSSFADE_DURATION=1
# Optimization settings
PARALLEL_JOBS=$(nproc)  # Use all CPU cores
BATCH_SIZE=4  # Process multiple chunks at once in Demucs
USE_MP3=false  # Set to true for faster processing (lower quality)
DEVICE="cuda"  # Change to "cpu" if no GPU
# ----------------

# --- Progress bar ---
progress_bar() {
    local current=$1 total=$2 task_name="${3:-Processing}" width=40
    local percentage=$((current*100/total))
    local filled=$((current*width/total))
    local spinner="/-\\|"
    local spin_char=${spinner:$((current % ${#spinner})):1}
    local bar=""
    for ((i=0;i<filled;i++)); do bar+="█"; done
    for ((i=filled;i<width;i++)); do bar+="░"; done
    printf "\r%s [%s] %d%% (%d/%d) %s" "$spin_char" "$bar" "$percentage" "$current" "$total" "$task_name"
}

# --- Input ---
if [ $# -lt 1 ]; then
    echo "Usage: $0 input.wav"
    exit 1
fi

INPUT="$(realpath "$1")"
INPUT_DIR="$(dirname "$INPUT")"
BASENAME="$(basename "$INPUT" .wav)"
WORKDIR="${INPUT_DIR}/${BASENAME}_work"
CHUNKS_DIR="${WORKDIR}/chunks"
OUTDIR="${WORKDIR}/stitched"

# Clean up any previous work directory
[ -d "$WORKDIR" ] && rm -rf "$WORKDIR"
mkdir -p "$CHUNKS_DIR" "$OUTDIR"

# --- Get audio info once ---
AUDIO_INFO=$(ffprobe -v error -show_entries format=duration:stream=sample_rate,channels \
    -of json "$INPUT")
DURATION=$(echo "$AUDIO_INFO" | jq -r '.format.duration' | cut -d'.' -f1)
SAMPLE_RATE=$(echo "$AUDIO_INFO" | jq -r '.streams[0].sample_rate // 44100')

echo ">>> Input length: ${DURATION}s, Sample rate: ${SAMPLE_RATE}Hz"
echo ">>> Using ${PARALLEL_JOBS} parallel jobs"
echo ">>> Splitting $INPUT into ${SEGMENT_TIME}s chunks with ${OVERLAP}s overlap..."

# --- Parallel chunk splitting ---
TOTAL_CHUNKS=$(( (DURATION + SEGMENT_TIME - 1) / SEGMENT_TIME ))

split_chunk() {
    local idx=$1
    local start=$((idx * SEGMENT_TIME))
    local end=$((start + SEGMENT_TIME + OVERLAP))
    [ $end -gt $DURATION ] && end=$DURATION
    local out=$(printf "%s/chunk_%03d.wav" "$CHUNKS_DIR" "$idx")
    
    # Use faster codec and avoid re-encoding if possible
    ffmpeg -hide_banner -loglevel error -y -ss "$start" -t $((end - start)) \
        -i "$INPUT" \
        -ar "$SAMPLE_RATE" -ac 1 -c:a pcm_s16le \
        -threads 1 "$out" 2>/dev/null
    
    echo "$idx"
}

export -f split_chunk
export CHUNKS_DIR SEGMENT_TIME OVERLAP DURATION INPUT SAMPLE_RATE

# Parallel chunk creation
echo ">>> Creating chunks in parallel..."
seq 0 $((TOTAL_CHUNKS - 1)) | \
    xargs -P "$PARALLEL_JOBS" -I {} bash -c 'split_chunk "{}"' | \
    while read idx; do
        progress_bar $((idx + 1)) $TOTAL_CHUNKS "Splitting chunks"
    done
echo ""

# --- Verify all chunks exist ---
chunk_count=$(ls -1 "${CHUNKS_DIR}"/chunk_*.wav 2>/dev/null | wc -l)
if [ "$chunk_count" -lt "$TOTAL_CHUNKS" ]; then
    echo "Error: Only $chunk_count out of $TOTAL_CHUNKS chunks exist. Aborting."
    exit 1
fi

# --- Run Demucs with optimizations ---
echo ">>> Running Demucs ($MODEL) on chunks (batch size: $BATCH_SIZE)..."
cd "$WORKDIR"

# Process in batches for better GPU utilization
chunk_files=(chunks/chunk_*.wav)
total_files=${#chunk_files[@]}

if [ "$USE_MP3" = true ]; then
    MP3_FLAG="--mp3"
    MP3_QUALITY="--mp3-bitrate 320"
else
    MP3_FLAG=""
    MP3_QUALITY=""
fi

# Determine optimal segment size based on model
if [[ "$MODEL" == *"transformer"* ]] || [[ "$MODEL" == "htdemucs"* ]]; then
    SEGMENT_SIZE=7  # Max for transformer models
else
    SEGMENT_SIZE=7   # Default for other models
fi

# Process all chunks at once if GPU memory allows, otherwise batch them
if [ "$DEVICE" = "cuda" ] && [ "$total_files" -le 8 ]; then
    # Process all at once for small sets
    demucs -n "$MODEL" \
        --two-stems=vocals \
        --device "$DEVICE" \
        --jobs "$BATCH_SIZE" \
        --segment "$SEGMENT_SIZE" \
        $MP3_FLAG $MP3_QUALITY \
        chunks/chunk_*.wav
else
    # Batch processing for larger sets
    for ((i=0; i<total_files; i+=BATCH_SIZE)); do
        batch_end=$((i + BATCH_SIZE))
        [ $batch_end -gt $total_files ] && batch_end=$total_files
        
        batch_files=("${chunk_files[@]:$i:$BATCH_SIZE}")
        
        echo "Processing batch $((i/BATCH_SIZE + 1)) of $(((total_files + BATCH_SIZE - 1)/BATCH_SIZE))..."
        
        demucs -n "$MODEL" \
            --two-stems=vocals \
            --device "$DEVICE" \
            --jobs "$BATCH_SIZE" \
            --segment "$SEGMENT_SIZE" \
            $MP3_FLAG $MP3_QUALITY \
            "${batch_files[@]}"
    done
fi

cd - >/dev/null

SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
[ -z "$SEPDIR" ] && { echo "Error: Could not find Demucs output"; exit 1; }

# --- Fast stitching with concat demuxer ---
stitch_stem_fast() {
    local stem="$1"
    echo ">>> Fast stitching stem: $stem"
    
    # Create list file for concat
    local list_file="${OUTDIR}/${stem}_list.txt"
    > "$list_file"
    
    local has_files=false
    for f in "${CHUNKS_DIR}"/chunk_*.wav; do
        name=$(basename "$f" .wav)
        stem_file="${SEPDIR}/${name}/${stem}.wav"
        
        if [ -f "$stem_file" ] && [ -s "$stem_file" ]; then
            # Trim overlap from all chunks except the first
            if [ "$has_files" = true ]; then
                # Create trimmed version
                local trimmed="${OUTDIR}/${name}_${stem}_trimmed.wav"
                ffmpeg -y -hide_banner -loglevel error \
                    -i "$stem_file" \
                    -ss "$OVERLAP" \
                    -ar "$SAMPLE_RATE" -ac 1 -c:a pcm_s16le \
                    "$trimmed"
                echo "file '$(realpath "$trimmed")'" >> "$list_file"
            else
                echo "file '$(realpath "$stem_file")'" >> "$list_file"
                has_files=true
            fi
        fi
    done
    
    if [ "$has_files" = false ]; then
        echo "Error: No valid $stem files found."
        return 1
    fi
    
    # Fast concatenation
    local stitched="${OUTDIR}/${stem}.wav"
    ffmpeg -y -hide_banner -loglevel error \
        -f concat -safe 0 -i "$list_file" \
        -ar "$SAMPLE_RATE" -ac 1 -c:a pcm_s16le \
        "$stitched"
    
    # Clean up temp files
    rm -f "${OUTDIR}"/*_trimmed.wav "$list_file"
    
    [ -s "$stitched" ] || { echo "Error: stitching failed for $stem"; return 1; }
    echo "✓ Completed stitching: $stem"
}

# --- Crossfade stitching (slower but smoother) ---
stitch_stem_crossfade() {
    local stem="$1"
    echo ">>> Stitching stem with crossfade: $stem"

    local chunk_files=()
    for f in "${CHUNKS_DIR}"/chunk_*.wav; do
        name=$(basename "$f" .wav)
        stem_file="${SEPDIR}/${name}/${stem}.wav"
        if [ -f "$stem_file" ] && [ -s "$stem_file" ]; then
            chunk_files+=("$(realpath "$stem_file")")
        fi
    done

    if [ ${#chunk_files[@]} -eq 0 ]; then
        echo "Error: No valid $stem files found."
        return 1
    fi

    local stitched="${OUTDIR}/${stem}.wav"
    
    if [ ${#chunk_files[@]} -eq 1 ]; then
        # Single file, just copy
        cp "${chunk_files[0]}" "$stitched"
    else
        # Build complex filter for all crossfades at once
        local filter=""
        local last_output="0"
        
        for ((i=1;i<${#chunk_files[@]};i++)); do
            if [ $i -eq 1 ]; then
                filter="[0][1]acrossfade=d=${CROSSFADE_DURATION}:c1=tri:c2=tri[a1]"
                last_output="a1"
            else
                filter="${filter};[${last_output}][${i}]acrossfade=d=${CROSSFADE_DURATION}:c1=tri:c2=tri[a${i}]"
                last_output="a${i}"
            fi
        done
        
        # Apply all crossfades in one pass
        local input_args=()
        for f in "${chunk_files[@]}"; do
            input_args+=("-i" "$f")
        done
        
        ffmpeg -y -hide_banner -loglevel error \
            "${input_args[@]}" \
            -filter_complex "$filter" \
            -map "[${last_output}]" \
            -ar "$SAMPLE_RATE" -ac 1 -c:a pcm_s16le \
            "$stitched"
    fi

    [ -s "$stitched" ] || { echo "Error: stitching failed for $stem"; return 1; }
    echo "✓ Completed stitching: $stem"
}

# --- Process stems in parallel ---
export -f stitch_stem_fast stitch_stem_crossfade progress_bar
export CHUNKS_DIR SEPDIR OUTDIR OVERLAP CROSSFADE_DURATION SAMPLE_RATE

if [ "$APPLY_CROSSFADE" = true ]; then
    printf "%s\n" "${STEMS[@]}" | \
        xargs -P "$PARALLEL_JOBS" -I {} bash -c 'stitch_stem_crossfade "{}"'
else
    printf "%s\n" "${STEMS[@]}" | \
        xargs -P "$PARALLEL_JOBS" -I {} bash -c 'stitch_stem_fast "{}"'
fi

# --- Combine non-vocals (optimized) ---
combine_nonvocals() {
    local non_vocals=()
    for stem in "${STEMS[@]}"; do
        if [ "$stem" != "vocals" ] && [ -f "${OUTDIR}/${stem}.wav" ] && [ -s "${OUTDIR}/${stem}.wav" ]; then
            non_vocals+=("${OUTDIR}/${stem}.wav")
        fi
    done

    if [ ${#non_vocals[@]} -eq 0 ]; then
        echo "Warning: No non-vocal stems found. Skipping combination."
        return
    fi

    # For single non-vocal, just copy
    if [ ${#non_vocals[@]} -eq 1 ]; then
        cp "${non_vocals[0]}" "${INPUT_DIR}/${BASENAME}_nonvocals.wav"
    else
        # Mix multiple stems
        local input_args=()
        for f in "${non_vocals[@]}"; do
            input_args+=("-i" "$f")
        done

        ffmpeg -y -hide_banner -loglevel error "${input_args[@]}" \
            -filter_complex "amix=inputs=${#non_vocals[@]}:duration=longest:normalize=0[out]" \
            -map "[out]" -ar "$SAMPLE_RATE" -ac 1 -c:a pcm_s16le \
            "${INPUT_DIR}/${BASENAME}_nonvocals.wav"
    fi

    [ -s "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] || { echo "Error: Non-vocal combination failed"; return 1; }
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
}

combine_nonvocals

# --- Move vocals ---
if [ -f "${OUTDIR}/vocals.wav" ] && [ -s "${OUTDIR}/vocals.wav" ]; then
    cp "${OUTDIR}/vocals.wav" "${INPUT_DIR}/${BASENAME}_vocals.wav"
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_vocals.wav"
else
    echo "Warning: Vocals file missing or empty."
fi

# --- Cleanup ---
if [ "${KEEP_WORKDIR:-false}" != "true" ]; then
    rm -rf "$WORKDIR"
    echo "✓ Cleanup done"
fi

echo ">>> All done!"
echo "Final files in: $INPUT_DIR"
[ -f "${INPUT_DIR}/${BASENAME}_vocals.wav" ] && echo "  - ${BASENAME}_vocals.wav"
[ -f "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] && echo "  - ${BASENAME}_nonvocals.wav"

# Report timing
if [ -n "${SECONDS:-}" ]; then
    echo "Total time: $((SECONDS / 60))m $((SECONDS % 60))s"
fi