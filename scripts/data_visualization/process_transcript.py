import csv
import math

def export_csv(transcript, output_file, interval=0.2):
    """
    Export transcript into CSV with fixed intervals.
    transcript: whisper output (list of segments with start, end, text)
    output_file: path to save CSV
    interval: step size in seconds (default 0.2s = 200ms)
    """
    # Find total duration
    total_duration = max(seg["end"] for seg in transcript["segments"])
    
    with open(output_file, "w", newline="") as f:
        writer = csv.writer(f)
        # CSV header
        writer.writerow(["time_sec", "word"])
        
        t = 0.0
        seg_idx = 0
        while t <= total_duration:
            word_at_time = ""
            
            # Find active segment
            while seg_idx < len(transcript["segments"]) and transcript["segments"][seg_idx]["end"] < t:
                seg_idx += 1
            if seg_idx < len(transcript["segments"]):
                seg = transcript["segments"][seg_idx]
                if seg["start"] <= t <= seg["end"]:
                    word_at_time = seg["text"].strip()
            
            writer.writerow([f"{t:.4f}", word_at_time])
            t += interval
