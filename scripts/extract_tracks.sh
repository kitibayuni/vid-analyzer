#!/bin/bash
# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"

# --- Checks ---
echo "=== Media Track Extractor - Verbose Mode ==="
echo "Script started at: $(date)"
echo ""

if [ -z "$INPUT" ]; then
    echo "ERROR: No input file provided!"
    echo "Usage: $0 <input_video>"
    exit 1
fi

echo "Processing input file: $INPUT"
echo "Output basename: $BASENAME"
echo "Output directory: $OUTDIR"
echo ""

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

# Get file info
echo "=== FILE ANALYSIS ==="
echo "Analyzing input file..."
FILE_SIZE=$(du -h "$INPUT" | cut -f1)
echo "File size: $FILE_SIZE"

# Get duration
TOTAL_DURATION=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$INPUT" 2>/dev/null)
if [ -n "$TOTAL_DURATION" ]; then
    DURATION_MIN=$(echo "$TOTAL_DURATION / 60" | bc -l | cut -d. -f1)
    DURATION_SEC=$(echo "$TOTAL_DURATION % 60" | bc -l | cut -d. -f1)
    echo "Total duration: ${DURATION_MIN}m ${DURATION_SEC}s"
fi
echo ""

echo "Separating audio tracks (48 kHz 32-bit float WAV) and re-encoding video tracks from: $INPUT"
echo ""

# --- Audio tracks ---
echo "=== AUDIO TRACK PROCESSING ==="
echo "Detecting audio streams..."

NUM_AUDIO_STREAMS=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

echo "Found $NUM_AUDIO_STREAMS audio stream(s)"
echo ""

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    echo "No audio streams found - skipping audio processing"
else
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        echo "--- Processing Audio Stream $STREAM_INDEX ---"
        
        # Get stream info
        echo "Getting stream information..."
        CODEC=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=codec_name -of csv=p=0 "$INPUT" 2>/dev/null)
        SAMPLE_RATE=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=sample_rate -of csv=p=0 "$INPUT" 2>/dev/null)
        CHANNELS=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=channels -of csv=p=0 "$INPUT" 2>/dev/null)
        
        echo "  Codec: ${CODEC:-unknown}"
        echo "  Sample rate: ${SAMPLE_RATE:-unknown} Hz"
        echo "  Channels: ${CHANNELS:-unknown}"
        
        # Get total duration of the audio stream (fallback to 1 second if empty)
        echo "Getting stream duration..."
        DURATION=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=duration -of csv=p=0 "$INPUT")
        DURATION=${DURATION%.*}
        DURATION=${DURATION:-1} # fallback to 1 if empty
        echo "  Duration: ${DURATION} seconds"
        
        # Detect silence segments
        echo "Analyzing silence patterns (this may take a moment)..."
        SILENCE_OUTPUT=$(ffmpeg -i "$INPUT" -map 0:a:$STREAM_INDEX \
            -af silencedetect=noise=-50dB:d=1 -f null - 2>&1)
        
        echo "Processing silence detection results..."
        
        # Sum silence durations safely
        TOTAL_SILENCE=0
        START=""
        SILENCE_COUNT=0
        while read -r LINE; do
            if [[ $LINE =~ silence_start ]]; then
                START=$(echo "$LINE" | awk '{print $5}')
                echo "  Found silence start at: ${START}s"
            elif [[ $LINE =~ silence_end ]] && [[ -n "$START" ]]; then
                END=$(echo "$LINE" | awk '{print $5}')
                SILENCE_DURATION=$(echo "$END - $START" | bc 2>/dev/null)
                SILENCE_DURATION=${SILENCE_DURATION:-0}
                TOTAL_SILENCE=$(echo "$TOTAL_SILENCE + $SILENCE_DURATION" | bc)
                SILENCE_COUNT=$((SILENCE_COUNT + 1))
                echo "  Silence end at: ${END}s (duration: ${SILENCE_DURATION}s)"
                START=""
            fi
        done <<< "$SILENCE_OUTPUT"
        
        echo "  Total silence segments: $SILENCE_COUNT"
        echo "  Total silence duration: ${TOTAL_SILENCE}s"
        
        # Calculate silence fraction safely
        SILENCE_FRAC=$(echo "$TOTAL_SILENCE / $DURATION" | bc -l)
        SILENCE_FRAC=${SILENCE_FRAC:-0}
        SILENCE_PERCENT=$(echo "$SILENCE_FRAC * 100" | bc -l | cut -d. -f1)
        echo "  Silence percentage: ${SILENCE_PERCENT}%"
        
        # Export only if less than 90% silent
        if (( $(echo "$SILENCE_FRAC < 0.9" | bc -l) )); then
            OUTFILE="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}.wav"
            echo "✓ Stream has acceptable audio content - exporting..."
            echo "  Output file: $OUTFILE"
            echo "  Format: 48 kHz 32-bit float WAV"
            echo "  Starting export..."
            
            ffmpeg -y -i "$INPUT" -map 0:a:$STREAM_INDEX -ar 48000 -c:a pcm_f32le "$OUTFILE"
            
            if [ $? -eq 0 ]; then
                OUTFILE_SIZE=$(du -h "$OUTFILE" | cut -f1)
                echo "✓ Audio stream $STREAM_INDEX exported successfully (${OUTFILE_SIZE})"
            else
                echo "✗ Failed to export audio stream $STREAM_INDEX"
            fi
        else
            echo "✗ Skipping mostly silent audio stream $STREAM_INDEX (${SILENCE_PERCENT}% silent)"
        fi
        echo ""
    done
