#!/bin/bash
# usage: ./extract_tracks.sh input.mp4
# requires: ffmpeg, ffprobe, ./perform_demucs.sh, ./formants_process, ./rms_energy_process, ./pitch_process, ./process_transcript.py
# requires: ./pre-process_emotions, ./process_emotion.py, ./spectrals_process, ./salience_process, ./transcript_process

# --- Logging setup ---
setup_logging() {
    local run_counter=1
    while [ -f "run${run_counter}.log" ]; do
        ((run_counter++))
    done
    LOG_FILE="run${run_counter}.log"
    SCRIPT_START_TIME=$(date +%s.%N)
    SCRIPT_START_DATE=$(date)
    
    echo "=== MEDIA TRACK EXTRACTOR LOG ===" > "$LOG_FILE"
    echo "Run: $run_counter" >> "$LOG_FILE"
    echo "Started: $SCRIPT_START_DATE" >> "$LOG_FILE"
    echo "Input: $INPUT" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"
}

# --- Logging functions ---
log_message() {
    local message="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $message" | tee -a "$LOG_FILE"
}

log_timing() {
    local step_name="$1"
    local start_time="$2"
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc -l)
    log_message "TIMING: $step_name completed in ${duration}s"
}

log_file_info() {
    local file_path="$1"
    local label="$2"
    if [ -f "$file_path" ]; then
        local size=$(stat -f%z "$file_path" 2>/dev/null || stat -c%s "$file_path" 2>/dev/null || echo "unknown")
        local duration=""
        
        # Try to get duration for media files
        if [[ "$file_path" =~ \.(mp4|mov|avi|mkv|wav|mp3|flac)$ ]]; then
            duration=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file_path" 2>/dev/null)
            if [ -n "$duration" ] && [ "$duration" != "N/A" ]; then
                duration=" (${duration}s duration)"
            else
                duration=""
            fi
        fi
        
        log_message "FILE INFO: $label - Size: ${size} bytes${duration}"
    fi
}

log_storage_usage() {
    local label="$1"
    if [ -d "$OUTDIR" ]; then
        local total_size=$(find "$OUTDIR" -type f -name "${BASENAME}*" -exec stat -f%z {} \; 2>/dev/null | awk '{sum += $1} END {print sum}')
        if [ -z "$total_size" ]; then
            total_size=$(find "$OUTDIR" -type f -name "${BASENAME}*" -exec stat -c%s {} \; 2>/dev/null | awk '{sum += $1} END {print sum}')
        fi
        total_size=${total_size:-0}
        
        # Track maximum storage usage
        if [ "$total_size" -gt "${MAX_STORAGE:-0}" ]; then
            MAX_STORAGE=$total_size
        fi
        
        log_message "STORAGE: $label - Current: ${total_size} bytes, Peak: ${MAX_STORAGE} bytes"
    fi
}

# --- Dependency validation ---
validate_dependencies() {
    local missing=""
    for cmd in ffmpeg ffprobe bc python; do
        command -v "$cmd" >/dev/null || missing="$missing $cmd"
    done
    if [ -n "$missing" ]; then
        log_message "ERROR: Missing required dependencies:$missing"
        echo "ERROR: Missing required dependencies:$missing"
        echo "Please install the missing tools and try again."
        exit 1
    fi
}

# --- Cleanup function for error handling ---
cleanup_temp_files() {
    local pattern="$1"
    if [ -n "$pattern" ] && [ -d "$OUTDIR" ]; then
        find "$OUTDIR" -name "$pattern" -type f -delete 2>/dev/null || true
    fi
}

# --- Trap for cleanup on exit ---
trap 'cleanup_temp_files "${BASENAME}_*_16k_16bit.wav"; cleanup_temp_files "${BASENAME}_*_44k_32bit.wav"; cleanup_temp_files "${BASENAME}_*_22k_16bit.wav"' EXIT

# --- Source virtual environment ---
source venv/bin/activate

ARG="$1"
INPUT="$(dirname "$0")/$ARG"
BASENAME=$(basename "$INPUT" | sed 's/\.[^.]*$//')
OUTDIR="$(dirname "$0")/../temp"
MAX_STORAGE=0

# Initialize logging
setup_logging

echo "=== Media Track Extractor - Enhanced with Differentiated Processing + Merging ==="
echo "Script started at: $SCRIPT_START_DATE"
echo "Logging to: $LOG_FILE"
echo ""

log_message "=== INITIALIZATION ==="
init_start=$(date +%s.%N)

# --- Validate dependencies before proceeding ---
validate_dependencies

if [ -z "$INPUT" ]; then
    log_message "ERROR: No input file provided!"
    echo "ERROR: No input file provided!"
    echo "Usage: $0 <input_video>"
    exit 1
fi

if [ ! -f "$INPUT" ]; then
    log_message "ERROR: Input file not found: $INPUT"
    echo "ERROR: Input file not found: $INPUT"
    exit 1
fi

if [ ! -d "$OUTDIR" ]; then
    log_message "ERROR: Output directory not found: $OUTDIR"
    echo "ERROR: Output directory not found: $OUTDIR"
    echo "Please create the directory first."
    exit 1
fi

