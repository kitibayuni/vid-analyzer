#!/bin/bash
# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe, ./perform_demucs.sh, ./formants_process, ./rms_energy_process, ./pitch_process, ./process_transcript.py
# requires: ./pre-process_emotions, ./process_emotion.py, ./spectrals_process, ./salience_process, ./transcript_process

# --- Dependency validation ---
validate_dependencies() {
    local missing=""
    for cmd in ffmpeg ffprobe bc python; do
        command -v "$cmd" >/dev/null || missing="$missing $cmd"
    done
    if [ -n "$missing" ]; then
        echo "ERROR: Missing required dependencies:$missing"
        echo "Please install the missing tools and try again."
        exit 1
    fi
}

# --- Cleanup function for error handling ---
cleanup_temp_files() {
    local pattern="$1"
    if [ -n "$pattern" ] && [ -d "$OUTDIR" ]; then
        find "$OUTDIR" -name "$pattern" -type f -delete 2>/dev/null || true
    fi
}

# --- Trap for cleanup on exit ---
trap 'cleanup_temp_files "${BASENAME}_*_16k_16bit.wav"; cleanup_temp_files "${BASENAME}_*_44k_32bit.wav"; cleanup_temp_files "${BASENAME}_*_22k_16bit.wav"' EXIT

# --- Source virtual environment ---
source venv/bin/activate

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"

echo "=== Media Track Extractor - Enhanced with Differentiated Processing + Merging ==="
echo "Script started at: $(date)"
echo ""

# --- Validate dependencies before proceeding ---
validate_dependencies

if [ -z "$INPUT" ]; then
    echo "ERROR: No input file provided!"
    echo "Usage: $0 <input_video>"
    exit 1
fi

if [ ! -f "$INPUT" ]; then
    echo "ERROR: Input file not found: $INPUT"
    exit 1
fi

if [ ! -d "$OUTDIR" ]; then
    echo "ERROR: Output directory not found: $OUTDIR"
    echo "Please create the directory first."
    exit 1
fi

echo "✓ Input file exists"
echo "✓ Output directory exists"
echo ""

# --- Helper function to run process_feature ---
run_process() {
    local infile="$1"
    local outbase="$2"
    local feature="$3"

    case "$feature" in
        rms)       ./process_feature --rms-in "$infile" --rms-out "${outbase}_rms.csv" ;;
        pitch)     ./process_feature --pitch-in "$infile" --pitch-out "${outbase}_pitch.csv" ;;
        spectral)  ./process_feature --spectral-in "$infile" --spectral-out "${outbase}_spectral.csv" ;;
        zcr)       ./process_feature --zcr-in "$infile" --zcr-out "${outbase}_zcr.csv" ;;
        formant)   ./process_feature --formant-in "$infile" --formant-out "${outbase}_formant.csv" ;;
        jitter)    ./process_feature --jitter-in "$infile" --jitter-out "${outbase}_jitter.csv" ;;
        *) echo "Unknown feature: $feature" ;;
    esac
}

# --- Audio track extraction ---
echo "=== AUDIO TRACK PROCESSING ==="
FFPROBE_OUTPUT=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT")

# --- Fix: Handle empty audio stream output correctly ---
if [ -z "$FFPROBE_OUTPUT" ] || [ "$FFPROBE_OUTPUT" = "" ]; then
    NUM_AUDIO_STREAMS=0
else
    NUM_AUDIO_STREAMS=$(echo "$FFPROBE_OUTPUT" | wc -l)
fi

