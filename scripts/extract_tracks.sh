#!/bin/bash

# usage: ./separate_av.sh input.mp4
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

echo "Separating audio tracks from: $INPUT"

# --- Audio tracks ---
NUM_AUDIO_STREAMS=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    echo "No audio streams found!"
else
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        OUTFILE="$OUTDIR/${BASENAME}_stream${STREAM_INDEX}.flac"
        echo "Exporting audio stream $STREAM_INDEX → $OUTFILE"
        ffmpeg -y -i "$INPUT" -map 0:a:$STREAM_INDEX -c:a flac "$OUTFILE"
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
        echo "Exporting video stream $STREAM_INDEX → $OUTFILE"
        # Copy video codec and remove audio
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -c:v copy "$OUTFILE"
    done
fi

echo "Audio and video separation complete. All files in $OUTDIR."
