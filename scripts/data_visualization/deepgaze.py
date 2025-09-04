import torch
from deepgaze_pytorch import DeepGazeIIE
import cv2
import numpy as np
import pandas as pd
from tqdm import tqdm

model = DeepGazeIIE(pretrained=True)

print("DeepGazeIIE imported successfully")

##########
## ARGS ##
##########

parser = argparse.ArgumentParser(description="Extract motion and saliency features using DeepGaze IIE")
parser.add_argument("input_video", type=str, help="Path to input video file")
parser.add_argument("output_csv", type=str, help="Path to output CSV file")
parser.add_argument("--frame_size", type=int, nargs=2, default=(224, 224), help="Frame size for model input (width height)")
parser.add_argument("--batch_size", type=int, default=64, help="Batch size for GPU inference")
args = parser.parse_args()

VIDEO_PATH = args.input_video
OUTPUT_CSV = args.output_csv
FRAME_SIZE = tuple(args.frame_size)
BATCH_SIZE = args.batch_size
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"


################
## LOAD MODEL ##
################

model = DeepGazeII(pretrained = True)
model.to(DEVICE)
model.eval()


###############
## VIDEO CAP ##
###############

cap = cv2.VideoCapture(VIDEO_PATH)
fps = cap.get(cv2.CAP_PROP_FPS)
total_frames = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))


#############
## PROCESS ##
#############
saliency_features = []
prev_gray = None
frames_batch = []
timestamps_batch = []

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
        timestamps_batch.append(frame_idx / fps)

        # Motion (grayscale frame difference)
        gray = cv2.cvtColor(frame_resized, cv2.COLOR_BGR2GRAY)
        motion = float(np.mean(np.abs(gray - prev_gray))) if prev_gray is not None else 0.0
        prev_gray = gray

        # Store temporary motion value
        saliency_features.append({
            "time_sec": frame_idx / fps,
            "motion": motion,
            "mean_salience": 0.0,
            "max_salience": 0.0
        })

        # Batch inference
        if len(frames_batch) == BATCH_SIZE or frame_idx == total_frames - 1:
            batch_tensor = torch.from_numpy(np.array(frames_batch)).permute(0, 3, 1, 2).float() / 255.0
            batch_tensor = batch_tensor.to(DEVICE)

            with torch.no_grad():
                saliency_maps = model(batch_tensor)  # (batch, 1, H, W)
                saliency_maps = saliency_maps.cpu().numpy()

            # Update saliency metrics
            for i, sal_map in enumerate(saliency_maps):
                sal_map = sal_map[0]  # single channel
                mean_sal = float(np.mean(sal_map))
                max_sal = float(np.max(sal_map))
                idx = frame_idx - len(frames_batch) + 1 + i
                saliency_features[idx]["mean_salience"] = mean_sal
                saliency_features[idx]["max_salience"] = max_sal

            frames_batch = []
            timestamps_batch = []

        frame_idx += 1
        pbar.update(1)

cap.release()

# ------------------------
# SAVE TO CSV
# ------------------------
df = pd.DataFrame(saliency_features)
df.to_csv(OUTPUT_CSV, index=False)
print(f"Features saved to {OUTPUT_CSV}")
