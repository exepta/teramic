#!/usr/bin/env bash
# Native ecology6 acceptance: real Blood/Spirit props and multi-block rocks.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/surface-ecology
run_dir=$(mktemp -d "$repo_dir/target/surface-ecology/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]);s=p.read_text();s=re.sub(r'(?m)^paused\s*=.*$','paused = true',s);p.write_text(s)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 6
vertical_view_distance = 4
fov_degrees = 55.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 6
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
printf 'Isolated surface ecology run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info TERRA_CAPTURE_DIR="$run_dir/captures"
unset TERRA_BIOME_ATMOSPHERE TERRA_ECOLOGY_ANCHOR TERRA_BLOOD_VIEW
for scene in blood-forest picker-blood-forest blood-props spirit-props rock-formations; do
    env TERRA_BIOME_SCENE="$scene" \
        timeout --signal=TERM 240s target/debug/terra-client --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
done
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,struct,sys,tomllib
p=Path(sys.argv[1]);screens={}
assert tomllib.loads((p/'configs/world.toml').read_text())['ecology_revision']==6
for scene in ('blood-forest','picker-blood-forest','blood-props','spirit-props','rock-formations'):
    log=(p/'captures'/f'{scene}.log').read_text(errors='replace')
    marker='Native biome preview passed:' if scene.startswith('picker-') else 'Native biome fixture passed:'
    assert marker in log,scene
    for error in ('panicked at','Validation Error','Shader processing error','failed to create shader module'):
        assert error not in log,(scene,error)
    for view in (('overview',) if scene.startswith('picker-') else ('overview','close')):
        f=p/'captures'/f'{scene}-{view}.png';data=f.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n'
        w,h=struct.unpack('>II',data[16:24]);assert w>=1920 and h>=1080
        screens[f.name]=dict(width=w,height=h,sha256=hashlib.sha256(data).hexdigest())
for scene in ('blood-props','spirit-props','rock-formations'):
    report=json.loads((p/'captures'/f'{scene}-source.json').read_text())
    assert report['ecology_revision']==6 and report['fixture_edits']==0
    assert report['source_cells'],scene
    if scene=='rock-formations':assert report['extra']['formation_voxels']>24 and len(report['source_cells'])==4
droplets=json.loads((p/'captures/blood-forest-droplets.json').read_text())
assert droplets['status']=='passed' and droplets['fixture_edits']==0
assert droplets['views']['close']['moving_frames']>0
(p/'verification.json').write_text(json.dumps(dict(status='passed',ecology_revision=6,
    fixture_edits=0,screens=screens,droplets=droplets),indent=2)+'\n')
print('Natural voxel props, Blood palette and multi-block rock formations passed: nine native captures')
PY
