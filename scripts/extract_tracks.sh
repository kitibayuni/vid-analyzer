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
                ./salience_process "$SALIENCE_OUT" --time_col time_sec
                rm -f "$SALIENCE_OUT"
                echo "✓ Salience processing complete"
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
        VIDEO_CSV="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience_processed.csv"
        if [ -f "$VIDEO_CSV" ]; then
            echo "  - Video${STREAM_INDEX}: $(basename "$VIDEO_CSV")"
        else
            echo "  - Video${STREAM_INDEX}: no salience CSV generated"
        fi
    done
else
    echo "  - No video streams detected"
fi
