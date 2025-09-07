#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"     # or mdx_extra_q / mdx_q if you prefer lighter models
SEGMENT_TIME=600     # seconds per chunk (10 minutes)
OVERLAP=2            # seconds overlap between chunks
STEMS=("vocals" "other")  # adjust for more stems if using full 4-stem model
# ---------------

if [ $# -lt 1 ]; then
  echo "Usage: $0 input.wav"
  exit 1
fi

INPUT="$(realpath "$1")"
INPUT_DIR="$(dirname "$INPUT")"
BASENAME="$(basename "$INPUT" .wav)"
WORKDIR="${INPUT_DIR}/${BASENAME}_work"
CHUNKS_DIR="${WORKDIR}/chunks"
OUTDIR="${WORKDIR}/stitched"

mkdir -p "$CHUNKS_DIR" "$OUTDIR"

# Get duration in seconds
DURATION=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$INPUT")
DURATION=${DURATION%.*} # round down to int

echo ">>> Input length: ${DURATION}s"
echo ">>> Splitting $INPUT into ${SEGMENT_TIME}s chunks with ${OVERLAP}s overlap..."

start=0
idx=0
while [ $start -lt $DURATION ]; do
  end=$((start + SEGMENT_TIME + OVERLAP))
  if [ $end -gt $DURATION ]; then
    end=$DURATION
  fi
  out=$(printf "%s/chunk_%03d.wav" "$CHUNKS_DIR" "$idx")
  ffmpeg -hide_banner -loglevel error -y -i "$INPUT" \
    -af "atrim=start=${start}:end=${end},asetpts=PTS-STARTPTS" "$out"
  echo "  wrote $out ($start–$end)"
  start=$((start + SEGMENT_TIME))
  idx=$((idx + 1))
done

echo ">>> Running Demucs ($MODEL) on all chunks..."
cd "$WORKDIR"
demucs -n "$MODEL" --two-stems=vocals -d cuda chunks/chunk_*.wav
cd - >/dev/null

# Find the actual Demucs model output dir (handles htdemucs_6s etc.)
SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
if [ -z "$SEPDIR" ]; then
  echo "Error: Could not find Demucs output directory under ${WORKDIR}/separated/"
  exit 1
fi

# Function to stitch stems with crossfade
stitch_stem () {
  local stem="$1"
  echo ">>> Stitching stem: $stem"
  local chunk_files=( $(ls "${CHUNKS_DIR}"/chunk_*.wav | sort) )
  local inputs=()
  for f in "${chunk_files[@]}"; do
    name=$(basename "$f" .wav)
    inputs+=("${SEPDIR}/${name}/${stem}.wav")
  done

  if [[ ${#inputs[@]} -eq 1 ]]; then
    cp "${inputs[0]}" "${OUTDIR}/${stem}.wav"
    return
  fi

  tmp="${OUTDIR}/${stem}_tmp0.wav"
  cp "${inputs[0]}" "$tmp"
  part_idx=1
  for ((i=1; i<${#inputs[@]}; i++)); do
    next="${inputs[i]}"
    outtmp="${OUTDIR}/${stem}_tmp${part_idx}.wav"
    ffmpeg -hide_banner -loglevel error -y -i "$tmp" -i "$next" \
      -filter_complex "acrossfade=d=${OVERLAP}:c1=tri:c2=tri" \
      -c:a pcm_s16le "$outtmp"
    rm -f "$tmp"
    tmp="$outtmp"
    ((part_idx++))
  done
  mv "$tmp" "${OUTDIR}/${stem}.wav"
}

for stem in "${STEMS[@]}"; do
  stitch_stem "$stem"
done

echo ">>> All done!"
echo "Stitched stems available in: $OUTDIR"
