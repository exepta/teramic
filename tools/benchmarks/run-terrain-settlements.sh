#!/usr/bin/env bash
# Actual revision-10 outpost and kingdom on natural terrain; disposable saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/terrain-settlements
run_dir=$(mktemp -d "$repo_dir/target/terrain-settlements/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import re, sys
p = Path(sys.argv[1]) / 'configs/world.toml'
s = re.sub(r'(?m)^paused\s*=.*$', 'paused = true', p.read_text())
p.write_text(s)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 12
vertical_view_distance = 4
fov_degrees = 55.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 12
fov = 55.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "high"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
debug_overlay = false
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated terrain settlement run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
unset TERRA_BIOME_ATMOSPHERE
for scene in outpost kingdom-temperate; do
    if [[ "$scene" == outpost ]]; then region='[-7,-4]'; else region='[-27,7]'; fi
    env TERRA_BIOME_SCENE="$scene" TERRA_STRUCTURE_REGION="$region" \
        timeout --signal=TERM 300s target/debug/terra-client --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
done
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib, json, struct, sys
p = Path(sys.argv[1]); result = {}
for scene in ('outpost', 'kingdom-temperate'):
    log = (p/'captures'/f'{scene}.log').read_text(errors='replace')
    assert 'Native biome fixture passed:' in log, scene
    assert 'verified occupied-bed metadata' in log, scene
    for failure in ('panicked at', 'Validation Error', 'Shader processing error'):
        assert failure not in log, (scene, failure)
    screens = {}
    for view in ('overview', 'close'):
        data = (p/'captures'/f'{scene}-{view}.png').read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        w,h = struct.unpack('>II',data[16:24]); assert w >= 1920 and h >= 1080
        screens[view] = dict(width=w, height=h, sha256=hashlib.sha256(data).hexdigest())
    result[scene] = screens
(p/'verification.json').write_text(json.dumps(dict(status='passed', structures_revision=10,
    fixture_edits=0, sites=result, note='Production terrain; overview camera disables distance fog only for inspection.'),indent=2)+'\n')
print('Terrain-following outpost and kingdom passed: generated voxels, occupied beds and four native captures')
PY
