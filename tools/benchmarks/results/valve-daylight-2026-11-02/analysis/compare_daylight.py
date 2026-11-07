"""Compare identical fixed-camera screenshots; image changes are visual evidence,
not a luminance-meter reading. Green pixels can include grass and foliage."""
from pathlib import Path
import json
import numpy as np
import struct
import subprocess

def read_image(path):
    width, height = struct.unpack(">II", path.read_bytes()[16:24])
    raw = subprocess.check_output(["magick", str(path), "-depth", "8", "rgb:-"])
    return np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 3).astype(float) / 255

root = Path(__file__).resolve().parents[1]
result = {}
for name in ["day", "night", "underwater-day", "underwater-night"]:
    before = read_image(root / "before" / f"{name}.png")
    after = read_image(root / "after" / f"{name}.png")
    assert before.shape == after.shape
    luma = lambda im: im @ np.array([0.2126, 0.7152, 0.0722])
    a, b = luma(before), luma(after)
    region = np.indices(a.shape)[0] >= a.shape[0] * 0.52
    entry = {"region_mean_before": float(a[region].mean()), "region_mean_after": float(b[region].mean())}
    if name == "day":
        green = lambda im: (im[:, :, 1] > im[:, :, 0] * 1.15) & (im[:, :, 1] > im[:, :, 2] * 1.15)
        mask = region & green(before) & green(after) & (a > 0.15)
        assert mask.sum() > 1000
        entry.update(green_pixels=int(mask.sum()), green_mean_before=float(a[mask].mean()), green_mean_after=float(b[mask].mean()), green_p90_before=float(np.quantile(a[mask], .9)), green_p90_after=float(np.quantile(b[mask], .9)))
    result[name] = entry
(root / "analysis" / "daylight-comparison.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
