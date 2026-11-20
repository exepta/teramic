#!/usr/bin/env bash
# Short F80 acceptance against an already-built client. Never touches player saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/kingdom-starters"
run_dir=$(mktemp -d "$repo_dir/target/kingdom-starters/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic F80 Founding Acceptance"
width = 1600
height = 900
view_distance = 2
vertical_view_distance = 2
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
id = 235080
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
printf 'Isolated F80 run: %s\n' "$run_dir"
test -x "$repo_dir/target/debug/terra-client"
for reload in 0 1; do
    env \
        TERRA_CONFIG_DIR="$config_dir" \
        TERRA_MENU_CONFIG_DIR="$config_dir" \
        TERRA_PROFILE_FILE="$run_dir/profiles/founder.toml" \
        TERRA_WORLD_DIR="$run_dir/worlds" \
        TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" \
        TERRA_CAPTURE_DIR="$run_dir/captures" \
        TERRA_ASSET_DIR="$repo_dir/assets" \
        TERRA_KINGDOM_SMOKE=1 \
        TERRA_KINGDOM_STARTERS=1 \
        TERRA_KINGDOM_RELOAD="$reload" \
        TERRA_NATIVE_OS_POINTER=1 \
        TERRA_BENCH_GRAPHICS=1 \
        TERRA_PREVIEW_WARMUP=1 \
        TERRA_COMPUTE_THREADS=4 \
        RUST_LOG=info \
        timeout --signal=TERM 210s "$repo_dir/target/debug/terra-client" --smoke-test-inventory \
        > "$run_dir/native-$reload.log" 2>&1
done
python3 - "$run_dir" <<'PY'
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
markers = ["Native kingdom creation passed", "Native kingdom reload passed"]
for index, marker in enumerate(markers):
    log = (root / f"native-{index}.log").read_text(errors="replace")
    assert marker in log, f"Missing acceptance marker: {marker}"
    assert "Native F80 accepted: 18 persisted and replicated starters" in log
receipt = json.loads((root / "captures/kingdom-starters.json").read_text())
assert len(set(receipt["ids"])) == 18
assert receipt["roles"] == {"10": 3, "12": 2, "11": 3, "15": 10}
for name in ["unbound", "founded", "renamed", "reloaded", "dissolve-confirm", "dissolved"]:
    image = (root / f"captures/kingdom-{name}.png").read_bytes()
    assert image[:8] == b"\x89PNG\r\n\x1a\n"
    assert struct.unpack(">II", image[16:24]) == (1600, 900)
print("F80 passed: real Found/Rename/Save/Reload/Dissolve, 18 replicated NPCs, stable IDs and ownership")
PY
