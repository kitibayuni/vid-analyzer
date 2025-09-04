import argparse
import csv
import stable_whisper

def export_csv(transcript, output_file, interval=0.2):
    """
    Export transcript into CSV with fixed 0.2s intervals.
    Each row = [time_sec, word], deduplicated.
    """
    words = []
    for seg in transcript["segments"]:
        if "words" in seg:
            for w in seg["words"]:
                words.append((w["start"], w["word"].strip()))
    
    # Sort by time
    words.sort(key=lambda x: x[0])

    with open(output_file, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["time_sec", "word"])
        
        last_word = None
        for start_time, word in words:
            snapped_t = round(start_time / interval) * interval
            if word and word != last_word:
                writer.writerow([f"{snapped_t:.4f}", word])
                last_word = word

def main(input_file, output_file, model_size):
    model = stable_whisper.load_model(model_size)
    result = model.transcribe(input_file, word_timestamps=True)  # <-- key flag
    export_csv(result.to_dict(), output_file)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Transcribe audio with Whisper + stable-ts and export word-level CSV (0.2s intervals).")
    parser.add_argument("input_file", type=str, help="Path to the input audio file")
    parser.add_argument("--output", type=str, default=None, help="Output CSV file (default: input name + '_words.csv')")
    parser.add_argument("--model", type=str, default="small", help="Whisper model size (tiny, base, small, medium, large)")

    args = parser.parse_args()
    output_file = args.output or args.input_file.rsplit(".", 1)[0] + "_words.csv"

    main(args.input_file, output_file, args.model)
