#!/usr/bin/env bash
# Run only after the coordinated build and GPU slot are ready. No Cargo here.
set -euo pipefail
report_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_dir=$(cd -- "$report_dir/../../../../.." && pwd)
mkdir -p "$repo_dir/target/phase199-rights" "$report_dir/logs"
run_dir=$(mktemp -d "$repo_dir/target/phase199-rights/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic F57 Rights Gallery"
width = 2560
height = 1440
view_distance = 4
vertical_view_distance = 3
vsync = false
debug_overlay = false
TOML
cat > "$config_dir/graphics.toml" <<'TOML'
width = 2560
height = 1440
window_mode = "windowed"
render_distance = 4
vsync = false
fps_limit = 60
ui_scale = 1.0
ui_fps = 60
shadow_quality = "medium"
water_reflections = false
weather = "clear"
weather_intensity = 0.0
debug_overlay = false
TOML
cat > "$config_dir/server.toml" <<'TOML'
natural_mob_limit = 0
simulation_distance = 1
lan_discovery = false
require_verified_accounts = false
TOML
cat > "$config_dir/world.toml" <<'TOML'
id = 199057
seed = 42042
landscape_revision = 3
structures_revision = 8
sea_level = 8.0
base_height = 12.0
hill_amplitude = 0.0
mountain_amplitude = 0.0
caves = false
cave_structure_revision = 0
rivers = false
water = false
biomes = false
trees = false
natural_mob_limit = 0
[structures]
villages = false
ruins = false
mines = false
kingdoms = false
outposts = false
[time]
start_hour = 10.0
paused = true
TOML
printf '%s\n' "$run_dir" > "$report_dir/run-path.txt"
printf 'Isolated rights gallery data: %s\n' "$run_dir"
test -x "$repo_dir/target/debug/terra-client"
if env \
  TERRA_CONFIG_DIR="$config_dir" \
  TERRA_MENU_CONFIG_DIR="$config_dir" \
  TERRA_PROFILE_FILE="$run_dir/profiles/founder.toml" \
  TERRA_WORLD_DIR="$run_dir/worlds" \
  TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" \
  TERRA_CAPTURE_DIR="$report_dir" \
  TERRA_ASSET_DIR="$repo_dir/assets" \
  TERRA_KINGDOM_UI_GALLERY=1 \
  TERRA_KINGDOM_RELOAD=0 \
  TERRA_NATIVE_OS_POINTER=0 \
  TERRA_BENCH_GRAPHICS=1 \
  TERRA_PREVIEW_WARMUP=1 \
  TERRA_COMPUTE_THREADS=4 \
  RUST_LOG=info \
  timeout --signal=TERM 240s "$repo_dir/target/debug/terra-client" --smoke-test-inventory \
  > "$report_dir/logs/native.log" 2>&1
then
  python3 - "$report_dir" "$run_dir" <<'PY'
import json, pathlib, struct, sys
root = pathlib.Path(sys.argv[1])
expected = json.loads((root / "expected.json").read_text())
log = (root / "logs/native.log").read_text(errors="replace")
assert expected["success_marker"] in log, "Native success marker is missing"
images = []
for name in expected["gallery_screens"]:
    data = (root / name).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {name}"
    width, height = struct.unpack(">II", data[16:24])
    nominal = expected["screen_sizes"][name]
    assert all(abs(a-b) <= 1 for a,b in zip((width,height),nominal)), f"Unexpected native size: {name}: {width}x{height}"
    images.append({"path": name, "width": width, "height": height, "bytes": len(data), "requested_size": nominal})
(root / "verification.json").write_text(json.dumps({"status": "passed", "exit_code": 0, "window_rounding_tolerance_pixels": 1, "private_run_directory": sys.argv[2], "success_marker": expected["success_marker"], "gallery_images": images, "setup_image": "kingdom-unbound.png"}, indent=2)+"\n")
print(f"Rights gallery passed: {len(images)} gallery captures plus the founding setup frame")
PY
else
  native_status=$?
  printf 'Rights gallery failed with exit %s; see %s\n' "$native_status" "$report_dir/logs/native.log" >&2
  exit "$native_status"
fi
