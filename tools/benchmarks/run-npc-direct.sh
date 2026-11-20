#!/usr/bin/env bash
# Short real-client acceptance. Build first; all profiles/worlds stay isolated.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/npc-work-direct"
run_dir=$(mktemp -d "$repo_dir/target/npc-work-direct/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic Direct NPC Acceptance"
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
id = 236002
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
printf 'Isolated direct NPC run: %s\n' "$run_dir"
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
    TERRA_NPC_WORK_DIRECT_GALLERY=1 \
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
assert "Native direct NPC work passed:" in log, "Native acceptance marker is missing"
assert "Native lumberjack station passed:" in log, "Lumberjack station progression was not acknowledged"
assert "Native Kingdom hotkey opened the authoritative own-territory keep" in log, "Kingdom hotkey was not acknowledged"
screens = {}
for name in ["npc-direct-de-wide", "npc-selection-de-wide", "npc-selection-en-compact", "npc-direct-en-wide", "npc-direct-de-compact", "npc-direct-en-compact", "npc-direct-en-compact-paused", "npc-lumberjack-missing-station-compact", "npc-lumberjack-profession-compact"]:
    path = root / f"captures/{name}.png"
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {name}"
    size = struct.unpack(">II", data[16:24])
    expected = (960, 640) if "compact" in name else (1600, 900)
    assert size == expected, f"Wrong size for {name}: {size}"
    screens[name] = {"width": size[0], "height": size[1], "bytes": len(data)}
(root / "verification.json").write_text(json.dumps({"status": "passed", "screens": screens, "input": "real target raycasts and frame-controlled gameplay/HTML handlers", "transport": "real local UDP server", "private_inventory": True, "kingdom_hotkey": True, "responsive_geometry": True}, indent=2) + "\n")
print("Direct NPC passed: right-click, private inventory, HUD selection/cancel, real assignment/pause and real lumberjack station progression with nine captures")
PY
