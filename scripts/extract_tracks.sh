#!/bin/bash
# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe, ./perform_demucs.sh, ./formants_process, ./rms_energy_process, ./pitch_process, ./process_transcript.py
# requires: ./pre-process_emotions, ./process_emotion.py, ./spectrals_process, ./salience_process, ./transcript_process

# --- Source virtual environment ---
source venv/bin/activate

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"

echo "=== Media Track Extractor - Enhanced with Emotion Processing ==="
echo "Script started at: $(date)"
echo ""

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
NUM_AUDIO_STREAMS=$(echo "$FFPROBE_OUTPUT" | wc -l)
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
                SILENCE_DURATION=$(echo "$END - $START" | bc 2>/dev/null)
                TOTAL_SILENCE=$(echo "$TOTAL_SILENCE + ${SILENCE_DURATION:-0}" | bc)
                SILENCE_COUNT=$((SILENCE_COUNT + 1))
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
            ./perform_demucs.sh "$OUTFILE" || continue

            DEMUCS_FILES=("${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_vocals.wav" \
                          "${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_nonvocals.wav")

            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                [ ! -f "$DEMUCS_FILE" ] && { echo "✗ Missing Demucs output: $DEMUCS_FILE"; continue; }
                BASE_DEMUCS=$(basename "$DEMUCS_FILE" .wav)
                echo "Creating 3 variants for $DEMUCS_FILE"

                # --- Create audio variants ---
                OUT16="$OUTDIR/${BASE_DEMUCS}_16k_16bit.wav"
                OUT44="$OUTDIR/${BASE_DEMUCS}_44k_32bit.wav"
                OUT22="$OUTDIR/${BASE_DEMUCS}_22k_16bit.wav"

                ffmpeg -y -i "$DEMUCS_FILE" -ar 16000 -c:a pcm_s16le "$OUT16"
                ffmpeg -y -i "$DEMUCS_FILE" -ar 44100 -c:a pcm_f32le "$OUT44"
                ffmpeg -y -i "$DEMUCS_FILE" -ar 22050 -c:a pcm_s16le "$OUT22"

                echo "✓ Variants created for $DEMUCS_FILE"

                # --- Process features for VOCALS ---
                if [[ "$BASE_DEMUCS" == *"_vocals" ]]; then
                    echo "Processing features for $BASE_DEMUCS (vocals)..."

                    # Generate CSV features
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" pitch       # <-- restored pitch CSV
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" formant
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" jitter

                    # --- Post-process RMS CSV ---
                    RMS_CSV="$OUTDIR/${BASE_DEMUCS}_rms.csv"
                    [ -f "$RMS_CSV" ] && ./rms_energy_process "$RMS_CSV" --time_col time_sec

                    # --- Post-process Pitch CSV ---
                    PITCH_CSV="$OUTDIR/${BASE_DEMUCS}_pitch.csv"
                    [ -f "$PITCH_CSV" ] && ./pitch_process "$PITCH_CSV" --time_col time_sec

                    # --- Post-process Formant CSV ---
                    FORMANT_CSV="$OUTDIR/${BASE_DEMUCS}_formant.csv"
                    if [ -f "$FORMANT_CSV" ]; then
                        FORMANT_PROCESSED="$OUTDIR/${BASE_DEMUCS}_formant_processed.csv"
                        ./formants_process "$FORMANT_CSV" "$FORMANT_PROCESSED"
                        rm -f "$FORMANT_CSV"
                    fi

                    # --- NEW: Post-process Spectral CSV ---
                    SPECTRAL_CSV="$OUTDIR/${BASE_DEMUCS}_spectral.csv"
                    if [ -f "$SPECTRAL_CSV" ]; then
                        echo "Processing spectral CSV: $SPECTRAL_CSV"
                        ./spectrals_process "$SPECTRAL_CSV"
                        echo "✓ Spectral processing complete"
                    fi

                    # --- NEW: Emotion preprocessing and analysis ---
                    echo "Processing emotions for $BASE_DEMUCS..."
                    EMOTION_NPY="$OUTDIR/${BASE_DEMUCS}_emotion.npy"
                    EMOTION_CSV="$OUTDIR/${BASE_DEMUCS}_emotion_processed.csv"
                    
                    # Pre-process emotions (16kHz input, target_sr=16000)
                    echo "Pre-processing emotions: $OUT16 -> $EMOTION_NPY"
                    ./pre-process_emotions "$OUT16" "$EMOTION_NPY" 16000
                    
                    # Process emotions
                    if [ -f "$EMOTION_NPY" ]; then
                        echo "Processing emotions: $EMOTION_NPY -> $EMOTION_CSV"
                        python process_emotion.py "$EMOTION_NPY" "$EMOTION_CSV" --chunk_sec 5
                        echo "✓ Emotion processing complete"
                        # Clean up intermediate .npy file
                        rm -f "$EMOTION_NPY"
                    else
                        echo "✗ Emotion preprocessing failed - no .npy file generated"
                    fi

                    # --- Transcript ---
                    TRANSCRIPT_OUT="$OUTDIR/${BASE_DEMUCS}_transcript.csv"
                    python process_transcript.py "$OUT16" --output "$TRANSCRIPT_OUT"
                    
                    # --- NEW: Enhanced transcript processing with lexicons ---
                    if [ -f "$TRANSCRIPT_OUT" ]; then
                        echo "Processing transcript with lexicons..."
                        TRANSCRIPT_PROCESSED="$OUTDIR/${BASE_DEMUCS}_transcript_processed.csv"
                        ./transcript_process "$TRANSCRIPT_OUT" \
                            lexicons/NRC-Emo-Lex-v0.92.csv \
                            lexicons/NRC-VAD-Lex-v2.1.csv \
                            lexicons/NRC-WCST-Lex-v1.0.csv \
                            lexicons/WW-Lex-v1.csv \
                            lexicons/swears.txt
                        echo "✓ Transcript lexicon processing complete"
                    fi
                    
                else
                    # --- Process features for NONVOCALS ---
                    echo "Processing features for $BASE_DEMUCS (nonvocals)..."

                    # Generate CSV features
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral

                    # --- Post-process RMS CSV ---
                    RMS_CSV="$OUTDIR/${BASE_DEMUCS}_rms.csv"
                    if [ -f "$RMS_CSV" ]; then
                        ./rms_energy_process "$RMS_CSV" --time_col time_sec
                        # Keep only processed version, remove intermediate
                        rm -f "$RMS_CSV"
                    fi

                    # --- Post-process ZCR CSV ---
                    ZCR_CSV="$OUTDIR/${BASE_DEMUCS}_zcr.csv"
                    if [ -f "$ZCR_CSV" ]; then
                        ./zcr_process "$ZCR_CSV" --time_col time_sec
                        # Keep only processed version, remove intermediate
                        rm -f "$ZCR_CSV"
                    fi
                    
                    # --- NEW: Post-process Spectral CSV for nonvocals too ---
                    SPECTRAL_CSV="$OUTDIR/${BASE_DEMUCS}_spectral.csv"
                    if [ -f "$SPECTRAL_CSV" ]; then
                        echo "Processing spectral CSV: $SPECTRAL_CSV"
                        ./spectrals_process "$SPECTRAL_CSV"
                        echo "✓ Spectral processing complete"
                        # Keep only processed version, remove intermediate
                        rm -f "$SPECTRAL_CSV"
                    fi
                fi

                # --- Delete intermediate WAVs ---
                rm -f "$DEMUCS_FILE" "$OUT16" "$OUT22" "$OUT44"
            done

            # --- Delete the extracted audio once Demucs variants exist ---
            rm -f "$OUTFILE"
        else
            echo "Skipping mostly silent audio stream $STREAM_INDEX"
        fi
        echo ""
    done
fi

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

        # Re-encode video
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -vf "fps=15,scale=224:224" \
            -c:v libx264 -preset fast -crf 16 "$OUTFILE"
        echo "✓ Video stream re-encoded to $OUTFILE"

        # Run DeepGaze salience extraction
        echo "Running DeepGaze on $OUTFILE -> $SALIENCE_OUT"
        python deepgaze.py "$OUTFILE" "$SALIENCE_OUT"

        # --- NEW: Process salience CSV ---
        if [ -f "$SALIENCE_OUT" ]; then
            echo "Processing salience CSV: $SALIENCE_OUT"
            ./salience_process "$SALIENCE_OUT" --time_col time_sec
            echo "✓ Salience processing complete"
        fi

        # Cleanup
        rm -f "$OUTFILE"
        echo "✓ Cleaned up $OUTFILE"
    done
fi

echo "=== PROCESSING COMPLETE ==="
echo "Finished at: $(date)"
echo "All output files saved to: $OUTDIR"

deactivate