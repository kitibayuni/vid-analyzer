#!/bin/bash

# usage is ./extract_audio.sh input.mp4
# requires ffmpeg, ffprobe


###################
## INITIAL CHECK ##
###################

INPUT="$1"
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

## AUDIO STREAMS

# count the streams
AUDIO_STREAMS=$(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$INPUT")

# if no audio streams found
if [ -z "$AUDIO_STREAMS" ]; then
  echo "No audio streams found in file!"
  exit 1
fi

###################
## STORAGE CHECK ##
###################

echo "Estimating output size per stream..."

TOTAL_ESTIMATE_MB=0

for STREAM in $AUDIO_STREAMS; do
  # Get duration in seconds
  DURATION=$(ffprobe -v error -select_streams a:$STREAM -show_entries stream=duration -of csv=p=0 "$INPUT")
  if [ -z "$DURATION" ]; then
    DURATION=0
  fi

  # Whisper WAV: 16kHz mono, 16-bit PCM
  WHISPER_BYTES=$(echo "$DURATION * 16000 * 1 * 2" | bc)
  WHISPER_MB=$(echo "scale=1; $WHISPER_BYTES/1048576" | bc)

  # FLAC outputs (22.05kHz mono, ~40% of PCM size)
  PCM_BYTES=$(echo "$DURATION * 22050 * 1 * 2" | bc)
  FLAC_BYTES=$(echo "$PCM_BYTES * 0.4" | bc)
  FLAC_MB=$(echo "scale=1; $FLAC_BYTES/1048576" | bc)

  # Total per stream
  STREAM_TOTAL_MB=$(echo "$WHISPER_MB + (3 * $FLAC_MB)" | bc)
  echo " - Stream $STREAM (~${DURATION}s): ~${STREAM_TOTAL_MB} MB total for all outputs"

  # Accumulate overall total
  TOTAL_ESTIMATE_MB=$(echo "$TOTAL_ESTIMATE_MB + $STREAM_TOTAL_MB" | bc)
done

echo "=== Overall estimated size for all streams: ~${TOTAL_ESTIMATE_MB} MB ==="

# Confirm with user this is fine
read -p "Proceed with extraction? (y/n) " CONFIRM
if [ "$CONFIRM" != "y" ]; then
  echo "Aborting."
  exit 0
fi

################
## EXTRACTION ##
################

for STREAM in $AUDIO_STREAMS; do
	echo "Processing audio stream: $STREAM for $INPUT"

	ffmpeg -y -i "$INPUT" \
	-map 0:a:$STREAM -ac 1 -ar 16000 -af "afftdn=nr=20" -c:a pcm_s16le \
      "$OUTDIR/${BASENAME}_stream${STREAM}_whisper.wav" \
    -map 0:a:$STREAM -ac 1 -ar 22050 -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM}_volume.flac" \
    -map 0:a:$STREAM -ac 1 -ar 22050 -af "afftdn=nr=20,loudnorm" -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM}_analysis.flac" \
    -map 0:a:$STREAM -ac 1 -ar 22050 -af "loudnorm" -c:a flac \
      "$OUTDIR/${BASENAME}_stream${STREAM}_events.flac"

done