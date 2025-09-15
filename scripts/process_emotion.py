import argparse
import numpy as np
import torch
import torch.nn.functional as F
import pandas as pd
import tempfile
import os
import soundfile as sf
from transformers import Wav2Vec2ForSequenceClassification, Wav2Vec2FeatureExtractor
from speechbrain.inference.interfaces import foreign_class

def main(input_file, output_file, chunk_sec=5, msp_model_path=None, device='cpu'):
    # -------------------------------
    # Load audio
    # -------------------------------
    audio = np.load(input_file)
    sr = 16000
    if audio.ndim == 2:
        audio = np.mean(audio, axis=0)
    print(f"Audio shape: {audio.shape}, duration: {len(audio)/sr:.2f}s")

    # -------------------------------
    # Load MSP-Dim model for VAD
    # -------------------------------
    msp_model = None
    msp_feature_extractor = None
    if msp_model_path:
        print(f"Loading MSP-Dim model from {msp_model_path}...")
        try:
            msp_feature_extractor = Wav2Vec2FeatureExtractor.from_pretrained(msp_model_path)
            msp_model = Wav2Vec2ForSequenceClassification.from_pretrained(msp_model_path)
            msp_model.to(device)
            msp_model.eval()
            print("MSP-Dim model loaded successfully")
        except Exception as e:
            print(f"Error loading MSP-Dim model: {e}")

    # -------------------------------
    # Load SpeechBrain categorical model
    # -------------------------------
    print("Loading SpeechBrain categorical emotion model...")
    sb_model = None
    try:
        sb_model = foreign_class(
            source="speechbrain/emotion-recognition-wav2vec2-IEMOCAP",
            pymodule_file="custom_interface.py",
            classname="CustomEncoderWav2vec2Classifier",
            savedir="tmp_speechbrain_model",
            run_opts={"device": device}
        )
        print("SpeechBrain model loaded successfully")
    except Exception as e:
        print(f"Error loading SpeechBrain model: {e}")
        sb_model = None

    # -------------------------------
    # Chunk audio
    # -------------------------------
    chunk_samples = int(sr * chunk_sec)
    chunks = [audio[i:i + chunk_samples] for i in range(0, len(audio), chunk_samples)]
    print(f"Total chunks: {len(chunks)}, chunk size: {chunk_samples} samples (~{chunk_sec}s)")

    # -------------------------------
    # Prepare output
    # -------------------------------
    output_rows = []

    for i, chunk in enumerate(chunks):
        if len(chunk) == 0:
            continue

        # Pad short chunks
        if len(chunk) < 1600:
            chunk = np.pad(chunk, (0, 1600 - len(chunk)), mode='constant')

        row = {
            "chunk_index": i,
            "start_sec": i * chunk_sec,
            "end_sec": min((i+1) * chunk_sec, len(audio)/sr)
        }

        # -------------------------------
        # MSP-Dim VAD inference
        # -------------------------------
        if msp_model is not None and msp_feature_extractor is not None:
            try:
                inputs = msp_feature_extractor(chunk, sampling_rate=sr, return_tensors="pt", padding=True)
                input_values = inputs.input_values.to(device)
                with torch.no_grad():
                    logits = msp_model(input_values).logits
                probs = F.softmax(logits, dim=-1)[0]
                # MSP-Dim outputs: valence, arousal, dominance
                row.update({
                    "valence": float(probs[0]),
                    "arousal": float(probs[1]),
                    "dominance": float(probs[2])
                })
            except Exception as e:
                print(f"Error processing chunk {i} with MSP-Dim model: {e}")
                row.update({
                    "valence": 0.0,
                    "arousal": 0.0,
                    "dominance": 0.0
                })
        else:
            row.update({
                "valence": 0.0,
                "arousal": 0.0,
                "dominance": 0.0
            })

        # -------------------------------
        # SpeechBrain categorical emotions
        # -------------------------------
        if sb_model is not None:
            try:
                # Ensure chunk is float32 and properly normalized
                chunk_normalized = chunk.astype(np.float32)
                if np.max(np.abs(chunk_normalized)) > 1.0:
                    chunk_normalized = chunk_normalized / np.max(np.abs(chunk_normalized))
                
                # Create temporary wav file
                with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as temp_wav:
                    sf.write(temp_wav.name, chunk_normalized, sr)
                    temp_wav_path = temp_wav.name
                
                # Get emotion prediction
                with torch.no_grad():
                    out_prob, score, index, text_lab = sb_model.classify_file(temp_wav_path)
                
                # Clean up temp file
                os.unlink(temp_wav_path)
                
                # Process probabilities
                if isinstance(out_prob, torch.Tensor):
                    probs = out_prob.detach().cpu().numpy()
                else:
                    probs = np.array(out_prob)
                
                if len(probs.shape) > 1:
                    probs = probs.squeeze()
                
                # IEMOCAP labels in model order: neu, hap, ang, sad
                iemocap_labels = ['neu', 'hap', 'ang', 'sad']
                
                # Add emotion probabilities
                for j, label in enumerate(iemocap_labels):
                    row[f"cat_{label}"] = float(probs[j]) if j < len(probs) else 0.0
                
                # Add predicted emotion and confidence
                predicted_emotion = str(text_lab[0]) if isinstance(text_lab, list) else str(text_lab)
                predicted_emotion = predicted_emotion.strip("[]'\"")  # Clean up formatting
                
                row["predicted_emotion"] = predicted_emotion
                row["confidence"] = float(score.item()) if isinstance(score, torch.Tensor) else float(score)
                
            except Exception as e:
                print(f"Error processing chunk {i} with SpeechBrain model: {e}")
                # Add default emotion categories
                for label in ['neu', 'hap', 'ang', 'sad']:
                    row[f"cat_{label}"] = 0.0
                row["predicted_emotion"] = "unknown"
                row["confidence"] = 0.0
        else:
            # If SpeechBrain model failed to load, add default categories
            for label in ['neu', 'hap', 'ang', 'sad']:
                row[f"cat_{label}"] = 0.0
            row["predicted_emotion"] = "unknown" 
            row["confidence"] = 0.0

        output_rows.append(row)
        
        # Print progress every 50 chunks
        if (i + 1) % 50 == 0:
            print(f"Processed {i + 1}/{len(chunks)} chunks...")

    # -------------------------------
    # Save CSV
    # -------------------------------
    df = pd.DataFrame(output_rows)
    df.to_csv(output_file, index=False)
    print(f"Saved combined emotion scores per chunk to {output_file}")
    print(f"Output shape: {df.shape}")
    print(f"Columns: {list(df.columns)}")
    
    # Print summary statistics
    if 'predicted_emotion' in df.columns:
        print(f"Emotion distribution:")
        print(df['predicted_emotion'].value_counts())
    
    if 'confidence' in df.columns:
        print(f"Average confidence: {df['confidence'].mean():.3f}")

# -------------------------------
# CLI
# -------------------------------
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Chunked MSP-Dim + SpeechBrain emotion analysis")
    parser.add_argument("input_file", type=str)
    parser.add_argument("output_file", type=str)
    parser.add_argument("--chunk_sec", type=float, default=5.0)
    parser.add_argument("--msp_model_path", type=str, default=None)
    parser.add_argument("--device", type=str, default="cpu")
    args = parser.parse_args()

    main(args.input_file, args.output_file, args.chunk_sec, args.msp_model_path, args.device)