echo "DEBUG: ffprobe output: '$FFPROBE_OUTPUT'"
echo "DEBUG: wc -l result: '$NUM_AUDIO_STREAMS'"
echo "Found $NUM_AUDIO_STREAMS audio stream(s)"
echo ""

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    echo "No audio streams found - skipping audio processing"
else
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        echo "--- Processing Audio Stream $STREAM_INDEX ---"
        OUTFILE="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}.wav"

        # --- Silence detection ---
        echo "Detecting silence..."
        SILENCE_OUTPUT=$(ffmpeg -i "$INPUT" -map 0:a:$STREAM_INDEX \
            -af silencedetect=noise=-50dB:d=1 -f null - 2>&1)

        TOTAL_SILENCE=0
        START=""
        SILENCE_COUNT=0
        while read -r LINE; do
            if [[ $LINE =~ silence_start ]]; then
                START=$(echo "$LINE" | awk '{print $5}')
            elif [[ $LINE =~ silence_end ]] && [[ -n "$START" ]]; then
                END=$(echo "$LINE" | awk '{print $5}')
                # --- Fix: Better error handling for mathematical operations ---
                if [[ -n "$END" ]] && [[ -n "$START" ]]; then
                    SILENCE_DURATION=$(echo "$END - $START" | bc 2>/dev/null)
                    if [[ -n "$SILENCE_DURATION" ]] && [[ "$SILENCE_DURATION" != "" ]]; then
                        TOTAL_SILENCE=$(echo "$TOTAL_SILENCE + $SILENCE_DURATION" | bc 2>/dev/null || echo "$TOTAL_SILENCE")
                        SILENCE_COUNT=$((SILENCE_COUNT + 1))
                    fi
                fi
                START=""
            fi
        done <<< "$SILENCE_OUTPUT"

        DURATION=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=duration -of csv=p=0 "$INPUT")
        DURATION=${DURATION%.*}
        DURATION=${DURATION:-1}
        SILENCE_FRAC=$(echo "$TOTAL_SILENCE / $DURATION" | bc -l)
        SILENCE_FRAC=${SILENCE_FRAC:-0}

        echo "Silence: ${SILENCE_COUNT} segments, $(echo "$SILENCE_FRAC * 100" | bc -l | cut -d. -f1)%"

        if (( $(echo "$SILENCE_FRAC < 0.9" | bc -l) )); then
            echo "Exporting non-silent audio stream to $OUTFILE"
            ffmpeg -y -i "$INPUT" -map 0:a:$STREAM_INDEX -ar 48000 -ac 1 -c:a pcm_f32le "$OUTFILE" || continue

            # --- Demucs processing ---
            echo "Running Demucs on $OUTFILE..."
            if ! ./perform_demucs.sh "$OUTFILE"; then
                echo "✗ Demucs processing failed for $OUTFILE"
                continue
            fi

            DEMUCS_FILES=("${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_vocals.wav" \
                          "${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_nonvocals.wav")

            # --- Fix: Validate both Demucs outputs exist ---
            MISSING_DEMUCS=""
            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                [ ! -f "$DEMUCS_FILE" ] && MISSING_DEMUCS="$MISSING_DEMUCS $(basename "$DEMUCS_FILE")"
            done
            
            if [ -n "$MISSING_DEMUCS" ]; then
                echo "✗ Missing Demucs outputs:$MISSING_DEMUCS - skipping this audio stream"
                continue
            fi

            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                BASE_DEMUCS=$(basename "$DEMUCS_FILE" .wav)
                echo "Creating 3 variants for $DEMUCS_FILE"

                # --- Create audio variants ---
                OUT16="$OUTDIR/${BASE_DEMUCS}_16k_16bit.wav"
                OUT44="$OUTDIR/${BASE_DEMUCS}_44k_32bit.wav"
                OUT22="$OUTDIR/${BASE_DEMUCS}_22k_16bit.wav"

                # --- Fix: Validate ffmpeg conversions ---
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 16000 -c:a pcm_s16le "$OUT16" 2>/dev/null; then
                    echo "✗ Failed to create 16kHz variant for $DEMUCS_FILE"
                    continue
                fi
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 44100 -c:a pcm_f32le "$OUT44" 2>/dev/null; then
                    echo "✗ Failed to create 44kHz variant for $DEMUCS_FILE"
                    rm -f "$OUT16"
                    continue
                fi
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 22050 -c:a pcm_s16le "$OUT22" 2>/dev/null; then
                    echo "✗ Failed to create 22kHz variant for $DEMUCS_FILE"
                    rm -f "$OUT16" "$OUT44"
                    continue
                fi

                echo "✓ Variants created for $DEMUCS_FILE"

                # --- DIFFERENTIATED PROCESSING: VOCALS vs NONVOCALS ---
                if [[ "$BASE_DEMUCS" == *"_vocals" ]]; then
                    echo "Processing features for $BASE_DEMUCS (vocals - full processing)..."

                    # --- Generate all vocal-specific CSV features ---
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" pitch
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" formant
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" jitter

                    # --- Post-process vocal CSVs ---
                    for CSV in rms pitch formant spectral zcr jitter; do
                        CSV_FILE="$OUTDIR/${BASE_DEMUCS}_${CSV}.csv"
                        [ -f "$CSV_FILE" ] && {
                            case $CSV in
                                rms)       ./rms_energy_process "$CSV_FILE" --time_col time_sec ;;
                                pitch)     ./pitch_process "$CSV_FILE" --time_col time_sec ;;
                                formant)   FORMANT_PROCESSED="$OUTDIR/${BASE_DEMUCS}_formant_processed.csv"; ./formants_process "$CSV_FILE" "$FORMANT_PROCESSED" ;;
                                spectral)  ./spectrals_process "$CSV_FILE" ;;
                                zcr)       ./zcr_process "$CSV_FILE" --time_col time_sec ;;
                                jitter)    echo "Jitter CSV post-process if needed" ;;
                            esac
                            rm -f "$CSV_FILE"
                        }
                    done

                    # --- Vocal-specific: Emotion processing ---
                    echo "Processing emotions for $BASE_DEMUCS..."
                    EMOTION_NPY="$OUTDIR/${BASE_DEMUCS}_emotion.npy"
                    EMOTION_CSV="$OUTDIR/${BASE_DEMUCS}_emotion_processed.csv"
                    ./pre-process_emotions "$OUT16" "$EMOTION_NPY" 16000
                    if [ -f "$EMOTION_NPY" ]; then
                        python process_emotion.py "$EMOTION_NPY" "$EMOTION_CSV" --chunk_sec 5 \
                            --msp_model_path "./models/wav2vec2-large-robust-12-ft-emotion-msp-dim" \
                            --device "cuda"
                        rm -f "$EMOTION_NPY"
                        if [ -f "$EMOTION_CSV" ]; then
                            EMOTION_FINAL="$OUTDIR/${BASE_DEMUCS}_emotion_final.csv"
                            ./post-process_emotion "$EMOTION_CSV" "$EMOTION_FINAL"
                            rm -f "$EMOTION_CSV"
                        fi
                    fi

                    # --- Vocal-specific: Transcript processing ---
                    echo "Processing transcript for $BASE_DEMUCS..."
                    TRANSCRIPT_OUT="$OUTDIR/${BASE_DEMUCS}_transcript.csv"
                    python process_transcript.py "$OUT16" --output "$TRANSCRIPT_OUT"
                    if [ -f "$TRANSCRIPT_OUT" ]; then
                        ./transcript_process "$TRANSCRIPT_OUT" \
                            lexicons/NRC-Emo-Lex-v0.92.csv \
                            lexicons/NRC-VAD-Lex-v2.1.csv \
                            lexicons/NRC-WCST-Lex-v1.0.csv \
                            lexicons/WW-Lex-v1.csv \
                            lexicons/swears.txt
                        rm -f "$TRANSCRIPT_OUT"
                    fi
                    
                else
                    # --- NONVOCALS: Limited processing (no speech-specific features) ---
                    echo "Processing features for $BASE_DEMUCS (nonvocals - limited processing)..."

                    # --- Generate only relevant nonvocal CSV features ---
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral

                    # --- Post-process nonvocal CSVs ---
                    RMS_CSV="$OUTDIR/${BASE_DEMUCS}_rms.csv"
                    if [ -f "$RMS_CSV" ]; then
                        ./rms_energy_process "$RMS_CSV" --time_col time_sec
                        rm -f "$RMS_CSV"
                    fi

                    ZCR_CSV="$OUTDIR/${BASE_DEMUCS}_zcr.csv"
                    if [ -f "$ZCR_CSV" ]; then
                        ./zcr_process "$ZCR_CSV" --time_col time_sec
                        rm -f "$ZCR_CSV"
                    fi
                    
                    SPECTRAL_CSV="$OUTDIR/${BASE_DEMUCS}_spectral.csv"
                    if [ -f "$SPECTRAL_CSV" ]; then
                        ./spectrals_process "$SPECTRAL_CSV"
                        rm -f "$SPECTRAL_CSV"
                    fi

                    echo "✓ Nonvocals processing complete (skipped: pitch, formant, jitter, emotion, transcript)"
                fi

                # --- Clean up WAV variants ---
                rm -f "$DEMUCS_FILE" "$OUT16" "$OUT22" "$OUT44"
            done

            rm -f "$OUTFILE"
        else
            echo "Skipping mostly silent audio stream $STREAM_INDEX"
        fi
        echo ""
    done
