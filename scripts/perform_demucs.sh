#!/usr/bin/env bash
set -euo pipefail

# --- CONFIG ---
MODEL="htdemucs"            # Demucs model: htdemucs, mdx_extra_q, etc.
SEGMENT_TIME=600            # seconds per chunk (10 minutes)
OVERLAP=2                   # seconds overlap between chunks
STEMS=("vocals" "no_vocals") # two-stems
# ----------------

# --- Advanced progress bar ---
progress_bar() {
    local current=$1 total=$2 task_name="${3:-Processing}" width=40
    local percentage=$((current*100/total))
    local filled=$((current*width/total))
    local spinner="/-\|" spin_char=${spinner:$((current%4)):1}
    local bar=""
    for ((i=0;i<filled;i++)); do bar+="█"; done
    for ((i=filled;i<width;i++)); do bar+="░"; done
    printf "\r%s [%s] %d%% (%d/%d) %s" "$spin_char" "$bar" "$percentage" "$current" "$total" "$task_name"
}

# --- Check input ---
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

# --- Split audio into chunks ---
DURATION=$(ffprobe -v error -show_entries format=duration \
    -of default=noprint_wrappers=1:nokey=1 "$INPUT")
DURATION=${DURATION%.*}
echo ">>> Input length: ${DURATION}s"
TOTAL_CHUNKS=$(( (DURATION + SEGMENT_TIME - 1)/SEGMENT_TIME ))

start=0 idx=0
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

# --- Run Demucs on all chunks ---
echo ">>> Running Demucs ($MODEL) on all chunks..."
cd "$WORKDIR"
demucs -n "$MODEL" --two-stems=vocals --float32 -d cuda chunks/chunk_*.wav
cd - >/dev/null

# --- Locate Demucs output ---
SEPDIR=$(find "${WORKDIR}/separated" -maxdepth 1 -type d -name "${MODEL}*" | head -n1)
[ -z "$SEPDIR" ] && { echo "Error: Could not find Demucs output"; exit 1; }

# --- Collect all chunk files for each stem ---
declare -A STEM_CHUNKS
for stem in "${STEMS[@]}"; do
    STEM_CHUNKS["$stem"]=( )
done

for chunk_dir in "${SEPDIR}"/chunk_*; do
    for stem in "${STEMS[@]}"; do
        stem_file="${chunk_dir}/${stem}.wav"
        [ -f "$stem_file" ] && STEM_CHUNKS["$stem"]+=( "$stem_file" )
    done
done

# --- Stitch function with crossfade ---
stitch_stem() {
    local stem="$1"
    local inputs=( "${STEM_CHUNKS[$stem][@]}" )
    [ ${#inputs[@]} -eq 0 ] && { echo "Error: No $stem files found"; return 1; }

    echo ">>> Stitching stem: $stem"
    if [ ${#inputs[@]} -eq 1 ]; then
        ffmpeg -hide_banner -loglevel error -y -i "${inputs[0]}" -ac 1 -c:a pcm_f32le "${OUTDIR}/${stem}.wav"
        return
    fi

    tmp="${OUTDIR}/${stem}_tmp0.wav"
    ffmpeg -hide_banner -loglevel error -y -i "${inputs[0]}" -ac 1 -c:a pcm_f32le "$tmp"

    total_ops=$((${#inputs[@]}-1))
    for ((i=1;i<${#inputs[@]};i++)); do
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

# --- Combine all non-vocal stems ---
echo ">>> Combining non-vocal stems..."
non_vocals=()
for stem in "${STEMS[@]}"; do
    [ "$stem" != "vocals" ] && [ -f "${OUTDIR}/${stem}.wav" ] && non_vocals+=("${OUTDIR}/${stem}.wav")
done

if [ ${#non_vocals[@]} -gt 0 ]; then
    ffmpeg -hide_banner -loglevel error -y $(printf -- '-i %q ' "${non_vocals[@]}") \
        -filter_complex "amix=inputs=${#non_vocals[@]}:duration=longest[out]" \
        -map "[out]" -ac 1 -c:a pcm_f32le "${INPUT_DIR}/${BASENAME}_nonvocals.wav"
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_nonvocals.wav"
fi

# --- Move vocals ---
[ -f "${OUTDIR}/vocals.wav" ] && mv "${OUTDIR}/vocals.wav" "${INPUT_DIR}/${BASENAME}_vocals.wav" && \
    echo "✓ Created: ${INPUT_DIR}/${BASENAME}_vocals.wav"

# --- Cleanup ---
rm -rf "$WORKDIR"
echo "✓ Cleanup done"
echo ">>> All done! Final files in $INPUT_DIR"
