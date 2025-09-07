#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"
SEGMENT_TIME=600
OVERLAP=2
STEMS=("vocals" "no_vocals")
APPLY_CROSSFADE=false
CROSSFADE_DURATION=1
# ----------------

# --- Progress bar ---
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

# --- Split input into overlapping chunks ---
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
        -af "atrim=start=${start}:end=${end},asetpts=PTS-STARTPTS" \
        -ac 1 -c:a pcm_f32le "$out"

    progress_bar $((idx+1)) $TOTAL_CHUNKS "Splitting chunks"
    start=$((start + SEGMENT_TIME))
    idx=$((idx + 1))
done
echo ""

# --- Verify all chunks exist ---
chunk_count=$(ls -1 "${CHUNKS_DIR}"/chunk_*.wav 2>/dev/null | wc -l)
if [ "$chunk_count" -lt "$TOTAL_CHUNKS" ]; then
    echo "Error: Only $chunk_count out of $TOTAL_CHUNKS chunks exist. Aborting."
    exit 1
fi

# --- Run Demucs ---
echo ">>> Running Demucs ($MODEL) on all chunks..."
cd "$WORKDIR"
demucs -n "$MODEL" --two-stems=vocals --float32 -d cuda chunks/chunk_*.wav
cd - >/dev/null

SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
[ -z "$SEPDIR" ] && { echo "Error: Could not find Demucs output"; exit 1; }

# --- Overlap-aware stitching ---
stitch_stem() {
    local stem="$1"
    echo ">>> Stitching stem with crossfade: $stem"

    local chunk_files=()
    for f in "${CHUNKS_DIR}"/chunk_*.wav; do
        name=$(basename "$f" .wav)
        stem_file="${SEPDIR}/${name}/${stem}.wav"
        if [ -f "$stem_file" ] && [ -s "$stem_file" ]; then
            chunk_files+=("$(realpath "$stem_file")")
        else
            echo "Warning: Missing or empty $stem for chunk $name"
        fi
    done

    if [ ${#chunk_files[@]} -eq 0 ]; then
        echo "Error: No valid $stem files found. Skipping stitching."
        return 1
    fi

    local stitched="${OUTDIR}/${stem}.wav"
    cp "${chunk_files[0]}" "$stitched"

    for ((i=1;i<${#chunk_files[@]};i++)); do
        tmp="${OUTDIR}/tmp_${stem}_${i}.wav"
        ffmpeg -y -hide_banner -loglevel error -i "$stitched" -i "${chunk_files[i]}" \
            -filter_complex "acrossfade=d=${CROSSFADE_DURATION}:c1=tri:c2=tri" \
            -ac 1 -c:a pcm_f32le "$tmp"  # <-- FORCE MONO HERE
        mv "$tmp" "$stitched"
    done

    [ -s "$stitched" ] || { echo "Error: stitching failed for $stem"; return 1; }
    echo "✓ Completed stitching: $stem"
}

# --- Process all stems ---
for stem in "${STEMS[@]}"; do
    stitch_stem "$stem"
done

# --- Combine non-vocals ---
combine_nonvocals() {
    local non_vocals=()
    for stem in "${STEMS[@]}"; do
        if [ "$stem" != "vocals" ] && [ -f "${OUTDIR}/${stem}.wav" ] && [ -s "${OUTDIR}/${stem}.wav" ]; then
            non_vocals+=("$(realpath "${OUTDIR}/${stem}.wav")")
        fi
    done

    if [ ${#non_vocals[@]} -eq 0 ]; then
        echo "Warning: No non-vocal stems found. Skipping combination."
        return
    fi

    local input_args=()
    for f in "${non_vocals[@]}"; do
        input_args+=("-i" "$f")
    done

    ffmpeg -y -hide_banner -loglevel error "${input_args[@]}" \
        -filter_complex "amix=inputs=${#non_vocals[@]}:duration=longest[out]" \
        -map "[out]" -ac 1 -c:a pcm_f32le "${INPUT_DIR}/${BASENAME}_nonvocals.wav"  # <-- FORCE MONO HERE

    [ -s "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] || { echo "Error: Non-vocal combination failed"; return 1; }
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
}

combine_nonvocals

# --- Move vocals ---
if [ -f "${OUTDIR}/vocals.wav" ] && [ -s "${OUTDIR}/vocals.wav" ]; then
    mv "${OUTDIR}/vocals.wav" "${INPUT_DIR}/${BASENAME}_vocals.wav"
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_vocals.wav"
else
    echo "Warning: Vocals file missing or empty."
fi

# --- Cleanup ---
rm -rf "$WORKDIR"
echo "✓ Cleanup done"

echo ">>> All done!"
echo "Final files in: $INPUT_DIR"
[ -f "${INPUT_DIR}/${BASENAME}_vocals.wav" ] && echo "  - ${BASENAME}_vocals.wav"
[ -f "${INPUT_DIR}/${BASENAME}_nonvocals.wav" ] && echo "  - ${BASENAME}_nonvocals.wav"
