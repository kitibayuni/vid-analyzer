#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"        # or mdx_extra_q / mdx_q
SEGMENT_TIME=600        # seconds per chunk (10 minutes)
OVERLAP=2               # seconds overlap between chunks
STEMS=("vocals" "no_vocals")
# ----------------

# Advanced progress bar function
progress_bar() {
    local current=$1 total=$2 task_name="${3:-Processing}" width=40
    local percentage=$((current*100/total))
    local filled=$((current*width/total))

    # Spinner characters
    local spinner="/-\\|"
    local spin_char=${spinner:$((current % ${#spinner})):1}

    # Build the bar
    local bar=""
    for ((i=0;i<filled;i++)); do bar+="█"; done
    for ((i=filled;i<width;i++)); do bar+="░"; done

    printf "\r%s [%s] %d%% (%d/%d) %s" "$spin_char" "$bar" "$percentage" "$current" "$total" "$task_name"
}

# --- INPUT ---
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

    ffmpeg -hide_banner -loglevel error -y -i "$INPUT" \
        -af "atrim=start=${start}:end=${end},asetpts=PTS-STARTPTS" "$out"

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

# --- Stitch function ---
stitch_stem () {
    local stem="$1"
    echo ">>> Stitching stem: $stem"

    local inputs=()
    for f in "${CHUNKS_DIR}"/chunk_*.wav; do
        name=$(basename "$f" .wav)
        stem_file="${SEPDIR}/${name}/${stem}.wav"
        [ -f "$stem_file" ] && inputs+=("$stem_file")
    done

    [ ${#inputs[@]} -eq 0 ] && { echo "Error: No $stem files found"; return 1; }

    tmp="${OUTDIR}/${stem}_tmp0.wav"
    ffmpeg -hide_banner -loglevel error -y -i "${inputs[0]}" -ac 1 -c:a pcm_f32le "$tmp"

    total_ops=$((${#inputs[@]} - 1))
    for ((i=1; i<${#inputs[@]}; i++)); do
        next="${inputs[i]}"
        outtmp="${OUTDIR}/${stem}_tmp${i}.wav"
        progress_bar $i $total_ops "Crossfading $stem"
        ffmpeg -hide_banner -loglevel error -y -i "$tmp" -i "$next" \
            -filter_complex "acrossfade=d=${OVERLAP}:c1=tri:c2=tri" \
            -ac 1 -c:a pcm_f32le "$outtmp"
        rm -f "$tmp"
        tmp="$outtmp"
    done
    echo ""
    mv "$tmp" "${OUTDIR}/${stem}.wav"
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
    # Build ffmpeg input arguments
    input_args=()
    for f in "${non_vocals[@]}"; do
        input_args+=("-i" "$f")
    done

    # Filter complex for amix
    ffmpeg -hide_banner -loglevel error -y \
        "${input_args[@]}" \
        -filter_complex "amix=inputs=${#non_vocals[@]}:duration=longest[out]" \
        -map "[out]" -ac 1 -c:a pcm_f32le "${OUTDIR}/${BASENAME}_nonvocals.wav"

    echo "✓ Created: ${OUTDIR}/${BASENAME}_nonvocals.wav"
fi


# --- Cleanup ---
rm -rf "$WORKDIR"
echo "✓ Cleanup done"

echo ">>> All done!"
echo "Final files in: $INPUT_DIR"
echo "  - ${BASENAME}_vocals.wav"
[ -f "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] && echo "  - ${BASENAME}_nonvocals.wav"
