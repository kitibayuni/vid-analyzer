import argparse
import torch
from deepgaze_pytorch import DeepGazeIIE
import cv2
import numpy as np
import pandas as pd
from tqdm import tqdm
import warnings
import gc
warnings.filterwarnings("ignore")

print("DeepGazeIIE imported successfully")

##########
## ARGS ##
##########
parser = argparse.ArgumentParser(description="Extract motion, saliency, and attention features using DeepGaze IIE")
parser.add_argument("input_video", type=str, help="Path to input video file")
parser.add_argument("output_csv", type=str, help="Path to output CSV file")
parser.add_argument("--frame_size", type=int, nargs=2, default=(224, 224), help="Frame size for model input (width height)")
parser.add_argument("--batch_size", type=int, default=32, help="Batch size for GPU inference")
parser.add_argument("--chunk_size", type=int, default=5000, help="Number of frames to process before writing to CSV")
args = parser.parse_args()

VIDEO_PATH = args.input_video
OUTPUT_CSV = args.output_csv
FRAME_SIZE = tuple(args.frame_size)
BATCH_SIZE = args.batch_size
CHUNK_SIZE = args.chunk_size
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"

print(f"Using device: {DEVICE}")
print(f"Frame size: {FRAME_SIZE}, Batch size: {BATCH_SIZE}, Chunk size: {CHUNK_SIZE}")

################
## LOAD MODEL ##
################
try:
    model = DeepGazeIIE(pretrained=True)
    model.to(DEVICE)
    model.eval()
    print("Model loaded successfully")
except Exception as e:
    print(f"Error loading model: {e}")
    exit(1)

###############
## VIDEO CAP ##
###############
cap = None
try:
    cap = cv2.VideoCapture(VIDEO_PATH)
    if not cap.isOpened():
        raise ValueError(f"Cannot open video file: {VIDEO_PATH}")
    
    fps = cap.get(cv2.CAP_PROP_FPS)
    total_frames = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    duration = total_frames / fps
    
    print(f"Video: {total_frames} frames @ {fps:.2f} FPS ({duration:.2f} seconds)")
except Exception as e:
    print(f"Error opening video: {e}")
    if cap:
        cap.release()
    exit(1)

######################
## HELPER FUNCTIONS ##
######################
def calculate_saliency_features(saliency_map):
    """Calculate saliency and attention features from a probability map"""
    try:
        if len(saliency_map.shape) > 2:
            saliency_map = saliency_map.squeeze()

        # Saliency features
        mean_sal = float(np.mean(saliency_map))
        max_sal = float(np.max(saliency_map))
        
        # Saliency entropy (vectorized)
        epsilon = 1e-10
        sal_norm = saliency_map + epsilon
        sal_norm = sal_norm / np.sum(sal_norm)
        entropy = -np.sum(sal_norm * np.log2(sal_norm))

        # Attention features (vectorized)
        h, w = saliency_map.shape
        y_coords, x_coords = np.mgrid[0:h, 0:w]
        total_mass = np.sum(saliency_map)
        
        if total_mass > epsilon:
            center_y = np.sum(y_coords * saliency_map) / total_mass / h
            center_x = np.sum(x_coords * saliency_map) / total_mass / w
            distances = np.sqrt((y_coords - center_y*h)**2 + (x_coords - center_x*w)**2)
            mean_distance = np.sum(distances * saliency_map) / total_mass
            max_distance = np.sqrt(h**2 + w**2) / 2
            concentration = 1.0 - (mean_distance / max_distance)
        else:
            center_y = center_x = 0.5
            concentration = 0.0

        return {
            'mean_saliency': mean_sal,
            'max_saliency': max_sal,
            'saliency_entropy': entropy,
            'attention_center_x': center_x,
            'attention_center_y': center_y,
            'attention_concentration': concentration
        }
    except:
        # Fast fallback without error printing
        return {
            'mean_saliency': 0.0,
            'max_saliency': 0.0,
            'saliency_entropy': 0.0,
            'attention_center_x': 0.5,
            'attention_center_y': 0.5,
            'attention_concentration': 0.0
        }

