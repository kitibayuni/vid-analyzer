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
parser = argparse.ArgumentParser(description="Extract motion and saliency features using DeepGaze IIE")
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

def calculate_saliency_features(saliency_map):
    """Calculate comprehensive saliency features from a probability map"""
    # Ensure we have a 2D array
    if len(saliency_map.shape) > 2:
        saliency_map = saliency_map.squeeze()
    
    # Basic statistics
    mean_sal = float(np.mean(saliency_map))
    max_sal = float(np.max(saliency_map))
    std_sal = float(np.std(saliency_map))
    
    # Entropy (information content)
    # Add small epsilon to avoid log(0)
    epsilon = 1e-10
    sal_norm = saliency_map + epsilon
    sal_norm = sal_norm / np.sum(sal_norm)  # Normalize to probability
    entropy = -np.sum(sal_norm * np.log2(sal_norm))
    
    # Center of mass (where attention is focused)
    h, w = saliency_map.shape
    y_coords, x_coords = np.mgrid[0:h, 0:w]
    total_mass = np.sum(saliency_map)
    if total_mass > 0:
        center_y = np.sum(y_coords * saliency_map) / total_mass / h  # Normalized 0-1
        center_x = np.sum(x_coords * saliency_map) / total_mass / w  # Normalized 0-1
    else:
        center_y = center_x = 0.5
    
    # Concentration (how focused vs distributed the attention is)
    # Calculate distance from center of mass
    center_y_px = center_y * h
    center_x_px = center_x * w
    distances = np.sqrt((y_coords - center_y_px)**2 + (x_coords - center_x_px)**2)
    if total_mass > 0:
        mean_distance = np.sum(distances * saliency_map) / total_mass
        # Normalize by maximum possible distance
        max_distance = np.sqrt(h**2 + w**2) / 2
        concentration = 1.0 - (mean_distance / max_distance)  # 1 = focused, 0 = spread out
    else:
        concentration = 0.0
    
    # Top 10% saliency mass (how much attention is in the most salient regions)
    threshold_90 = np.percentile(saliency_map, 90)
    top_10_percent_mass = np.sum(saliency_map[saliency_map >= threshold_90]) / total_mass if total_mass > 0 else 0.0
    
    return {
        'mean_saliency': mean_sal,
        'max_saliency': max_sal,
        'std_saliency': std_sal,
        'saliency_entropy': entropy,
        'attention_center_x': center_x,
        'attention_center_y': center_y,
        'attention_concentration': concentration,
        'top10_saliency_mass': top_10_percent_mass
    }

def smooth_series(data, window_frames):
    """Apply moving average smoothing"""
    if window_frames <= 1:
        return data
    return pd.Series(data).rolling(window=window_frames, center=True, min_periods=1).mean().values

#############
## PROCESS ##
#############
saliency_features = []
prev_gray = None
frames_batch = []

with tqdm(total=total_frames, desc="Processing Video") as pbar:
    frame_idx = 0
    while True:
        ret, frame = cap.read()
        if not ret:
            break
        
        # Resize and convert to RGB
        frame_resized = cv2.resize(frame, FRAME_SIZE)
        frame_rgb = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2RGB)
        frames_batch.append(frame_rgb)
        
        # Motion (grayscale frame difference) - improved calculation
        gray = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2GRAY)
        if prev_gray is not None:
            # Calculate optical flow magnitude as motion metric
            diff = cv2.absdiff(gray, prev_gray)
            motion_intensity = float(np.mean(diff)) / 255.0  # Normalize 0-1
            motion_variance = float(np.std(diff)) / 255.0    # Motion consistency
        else:
            motion_intensity = 0.0
            motion_variance = 0.0
        prev_gray = gray
        
        # Store frame data with improved motion metrics
        frame_data = {
            "time_sec": frame_idx / fps,
            "frame_idx": frame_idx,
            "motion_intensity": motion_intensity,
            "motion_variance": motion_variance,
        }
        
        # Initialize saliency features (will be filled during batch processing)
        saliency_keys = ['mean_saliency', 'max_saliency', 'std_saliency', 'saliency_entropy',
                        'attention_center_x', 'attention_center_y', 'attention_concentration', 
                        'top10_saliency_mass']
        for key in saliency_keys:
            frame_data[key] = 0.0
            
        saliency_features.append(frame_data)
        
        # Batch inference
        if len(frames_batch) == BATCH_SIZE or frame_idx == total_frames - 1:
            batch_tensor = torch.from_numpy(np.array(frames_batch)).permute(0, 3, 1, 2).float() / 255.0
            batch_tensor = batch_tensor.to(DEVICE)
            
            with torch.no_grad():
                # Create proper centerbias tensor (log probabilities, uniform = 0)
                batch_size = batch_tensor.shape[0]
                height, width = batch_tensor.shape[2], batch_tensor.shape[3]
                centerbias = torch.zeros(batch_size, height, width).to(DEVICE)
                
                # Get saliency predictions (these are already log probabilities)
                saliency_logits = model(batch_tensor, centerbias=centerbias)  # (batch, 1, H, W)
                
                # Convert to probabilities using softmax over spatial dimensions
                batch_size, channels, h, w = saliency_logits.shape
                saliency_probs = torch.nn.functional.softmax(
                    saliency_logits.view(batch_size, -1), dim=1
                ).view(batch_size, channels, h, w)
                
                saliency_maps = saliency_probs.cpu().numpy()
            
            # Update saliency metrics for each frame in batch
            for i, sal_map in enumerate(saliency_maps):
                sal_map_2d = sal_map[0]  # Remove channel dimension
                
                # Calculate comprehensive saliency features
                sal_features = calculate_saliency_features(sal_map_2d)
                
                # Update the corresponding frame's data
                idx = frame_idx - len(frames_batch) + 1 + i
                for key, value in sal_features.items():
                    saliency_features[idx][key] = value
            
            frames_batch = []
        
        frame_idx += 1
        pbar.update(1)

