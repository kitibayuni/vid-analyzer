#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"     # or mdx_extra_q / mdx_q if you prefer lighter models
SEGMENT_TIME=600     # seconds per chunk (10 minutes)
OVERLAP=2            # seconds overlap between chunks
STEMS=("vocals" "no_vocals")  # two-stems mode creates vocals and no_vocals
# For 4-stem mode, use: STEMS=("vocals" "drums" "bass" "other")
# and remove --two-stems=vocals from the demucs command
# ---------------

# Advanced progress bar function
progress_bar() {
    local current=$1
    local total=$2
    local task_name="${3:-Processing}"
    local width=40
    local percentage=$((current * 100 / total))
    local filled=$((current * width / total))
    
    # Spinner characters
    local spinner="/-\|"
    local spin_char=${spinner:$((current % 4)):1}
    
    # Build the bar
    local bar=""
    for ((i=0; i<filled; i++)); do
        bar+="█"
    done
    for ((i=filled; i<width; i++)); do
        bar+="░"
    done
    
    # Print without newline
    printf "\r%s [%s] %d%% (%d/%d) %s" "$spin_char" "$bar" "$percentage" "$current" "$total" "$task_name"
}

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

# Calculate total chunks for progress bar
TOTAL_CHUNKS=$(( (DURATION + SEGMENT_TIME - 1) / SEGMENT_TIME ))

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
  
  # Show progress
  progress_bar $((idx + 1)) $TOTAL_CHUNKS "Splitting chunks"
  
  start=$((start + SEGMENT_TIME))
  idx=$((idx + 1))
done
echo ""  # New line after progress bar

echo ">>> Running Demucs ($MODEL) on all chunks..."
cd "$WORKDIR"
demucs -n "$MODEL" --two-stems=vocals -d cuda chunks/chunk_*.wav
cd - >/dev/null

# Debug: List the separated directory structure
echo ">>> Checking Demucs output structure..."
find "${WORKDIR}/separated" -type f -name "*.wav" | head -5

# Find the actual Demucs model output dir (handles htdemucs_6s etc.)
SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
if [ -z "$SEPDIR" ]; then
  echo "Error: Could not find Demucs output directory under ${WORKDIR}/separated/"
  exit 1
fi

echo ">>> Found separated files in: $SEPDIR"

# Function to stitch stems with crossfade and progress bar
stitch_stem () {
  local stem="$1"
  echo ">>> Stitching stem: $stem"
  
  local chunk_files=( $(ls "${CHUNKS_DIR}"/chunk_*.wav | sort) )
  local inputs=()
  for f in "${chunk_files[@]}"; do
    name=$(basename "$f" .wav)
    # Check if the stem file exists in the expected location
    stem_file="${SEPDIR}/${name}/${stem}.wav"
    if [ -f "$stem_file" ]; then
      inputs+=("$stem_file")
    else
      echo "Warning: Missing stem file: $stem_file"
    fi
  done
  
  if [ ${#inputs[@]} -eq 0 ]; then
    echo "Error: No $stem files found for stitching!"
    return 1
  fi

  if [[ ${#inputs[@]} -eq 1 ]]; then
    cp "${inputs[0]}" "${OUTDIR}/${stem}.wav"
    echo ">>> Only one chunk, copied directly"
    return
  fi

  tmp="${OUTDIR}/${stem}_tmp0.wav"
  cp "${inputs[0]}" "$tmp"
  part_idx=1
  
  # Progress tracking for crossfade operations
  total_operations=$((${#inputs[@]} - 1))
  
  for ((i=1; i<${#inputs[@]}; i++)); do
    next="${inputs[i]}"
    outtmp="${OUTDIR}/${stem}_tmp${part_idx}.wav"
    
    progress_bar $i $total_operations "Crossfading $stem"
    
    ffmpeg -hide_banner -loglevel error -y -i "$tmp" -i "$next" \
      -filter_complex "acrossfade=d=${OVERLAP}:c1=tri:c2=tri" \
      -c:a pcm_s16le "$outtmp"
    rm -f "$tmp"
    tmp="$outtmp"
    ((part_idx++))
  done
  echo ""  # New line after progress bar
  
  mv "$tmp" "${OUTDIR}/${stem}.wav"
  echo ">>> Completed stitching $stem"
}

# Process all stems
for stem in "${STEMS[@]}"; do
  stitch_stem "$stem"
done

# Collect all non-vocal stems for concatenation
echo ">>> Collecting non-vocal stems..."
non_vocal_files=()
for stem in "${STEMS[@]}"; do
  if [ "$stem" != "vocals" ]; then
    if [ -f "${OUTDIR}/${stem}.wav" ]; then
      non_vocal_files+=("${OUTDIR}/${stem}.wav")
      echo ">>> Found non-vocal stem: ${OUTDIR}/${stem}.wav"
    fi
  fi
done

echo ">>> Moving final files to original directory with proper naming..."

# Move vocals file with proper naming
if [ -f "${OUTDIR}/vocals.wav" ]; then
  mv "${OUTDIR}/vocals.wav" "${INPUT_DIR}/${BASENAME}_vocals.wav"
  echo "✓ Created: ${INPUT_DIR}/${BASENAME}_vocals.wav"
fi

# Handle non-vocal stems
if [ ${#non_vocal_files[@]} -gt 1 ]; then
  echo ">>> Combining multiple non-vocal stems..."
  
  # Create filter complex for mixing multiple non-vocal stems
  filter_inputs=""
  filter_mix=""
  for ((i=0; i<${#non_vocal_files[@]}; i++)); do
    filter_inputs+=" -i \"${non_vocal_files[i]}\""
    if [ $i -eq 0 ]; then
      filter_mix="[0:a]"
    else
      filter_mix+="[${i}:a]"
    fi
  done
  filter_mix+="amix=inputs=${#non_vocal_files[@]}:duration=longest[out]"
  
  eval ffmpeg -hide_banner -loglevel error -y $filter_inputs \
    -filter_complex "\"$filter_mix\"" -map "[out]" \
    "\"${INPUT_DIR}/${BASENAME}_nonvocals.wav\""
  
  echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
  
elif [ ${#non_vocal_files[@]} -eq 1 ]; then
  # Only one non-vocal stem, just move it
  mv "${non_vocal_files[0]}" "${INPUT_DIR}/${BASENAME}_nonvocals.wav"
  echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
else
  echo ">>> No non-vocal stems found to process"
fi

# Clean up work directory
echo ">>> Cleaning up temporary files..."
rm -rf "$WORKDIR"
echo "✓ Temporary work directory removed"

echo ""
echo ">>> All done!"
echo "Final files created in: $INPUT_DIR"
echo "  - ${BASENAME}_vocals.wav"
if [ ${#non_vocal_files[@]} -gt 0 ]; then
  echo "  - ${BASENAME}_nonvocals.wav"
fi