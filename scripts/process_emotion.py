import argparse
import numpy as np
import torch
import torch.nn.functional as F
import pandas as pd
import gc
import psutil
import os
from transformers import Wav2Vec2ForSequenceClassification, Wav2Vec2FeatureExtractor
from speechbrain.inference.interfaces import foreign_class

def get_memory_info():
    """Get current memory usage information."""
    process = psutil.Process(os.getpid())
    mem_info = process.memory_info()
    return mem_info.rss / 1024 / 1024  # MB

def conservative_batch_size(chunk_length, device, base_batch_size=8):
    """Calculate a conservative batch size to avoid OOM."""
    if device == "cpu" or not torch.cuda.is_available():
        # For CPU, use smaller batches based on system memory
        system_memory_gb = psutil.virtual_memory().available / (1024**3)
        if system_memory_gb > 16:
            return min(32, base_batch_size * 2)
        elif system_memory_gb > 8:
            return min(16, base_batch_size)
        else:
            return min(8, base_batch_size // 2)
    
    try:
        # Conservative GPU memory estimation
        total_mem = torch.cuda.get_device_properties(device).total_memory
        allocated_mem = torch.cuda.memory_allocated(device)
        available_mem = total_mem - allocated_mem
        
        # Use only 30% of available memory for safety
        safe_mem = available_mem * 0.3
        
        # Estimate memory per chunk (very conservative)
        # Account for model weights, feature extraction, and intermediate tensors
        chunk_mem = chunk_length * 4 * 8  # 8x safety factor for all operations
        
        batch_size = max(1, int(safe_mem / chunk_mem))
        return min(batch_size, base_batch_size)
    
    except Exception:
        return min(4, base_batch_size // 2)  # Very conservative fallback

class StreamingAudioProcessor:
    def __init__(self, device='cpu', msp_model_path=None):
        self.device = device
        self.sr = 16000
        self.msp_model = None
        self.msp_feature_extractor = None
        self.sb_model = None
        
        # Load models
        self._load_msp_model(msp_model_path)
        self._load_sb_model()
        
    def _load_msp_model(self, msp_model_path):
        """Load MSP-Dim model if path provided."""
        if msp_model_path:
            print(f"Loading MSP-Dim model from {msp_model_path}...")
            try:
                self.msp_feature_extractor = Wav2Vec2FeatureExtractor.from_pretrained(msp_model_path)
                self.msp_model = Wav2Vec2ForSequenceClassification.from_pretrained(msp_model_path)
                self.msp_model.to(self.device)
                self.msp_model.eval()
                print("MSP-Dim model loaded successfully")
            except Exception as e:
                print(f"Error loading MSP-Dim model: {e}")
                self.msp_model = None
                self.msp_feature_extractor = None

    def _load_sb_model(self):
        """Load SpeechBrain model."""
        print("Loading SpeechBrain categorical emotion model...")
        try:
            self.sb_model = foreign_class(
                source="speechbrain/emotion-recognition-wav2vec2-IEMOCAP",
                pymodule_file="custom_interface.py",
                classname="CustomEncoderWav2vec2Classifier",
                savedir="tmp_speechbrain_model",
                run_opts={"device": self.device}
            )
            print("SpeechBrain model loaded successfully")
        except Exception as e:
            print(f"Error loading SpeechBrain model: {e}")
            self.sb_model = None

    def preprocess_chunk(self, chunk):
        """Preprocess a single audio chunk."""
        if len(chunk) == 0:
            return None

        # Pad very short chunks
        if len(chunk) < 1600:
            chunk = np.pad(chunk, (0, 1600 - len(chunk)), mode="constant")

        # Normalize to [-1,1]
        chunk = chunk.astype(np.float32)
        if np.max(np.abs(chunk)) > 1.0:
            chunk = chunk / np.max(np.abs(chunk))

        return torch.tensor(chunk, dtype=torch.float32)

    def process_msp_batch(self, batch_tensors):
        """Process batch with MSP-Dim model."""
        if self.msp_model is None or self.msp_feature_extractor is None:
            return [{"valence": 0.0, "arousal": 0.0, "dominance": 0.0} for _ in batch_tensors]

        try:
            inputs = self.msp_feature_extractor(
                [x.numpy() for x in batch_tensors],
                sampling_rate=self.sr,
                return_tensors="pt",
                padding=True
            )
            
            with torch.no_grad():
                logits = self.msp_model(inputs.input_values.to(self.device)).logits
            probs = F.softmax(logits, dim=-1).cpu().numpy()

            results = []
            for j in range(len(batch_tensors)):
                results.append({
                    "valence": float(probs[j][0]),
                    "arousal": float(probs[j][1]),
                    "dominance": float(probs[j][2]),
                })
            return results

        except Exception as e:
            print(f"Error in MSP-Dim processing: {e}")
            return [{"valence": 0.0, "arousal": 0.0, "dominance": 0.0} for _ in batch_tensors]

    def process_sb_batch(self, batch_tensors):
        """Process batch with SpeechBrain model."""
        if self.sb_model is None:
            return [{
                "cat_neu": 0.0, "cat_hap": 0.0, "cat_ang": 0.0, "cat_sad": 0.0,
                "predicted_emotion": "unknown", "confidence": 0.0
            } for _ in batch_tensors]

        try:
            batch_tensor = torch.stack(batch_tensors).to(self.device)
            
            with torch.no_grad():
                out_prob, score, index, text_lab = self.sb_model.classify_batch(batch_tensor)

            probs = out_prob.detach().cpu().numpy()
            scores = score.detach().cpu().numpy() if isinstance(score, torch.Tensor) else np.array(score)

            results = []
            labels = ["neu", "hap", "ang", "sad"]
            
            for j in range(len(batch_tensors)):
                row = {}
                for k, lab in enumerate(labels):
                    row[f"cat_{lab}"] = float(probs[j][k]) if k < probs.shape[1] else 0.0
                
                pred = str(text_lab[j]) if isinstance(text_lab, (list, tuple)) else str(text_lab)
                row["predicted_emotion"] = pred.strip("[]'\"")
                row["confidence"] = float(scores[j])
                results.append(row)
                
            return results

        except Exception as e:
            print(f"Error in SpeechBrain processing: {e}")
            return [{
                "cat_neu": 0.0, "cat_hap": 0.0, "cat_ang": 0.0, "cat_sad": 0.0,
                "predicted_emotion": "unknown", "confidence": 0.0
            } for _ in batch_tensors]

def stream_npy_chunks(npy_file, chunk_sec=5.0, sr=16000):
    """Stream audio chunks from .npy file using memory mapping."""
    print(f"Streaming audio from {npy_file}...")
    
    # Validate file extension
    if not npy_file.endswith('.npy'):
        raise ValueError("Input file must be a .npy file")
    
    try:
        # Load .npy file with memory mapping
        audio_mmap = np.load(npy_file, mmap_mode='r')
        print(f"Audio array shape: {audio_mmap.shape}")
        
        # Handle multi-channel audio
        if audio_mmap.ndim == 2:
            print(f"Multi-channel audio detected ({audio_mmap.shape[1]} channels), converting to mono...")
            audio_length = audio_mmap.shape[0]
            chunk_samples = int(sr * chunk_sec)
            
            for i in range(0, audio_length, chunk_samples):
                end_idx = min(i + chunk_samples, audio_length)
                # Convert to mono by averaging channels
                chunk = np.mean(audio_mmap[i:end_idx], axis=1)
                
                start_sec = i / sr
                end_sec = end_idx / sr
                
                yield chunk, {
                    "chunk_index": i // chunk_samples,
                    "start_sec": start_sec,
                    "end_sec": end_sec,
                }
        
        elif audio_mmap.ndim == 1:
            # Single channel audio
            audio_length = len(audio_mmap)
            chunk_samples = int(sr * chunk_sec)
            print(f"Single channel audio, duration: {audio_length/sr:.2f}s")
            
            for i in range(0, audio_length, chunk_samples):
                end_idx = min(i + chunk_samples, audio_length)
                chunk = audio_mmap[i:end_idx]
                
                start_sec = i / sr
                end_sec = end_idx / sr
                
                yield chunk, {
                    "chunk_index": i // chunk_samples,
                    "start_sec": start_sec,
                    "end_sec": end_sec,
                }
        else:
            raise ValueError(f"Unsupported audio array dimensions: {audio_mmap.ndim}. Expected 1D or 2D array.")
                    
    except Exception as e:
        print(f"Error streaming .npy audio: {e}")
        raise

def main(input_file, output_file, chunk_sec=5, msp_model_path=None, device='cpu', batch_size=None, sr=16000):
    print(f"Starting memory-efficient streaming processing...")
    print(f"Initial memory usage: {get_memory_info():.1f} MB")
    
    # Validate input file
    if not input_file.endswith('.npy'):
        raise ValueError("Input file must be a .npy file")
    
    # Initialize processor
    processor = StreamingAudioProcessor(device=device, msp_model_path=msp_model_path)
    
    # Prepare for streaming processing
    output_rows = []
    batch_tensors = []
    batch_metas = []
    total_chunks = 0
    
    # Determine batch size
    sample_chunk_length = int(sr * chunk_sec)  # Approximate chunk length
    if batch_size is None:
        batch_size = conservative_batch_size(sample_chunk_length, device)
    print(f"Using batch size: {batch_size}")
    print(f"Using sample rate: {sr} Hz")
    
    try:
        # Stream and process audio chunks
        for chunk_data, meta in stream_npy_chunks(input_file, chunk_sec, sr):
            # Preprocess chunk
            chunk_tensor = processor.preprocess_chunk(chunk_data)
            if chunk_tensor is None:
                continue
            
            batch_tensors.append(chunk_tensor)
            batch_metas.append(meta)
            total_chunks += 1
            
            # Process batch when full or at end
            if len(batch_tensors) >= batch_size:
                # Process current batch
                vad_results = processor.process_msp_batch(batch_tensors)
                sb_results = processor.process_sb_batch(batch_tensors)
                
                # Combine results
                for j, meta in enumerate(batch_metas):
                    row = {}
                    row.update(meta)
                    row.update(vad_results[j])
                    row.update(sb_results[j])
                    output_rows.append(row)
                
                print(f"Processed {total_chunks} chunks... Memory: {get_memory_info():.1f} MB")
                
                # Clear batch and force garbage collection
                batch_tensors.clear()
                batch_metas.clear()
                del vad_results, sb_results
                gc.collect()
                
                if device != 'cpu' and torch.cuda.is_available():
                    torch.cuda.empty_cache()
        
        # Process remaining chunks in final batch
        if batch_tensors:
            vad_results = processor.process_msp_batch(batch_tensors)
            sb_results = processor.process_sb_batch(batch_tensors)
            
            for j, meta in enumerate(batch_metas):
                row = {}
                row.update(meta)
                row.update(vad_results[j])
                row.update(sb_results[j])
                output_rows.append(row)
            
            print(f"Processed final batch. Total chunks: {total_chunks}")
    
    except Exception as e:
        print(f"Error during processing: {e}")
        raise
    finally:
        # Clean up
        gc.collect()
        if device != 'cpu' and torch.cuda.is_available():
            torch.cuda.empty_cache()
    
    # Save results
    if output_rows:
        df = pd.DataFrame(output_rows)
        df.to_csv(output_file, index=False)
        print(f"Saved combined emotion scores per chunk to {output_file}")
        print(f"Output shape: {df.shape}")
        print(f"Columns: {list(df.columns)}")
        print(f"Final memory usage: {get_memory_info():.1f} MB")
        
        if 'predicted_emotion' in df.columns:
            print("Emotion distribution:")
            print(df['predicted_emotion'].value_counts())

        if 'confidence' in df.columns:
            print(f"Average confidence: {df['confidence'].mean():.3f}")
    else:
        print("No chunks were processed successfully.")

# -------------------------------
# CLI
# -------------------------------
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Memory-efficient streaming MSP-Dim + SpeechBrain emotion analysis for .npy files")
    parser.add_argument("input_file", type=str, help="Input .npy audio file")
    parser.add_argument("output_file", type=str, help="Output CSV file")
    parser.add_argument("--chunk_sec", type=float, default=5.0, help="Chunk duration in seconds")
    parser.add_argument("--msp_model_path", type=str, default=None, help="Path to MSP-Dim model")
    parser.add_argument("--device", type=str, default="cpu", help="Device to use (cpu/cuda)")
    parser.add_argument("--batch_size", type=int, default=None, help="Batch size (auto-calculated if not provided)")
    parser.add_argument("--sr", type=int, default=16000, help="Sample rate of the audio (default: 16000)")
    
    args = parser.parse_args()
    
    main(args.input_file, args.output_file, args.chunk_sec, args.msp_model_path, args.device, args.batch_size, args.sr)