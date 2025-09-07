#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"        # or mdx_extra_q / mdx_q
SEGMENT_TIME=600        # seconds per chunk (10 minutes)
OVERLAP=2               # seconds overlap between chunks (for splitting)
STEMS=("vocals" "no_vocals")
APPLY_CROSSFADE=false   # Set to true to smooth chunk boundaries (recommended small value)
CROSSFADE_DURATION=1    # seconds for crossfade at boundaries if enabled
# ----------------

# --- Advanced progress bar ---
progress_bar() {
    local current=$1 total=$2 task_name="${3:-Processing}" width=40
    local percentage=$((current*100/total))
    local filled=$((current*width/total))
    local spinner="/-\\|"
    local spin_char=${spinner:$((current % ${#spinner})):1}
    local bar=""
    for ((i=0;i<filled;i++)); do bar+="█"; done
    for ((i=filled;i<width;i++)); do bar+="░"; done
    printf "\r%s [%s] %d%% (%d/%d) %s" "$spin_char" "$bar" "$percentage" "$current" "$total" "$task_name"
}

# --- Input ---
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

# --- Get duration ---
DURATION=$(ffprobe -v error -show_entries format=duration \
    -of default=noprint_wrappers=1:nokey=1 "$INPUT")
DURATION=${DURATION%.*}

echo ">>> Input length: ${DURATION}s"
echo ">>> Splitting $INPUT into ${SEGMENT_TIME}s chunks with ${OVERLAP}s overlap..."

TOTAL_CHUNKS=$(( (DURATION + SEGMENT_TIME - 1) / SEGMENT_TIME ))
start=0
idx=0
while [ $start -lt $DURATION ]; do
    end=$((start + SEGMENT_TIME + OVERLAP))
    [ $end -gt $DURATION ] && end=$DURATION
    out=$(printf "%s/chunk_%03d.wav" "$CHUNKS_DIR" "$idx")

    # --- Export chunks as 32-bit float mono ---
    ffmpeg -hide_banner -loglevel error -y -i "$INPUT" \
        -af "atrim=start=${start}:end=${end},asetpts=PTS-STARTPTS" \
        -ac 1 -c:a pcm_f32le "$out"

    progress_bar $((idx+1)) $TOTAL_CHUNKS "Splitting chunks"

    start=$((start + SEGMENT_TIME))
    idx=$((idx + 1))
done
echo ""

# --- Run Demucs ---
echo ">>> Running Demucs ($MODEL) on all chunks..."
cd "$WORKDIR"
demucs -n "$MODEL" --two-stems=vocals --float32 -d cuda chunks/chunk_*.wav
cd - >/dev/null

SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
[ -z "$SEPDIR" ] && { echo "Error: Could not find Demucs output"; exit 1; }

# --- Stitch function: concatenates all chunks in 32-bit float mono ---
stitch_stem() {
    local stem="$1"
    echo ">>> Stitching stem: $stem"

    local chunk_files=()
    for f in "${CHUNKS_DIR}"/chunk_*.wav; do
        name=$(basename "$f" .wav)
        stem_file="${SEPDIR}/${name}/${stem}.wav"
        [ -f "$stem_file" ] && chunk_files+=("$stem_file")
    done

    [ ${#chunk_files[@]} -eq 0 ] && { echo "Error: No $stem files found"; return 1; }

    # Create a list file for ffmpeg concat
    concat_list="${OUTDIR}/${stem}_concat.txt"
    rm -f "$concat_list"
    for cf in "${chunk_files[@]}"; do
        echo "file '$cf'" >> "$concat_list"
    done

    # Concatenate all chunks as 32-bit float mono
    ffmpeg -hide_banner -loglevel error -f concat -safe 0 -i "$concat_list" \
        -ac 1 -c:a pcm_f32le "${OUTDIR}/${stem}.wav"

    # Optional: crossfade at boundaries (if enabled)
    if [ "$APPLY_CROSSFADE" = true ]; then
        tmp="${OUTDIR}/${stem}_xf.wav"
        ffmpeg -hide_banner -loglevel error -y -i "${OUTDIR}/${stem}.wav}" \
            -af "acrossfade=d=${CROSSFADE_DURATION}:c1=tri:c2=tri" \
            -ac 1 -c:a pcm_f32le "$tmp"
        mv "$tmp" "${OUTDIR}/${stem}.wav"
    fi
}

# --- Process all stems ---
for stem in "${STEMS[@]}"; do
    stitch_stem "$stem"
done

# --- Combine non-vocals ---
echo ">>> Combining non-vocal stems..."
non_vocals=()
for stem in "${STEMS[@]}"; do
    if [ "$stem" != "vocals" ] && [ -f "${OUTDIR}/${stem}.wav" ]; then
        non_vocals+=("${OUTDIR}/${stem}.wav")
    fi
done

if [ ${#non_vocals[@]} -gt 0 ]; then
    input_args=()
    for f in "${non_vocals[@]}"; do input_args+=("-i" "$f"); done

    ffmpeg -hide_banner -loglevel error -y "${input_args[@]}" \
        -filter_complex "amix=inputs=${#non_vocals[@]}:duration=longest[out]" \
        -map "[out]" -ac 1 -c:a pcm_f32le "${INPUT_DIR}/${BASENAME}_nonvocals.wav"

    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
fi

# --- Move vocals ---
if [ -f "${OUTDIR}/vocals.wav" ]; then
    mv "${OUTDIR}/vocals.wav" "${INPUT_DIR}/${BASENAME}_vocals.wav"
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_vocals.wav"
fi

# --- Cleanup ---
rm -rf "$WORKDIR"
echo "✓ Cleanup done"

echo ">>> All done!"
echo "Final files in: $INPUT_DIR"
echo "  - ${BASENAME}_vocals.wav"
[ -f "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] && echo "  - ${BASENAME}_nonvocals.wav"
