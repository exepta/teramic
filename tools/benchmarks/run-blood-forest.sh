#!/usr/bin/env bash
# Actual generated crimson woodland, droplets and revised fauna; disposable saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/blood-forest
run_dir=$(mktemp -d "$repo_dir/target/blood-forest/run.XXXXXX")
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
view_distance = 8
vertical_view_distance = 4
fov_degrees = 55.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 8
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
printf 'Isolated Blood Forest run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
unset TERRA_BIOME_ATMOSPHERE
for scene in blood-forest picker-blood-forest; do
    env TERRA_BIOME_SCENE="$scene" TERRA_CAPTURE_DIR="$run_dir/captures" \
        timeout --signal=TERM 300s target/debug/terra-client --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
done
for scene in forest spirit; do
    mkdir -p "$run_dir/$scene"
    if [[ "$scene" == forest ]]; then flag=TERRA_FOREST_INSPECT; else flag=TERRA_SPIRIT_INSPECT; fi
    env "$flag=1" TERRA_CAPTURE_DIR="$run_dir/$scene" \
        timeout --signal=TERM 180s target/debug/terra-client --smoke-test-creatures \
        > "$run_dir/$scene/native.log" 2>&1
done
python3 - "$run_dir" <<'PY'
from pathlib import Path
import json,re,struct,sys
p=Path(sys.argv[1]);screens=[]
for f in p.rglob('*.log'):
    s=f.read_text(errors='replace')
    for fail in ('panicked at','Shader processing error','Validation Error','failed to create shader module'):
        assert fail not in s,(f,fail)
for scene in ('blood-forest','picker-blood-forest'):
    log=(p/'captures'/f'{scene}.log').read_text()
    marker='Native biome preview passed:' if scene.startswith('picker-') else 'Native biome fixture passed:'
    assert marker in log,scene
    views=('overview',) if scene.startswith('picker-') else ('overview','close')
    for view in views:assert (p/'captures'/f'{scene}-{view}.png').exists()
log=(p/'captures/blood-forest.log').read_text()
assert 'Native Blood Forest uses natural blood water and leaf undersides' in log
assert 'Native Blood Forest droplet inspection' in log
droplets=json.loads((p/'captures/blood-forest-droplets.json').read_text())
assert droplets['fixture_edits']==0 and droplets['status']=='passed'
close=droplets['views']['close']
assert close['observed_seconds']>=2 and close['samples']>=2,close
assert 0<close['peak_candidates']<=192 and 0<close['peak_falling']<=192,close
assert close['active_frames']>0 and close['moving_frames']>0,close
assert close['maximum_pool_capacity']==192 and close['maximum_scanned_sections']<=32,close
assert close['water_triangles']>0,close
for filename, expected_rays in [('blood-forest-view.json',5),('picker-blood-forest-view.json',1)]:
    report=json.loads((p/'captures'/filename).read_text())
    assert report['fixture_edits']==0
    overview=report['captures']['0']
    assert overview['camera_in_loaded_air'] and len(overview['sightlines'])==expected_rays,overview
    assert overview['ambient_brightness']>0 and overview['exposure_ev100']>0,overview
    assert 0<=overview['cave_blend']<=1,overview
    for ray in overview['sightlines']:
        assert ray['clear_loaded_voxels'],ray
        assert abs(ray['ndc'][0])<.85 and abs(ray['ndc'][1])<.85,ray
        if expected_rays==5:assert ray['water']['depth']>0,ray
for scene in ('forest','spirit'):
    log=(p/scene/'native.log').read_text()
    assert 'gallery' in log and ('passed' in log or 'rendered' in log),scene
    assert len(list((p/scene).glob('*.png')))>=4,scene
for f in p.rglob('*.png'):
    data=f.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n';w,h=struct.unpack('>II',data[16:24]);assert w>=1536 and h>=768
    screens.append(str(f.relative_to(p)))
(p/'verification.json').write_text(json.dumps({'status':'passed','fixture_edits':0,'screens':screens,'droplets':droplets,'world':'real generated Blood Forest with blood_forest_revision=1'},indent=2)+'\n')
print('Natural Blood Forest, animated leaf droplets, skeleton/Blood fauna and redesigned Spirit gallery passed')
PY