fi

# --- Video tracks (RE-ENCODED) ---
echo "=== VIDEO TRACK PROCESSING ==="
echo "Detecting video streams..."

NUM_VIDEO_STREAMS=$(ffprobe -v error -select_streams v \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

echo "Found $NUM_VIDEO_STREAMS video stream(s)"
echo ""

if [ "$NUM_VIDEO_STREAMS" -eq 0 ]; then
    echo "No video streams found - skipping video processing"
else
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        echo "--- Processing Video Stream $STREAM_INDEX ---"
        
        # Get stream info
        echo "Getting stream information..."
        CODEC=$(ffprobe -v error -select_streams v:$STREAM_INDEX \
            -show_entries stream=codec_name -of csv=p=0 "$INPUT" 2>/dev/null)
        WIDTH=$(ffprobe -v error -select_streams v:$STREAM_INDEX \
            -show_entries stream=width -of csv=p=0 "$INPUT" 2>/dev/null)
        HEIGHT=$(ffprobe -v error -select_streams v:$STREAM_INDEX \
            -show_entries stream=height -of csv=p=0 "$INPUT" 2>/dev/null)
        FPS=$(ffprobe -v error -select_streams v:$STREAM_INDEX \
            -show_entries stream=r_frame_rate -of csv=p=0 "$INPUT" 2>/dev/null)
        
        echo "  Original codec: ${CODEC:-unknown}"
        echo "  Original resolution: ${WIDTH:-?}x${HEIGHT:-?}"
        echo "  Original frame rate: ${FPS:-unknown}"
        
        OUTFILE="$OUTDIR/${BASENAME}_video${STREAM_INDEX}.mp4"
        echo "  Output file: $OUTFILE"
        echo "  Target settings: 15 fps, 224x224, libx264, CRF 16, no audio"
        echo "  Starting re-encoding..."
        
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -vf "fps=15,scale=224:224" \
            -c:v libx264 -preset fast -crf 16 "$OUTFILE"
        
        if [ $? -eq 0 ]; then
            OUTFILE_SIZE=$(du -h "$OUTFILE" | cut -f1)
            echo "✓ Video stream $STREAM_INDEX re-encoded successfully (${OUTFILE_SIZE})"
        else
            echo "✗ Failed to re-encode video stream $STREAM_INDEX"
        fi
        echo ""
    done
fi

echo "=== PROCESSING COMPLETE ==="
echo "Finished at: $(date)"
echo "Audio tracks exported with partial silence allowed"
echo "Video tracks re-encoded to 224x224 @ 15fps"
echo "All output files saved to: $OUTDIR"