# Log input file information
log_file_info "$INPUT" "Input file"
echo "✓ Input file exists"
echo "✓ Output directory exists"
echo ""

log_timing "Initialization" $init_start

# --- Helper function to run process_feature ---
run_process() {
    local infile="$1"
    local outbase="$2"
    local feature="$3"
    local process_start=$(date +%s.%N)

    case "$feature" in
        rms)       ./process_feature --rms-in "$infile" --rms-out "${outbase}_rms.csv" ;;
        pitch)     ./process_feature --pitch-in "$infile" --pitch-out "${outbase}_pitch.csv" ;;
        spectral)  ./process_feature --spectral-in "$infile" --spectral-out "${outbase}_spectral.csv" ;;
        zcr)       ./process_feature --zcr-in "$infile" --zcr-out "${outbase}_zcr.csv" ;;
        formant)   ./process_feature --formant-in "$infile" --formant-out "${outbase}_formant.csv" ;;
        jitter)    ./process_feature --jitter-in "$infile" --jitter-out "${outbase}_jitter.csv" ;;
        *) log_message "Unknown feature: $feature" ;;
    esac
    
    log_timing "Feature extraction ($feature)" $process_start
}

# --- Audio track extraction ---
echo "=== AUDIO TRACK PROCESSING ==="
log_message "=== AUDIO TRACK PROCESSING START ==="
audio_start=$(date +%s.%N)

FFPROBE_OUTPUT=$(ffprobe -v error -select_streams a \
    -show_entries stream=index -of csv=p=0 "$INPUT")

# --- Fix: Handle empty audio stream output correctly ---
if [ -z "$FFPROBE_OUTPUT" ] || [ "$FFPROBE_OUTPUT" = "" ]; then
    NUM_AUDIO_STREAMS=0
else
    NUM_AUDIO_STREAMS=$(echo "$FFPROBE_OUTPUT" | wc -l)
fi

log_message "Found $NUM_AUDIO_STREAMS audio stream(s)"
echo "DEBUG: ffprobe output: '$FFPROBE_OUTPUT'"
echo "DEBUG: wc -l result: '$NUM_AUDIO_STREAMS'"
echo "Found $NUM_AUDIO_STREAMS audio stream(s)"
echo ""

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    log_message "No audio streams found - skipping audio processing"
    echo "No audio streams found - skipping audio processing"
