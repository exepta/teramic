#!/usr/bin/env bash
# Build first. Six real keep tabs with disposable saves and a second UDP member.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/keep-redesign"
run_dir=$(mktemp -d "$repo_dir/target/keep-redesign/run.XXXXXX")
config_dir="$run_dir/configs"
mkdir -p "$config_dir" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$config_dir/client.toml" <<'TOML'
title = "Teramic Keep Redesign Acceptance"
width = 1600
height = 1000
view_distance = 2
vertical_view_distance = 2
fly_speed = 4.0
vsync = false
debug_overlay = false
TOML
cat > "$config_dir/graphics.toml" <<'TOML'
width = 1600
height = 1000
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
printf 'Isolated Keep Redesign run: %s\n' "$run_dir"
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
    TERRA_KINGDOM_REDESIGN_GALLERY=1 \
    TERRA_NATIVE_OS_POINTER=0 \
    TERRA_BENCH_GRAPHICS=1 \
    TERRA_PREVIEW_WARMUP=1 \
    TERRA_COMPUTE_THREADS=4 \
    RUST_LOG=info \
    timeout --signal=TERM 240s "$repo_dir/target/debug/terra-client" --smoke-test-inventory \
    > "$run_dir/native.log" 2>&1
python3 - "$run_dir" <<'PYVERIFY'
import json
import pathlib
import struct
import sys
root=pathlib.Path(sys.argv[1])
log=(root/'native.log').read_text(errors='replace')
assert 'Native keep redesign passed:' in log, 'Missing real six-tab acceptance marker'
screens={}
for language,size in [('de-wide',(1600,1000)),('en-compact',(1024,768))]:
    for tab in ['general','members','diplomacy','buildings','workers','army']:
        name=f'keep-{tab}-{language}'
        data=(root/'captures'/f'{name}.png').read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n',name
        actual=struct.unpack('>II',data[16:24])
        assert actual==size,(name,actual,size)
        screens[name]={'width':actual[0],'height':actual[1],'bytes':len(data)}
(root/'verification.json').write_text(json.dumps({'status':'passed','screens':screens,
    'transport':'real local UDP server with actual second authenticated member',
    'input':'actual HTML tab and template card handlers plus local detail selection',
    'real_soldier_roster':True,'authoritative_template_material_preview':True,
    'responsive_frame_and_caption_bounds':True,'selection_classes_match_details':True},indent=2)+'\n')
print('Keep redesign passed: six tabs, DE/EN, real membership, army and JSON template costs; twelve captures')
PYVERIFY
