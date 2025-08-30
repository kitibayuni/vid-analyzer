#!/bin/bash

# usage is ./extract_audio.sh input.mp4
# requires ffmpeg, ffprobe

INPUT="$1"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//') # remove extension from file name
OUTDIR="./temp"

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