else
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        stream_start=$(date +%s.%N)
        log_message "=== Processing Audio Stream $STREAM_INDEX ==="
        echo "--- Processing Audio Stream $STREAM_INDEX ---"
        OUTFILE="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}.wav"

        # --- Silence detection ---
        silence_start=$(date +%s.%N)
        echo "Detecting silence..."
        log_message "Starting silence detection for stream $STREAM_INDEX"
        
        SILENCE_OUTPUT=$(ffmpeg -i "$INPUT" -map 0:a:$STREAM_INDEX \
            -af silencedetect=noise=-50dB:d=1 -f null - 2>&1)

        TOTAL_SILENCE=0
        START=""
        SILENCE_COUNT=0
        while read -r LINE; do
            if [[ $LINE =~ silence_start ]]; then
                START=$(echo "$LINE" | awk '{print $5}')
            elif [[ $LINE =~ silence_end ]] && [[ -n "$START" ]]; then
                END=$(echo "$LINE" | awk '{print $5}')
                # --- Fix: Better error handling for mathematical operations ---
                if [[ -n "$END" ]] && [[ -n "$START" ]]; then
                    SILENCE_DURATION=$(echo "$END - $START" | bc 2>/dev/null)
                    if [[ -n "$SILENCE_DURATION" ]] && [[ "$SILENCE_DURATION" != "" ]]; then
                        TOTAL_SILENCE=$(echo "$TOTAL_SILENCE + $SILENCE_DURATION" | bc 2>/dev/null || echo "$TOTAL_SILENCE")
                        SILENCE_COUNT=$((SILENCE_COUNT + 1))
                    fi
                fi
                START=""
            fi
        done <<< "$SILENCE_OUTPUT"

        DURATION=$(ffprobe -v error -select_streams a:$STREAM_INDEX \
            -show_entries stream=duration -of csv=p=0 "$INPUT")
        DURATION=${DURATION%.*}
        DURATION=${DURATION:-1}
        SILENCE_FRAC=$(echo "$TOTAL_SILENCE / $DURATION" | bc -l)
        SILENCE_FRAC=${SILENCE_FRAC:-0}

        log_timing "Silence detection (stream $STREAM_INDEX)" $silence_start
        log_message "Silence analysis: ${SILENCE_COUNT} segments, $(echo "$SILENCE_FRAC * 100" | bc -l | cut -d. -f1)% of ${DURATION}s total"
        echo "Silence: ${SILENCE_COUNT} segments, $(echo "$SILENCE_FRAC * 100" | bc -l | cut -d. -f1)%"

        if (( $(echo "$SILENCE_FRAC < 0.9" | bc -l) )); then
            # Audio extraction
            extract_start=$(date +%s.%N)
            echo "Exporting non-silent audio stream to $OUTFILE"
            log_message "Extracting audio stream $STREAM_INDEX"
            
            ffmpeg -y -i "$INPUT" -map 0:a:$STREAM_INDEX -ar 48000 -ac 1 -c:a pcm_f32le "$OUTFILE" || continue
            log_timing "Audio extraction (stream $STREAM_INDEX)" $extract_start
            log_file_info "$OUTFILE" "Extracted audio stream $STREAM_INDEX"
            log_storage_usage "After audio extraction (stream $STREAM_INDEX)"

            # --- Demucs processing ---
            demucs_start=$(date +%s.%N)
            echo "Running Demucs on $OUTFILE..."
            log_message "Starting Demucs processing for stream $STREAM_INDEX"
            
            if ! ./perform_demucs.sh "$OUTFILE"; then
                log_message "ERROR: Demucs processing failed for $OUTFILE"
                echo "✗ Demucs processing failed for $OUTFILE"
                continue
            fi
            
            log_timing "Demucs processing (stream $STREAM_INDEX)" $demucs_start
            log_storage_usage "After Demucs (stream $STREAM_INDEX)"

            DEMUCS_FILES=("${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_vocals.wav" \
                          "${OUTDIR}/${BASENAME}_audio${STREAM_INDEX}_nonvocals.wav")

            # --- Fix: Validate both Demucs outputs exist ---
            MISSING_DEMUCS=""
            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                [ ! -f "$DEMUCS_FILE" ] && MISSING_DEMUCS="$MISSING_DEMUCS $(basename "$DEMUCS_FILE")"
            done
            
            if [ -n "$MISSING_DEMUCS" ]; then
                log_message "ERROR: Missing Demucs outputs:$MISSING_DEMUCS - skipping stream $STREAM_INDEX"
                echo "✗ Missing Demucs outputs:$MISSING_DEMUCS - skipping this audio stream"
                continue
            fi

            # Log Demucs output files
            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                log_file_info "$DEMUCS_FILE" "Demucs output: $(basename "$DEMUCS_FILE")"
            done

            for DEMUCS_FILE in "${DEMUCS_FILES[@]}"; do
                processing_start=$(date +%s.%N)
                BASE_DEMUCS=$(basename "$DEMUCS_FILE" .wav)
                echo "Creating 3 variants for $DEMUCS_FILE"
                log_message "Processing $BASE_DEMUCS - creating variants and features"

                # --- Create audio variants ---
                variant_start=$(date +%s.%N)
                OUT16="$OUTDIR/${BASE_DEMUCS}_16k_16bit.wav"
                OUT44="$OUTDIR/${BASE_DEMUCS}_44k_32bit.wav"
                OUT22="$OUTDIR/${BASE_DEMUCS}_22k_16bit.wav"

                # --- Fix: Validate ffmpeg conversions ---
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 16000 -c:a pcm_s16le "$OUT16" 2>/dev/null; then
                    log_message "ERROR: Failed to create 16kHz variant for $DEMUCS_FILE"
                    echo "✗ Failed to create 16kHz variant for $DEMUCS_FILE"
                    continue
                fi
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 44100 -c:a pcm_f32le "$OUT44" 2>/dev/null; then
                    log_message "ERROR: Failed to create 44kHz variant for $DEMUCS_FILE"
                    echo "✗ Failed to create 44kHz variant for $DEMUCS_FILE"
                    rm -f "$OUT16"
                    continue
                fi
                if ! ffmpeg -y -i "$DEMUCS_FILE" -ar 22050 -c:a pcm_s16le "$OUT22" 2>/dev/null; then
                    log_message "ERROR: Failed to create 22kHz variant for $DEMUCS_FILE"
                    echo "✗ Failed to create 22kHz variant for $DEMUCS_FILE"
                    rm -f "$OUT16" "$OUT44"
                    continue
                fi

                log_timing "Audio variants creation ($BASE_DEMUCS)" $variant_start
                echo "✓ Variants created for $DEMUCS_FILE"

                # --- DIFFERENTIATED PROCESSING: VOCALS vs NONVOCALS ---
                if [[ "$BASE_DEMUCS" == *"_vocals" ]]; then
                    vocals_processing_start=$(date +%s.%N)
                    echo "Processing features for $BASE_DEMUCS (vocals - full processing)..."
                    log_message "Starting vocal processing for $BASE_DEMUCS"

                    # --- Generate all vocal-specific CSV features ---
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" pitch
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral
                    run_process "$OUT22" "$OUTDIR/${BASE_DEMUCS}" formant
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" jitter

                    # --- Post-process vocal CSVs ---
                    postprocess_start=$(date +%s.%N)
                    for CSV in rms pitch formant spectral zcr jitter; do
                        CSV_FILE="$OUTDIR/${BASE_DEMUCS}_${CSV}.csv"
                        [ -f "$CSV_FILE" ] && {
                            case $CSV in
                                rms)       ./rms_energy_process "$CSV_FILE" --time_col time_sec ;;
                                pitch)     ./pitch_process "$CSV_FILE" --time_col time_sec ;;
                                formant)   FORMANT_PROCESSED="$OUTDIR/${BASE_DEMUCS}_formant_processed.csv"; ./formants_process "$CSV_FILE" "$FORMANT_PROCESSED" ;;
                                spectral)  ./spectrals_process "$CSV_FILE" ;;
                                zcr)       ./zcr_process "$CSV_FILE" --time_col time_sec ;;
                                jitter)    echo "Jitter CSV post-process if needed" ;;
                            esac
                            rm -f "$CSV_FILE"
                        }
                    done
                    log_timing "CSV post-processing ($BASE_DEMUCS)" $postprocess_start

                    # --- Vocal-specific: Emotion processing ---
                    emotion_start=$(date +%s.%N)
                    echo "Processing emotions for $BASE_DEMUCS..."
                    log_message "Starting emotion processing for $BASE_DEMUCS"
                    
                    EMOTION_NPY="$OUTDIR/${BASE_DEMUCS}_emotion.npy"
                    EMOTION_CSV="$OUTDIR/${BASE_DEMUCS}_emotion_processed.csv"
                    ./pre-process_emotions "$OUT16" "$EMOTION_NPY" 16000
                    if [ -f "$EMOTION_NPY" ]; then
                        python process_emotion.py "$EMOTION_NPY" "$EMOTION_CSV" --chunk_sec 5 \
                            --msp_model_path "./models/wav2vec2-large-robust-12-ft-emotion-msp-dim" \
                            --device "cuda"
                        rm -f "$EMOTION_NPY"
                        if [ -f "$EMOTION_CSV" ]; then
                            EMOTION_FINAL="$OUTDIR/${BASE_DEMUCS}_emotion_final.csv"
                            ./post-process_emotion "$EMOTION_CSV" "$EMOTION_FINAL"
                            rm -f "$EMOTION_CSV"
                        fi
                    fi
                    log_timing "Emotion processing ($BASE_DEMUCS)" $emotion_start

                    # --- Vocal-specific: Transcript processing ---
                    transcript_start=$(date +%s.%N)
                    echo "Processing transcript for $BASE_DEMUCS..."
                    log_message "Starting transcript processing for $BASE_DEMUCS"
                    
                    TRANSCRIPT_OUT="$OUTDIR/${BASE_DEMUCS}_transcript.csv"
                    python process_transcript.py "$OUT16" --output "$TRANSCRIPT_OUT"
                    if [ -f "$TRANSCRIPT_OUT" ]; then
                        ./transcript_process "$TRANSCRIPT_OUT" \
                            lexicons/NRC-Emo-Lex-v0.92.csv \
                            lexicons/NRC-VAD-Lex-v2.1.csv \
                            lexicons/NRC-WCST-Lex-v1.0.csv \
                            lexicons/WW-Lex-v1.csv \
                            lexicons/swears.txt
                        rm -f "$TRANSCRIPT_OUT"
                    fi
                    log_timing "Transcript processing ($BASE_DEMUCS)" $transcript_start
                    log_timing "Complete vocal processing ($BASE_DEMUCS)" $vocals_processing_start
                    
                else
                    # --- NONVOCALS: Limited processing (no speech-specific features) ---
                    nonvocals_processing_start=$(date +%s.%N)
                    echo "Processing features for $BASE_DEMUCS (nonvocals - limited processing)..."
                    log_message "Starting nonvocal processing for $BASE_DEMUCS"

                    # --- Generate only relevant nonvocal CSV features ---
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" rms
                    run_process "$OUT16" "$OUTDIR/${BASE_DEMUCS}" zcr
                    run_process "$OUT44" "$OUTDIR/${BASE_DEMUCS}" spectral

                    # --- Post-process nonvocal CSVs ---
                    RMS_CSV="$OUTDIR/${BASE_DEMUCS}_rms.csv"
                    if [ -f "$RMS_CSV" ]; then
                        ./rms_energy_process "$RMS_CSV" --time_col time_sec
                        rm -f "$RMS_CSV"
                    fi

                    ZCR_CSV="$OUTDIR/${BASE_DEMUCS}_zcr.csv"
                    if [ -f "$ZCR_CSV" ]; then
                        ./zcr_process "$ZCR_CSV" --time_col time_sec
                        rm -f "$ZCR_CSV"
                    fi
                    
                    SPECTRAL_CSV="$OUTDIR/${BASE_DEMUCS}_spectral.csv"
                    if [ -f "$SPECTRAL_CSV" ]; then
                        ./spectrals_process "$SPECTRAL_CSV"
                        rm -f "$SPECTRAL_CSV"
                    fi

                    log_timing "Complete nonvocal processing ($BASE_DEMUCS)" $nonvocals_processing_start
                    echo "✓ Nonvocals processing complete (skipped: pitch, formant, jitter, emotion, transcript)"
                fi

                log_timing "Complete processing ($BASE_DEMUCS)" $processing_start
                log_storage_usage "After processing $BASE_DEMUCS"

                # --- Clean up WAV variants ---
                rm -f "$DEMUCS_FILE" "$OUT16" "$OUT22" "$OUT44"
            done

            rm -f "$OUTFILE"
        else
            log_message "Skipping mostly silent audio stream $STREAM_INDEX (${SILENCE_COUNT} silence segments)"
            echo "Skipping mostly silent audio stream $STREAM_INDEX"
        fi
        
        log_timing "Complete audio stream $STREAM_INDEX processing" $stream_start
        echo ""
    done
