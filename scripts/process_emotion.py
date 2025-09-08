import argparse
import numpy as np
import torch
import torch.nn.functional as F
from transformers import HubertForSequenceClassification, Wav2Vec2FeatureExtractor
import pandas as pd

def main(input_file: str, output_file: str, chunk_sec: float = 5.0):
    # -------------------------------
    # Load preprocessed audio
    # -------------------------------
    audio = np.load(input_file)  # could be mono or stereo
    sr = 16000  # sampling rate used in preprocessing

    # Convert stereo to mono if necessary
    if audio.ndim == 2:
        print(f"Converting stereo audio {audio.shape} to mono")
        audio = np.mean(audio, axis=0)
    print(f"Audio shape after conversion: {audio.shape}, duration: {len(audio)/sr:.2f}s")

    # -------------------------------
    # Load HuBERT emotion model
    # -------------------------------
    model_name = "superb/hubert-large-superb-er"
    feature_extractor = Wav2Vec2FeatureExtractor.from_pretrained(model_name)
    model = HubertForSequenceClassification.from_pretrained(model_name)
    
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    model.to(device)
    model.eval()
    
    # -------------------------------
    # Chunk audio
    # -------------------------------
    chunk_samples = int(sr * chunk_sec)
    chunks = [audio[i:i + chunk_samples] for i in range(0, len(audio), chunk_samples)]
    print(f"Total chunks: {len(chunks)}, chunk size: {chunk_samples} samples (~{chunk_sec}s)")

    # -------------------------------
    # Run HuBERT emotion inference per chunk
    # -------------------------------
    chunk_scores = []
    emotion_labels = ["neutral", "happy", "excited", "sad", "angry"]
    
    for i, chunk in enumerate(chunks):
        # Skip empty chunks
        if len(chunk) == 0:
            continue
        
        # Pad short chunks to minimum length (~0.1s)
        if len(chunk) < 1600:
            chunk = np.pad(chunk, (0, 1600 - len(chunk)), mode='constant')
        
        inputs = feature_extractor(chunk, sampling_rate=sr, return_tensors="pt", padding=True)
        input_values = inputs.input_values.to(device)
        
        with torch.no_grad():
            logits = model(input_values).logits
            
        probs = F.softmax(logits, dim=-1)[0]
        predicted_index = torch.argmax(probs).item()
        predicted_emotion = emotion_labels[predicted_index]
        
        chunk_scores.append({
            "chunk_index": i,
            "start_sec": i * chunk_sec,
            "end_sec": min((i + 1) * chunk_sec, len(audio)/sr),
            "predicted_emotion": predicted_emotion,
            "emotion_probs": probs.cpu().numpy().tolist()
        })
    
    # -------------------------------
    # Save results
    # -------------------------------
    df_scores = pd.DataFrame(chunk_scores)
    df_scores.to_csv(output_file, index=False)
    print(f"Saved HuBERT emotion scores per chunk to {output_file}")


# -------------------------------
# CLI arguments
# -------------------------------
if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="HuBERT emotion detection from preprocessed .npy audio"
    )
    parser.add_argument("input_file", type=str, help="Path to input .npy audio file")
    parser.add_argument("output_file", type=str, help="Path to output CSV file")
    parser.add_argument("--chunk_sec", type=float, default=5.0, help="Chunk duration in seconds")
    
    args = parser.parse_args()
    main(args.input_file, args.output_file, args.chunk_sec)
