#!/bin/bash

# usage: ./extract_audio.sh input.mp4
# requires ffmpeg and ffprobe

###################
## INITIAL CHECK ##
###################

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//') # remove extension from file name
OUTDIR="$(dirname "$0")/../temp"

# check if input
if [ -z "$INPUT" ]; then
  echo "Usage: $0 <input_video>"
  exit 1
fi

# if input but no file
if [ ! -f "$INPUT" ]; then
  echo "Error: input file not found!"
  exit 1
fi

# if /temp or output directory isn't found
if [ ! -d "$OUTDIR" ]; then
  echo "Error: $OUTDIR directory not found! Please adjust this script or create the directory."
  exit 1
fi

echo "Extracting audio tracks from: $INPUT"

###################
## AUDIO STREAMS ##
###################

# count the audio streams
NUM_AUDIO_STREAMS=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
  echo "No audio streams found in file!"
  exit 1
fi

###################
## STORAGE CHECK ##
###################

echo "Estimating output size per stream..."
TOTAL_ESTIMATE_MB=0

for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
  # Get duration in seconds (integer)
  DURATION=$(ffprobe -v error -select_streams a:$STREAM_INDEX -show_entries stream=duration -of csv=p=0 "$INPUT")
  DURATION=${DURATION%.*}  # truncate to integer seconds
  if [ -z "$DURATION" ]; then
    DURATION=0
  fi

  # Estimate sizes without bc (integer math)
  WHISPER_MB=$(( DURATION * 16000 * 1 * 2 / 1048576 ))
  PCM_MB=$(( DURATION * 22050 * 1 * 2 / 1048576 ))
  FLAC_MB=$(( PCM_MB * 40 / 100 ))  # ~40% compression
  STREAM_TOTAL_MB=$(( WHISPER_MB + 3 * FLAC_MB ))

  echo " - Stream $STREAM_INDEX (~${DURATION}s): ~${STREAM_TOTAL_MB} MB total for all outputs"
  TOTAL_ESTIMATE_MB=$(( TOTAL_ESTIMATE_MB + STREAM_TOTAL_MB ))
done

echo "=== Overall estimated size for all streams: ~${TOTAL_ESTIMATE_MB} MB ==="

# Confirm with user
read -p "Proceed with extraction? (y/n) " CONFIRM
if [ "$CONFIRM" != "y" ]; then
  echo "Aborting."
  exit 0
fi

################
## EXTRACTION ##
################

for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
  echo "Processing audio stream: $STREAM_INDEX for $INPUT"
  ffmpeg -y -i "$INPUT" \
    -map 0:a:$STREAM_INDEX -ac 1 -ar 16000 -af "afftdn=nr=20" -c:a pcm_s16le \
      "$OUTDIR/${BASENAME}_stream${STREAM_INDEX}_whisper.wav" \
    -map 0:a:$STREAM_INDEX -ac 1 -ar 22050 -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM_INDEX}_volume.flac" \
    -map 0:a:$STREAM_INDEX -ac 1 -ar 22050 -af "afftdn=nr=20,loudnorm" -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM_INDEX}_analysis.flac" \
    -map 0:a:$STREAM_INDEX -ac 1 -ar 22050 -af "loudnorm" -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM_INDEX}_events.flac" \
    -map 0:a:$STREAM_INDEX -ac 1 -ar 16000 -af "volume=0.5" -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM_INDEX}_emotion.flac"
done

echo "All audio variants exported to $OUTDIR."

###########################
## RE-ENCODED VIDEO ONLY ##
###########################

echo "Creating optimized video-only file without audio..."
ffmpeg -y -i "$INPUT" \
  -an -vf "fps=15,scale=224:224" \
  -c:v libx264 -preset fast -crf 16 \
  "$OUTDIR/${BASENAME}_vision.mp4"
echo "Optimized video-only file (no audio) exported to $OUTDIR/${BASENAME}_vision.mp4"
