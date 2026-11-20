#!/usr/bin/env bash
# Inspect the real shared actor geometry in disposable scenes.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/model-galleries
run_dir=$(mktemp -d "$repo_dir/target/model-galleries/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 3
vertical_view_distance = 3
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 3
vsync = false
fps_limit = 60
shadow_quality = "high"
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
spirit_blossom_revision = 2
[time]
start_hour = 10.0
paused = true
TOML
printf 'Isolated model gallery run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
for scene in spirit villagers; do
    mkdir -p "$run_dir/$scene"
    if [[ "$scene" == spirit ]]; then flag=TERRA_SPIRIT_INSPECT; else flag=TERRA_SMOKE_VILLAGERS; fi
    env "$flag=1" TERRA_CAPTURE_DIR="$run_dir/$scene" \
        timeout --signal=TERM 180s target/debug/terra-client --smoke-test-creatures \
        > "$run_dir/$scene/native.log" 2>&1
done
python3 - "$run_dir" <<'PYTHON'
from pathlib import Path
import json,struct,sys
root=Path(sys.argv[1]);results={}
for scene,marker in [('spirit','Spirit gallery rendered seven butterfly variants'),('villagers','Villager gallery passed:')]:
    log=(root/scene/'native.log').read_text(errors='replace')
    assert marker in log,scene
    for failure in ('panicked at','Validation Error','Shader processing error','failed to create shader module'):
        assert failure not in log,(scene,failure)
    screens=list((root/scene).glob('*.png')); assert len(screens)>=4,(scene,len(screens))
    for screen in screens:
        data=screen.read_bytes(); assert data[:8]==b'\x89PNG\r\n\x1a\n'
        w,h=struct.unpack('>II',data[16:24]); assert w>=1536 and h>=768
    results[scene]=[p.name for p in screens]
(root/'verification.json').write_text(json.dumps(dict(status='passed',screens=results),indent=2)+'\n')
print('Real Spirit and villager model gallery captures passed')
PYTHON