fi

echo "=== MERGING PROCESSED CSVs BY AUDIO GROUP ==="

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    NUM_AUDIO_STREAMS=1   # force at least one loop to run
fi

for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
    for TYPE in vocals nonvocals; do
        GROUP_PREFIX="${BASENAME}_audio${STREAM_INDEX}_${TYPE}"
        CSV_FILES=$(ls "$OUTDIR/${GROUP_PREFIX}"_*.csv 2>/dev/null)
        [ -z "$CSV_FILES" ] && { echo "No CSVs to merge for $GROUP_PREFIX"; continue; }
        MERGED_OUT="$OUTDIR/${GROUP_PREFIX}_merged.csv"
        echo "Merging CSVs for $GROUP_PREFIX into $MERGED_OUT"
        
        if ./compile_data time_sec "$MERGED_OUT" $CSV_FILES 2>/dev/null; then
            rm -f $CSV_FILES
            echo "✓ Merge complete for $GROUP_PREFIX"
        else
            echo "✗ Merge failed for $GROUP_PREFIX - keeping individual files"
            echo "  Source files preserved: $CSV_FILES"
        fi
    done
done

# --- Video tracks (RE-ENCODED) ---
echo "=== VIDEO TRACK PROCESSING ==="
NUM_VIDEO_STREAMS=$(ffprobe -v error -select_streams v \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)
echo "Found $NUM_VIDEO_STREAMS video stream(s)"
echo ""

