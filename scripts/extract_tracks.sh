#!/bin/bash
# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe, ./perform_demucs.sh

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"

echo "=== Media Track Extractor - Verbose Mode ==="
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

# --- Audio track extraction ---
echo "=== AUDIO TRACK PROCESSING ==="
NUM_AUDIO_STREAMS=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)
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

            DEMUCS_FILES=("${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_vocals.wav" "${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_nonvocals.wav")
            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                [ ! -f "$DEMUCS_FILE" ] && { echo "✗ Missing Demucs output: $DEMUCS_FILE"; continue; }
                BASE_DEMUCS=$(basename "$DEMUCS_FILE" .wav)
                echo "Creating 3 variants for $DEMUCS_FILE"

                # Variant 1: 16 kHz 16-bit
                ffmpeg -y -i "$DEMUCS_FILE" -ar 16000 -c:a pcm_s16le "${OUTDIR}/${BASE_DEMUCS}_16k_16bit.wav"
                # Variant 2: 44.1 kHz 32-bit float
                ffmpeg -y -i "$DEMUCS_FILE" -ar 44100 -c:a pcm_f32le "${OUTDIR}/${BASE_DEMUCS}_44k_32bit.wav"
                # Variant 3: 22.05 kHz 16-bit
                ffmpeg -y -i "$DEMUCS_FILE" -ar 22050 -c:a pcm_s16le "${OUTDIR}/${BASE_DEMUCS}_22k_16bit.wav"

                echo "✓ Variants created for $DEMUCS_FILE"

                # --- Delete original Demucs file after variants ---
                rm -f "$DEMUCS_FILE"
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
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -vf "fps=15,scale=224:224" \
            -c:v libx264 -preset fast -crf 16 "$OUTFILE"
        echo "✓ Video stream re-encoded to $OUTFILE"
    done
fi

# --- Delete original input after audio/video extraction ---
rm -f "$INPUT"
echo "✓ Original input file deleted: $INPUT"

echo "=== PROCESSING COMPLETE ==="
echo "Finished at: $(date)"
echo "All output files saved to: $OUTDIR"
