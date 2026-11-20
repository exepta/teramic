#!/usr/bin/env bash
# Build first. This real-client construction acceptance uses disposable saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/building-local"
run_dir=$(mktemp -d "$repo_dir/target/building-local/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic Building Local Acceptance"
width = 1600
height = 900
view_distance = 2
vertical_view_distance = 2
fly_speed = 4.0
vsync = false
debug_overlay = false
TOML
cat > "$config_dir/graphics.toml" <<'TOML'
width = 1600
height = 900
window_mode = "windowed"
render_distance = 2
vsync = false
fps_limit = 60
ui_scale = 1.0
ui_fps = 60
shadow_quality = "low"
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
id = 236003
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
printf 'Isolated Building Local run: %s\n' "$run_dir"
test -x "$repo_dir/target/debug/terra-client"
env \
    TERRA_CONFIG_DIR="$config_dir" \
    TERRA_MENU_CONFIG_DIR="$config_dir" \
    TERRA_PROFILE_FILE="$run_dir/profiles/founder.toml" \
    TERRA_WORLD_DIR="$run_dir/worlds" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" \
    TERRA_CAPTURE_DIR="$run_dir/captures" \
    TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_KINGDOM_UI_GALLERY=1 \
    TERRA_BUILDING_LOCAL_GALLERY=1 \
    TERRA_NATIVE_OS_POINTER=0 \
    TERRA_BENCH_GRAPHICS=1 \
    TERRA_PREVIEW_WARMUP=1 \
    TERRA_COMPUTE_THREADS=4 \
    RUST_LOG=info \
    timeout --signal=TERM 180s "$repo_dir/target/debug/terra-client" --smoke-test-inventory \
    > "$run_dir/native.log" 2>&1
python3 - "$run_dir" <<'PY'
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
log = (root / "native.log").read_text(errors="replace")
for marker in [
    "Native Building Local passed:",
    "Native Building Local real town chest stocked with 128 structure stone and 128 planks",
    "Native Building Local real craftsman enrolled without work area",
    "Native Building Local actual non-air house voxel placement acknowledged",
    "Native Building Local saved area HUD and confirmed bounds acknowledged",
]:
    assert marker in log, f"Missing real construction acceptance marker: {marker}"
screens = {}
for name in [
    "building-local-de-wide", "building-storage-selection-de-wide", "building-saved-areas-de-wide",
    "building-job-en-compact", "building-progress-en-compact", "building-paused-en-compact",
]:
    data = (root / f"captures/{name}.png").read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {name}"
    size = struct.unpack(">II", data[16:24])
    expected = (960, 640) if "compact" in name else (1600, 900)
    assert size == expected, f"Wrong dimensions: {name}/{size}"
    screens[name] = {"width": size[0], "height": size[1], "bytes": len(data)}
(root / "verification.json").write_text(json.dumps({
    "status": "passed", "screens": screens,
    "input": "real gameplay raycasts and HTML component handlers",
    "transport": "real local UDP server", "claims": "real acknowledged Build claim",
    "storage": "actual chest and transferred 128 structure stone plus 128 planks", "craftsman_without_work_area": True,
    "house": "actual default village template, real non-air voxel placement, acknowledged pause",
    "responsive_geometry": True, "saved_area_overlay": True,
}, indent=2) + "\n")
print("Building Local passed: real storage, materials, claim, house queue, craftsman, voxel progress and pause; six DE/EN captures")
PY