if [ "$NUM_VIDEO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        echo "--- Processing Video Stream $STREAM_INDEX ---"
        OUTFILE="$OUTDIR/${BASENAME}_video${STREAM_INDEX}.mp4"
        SALIENCE_OUT="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience.csv"

        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -vf "fps=15,scale=224:224" \
            -c:v libx264 -preset fast -crf 16 "$OUTFILE"
        echo "✓ Video stream re-encoded to $OUTFILE"

        echo "Running DeepGaze on $OUTFILE -> $SALIENCE_OUT"
        if python deepgaze.py "$OUTFILE" "$SALIENCE_OUT" 2>/dev/null; then
            if [ -f "$SALIENCE_OUT" ]; then
                echo "Processing salience CSV: $SALIENCE_OUT"
                ENGAGEMENT_OUT="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience_engagement.csv"
                ./salience_process "$SALIENCE_OUT" --time_col time_sec
                # The salience_process creates an engagement file, find it
                if [ -f "$ENGAGEMENT_OUT" ]; then
                    echo "✓ Salience processing complete: $ENGAGEMENT_OUT"
                    rm -f "$SALIENCE_OUT"  # Remove intermediate file
                else
                    echo "✗ Expected engagement file not found: $ENGAGEMENT_OUT"
                fi
            else
                echo "✗ DeepGaze did not generate expected output file"
            fi
        else
            echo "✗ DeepGaze processing failed for $OUTFILE"
        fi

        # --- Clean up video file ---
        rm -f "$OUTFILE"
    done
fi

echo ""
echo "=== ROBUST FINALIZE + GUI OVERLAY ==="

# --- Function to find best available merged files ---
find_best_files() {
    local vocals_files=()
    local nonvocals_files=()
    local video_files=()
    
    # Find all vocals merged files
    while IFS= read -r -d '' file; do
        vocals_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_audio*_vocals_merged.csv" -print0 2>/dev/null)
    
    # Find all nonvocals merged files  
    while IFS= read -r -d '' file; do
        nonvocals_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_audio*_nonvocals_merged.csv" -print0 2>/dev/null)
    
    # Find all video engagement files (corrected pattern)
    while IFS= read -r -d '' file; do
        video_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_video*_salience_engagement.csv" -print0 2>/dev/null)
    
    # Return counts and first file of each type
    echo "${#vocals_files[@]} ${#nonvocals_files[@]} ${#video_files[@]}"
    echo "${vocals_files[0]:-}"
    echo "${nonvocals_files[0]:-}"  
    echo "${video_files[0]:-}"
}

# Get available files
file_info=($(find_best_files))
VOCALS_COUNT="${file_info[0]}"
NONVOCALS_COUNT="${file_info[1]}"
VIDEO_COUNT="${file_info[2]}"
BEST_VOCALS="${file_info[3]}"
BEST_NONVOCALS="${file_info[4]}"
BEST_VIDEO="${file_info[5]}"

echo "Available files for finalization:"
echo "  Vocals files: $VOCALS_COUNT found"
echo "  Nonvocals files: $NONVOCALS_COUNT found"
echo "  Video files: $VIDEO_COUNT found"

# Validate file existence
[ -n "$BEST_VOCALS" ] && [ ! -f "$BEST_VOCALS" ] && BEST_VOCALS=""
[ -n "$BEST_NONVOCALS" ] && [ ! -f "$BEST_NONVOCALS" ] && BEST_NONVOCALS=""
[ -n "$BEST_VIDEO" ] && [ ! -f "$BEST_VIDEO" ] && BEST_VIDEO=""

echo ""
echo "Selected files for finalization:"
echo "  Best vocals: ${BEST_VOCALS:-NONE FOUND}"
echo "  Best nonvocals: ${BEST_NONVOCALS:-NONE FOUND}"
echo "  Best video: ${BEST_VIDEO:-NONE FOUND}"
echo ""

FINALIZED_OUT="$OUTDIR/${BASENAME}_finalized.csv"
FINALIZE_SUCCESS=false

# --- Multiple finalization strategies based on available files ---
if [ -n "$BEST_VOCALS" ] && [ -n "$BEST_NONVOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    echo "Strategy: Full finalization (vocals + nonvocals + video)"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_NONVOCALS"), $(basename "$BEST_VIDEO")"
    if ./finalize --vocals "$BEST_VOCALS" --nonvocals "$BEST_NONVOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        echo "✓ Full finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Full finalize failed, trying partial strategies..."
    fi
elif [ -n "$BEST_VOCALS" ] && [ -n "$BEST_NONVOCALS" ]; then
    echo "Strategy: Audio-only finalization (vocals + nonvocals)"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_NONVOCALS")"
    if ./finalize --vocals "$BEST_VOCALS" --nonvocals "$BEST_NONVOCALS" --output "$FINALIZED_OUT"; then
        echo "✓ Audio-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Audio-only finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_VOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    echo "Strategy: Vocals + Video finalization"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_VIDEO")"
    if ./finalize --vocals "$BEST_VOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        echo "✓ Vocals+Video finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Vocals+Video finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_NONVOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    echo "Strategy: Nonvocals + Video finalization"
    echo "  Using: $(basename "$BEST_NONVOCALS"), $(basename "$BEST_VIDEO")"
    if ./finalize --nonvocals "$BEST_NONVOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        echo "✓ Nonvocals+Video finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Nonvocals+Video finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_VOCALS" ]; then
    echo "Strategy: Vocals-only finalization"
    echo "  Using: $(basename "$BEST_VOCALS")"
    if ./finalize --vocals "$BEST_VOCALS" --output "$FINALIZED_OUT"; then
        echo "✓ Vocals-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Vocals-only finalize failed"
    fi