fi

log_timing "All audio processing" $audio_start

echo "=== MERGING PROCESSED CSVs BY AUDIO GROUP ==="
log_message "=== CSV MERGING START ==="
merge_start=$(date +%s.%N)

if [ "$NUM_AUDIO_STREAMS" -eq 0 ]; then
    NUM_AUDIO_STREAMS=1   # force at least one loop to run
fi

for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
    for TYPE in vocals nonvocals; do
        merge_step_start=$(date +%s.%N)
        GROUP_PREFIX="${BASENAME}_audio${STREAM_INDEX}_${TYPE}"
        CSV_FILES=$(ls "$OUTDIR/${GROUP_PREFIX}"_*.csv 2>/dev/null)
        [ -z "$CSV_FILES" ] && { 
            log_message "No CSVs to merge for $GROUP_PREFIX"
            echo "No CSVs to merge for $GROUP_PREFIX"; 
            continue; 
        }
        MERGED_OUT="$OUTDIR/${GROUP_PREFIX}_merged.csv"
        echo "Merging CSVs for $GROUP_PREFIX into $MERGED_OUT"
        log_message "Merging CSVs for $GROUP_PREFIX"
        
        if ./compile_data time_sec "$MERGED_OUT" $CSV_FILES 2>/dev/null; then
            rm -f $CSV_FILES
            log_timing "CSV merge ($GROUP_PREFIX)" $merge_step_start
            log_file_info "$MERGED_OUT" "Merged CSV: $GROUP_PREFIX"
            echo "✓ Merge complete for $GROUP_PREFIX"
        else
            log_message "ERROR: Merge failed for $GROUP_PREFIX - keeping individual files"
            echo "✗ Merge failed for $GROUP_PREFIX - keeping individual files"
            echo "  Source files preserved: $CSV_FILES"
        fi
    done
