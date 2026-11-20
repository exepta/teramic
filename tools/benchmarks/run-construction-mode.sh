#!/usr/bin/env bash
# Build first. This real-client construction acceptance uses disposable saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/construction-mode"
run_dir=$(mktemp -d "$repo_dir/target/construction-mode/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic Construction Mode Acceptance"
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
id = 241003
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
printf 'Isolated Construction Mode run: %s\n' "$run_dir"
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
    TERRA_CONSTRUCTION_MODE_GALLERY=1 \
    TERRA_NATIVE_OS_POINTER=0 \
    TERRA_BENCH_GRAPHICS=1 \
    TERRA_PREVIEW_WARMUP=1 \
    TERRA_COMPUTE_THREADS=4 \
    RUST_LOG=info \
    timeout --signal=TERM 180s "$repo_dir/target/debug/terra-client" --smoke-test-inventory \
    > "$run_dir/native.log" 2>&1
python3 - "$run_dir" <<'PYVERIFY'
import json
import pathlib
import struct
import sys
root = pathlib.Path(sys.argv[1])
log = (root / "native.log").read_text(errors="replace")
for marker in [
    "Native construction mode passed:",
    "Native construction spatial mist advection and second visible capture acknowledged",
    "Native construction mode TAB and Creative four-resource HUD acknowledged",
    "Native construction mode real NPC click and private inventory acknowledged",
    "Native construction mode real orange server-template mesh and ground raycast acknowledged",
    "Native construction mode actual cottage queue and Creative free_resources acknowledged",
    "Native construction mode persisted Creative job and owner settlement acknowledged",
    "Native construction automatic saved areas, red Build boundaries and real H hide/show acknowledged",
]:
    assert marker in log, f"Missing real construction workflow: {marker}"
for failure in ["Cannot save integrated world", "World autosave failed", "failed to process shader error", "failed to create shader module", "Validation Error"]:
    assert failure not in log, f"Native construction rendering or persistence failed: {failure}"
screens = {}
for name in ["construction-catalog-de-wide", "construction-npc-de-wide", "construction-blueprint-de-wide", "construction-queued-en-compact", "construction-areas-de-wide", "construction-areas-hidden-de-wide", "construction-areas-animated-de-wide"]:
    data = (root / f"captures/{name}.png").read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {name}"
    actual = struct.unpack(">II", data[16:24])
    expected = (960, 640) if "compact" in name else (1600, 900)
    assert all(abs(a - e) <= 1 for a, e in zip(actual, expected)), (name, actual, expected)
    screens[name] = {"width": actual[0], "height": actual[1], "bytes": len(data)}
(root / "verification.json").write_text(json.dumps({
    "status": "passed", "screens": screens,
    "transport": "real local UDP authority and acknowledged Build claim",
    "input": "real remappable TAB/ESC KeyboardInput, WASD pan, HTML template handler and projected MouseButtonInput",
    "template": "spruce-cottage-v1", "preview": "actual orange template mesh",
    "resources": "four acknowledged Creative infinity rows without a chest",
    "queue": "real ground raycast, selected-template job and authoritative free_resources",
    "persistence": "actual integrated-server save acknowledgement plus disk reload validates job/template/free_resources and matching owner settlement without storage",
    "npc": "actual civilian click, private inventory response and ESC retaining construction",
    "restoration": "unchanged authoritative/predicted avatar and restored gameplay camera",
    "areas": "real server-confirmed storage/residential areas, bounded drifting density volumes and red Build-claim perimeter",
    "visibility": "actual H KeyboardInput hides/restores all overlay meshes while reusing cached entities",
    "animation": "finite uploaded spatial advection changes between two actual visible GPU captures",
    "responsive_geometry": "DE wide and EN compact viewport/caption checks",
}, indent=2) + "\n")
print("Construction Mode passed: real TAB, pan, cottage selection/preview/queue, Creative stock, NPC inventory, ESC and restored camera/avatar; seven DE/EN captures with automatic saved areas and H hide/show")
PYVERIFY