elif [ -n "$BEST_NONVOCALS" ]; then
    echo "Strategy: Nonvocals-only finalization"
    echo "  Using: $(basename "$BEST_NONVOCALS")"
    if ./finalize --nonvocals "$BEST_NONVOCALS" --output "$FINALIZED_OUT"; then
        echo "✓ Nonvocals-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Nonvocals-only finalize failed"
    fi
elif [ -n "$BEST_VIDEO" ]; then
    echo "Strategy: Video-only finalization"
    echo "  Using: $(basename "$BEST_VIDEO")"
    if ./finalize --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        echo "✓ Video-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        echo "✗ Video-only finalize failed"
    fi
else
    echo "✗ No suitable CSV files found for finalization"
    echo "  Expected at least one of: vocals_merged.csv, nonvocals_merged.csv, salience_engagement.csv"
fi

# --- Handle multiple audio streams if needed ---
if [ "$FINALIZE_SUCCESS" = false ] && [ "$VOCALS_COUNT" -gt 1 ] || [ "$NONVOCALS_COUNT" -gt 1 ]; then
    echo ""
    echo "Attempting finalization with alternative audio streams..."
    
    # Find all vocals and nonvocals files for iteration
    ALL_VOCALS=($(find "$OUTDIR" -name "${BASENAME}_audio*_vocals_merged.csv" 2>/dev/null))
    ALL_NONVOCALS=($(find "$OUTDIR" -name "${BASENAME}_audio*_nonvocals_merged.csv" 2>/dev/null))
    
    for vocals_file in "${ALL_VOCALS[@]}"; do
        for nonvocals_file in "${ALL_NONVOCALS[@]}"; do
            # Skip if we already tried this combination
            if [ "$vocals_file" = "$BEST_VOCALS" ] && [ "$nonvocals_file" = "$BEST_NONVOCALS" ]; then
                continue
            fi
            
            echo "Trying: $(basename "$vocals_file") + $(basename "$nonvocals_file")"
            if [ -n "$BEST_VIDEO" ]; then
                if ./finalize --vocals "$vocals_file" --nonvocals "$nonvocals_file" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
                    echo "✓ Alternative finalize successful with video"
                    FINALIZE_SUCCESS=true
                    break 2
                fi
            else
                if ./finalize --vocals "$vocals_file" --nonvocals "$nonvocals_file" --output "$FINALIZED_OUT"; then
                    echo "✓ Alternative finalize successful (audio only)"
                    FINALIZE_SUCCESS=true
                    break 2
                fi
            fi
        done
    done