done

log_timing "All CSV merging" $merge_start
log_storage_usage "After CSV merging"

# --- Video tracks (RE-ENCODED) ---
echo "=== VIDEO TRACK PROCESSING ==="
log_message "=== VIDEO PROCESSING START ==="
video_start=$(date +%s.%N)

NUM_VIDEO_STREAMS=$(ffprobe -v error -select_streams v \
    -show_entries stream=index -of csv=p=0 "$INPUT" | wc -l)
log_message "Found $NUM_VIDEO_STREAMS video stream(s)"
echo "Found $NUM_VIDEO_STREAMS video stream(s)"
echo ""

if [ "$NUM_VIDEO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        video_stream_start=$(date +%s.%N)
        echo "--- Processing Video Stream $STREAM_INDEX ---"
        log_message "Processing Video Stream $STREAM_INDEX"
        
        OUTFILE="$OUTDIR/${BASENAME}_video${STREAM_INDEX}.mp4"
        SALIENCE_OUT="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience.csv"

        # Video encoding
        encode_start=$(date +%s.%N)
        ffmpeg -y -i "$INPUT" -map 0:v:$STREAM_INDEX -an -vf "fps=15,scale=224:224" \
            -c:v libx264 -preset fast -crf 16 "$OUTFILE"
        log_timing "Video encoding (stream $STREAM_INDEX)" $encode_start
        log_file_info "$OUTFILE" "Encoded video stream $STREAM_INDEX"
        echo "✓ Video stream re-encoded to $OUTFILE"

        # DeepGaze processing
        deepgaze_start=$(date +%s.%N)
        echo "Running DeepGaze on $OUTFILE -> $SALIENCE_OUT"
        log_message "Starting DeepGaze processing for video stream $STREAM_INDEX"
        
        if python deepgaze.py "$OUTFILE" "$SALIENCE_OUT" 2>/dev/null; then
            if [ -f "$SALIENCE_OUT" ]; then
                salience_start=$(date +%s.%N)
                echo "Processing salience CSV: $SALIENCE_OUT"
                log_message "Processing salience CSV for stream $STREAM_INDEX"
                
                ENGAGEMENT_OUT="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience_engagement.csv"
                ./salience_process "$SALIENCE_OUT" --time_col time_sec
                # The salience_process creates an engagement file, find it
                if [ -f "$ENGAGEMENT_OUT" ]; then
                    log_timing "Salience processing (stream $STREAM_INDEX)" $salience_start
                    log_file_info "$ENGAGEMENT_OUT" "Salience engagement: stream $STREAM_INDEX"
                    echo "✓ Salience processing complete: $ENGAGEMENT_OUT"
                    rm -f "$SALIENCE_OUT"  # Remove intermediate file
                else
                    log_message "ERROR: Expected engagement file not found: $ENGAGEMENT_OUT"
                    echo "✗ Expected engagement file not found: $ENGAGEMENT_OUT"
                fi
            else
                log_message "ERROR: DeepGaze did not generate expected output file"
                echo "✗ DeepGaze did not generate expected output file"
            fi
        else
            log_message "ERROR: DeepGaze processing failed for $OUTFILE"
            echo "✗ DeepGaze processing failed for $OUTFILE"
        fi
        
        log_timing "DeepGaze processing (stream $STREAM_INDEX)" $deepgaze_start
        log_timing "Complete video stream $STREAM_INDEX processing" $video_stream_start
        log_storage_usage "After video processing (stream $STREAM_INDEX)"

        # --- Clean up video file ---
        rm -f "$OUTFILE"
    done
fi

log_timing "All video processing" $video_start

echo ""
echo "=== ROBUST FINALIZE + GUI OVERLAY ==="
log_message "=== FINALIZATION START ==="
finalize_start=$(date +%s.%N)

# --- Function to find best available merged files ---
find_best_files() {
    local vocals_files=()
    local nonvocals_files=()
    local video_files=()
    
    # Find all vocals merged files
    while IFS= read -r -d '' file; do
        vocals_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_audio*_vocals_merged.csv" -print0 2>/dev/null)
    
    # Find all nonvocals merged files  
    while IFS= read -r -d '' file; do
        nonvocals_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_audio*_nonvocals_merged.csv" -print0 2>/dev/null)
    
    # Find all video engagement files (corrected pattern)
    while IFS= read -r -d '' file; do
        video_files+=("$file")
    done < <(find "$OUTDIR" -name "${BASENAME}_video*_salience_engagement.csv" -print0 2>/dev/null)
    
    # Return counts and first file of each type
    echo "${#vocals_files[@]} ${#nonvocals_files[@]} ${#video_files[@]}"
    echo "${vocals_files[0]:-}"
    echo "${nonvocals_files[0]:-}"  
    echo "${video_files[0]:-}"
}

# Get available files
file_info=($(find_best_files))
VOCALS_COUNT="${file_info[0]}"
NONVOCALS_COUNT="${file_info[1]}"
VIDEO_COUNT="${file_info[2]}"
BEST_VOCALS="${file_info[3]}"
BEST_NONVOCALS="${file_info[4]}"
BEST_VIDEO="${file_info[5]}"

log_message "Available files for finalization: Vocals=$VOCALS_COUNT, Nonvocals=$NONVOCALS_COUNT, Video=$VIDEO_COUNT"

echo "Available files for finalization:"
echo "  Vocals files: $VOCALS_COUNT found"
echo "  Nonvocals files: $NONVOCALS_COUNT found"
echo "  Video files: $VIDEO_COUNT found"

# Validate file existence
[ -n "$BEST_VOCALS" ] && [ ! -f "$BEST_VOCALS" ] && BEST_VOCALS=""
[ -n "$BEST_NONVOCALS" ] && [ ! -f "$BEST_NONVOCALS" ] && BEST_NONVOCALS=""
[ -n "$BEST_VIDEO" ] && [ ! -f "$BEST_VIDEO" ] && BEST_VIDEO=""

echo ""
echo "Selected files for finalization:"
echo "  Best vocals: ${BEST_VOCALS:-NONE FOUND}"
echo "  Best nonvocals: ${BEST_NONVOCALS:-NONE FOUND}"
echo "  Best video: ${BEST_VIDEO:-NONE FOUND}"
echo ""

log_message "Selected files: vocals=${BEST_VOCALS:-NONE}, nonvocals=${BEST_NONVOCALS:-NONE}, video=${BEST_VIDEO:-NONE}"

FINALIZED_OUT="$OUTDIR/${BASENAME}_finalized.csv"
FINALIZE_SUCCESS=false

# --- Multiple finalization strategies based on available files ---
if [ -n "$BEST_VOCALS" ] && [ -n "$BEST_NONVOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Full finalization (vocals + nonvocals + video)"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_NONVOCALS"), $(basename "$BEST_VIDEO")"
    log_message "Attempting full finalization strategy"
    
    if ./finalize --vocals "$BEST_VOCALS" --nonvocals "$BEST_NONVOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        log_timing "Full finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (full)"
        echo "✓ Full finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Full finalize failed, trying partial strategies..."
        echo "✗ Full finalize failed, trying partial strategies..."
    fi
elif [ -n "$BEST_VOCALS" ] && [ -n "$BEST_NONVOCALS" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Audio-only finalization (vocals + nonvocals)"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_NONVOCALS")"
    log_message "Attempting audio-only finalization strategy"
    
    if ./finalize --vocals "$BEST_VOCALS" --nonvocals "$BEST_NONVOCALS" --output "$FINALIZED_OUT"; then
        log_timing "Audio-only finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (audio-only)"
        echo "✓ Audio-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Audio-only finalize failed, trying individual strategies..."
        echo "✗ Audio-only finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_VOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Vocals + Video finalization"
    echo "  Using: $(basename "$BEST_VOCALS"), $(basename "$BEST_VIDEO")"
    log_message "Attempting vocals+video finalization strategy"
    
    if ./finalize --vocals "$BEST_VOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        log_timing "Vocals+Video finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (vocals+video)"
        echo "✓ Vocals+Video finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Vocals+Video finalize failed, trying individual strategies..."
        echo "✗ Vocals+Video finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_NONVOCALS" ] && [ -n "$BEST_VIDEO" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Nonvocals + Video finalization"
    echo "  Using: $(basename "$BEST_NONVOCALS"), $(basename "$BEST_VIDEO")"
    log_message "Attempting nonvocals+video finalization strategy"
    
    if ./finalize --nonvocals "$BEST_NONVOCALS" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        log_timing "Nonvocals+Video finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (nonvocals+video)"
        echo "✓ Nonvocals+Video finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Nonvocals+Video finalize failed, trying individual strategies..."
        echo "✗ Nonvocals+Video finalize failed, trying individual strategies..."
    fi
elif [ -n "$BEST_VOCALS" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Vocals-only finalization"
    echo "  Using: $(basename "$BEST_VOCALS")"
    log_message "Attempting vocals-only finalization strategy"
    
    if ./finalize --vocals "$BEST_VOCALS" --output "$FINALIZED_OUT"; then
        log_timing "Vocals-only finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (vocals-only)"
        echo "✓ Vocals-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Vocals-only finalize failed"
        echo "✗ Vocals-only finalize failed"
    fi
elif [ -n "$BEST_NONVOCALS" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Nonvocals-only finalization"
    echo "  Using: $(basename "$BEST_NONVOCALS")"
    log_message "Attempting nonvocals-only finalization strategy"
    
    if ./finalize --nonvocals "$BEST_NONVOCALS" --output "$FINALIZED_OUT"; then
        log_timing "Nonvocals-only finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (nonvocals-only)"
        echo "✓ Nonvocals-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Nonvocals-only finalize failed"
        echo "✗ Nonvocals-only finalize failed"
    fi
elif [ -n "$BEST_VIDEO" ]; then
    finalize_step_start=$(date +%s.%N)
    echo "Strategy: Video-only finalization"
    echo "  Using: $(basename "$BEST_VIDEO")"
    log_message "Attempting video-only finalization strategy"
    
    if ./finalize --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
        log_timing "Video-only finalization" $finalize_step_start
        log_file_info "$FINALIZED_OUT" "Finalized output (video-only)"
        echo "✓ Video-only finalize complete: $FINALIZED_OUT"
        FINALIZE_SUCCESS=true
    else
        log_message "ERROR: Video-only finalize failed"
        echo "✗ Video-only finalize failed"
    fi
else
    log_message "ERROR: No suitable CSV files found for finalization"
    echo "✗ No suitable CSV files found for finalization"
    echo "  Expected at least one of: vocals_merged.csv, nonvocals_merged.csv, salience_engagement.csv"
fi

# --- Handle multiple audio streams if needed ---
if [ "$FINALIZE_SUCCESS" = false ] && [ "$VOCALS_COUNT" -gt 1 ] || [ "$NONVOCALS_COUNT" -gt 1 ]; then
    alt_finalize_start=$(date +%s.%N)
    echo ""
    echo "Attempting finalization with alternative audio streams..."
    log_message "Attempting alternative audio stream combinations"
    
    # Find all vocals and nonvocals files for iteration
    ALL_VOCALS=($(find "$OUTDIR" -name "${BASENAME}_audio*_vocals_merged.csv" 2>/dev/null))
    ALL_NONVOCALS=($(find "$OUTDIR" -name "${BASENAME}_audio*_nonvocals_merged.csv" 2>/dev/null))
    
    for vocals_file in "${ALL_VOCALS[@]}"; do
        for nonvocals_file in "${ALL_NONVOCALS[@]}"; do
            # Skip if we already tried this combination
            if [ "$vocals_file" = "$BEST_VOCALS" ] && [ "$nonvocals_file" = "$BEST_NONVOCALS" ]; then
                continue
            fi
            
            alt_attempt_start=$(date +%s.%N)
            echo "Trying: $(basename "$vocals_file") + $(basename "$nonvocals_file")"
            log_message "Trying alternative combination: $(basename "$vocals_file") + $(basename "$nonvocals_file")"
            
            if [ -n "$BEST_VIDEO" ]; then
                if ./finalize --vocals "$vocals_file" --nonvocals "$nonvocals_file" --video "$BEST_VIDEO" --output "$FINALIZED_OUT"; then
                    log_timing "Alternative finalization (with video)" $alt_attempt_start
                    log_file_info "$FINALIZED_OUT" "Finalized output (alternative with video)"
                    echo "✓ Alternative finalize successful with video"
                    FINALIZE_SUCCESS=true
                    break 2
                fi
            else
                if ./finalize --vocals "$vocals_file" --nonvocals "$nonvocals_file" --output "$FINALIZED_OUT"; then
                    log_timing "Alternative finalization (audio only)" $alt_attempt_start
                    log_file_info "$FINALIZED_OUT" "Finalized output (alternative audio only)"
                    echo "✓ Alternative finalize successful (audio only)"
                    FINALIZE_SUCCESS=true
                    break 2
                fi
            fi
        done
    done
    
    if [ "$FINALIZE_SUCCESS" = true ]; then
        log_timing "Alternative finalization attempts" $alt_finalize_start
    else
        log_message "All alternative finalization attempts failed"
    fi
fi

log_timing "Finalization process" $finalize_start

# --- GUI Overlay generation (only if finalization succeeded) ---
if [ "$FINALIZE_SUCCESS" = true ] && [ -f "$FINALIZED_OUT" ]; then
    overlay_start=$(date +%s.%N)
    OVERLAY_OUT="$(dirname "$INPUT")/${BASENAME}_data_overlay.mp4"
    echo ""
    echo "Running GUI overlay generation..."
    echo "  Input video: $INPUT"
    echo "  Finalized data: $FINALIZED_OUT"
    echo "  Output video: $OVERLAY_OUT"
    
    log_message "Starting GUI overlay generation"
    log_message "Input: $INPUT, Data: $FINALIZED_OUT, Output: $OVERLAY_OUT"
    
    if ./gui_overlay "$INPUT" "$FINALIZED_OUT" "$OVERLAY_OUT"; then
        log_timing "GUI overlay generation" $overlay_start
        log_file_info "$OVERLAY_OUT" "Final overlay video"
        echo "✓ Overlay video created successfully: $OVERLAY_OUT"
    else
        log_message "ERROR: GUI overlay generation failed"
        echo "✗ GUI overlay generation failed"
        echo "  Finalized data is still available at: $FINALIZED_OUT"
    fi
else
    log_message "Skipping GUI overlay generation (finalization failed or no data)"
    echo "⚠ Skipping GUI overlay generation (finalization failed or no data)"
fi

# --- Final storage and timing summary ---
SCRIPT_END_TIME=$(date +%s.%N)
TOTAL_RUNTIME=$(echo "$SCRIPT_END_TIME - $SCRIPT_START_TIME" | bc -l)

log_message "=== FINAL SUMMARY ==="
log_message "Total runtime: ${TOTAL_RUNTIME}s"
log_message "Peak storage usage: ${MAX_STORAGE} bytes"
log_storage_usage "Final"

echo "=== PROCESSING COMPLETE ==="
echo "Finished at: $(date)"
echo "Total runtime: ${TOTAL_RUNTIME}s"
echo "Peak storage used: ${MAX_STORAGE} bytes"
echo "All output files saved to: $OUTDIR"
echo "Detailed log saved to: $LOG_FILE"
echo ""
echo "Final output structure:"

# --- List merged audio outputs (vocals/nonvocals per stream) ---
if [ "$NUM_AUDIO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_AUDIO_STREAMS - 1))); do
        VOCALS_MERGED="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}_vocals_merged.csv"
        NONVOCALS_MERGED="$OUTDIR/${BASENAME}_audio${STREAM_INDEX}_nonvocals_merged.csv"

        if [ -f "$VOCALS_MERGED" ]; then
            echo "  - Audio${STREAM_INDEX} (vocals): $(basename "$VOCALS_MERGED")"
            echo "      contains: RMS, ZCR, pitch, spectral, formant, jitter, emotion, transcript"
        else
            echo "  - Audio${STREAM_INDEX} (vocals): no merged CSV generated"
        fi

        if [ -f "$NONVOCALS_MERGED" ]; then
            echo "  - Audio${STREAM_INDEX} (nonvocals): $(basename "$NONVOCALS_MERGED")"
            echo "      contains: RMS, ZCR, spectral"
        else
            echo "  - Audio${STREAM_INDEX} (nonvocals): no merged CSV generated"
        fi
    done
else
    echo "  - No audio streams detected"
fi

# --- List processed video outputs ---
if [ "$NUM_VIDEO_STREAMS" -gt 0 ]; then
    for STREAM_INDEX in $(seq 0 $((NUM_VIDEO_STREAMS - 1))); do
        VIDEO_CSV="$OUTDIR/${BASENAME}_video${STREAM_INDEX}_salience_engagement.csv"
        if [ -f "$VIDEO_CSV" ]; then
            echo "  - Video${STREAM_INDEX}: $(basename "$VIDEO_CSV")"
        else
            echo "  - Video${STREAM_INDEX}: no salience engagement CSV generated"
        fi
    done
else
    echo "  - No video streams detected"
fi

# --- Show finalization results ---
echo ""
if [ -f "$FINALIZED_OUT" ]; then
    echo "✓ Final combined data: $(basename "$FINALIZED_OUT")"
    if [ -f "$(dirname "$INPUT")/${BASENAME}_data_overlay.mp4" ]; then
        echo "✓ Data overlay video: ${BASENAME}_data_overlay.mp4"
    fi
else
    echo "✗ No finalized combined data generated"
fi

# --- Write final summary to log ---
{
    echo ""
    echo "=== EXECUTION SUMMARY ==="
    echo "Script completed at: $(date)"
    echo "Total execution time: ${TOTAL_RUNTIME}s"
    echo "Peak storage usage: ${MAX_STORAGE} bytes"
    echo "Input file: $INPUT"
    echo "Audio streams processed: $NUM_AUDIO_STREAMS"
    echo "Video streams processed: $NUM_VIDEO_STREAMS"
    echo "Finalization successful: $FINALIZE_SUCCESS"
    echo "Output directory: $OUTDIR"
    if [ -f "$FINALIZED_OUT" ]; then
        echo "Final CSV: $FINALIZED_OUT"
    fi
    if [ -f "$(dirname "$INPUT")/${BASENAME}_data_overlay.mp4" ]; then
        echo "Final video: $(dirname "$INPUT")/${BASENAME}_data_overlay.mp4"
    fi
} >> "$LOG_FILE"