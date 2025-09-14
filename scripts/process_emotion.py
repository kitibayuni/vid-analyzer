import argparse
import numpy as np
import torch
import pandas as pd
from transformers import AutoFeatureExtractor, AutoModelForAudioClassification

def map_to_emotion_from_msp(valence, arousal):
    """Optional derived emotion from MSP-Dim (if you want both sources)"""
    if valence >= 0.3 and arousal >= 0.3:
        return "happy"
    elif valence <= -0.3 and arousal >= 0.3:
        return "angry"
    elif valence <= -0.3 and arousal <= -0.3:
        return "sad"
    elif valence >= 0.3 and arousal <= -0.3:
        return "calm"
    else:
        return "neutral"

def main(
    input_file: str,
    output_file: str,
    chunk_sec: float = 5.0,
    msp_model_path: str = "./models/audeering-msp-dim",
    cat_model_path: str = "./models/ehcalabres-emotion-categorical"
):
    # -------------------------------
    # Load preprocessed audio
    # -------------------------------
    audio = np.load(input_file)
    sr = 16000  # all models expect 16kHz

    if audio.ndim == 2:
        print(f"Converting stereo audio {audio.shape} to mono")
        audio = np.mean(audio, axis=0)
    print(f"Audio shape after conversion: {audio.shape}, duration: {len(audio)/sr:.2f}s")

    # -------------------------------
    # Load models
    # -------------------------------
    print(f"Loading MSP-Dim model from {msp_model_path}...")
    msp_extractor = AutoFeatureExtractor.from_pretrained(msp_model_path)
    msp_model = AutoModelForAudioClassification.from_pretrained(msp_model_path)
    print(f"Loading categorical emotion model from {cat_model_path}...")
    cat_extractor = AutoFeatureExtractor.from_pretrained(cat_model_path)
    cat_model = AutoModelForAudioClassification.from_pretrained(cat_model_path)

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    msp_model.to(device).eval()
    cat_model.to(device).eval()

    # -------------------------------
    # Chunk audio
    # -------------------------------
    chunk_samples = int(sr * chunk_sec)
    chunks = [audio[i:i + chunk_samples] for i in range(0, len(audio), chunk_samples)]
    print(f"Total chunks: {len(chunks)}, chunk size: {chunk_samples} samples (~{chunk_sec}s)")

    # -------------------------------
    # Run inference per chunk
    # -------------------------------
    cat_labels = ["angry", "disgust", "fear", "happy", "neutral", "sad", "surprise"]  # ehcalabres model

    chunk_scores = []
    for i, chunk in enumerate(chunks):
        if len(chunk) == 0:
            continue
        if len(chunk) < 1600:
            chunk = np.pad(chunk, (0, 1600 - len(chunk)), mode='constant')

        # MSP-Dim inference
        msp_inputs = msp_extractor(chunk, sampling_rate=sr, return_tensors="pt", padding=True)
        msp_inputs = {k: v.to(device) for k, v in msp_inputs.items()}
        with torch.no_grad():
            msp_logits = msp_model(**msp_inputs).logits
        valence, arousal, dominance = msp_logits.cpu().numpy()[0]

        # Categorical inference
        cat_inputs = cat_extractor(chunk, sampling_rate=sr, return_tensors="pt", padding=True)
        cat_inputs = {k: v.to(device) for k, v in cat_inputs.items()}
        with torch.no_grad():
            cat_logits = cat_model(**cat_inputs).logits
        cat_probs = torch.softmax(cat_logits, dim=-1)[0].cpu().numpy()
        cat_index = int(np.argmax(cat_probs))
        predicted_emotion = cat_labels[cat_index]

        chunk_scores.append({
            "chunk_index": i,
            "start_sec": i * chunk_sec,
            "end_sec": min((i + 1) * chunk_sec, len(audio)/sr),
            "valence": float(valence),
            "arousal": float(arousal),
            "dominance": float(dominance),
            "msp_derived_emotion": map_to_emotion_from_msp(valence, arousal),
            "categorical_emotion": predicted_emotion,
            "cat_probs": cat_probs.tolist()
        })

    # -------------------------------
    # Save results
    # -------------------------------
    df_scores = pd.DataFrame(chunk_scores)
    df_scores.to_csv(output_file, index=False)
    print(f"Saved combined emotion scores per chunk to {output_file}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="MSP-Dim + Categorical emotion detection (local, offline)")
    parser.add_argument("input_file", type=str, help="Path to input .npy audio file")
    parser.add_argument("output_file", type=str, help="Path to output CSV file")
    parser.add_argument("--chunk_sec", type=float, default=5.0, help="Chunk duration in seconds")
    parser.add_argument("--msp_model_path", type=str, default="./models/audeering-msp-dim", help="Local MSP-Dim model folder")
    parser.add_argument("--cat_model_path", type=str, default="./models/ehcalabres-emotion-categorical", help="Local categorical model folder")

    args = parser.parse_args()
    main(args.input_file, args.output_file, args.chunk_sec, args.msp_model_path, args.cat_model_path)