fi

# --- GUI Overlay generation (only if finalization succeeded) ---
if [ "$FINALIZE_SUCCESS" = true ] && [ -f "$FINALIZED_OUT" ]; then
    OVERLAY_OUT="$(dirname "$INPUT")/${BASENAME}_data_overlay.mp4"
    echo ""
    echo "Running GUI overlay generation..."
    echo "  Input video: $INPUT"
    echo "  Finalized data: $FINALIZED_OUT"
    echo "  Output video: $OVERLAY_OUT"
    
    if ./gui_overlay "$INPUT" "$FINALIZED_OUT" "$OVERLAY_OUT"; then
        echo "✓ Overlay video created successfully: $OVERLAY_OUT"
    else
        echo "✗ GUI overlay generation failed"
        echo "  Finalized data is still available at: $FINALIZED_OUT"
    fi
else
    echo "⚠ Skipping GUI overlay generation (finalization failed or no data)"
fi

echo "=== PROCESSING COMPLETE ==="
echo "Finished at: $(date)"
echo "All output files saved to: $OUTDIR"
echo ""
echo "Final output structure:"

# --- List merged audio outputs (vocals/nonvocals per stream) ---
if [ "$NUM_AUDIO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        VOCALS_MERGED="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}_vocals_merged.csv"
        NONVOCALS_MERGED="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}_nonvocals_merged.csv"

        if [ -f "$VOCALS_MERGED" ]; then
            echo "  - Audio${STREAM_INDEX} (vocals): $(basename "$VOCALS_MERGED")"
            echo "      contains: RMS, ZCR, pitch, spectral, formant, jitter, emotion, transcript"
        else
            echo "  - Audio${STREAM_INDEX} (vocals): no merged CSV generated"
        fi

        if [ -f "$NONVOCALS_MERGED" ]; then
            echo "  - Audio${STREAM_INDEX} (nonvocals): $(basename "$NONVOCALS_MERGED")"
            echo "      contains: RMS, ZCR, spectral"
        else
            echo "  - Audio${STREAM_INDEX} (nonvocals): no merged CSV generated"
        fi
    done
else
    echo "  - No audio streams detected"
fi

# --- List processed video outputs ---
if [ "$NUM_VIDEO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        VIDEO_CSV="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience_engagement.csv"
        if [ -f "$VIDEO_CSV" ]; then
            echo "  - Video${STREAM_INDEX}: $(basename "$VIDEO_CSV")"
        else
            echo "  - Video${STREAM_INDEX}: no salience engagement CSV generated"
        fi
    done
else
    echo "  - No video streams detected"
fi

# --- Show finalization results ---
echo ""
if [ -f "$FINALIZED_OUT" ]; then
    echo "✓ Final combined data: $(basename "$FINALIZED_OUT")"
    if [ -f "$(dirname "$INPUT")/${BASENAME}_data_overlay.mp4" ]; then
        echo "✓ Data overlay video: ${BASENAME}_data_overlay.mp4"
    fi
else
    echo "✗ No finalized combined data generated"
fi