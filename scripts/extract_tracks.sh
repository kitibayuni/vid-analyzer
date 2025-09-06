#!/bin/bash

# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"

# --- Checks ---
if [ -z "$INPUT" ]; then
    echo "Usage: $0 <input_video>"
    exit 1
fi

if [ ! -f "$INPUT" ]; then
    echo "Error: input file not found!"
    exit 1
fi

if [ ! -d "$OUTDIR" ]; then
    echo "Error: $OUTDIR directory not found! Please create it."
    exit 1
fi

echo "Separating audio tracks (48 kHz WAV) and video tracks from: $INPUT"

# --- Audio tracks ---
NUM_AUDIO_STREAMS=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    echo "No audio streams found!"
else
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        # Get total duration of the audio stream
        DURATION=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=duration -of csv=p=0 "$INPUT")
        DURATION=${DURATION%.*}

        # Detect silence segments
        SILENCE_OUTPUT=$(ffmpeg -i "$INPUT" -map 0:a:$STREAM_INDEX \
            -af silencedetect=noise=-50dB:d=1 -f null - 2>&1)

        # Sum silence durations
        TOTAL_SILENCE=0
        while read -r LINE; do
            if [[ $LINE =~ silence_start ]]; then
                START=$(echo "$LINE" | awk '{print $5}')
            elif [[ $LINE =~ silence_end ]]; then
                END=$(echo "$LINE" | awk '{print $5}')
                SILENCE_DURATION=$(echo "$END - $START" | bc)
                TOTAL_SILENCE=$(echo "$TOTAL_SILENCE + $SILENCE_DURATION" | bc)
            fi
        done <<< "$SILENCE_OUTPUT"

        # Calculate silence fraction
        SILENCE_FRAC=$(echo "$TOTAL_SILENCE / $DURATION" | bc -l)

        # Export only if less than 90% silent
        if (( $(echo "$SILENCE_FRAC < 0.9" | bc -l) )); then
            OUTFILE="$OUTDIR/${BASENAME}_stream${STREAM_INDEX}.wav"
            echo "Exporting audio stream $STREAM_INDEX → $OUTFILE (48 kHz WAV, $SILENCE_FRAC fraction silent)"
            ffmpeg -y -i "$INPUT" -map 0:a:$STREAM_INDEX -ar 48000 -c:a pcm_s16le "$OUTFILE"
        else
            echo "Skipping mostly silent audio stream $STREAM_INDEX ($SILENCE_FRAC fraction silent)"
        fi
    done
fi

# --- Video tracks ---
NUM_VIDEO_STREAMS=$(ffprobe -v error -select_streams v \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

if [ "$NUM_VIDEO_STREAMS" -eq 0 ]; then
    echo "No video streams found!"
else
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        OUTFILE="$OUTDIR/${BASENAME}_video${STREAM_INDEX}.mp4"
        echo "Exporting video stream $STREAM_INDEX → $OUTFILE (original codec, no audio)"
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -c:v copy "$OUTFILE"
    done
fi

echo "Audio tracks exported with partial silence allowed, video tracks preserved."