####################
## PROCESS VIDEO  ##
####################
try:
    chunk_data = []
    prev_gray = None
    frames_batch = []
    is_first_chunk = True
    frames_processed = 0

    with tqdm(total=total_frames, desc="Processing Video") as pbar:
        frame_idx = 0
        
        while True:
            ret, frame = cap.read()
            if not ret:
                break

            # Resize frame once
            frame_resized = cv2.resize(frame, FRAME_SIZE)
            
            # Motion calculation on grayscale
            gray = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2GRAY)
            if prev_gray is not None:
                diff = cv2.absdiff(gray, prev_gray)
                motion_intensity = float(np.mean(diff)) / 255.0
                motion_variance = float(np.std(diff)) / 255.0
            else:
                motion_intensity = motion_variance = 0.0
            prev_gray = gray

            # Add to batch for saliency processing
            frame_rgb = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2RGB)
            frames_batch.append(frame_rgb)

            # Store frame data (saliency will be updated later)
            frame_data = {
                "time_sec": frame_idx / fps,
                "motion_intensity": motion_intensity,
                "motion_variance": motion_variance,
                "mean_saliency": 0.0,
                "max_saliency": 0.0,
                "saliency_entropy": 0.0,
                "attention_center_x": 0.5,
                "attention_center_y": 0.5,
                "attention_concentration": 0.0
            }
            chunk_data.append(frame_data)

            # Process saliency batch
            if len(frames_batch) == BATCH_SIZE or frame_idx == total_frames - 1:
                try:
                    # Efficient tensor creation
                    batch_tensor = torch.from_numpy(np.stack(frames_batch, axis=0))
                    batch_tensor = batch_tensor.permute(0, 3, 1, 2).float().div(255.0).to(DEVICE)

                    with torch.no_grad():
                        batch_size, _, h, w = batch_tensor.shape
                        centerbias = torch.zeros(batch_size, h, w, device=DEVICE)
                        saliency_logits = model(batch_tensor, centerbias=centerbias)
                        saliency_probs = torch.nn.functional.softmax(
                            saliency_logits.view(batch_size, -1), dim=1
                        ).view(batch_size, -1, h, w)
                        saliency_maps = saliency_probs.cpu().numpy()

                    # Update saliency features efficiently
                    batch_start_idx = len(chunk_data) - len(frames_batch)
                    for i, sal_map in enumerate(saliency_maps):
                        sal_features = calculate_saliency_features(sal_map[0])
                        chunk_data[batch_start_idx + i].update(sal_features)

                    # Clear GPU memory periodically
                    del batch_tensor, saliency_logits, saliency_probs, saliency_maps, centerbias
                    if frames_processed % (BATCH_SIZE * 10) == 0 and torch.cuda.is_available():
                        torch.cuda.empty_cache()
                        
                except Exception as e:
                    print(f"Batch processing error at frame {frame_idx}: {e}")
                    # Continue with default saliency values

                frames_batch.clear()

            frame_idx += 1
            frames_processed += 1
            pbar.update(1)

            # Write chunk when full
            if len(chunk_data) >= CHUNK_SIZE:
                try:
                    df = pd.DataFrame(chunk_data)
                    
                    # Calculate derivatives efficiently
                    df['motion_change_rate'] = df['motion_intensity'].diff().fillna(0)
                    df['saliency_change_rate'] = df['mean_saliency'].diff().fillna(0)
                    df['attention_shift_rate'] = np.sqrt(
                        df['attention_center_x'].diff().fillna(0)**2 + 
                        df['attention_center_y'].diff().fillna(0)**2
                    )

                    # Select and round columns
                    final_columns = [
                        'time_sec', 'motion_intensity', 'motion_variance', 'motion_change_rate',
                        'mean_saliency', 'max_saliency', 'saliency_entropy', 'saliency_change_rate',
                        'attention_center_x', 'attention_center_y', 'attention_concentration', 'attention_shift_rate'
                    ]
                    # Scale saliency values for better readability and processing
                    df['mean_saliency'] = df['mean_saliency'] * 50000  # Scale to ~0-2 range
                    df['max_saliency'] = df['max_saliency'] * 50000
                    df['saliency_change_rate'] = df['saliency_change_rate'] * 50000
                    
                    df = df[final_columns].round(6)

                    # Write to CSV
                    mode = 'w' if is_first_chunk else 'a'
                    df.to_csv(OUTPUT_CSV, mode=mode, header=is_first_chunk, index=False)
                    is_first_chunk = False
                    
                    chunk_data.clear()
                    
                    # Occasional cleanup
                    if frames_processed % (CHUNK_SIZE * 2) == 0:
                        gc.collect()
                        
                except Exception as e:
                    print(f"CSV write error: {e}")

        # Process remaining data
        if chunk_data:
            try:
                df = pd.DataFrame(chunk_data)
                df['motion_change_rate'] = df['motion_intensity'].diff().fillna(0)
                df['saliency_change_rate'] = df['mean_saliency'].diff().fillna(0)
                df['attention_shift_rate'] = np.sqrt(
                    df['attention_center_x'].diff().fillna(0)**2 + 
                    df['attention_center_y'].diff().fillna(0)**2
                )
                final_columns = [
                    'time_sec', 'motion_intensity', 'motion_variance', 'motion_change_rate',
                    'mean_saliency', 'max_saliency', 'saliency_entropy', 'saliency_change_rate',
                    'attention_center_x', 'attention_center_y', 'attention_concentration', 'attention_shift_rate'
                ]
                df = df[final_columns].round(6)
                mode = 'w' if is_first_chunk else 'a'
                df.to_csv(OUTPUT_CSV, mode=mode, header=is_first_chunk, index=False)
            except Exception as e:
                print(f"Final CSV write error: {e}")

except Exception as e:
    print(f"Critical processing error: {e}")
finally:
    if cap:
        cap.release()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()

print(f"\n✅ Features saved to {OUTPUT_CSV}")
print(f"📊 Processing completed for {total_frames} frames ({duration:.2f} seconds)")