cap.release()

# ------------------------
# POST-PROCESSING & SMOOTHING
# ------------------------
print("Post-processing and smoothing features...")

# Convert to DataFrame for easier manipulation
df = pd.DataFrame(saliency_features)

# Apply temporal smoothing
window_frames = max(1, int(fps * WINDOW_SEC))
print(f"Applying {WINDOW_SEC}s smoothing window ({window_frames} frames)")

# Smooth motion and saliency features
smooth_columns = ['motion_intensity', 'motion_variance', 'mean_saliency', 'max_saliency', 
                 'std_saliency', 'attention_concentration', 'top10_saliency_mass']

for col in smooth_columns:
    if col in df.columns:
        df[f'{col}_smooth'] = smooth_series(df[col].values, window_frames)

# Add derivative features (rate of change)
df['motion_change_rate'] = df['motion_intensity'].diff().fillna(0)
df['saliency_change_rate'] = df['mean_saliency'].diff().fillna(0)
df['attention_shift_rate'] = np.sqrt(
    df['attention_center_x'].diff().fillna(0)**2 + 
    df['attention_center_y'].diff().fillna(0)**2
)

# Add summary statistics
df['is_high_motion'] = df['motion_intensity_smooth'] > df['motion_intensity_smooth'].quantile(0.75)
df['is_high_saliency'] = df['mean_saliency_smooth'] > df['mean_saliency_smooth'].quantile(0.75)
df['is_focused_attention'] = df['attention_concentration'] > df['attention_concentration'].quantile(0.75)

# ------------------------
# SAVE TO CSV
# ------------------------
# Reorder columns for better readability
column_order = ['time_sec', 'frame_idx', 
               'motion_intensity', 'motion_intensity_smooth', 'motion_variance', 'motion_change_rate',
               'mean_saliency', 'mean_saliency_smooth', 'max_saliency', 'std_saliency',
               'saliency_entropy', 'saliency_change_rate',
               'attention_center_x', 'attention_center_y', 'attention_concentration', 
               'attention_shift_rate', 'top10_saliency_mass',
               'is_high_motion', 'is_high_saliency', 'is_focused_attention']

# Add any remaining columns
remaining_cols = [col for col in df.columns if col not in column_order]
column_order.extend(remaining_cols)

df = df[column_order]

# Round numerical columns for cleaner output
numerical_cols = df.select_dtypes(include=[np.number]).columns
df[numerical_cols] = df[numerical_cols].round(6)

# Save to CSV
df.to_csv(OUTPUT_CSV, index=False)

########################
## SUMMARY STATISTICS ##
########################
print(f"\n✅ Features saved to {OUTPUT_CSV}")
print(f"📊 Analysis Summary:")
print(f"   Total frames: {len(df)}")
print(f"   Duration: {duration:.2f} seconds")
print(f"   Average motion intensity: {df['motion_intensity'].mean():.4f}")
print(f"   Average saliency: {df['mean_saliency'].mean():.4f}")
print(f"   High motion frames: {df['is_high_motion'].sum()} ({df['is_high_motion'].mean()*100:.1f}%)")
print(f"   High saliency frames: {df['is_high_saliency'].sum()} ({df['is_high_saliency'].mean()*100:.1f}%)")
print(f"   Attention concentration range: {df['attention_concentration'].min():.3f} - {df['attention_concentration'].max():.3f}")
