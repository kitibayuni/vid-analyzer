import argparse
import numpy as np
import torch
import torch.nn.functional as F
import pandas as pd
import soundfile as sf
from transformers import Wav2Vec2ForSequenceClassification, Wav2Vec2FeatureExtractor
from speechbrain.inference.interfaces import foreign_class

def auto_batch_size(chunk_tensors, device, memory_headroom=0.1):
    """Estimate max batch size given GPU memory and tensors, leaving some headroom."""
    if device == "cpu" or not torch.cuda.is_available():
        return 32  # reasonable default for CPU
    # Approximate memory per chunk in bytes
    sample_chunk = chunk_tensors[0]
    chunk_mem = sample_chunk.numel() * sample_chunk.element_size()
    # Get available GPU memory
    total_mem = torch.cuda.get_device_properties(device).total_memory
    reserved_mem = torch.cuda.memory_reserved(device)
    free_mem = total_mem - reserved_mem
    usable_mem = free_mem * (1 - memory_headroom)
    # Estimate batch size
    batch_size = int(usable_mem / chunk_mem)
    return max(1, batch_size)

def main(input_file, output_file, chunk_sec=5, msp_model_path=None, device='cpu', batch_size=None):
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
    # Collect chunk metadata & normalize
    # -------------------------------
    chunk_tensors = []
    chunk_meta = []

    for i, chunk in enumerate(chunks):
        if len(chunk) == 0:
            continue

        # Pad very short chunks
        if len(chunk) < 1600:
            chunk = np.pad(chunk, (0, 1600 - len(chunk)), mode="constant")

        # Normalize to [-1,1]
        chunk = chunk.astype(np.float32)
        if np.max(np.abs(chunk)) > 1.0:
            chunk = chunk / np.max(np.abs(chunk))

        # Torch tensor (time) → will batch later
        chunk_tensors.append(torch.tensor(chunk, dtype=torch.float32))
        chunk_meta.append({
            "chunk_index": i,
            "start_sec": i * chunk_sec,
            "end_sec": min((i + 1) * chunk_sec, len(audio) / sr),
        })

    # -------------------------------
    # Auto batch size if not provided
    # -------------------------------
    if batch_size is None:
        batch_size = auto_batch_size(chunk_tensors, device)
    print(f"Using batch size: {batch_size}")

    # -------------------------------
    # Process in batches
    # -------------------------------
    output_rows = []

    for b in range(0, len(chunk_tensors), batch_size):
        batch_tensors = chunk_tensors[b:b + batch_size]
        batch_meta = chunk_meta[b:b + batch_size]

        # MSP-Dim inference
        vad_results = []
        if msp_model is not None and msp_feature_extractor is not None:
            try:
                inputs = msp_feature_extractor(
                    [x.numpy() for x in batch_tensors],
                    sampling_rate=sr,
                    return_tensors="pt",
                    padding=True
                )
                with torch.no_grad():
                    logits = msp_model(inputs.input_values.to(device)).logits
                probs = F.softmax(logits, dim=-1).cpu().numpy()

                for j in range(len(batch_meta)):
                    vad_results.append({
                        "valence": float(probs[j][0]),
                        "arousal": float(probs[j][1]),
                        "dominance": float(probs[j][2]),
                    })
            except Exception as e:
                print(f"Error in MSP-Dim batch {b//batch_size}: {e}")
                for _ in batch_meta:
                    vad_results.append({"valence": 0.0, "arousal": 0.0, "dominance": 0.0})
        else:
            for _ in batch_meta:
                vad_results.append({"valence": 0.0, "arousal": 0.0, "dominance": 0.0})

        # SpeechBrain inference
        sb_results = []
        if sb_model is not None:
            try:
                batch_tensor = torch.stack(batch_tensors).to(device)
                with torch.no_grad():
                    out_prob, score, index, text_lab = sb_model.classify_batch(batch_tensor)

                probs = out_prob.detach().cpu().numpy()
                scores = score.detach().cpu().numpy() if isinstance(score, torch.Tensor) else np.array(score)

                for j in range(len(batch_meta)):
                    row = {}
                    labels = ["neu", "hap", "ang", "sad"]
                    for k, lab in enumerate(labels):
                        row[f"cat_{lab}"] = float(probs[j][k]) if k < probs.shape[1] else 0.0
                    pred = str(text_lab[j]) if isinstance(text_lab, (list, tuple)) else str(text_lab)
                    row["predicted_emotion"] = pred.strip("[]'\"")
                    row["confidence"] = float(scores[j])
                    sb_results.append(row)
            except Exception as e:
                print(f"Error in SpeechBrain batch {b//batch_size}: {e}")
                for _ in batch_meta:
                    sb_results.append({
                        "cat_neu": 0.0, "cat_hap": 0.0, "cat_ang": 0.0, "cat_sad": 0.0,
                        "predicted_emotion": "unknown", "confidence": 0.0
                    })
        else:
            for _ in batch_meta:
                sb_results.append({
                    "cat_neu": 0.0, "cat_hap": 0.0, "cat_ang": 0.0, "cat_sad": 0.0,
                    "predicted_emotion": "unknown", "confidence": 0.0
                })

        # Combine results
        for j, meta in enumerate(batch_meta):
            row = {}
            row.update(meta)
            row.update(vad_results[j])
            row.update(sb_results[j])
            output_rows.append(row)

        print(f"Processed {min(b + batch_size, len(chunk_tensors))}/{len(chunk_tensors)} chunks...")

    # Save CSV
    df = pd.DataFrame(output_rows)
    df.to_csv(output_file, index=False)
    print(f"Saved combined emotion scores per chunk to {output_file}")
    print(f"Output shape: {df.shape}")
    print(f"Columns: {list(df.columns)}")

    if 'predicted_emotion' in df.columns:
        print("Emotion distribution:")
        print(df['predicted_emotion'].value_counts())

    if 'confidence' in df.columns:
        print(f"Average confidence: {df['confidence'].mean():.3f}")


# -------------------------------
# CLI
# -------------------------------
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Chunked MSP-Dim + SpeechBrain emotion analysis (auto-batched)")
    parser.add_argument("input_file", type=str)
    parser.add_argument("output_file", type=str)
    parser.add_argument("--chunk_sec", type=float, default=5.0)
    parser.add_argument("--msp_model_path", type=str, default=None)
    parser.add_argument("--device", type=str, default="cpu")
    parser.add_argument("--batch_size", type=int, default=None, help="Automatically calculated if not provided")
    args = parser.parse_args()

    main(args.input_file, args.output_file, args.chunk_sec, args.msp_model_path, args.device, args.batch_size)
