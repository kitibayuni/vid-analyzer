import argparse
import torch
from deepgaze_pytorch import DeepGazeIIE
import cv2
import numpy as np
import pandas as pd
from tqdm import tqdm
import warnings
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
parser.add_argument("--window_sec", type=float, default=0.5, help="Time window for smoothing features (seconds)")
args = parser.parse_args()

VIDEO_PATH = args.input_video
OUTPUT_CSV = args.output_csv
FRAME_SIZE = tuple(args.frame_size)
BATCH_SIZE = args.batch_size
WINDOW_SEC = args.window_sec
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"

print(f"Using device: {DEVICE}")
print(f"Frame size: {FRAME_SIZE}, Batch size: {BATCH_SIZE}")

################
## LOAD MODEL ##
################
model = DeepGazeIIE(pretrained=True)
model.to(DEVICE)
model.eval()

###############
## VIDEO CAP ##
###############
cap = cv2.VideoCapture(VIDEO_PATH)
fps = cap.get(cv2.CAP_PROP_FPS)
total_frames = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
duration = total_frames / fps

print(f"Video: {total_frames} frames @ {fps:.2f} FPS ({duration:.2f} seconds)")

######################
## HELPER FUNCTIONS ##
######################
def calculate_saliency_features(saliency_map):
    """Calculate saliency and attention features from a probability map"""
    if len(saliency_map.shape) > 2:
        saliency_map = saliency_map.squeeze()

    # Saliency features
    mean_sal = float(np.mean(saliency_map))
    max_sal = float(np.max(saliency_map))
    
    # Saliency entropy
    epsilon = 1e-10
    sal_norm = saliency_map + epsilon
    sal_norm = sal_norm / np.sum(sal_norm)
    entropy = -np.sum(sal_norm * np.log2(sal_norm))

    # Attention features
    h, w = saliency_map.shape
    y_coords, x_coords = np.mgrid[0:h, 0:w]
    total_mass = np.sum(saliency_map)
    
    if total_mass > 0:
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

####################
## PROCESS VIDEO  ##
####################
features_data = []
prev_gray = None
frames_batch = []

with tqdm(total=total_frames, desc="Processing Video") as pbar:
    frame_idx = 0
    while True:
        ret, frame = cap.read()
        if not ret:
            break

        # Resize and RGB conversion
        frame_resized = cv2.resize(frame, FRAME_SIZE)
        frame_rgb = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2RGB)
        frames_batch.append(frame_rgb)

        # Grayscale frame for motion analysis
        gray = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2GRAY)

        # Motion features
        if prev_gray is not None:
            diff = cv2.absdiff(gray, prev_gray)
            motion_intensity = float(np.mean(diff)) / 255.0
            motion_variance = float(np.std(diff)) / 255.0
        else:
            motion_intensity = motion_variance = 0.0

        prev_gray = gray

        # Store frame data
        frame_data = {
            "time_sec": frame_idx / fps,
            "frame_idx": frame_idx,
            "motion_intensity": motion_intensity,
            "motion_variance": motion_variance,
            # Initialize saliency and attention features
            "mean_saliency": 0.0,
            "max_saliency": 0.0,
            "saliency_entropy": 0.0,
            "attention_center_x": 0.5,
            "attention_center_y": 0.5,
            "attention_concentration": 0.0
        }

        features_data.append(frame_data)

        # Batch inference for saliency
        if len(frames_batch) == BATCH_SIZE or frame_idx == total_frames - 1:
            batch_tensor = torch.from_numpy(np.array(frames_batch)).permute(0, 3, 1, 2).float() / 255.0
            batch_tensor = batch_tensor.to(DEVICE)

            with torch.no_grad():
                batch_size = batch_tensor.shape[0]
                h, w = batch_tensor.shape[2], batch_tensor.shape[3]
                centerbias = torch.zeros(batch_size, h, w).to(DEVICE)

                saliency_logits = model(batch_tensor, centerbias=centerbias)
                batch_size, channels, h, w = saliency_logits.shape
                saliency_probs = torch.nn.functional.softmax(
                    saliency_logits.view(batch_size, -1), dim=1
                ).view(batch_size, channels, h, w)
                saliency_maps = saliency_probs.cpu().numpy()

            # Update features with saliency data
            for i, sal_map in enumerate(saliency_maps):
                sal_map_2d = sal_map[0]
                sal_features = calculate_saliency_features(sal_map_2d)
                idx = frame_idx - len(frames_batch) + 1 + i
                features_data[idx].update(sal_features)

            frames_batch = []

        frame_idx += 1
        pbar.update(1)

cap.release()

#################################
## POST-PROCESSING & SMOOTHING ##
#################################
print("Computing derived features...")

df = pd.DataFrame(features_data)

# Calculate change rates (derivatives)
df['motion_change_rate'] = df['motion_intensity'].diff().fillna(0)
df['saliency_change_rate'] = df['mean_saliency'].diff().fillna(0)
df['attention_shift_rate'] = np.sqrt(
    df['attention_center_x'].diff().fillna(0)**2 + 
    df['attention_center_y'].diff().fillna(0)**2
)

# Final column order - only the requested features plus time
final_columns = [
    'time_sec', 
    'frame_idx',
    # Motion features
    'motion_intensity', 
    'motion_variance', 
    'motion_change_rate',
    # Saliency features  
    'mean_saliency', 
    'max_saliency', 
    'saliency_entropy', 
    'saliency_change_rate',
    # Attention features
    'attention_center_x', 
    'attention_center_y', 
    'attention_concentration', 
    'attention_shift_rate'
]

df = df[final_columns]

# Round numerical values
numerical_cols = df.select_dtypes(include=[np.number]).columns
df[numerical_cols] = df[numerical_cols].round(6)

# Save to CSV
df.to_csv(OUTPUT_CSV, index=False)

print(f"\n✅ Features saved to {OUTPUT_CSV}")
print(f"📊 Analysis Summary:")
print(f"   Total frames: {len(df)}")
print(f"   Duration: {duration:.2f} seconds")
print(f"   Motion intensity range: {df['motion_intensity'].min():.4f} - {df['motion_intensity'].max():.4f}")
print(f"   Saliency range: {df['mean_saliency'].min():.4f} - {df['mean_saliency'].max():.4f}")
print(f"   Attention concentration range: {df['attention_concentration'].min():.3f} - {df['attention_concentration'].max():.3f}")
print(f"\n📋 Output columns: {', '.join(final_columns)}")
