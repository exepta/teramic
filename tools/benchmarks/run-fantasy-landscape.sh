#!/usr/bin/env bash
# Actual fantasy relief and routed river; all configs and saves are disposable.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/fantasy-landscape
run_dir=$(mktemp -d "$repo_dir/target/fantasy-landscape/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys,tomllib
p=Path(sys.argv[1]);text=p.read_text()
text=re.sub(r'(?m)^paused\s*=.*$', 'paused = true', text)
text=re.sub(r'(?m)^start_hour\s*=.*$', 'start_hour = 10.0', text)
p.write_text(text)
world=tomllib.loads(text)
assert (world['landscape_revision'],world['river_terrain_revision'],world['ecology_revision'])==(7,6,11)
assert world['rivers'] and world['water']
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 6
vertical_view_distance = 4
fov_degrees = 55.0
# Detached overview cameras sit beyond normal gameplay fog; expose the actual
# generated shape for inspection without altering persisted world geometry.
fog_distance = 0.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 6
fog_distance = 0.0
fov = 55.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "high"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated native fantasy landscape run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
unset TERRA_HYDRO_ANCHOR TERRA_BIOME_ATMOSPHERE TERRA_REFERENCE_BIOMES TERRA_REFERENCE_ANCHOR TERRA_FANTASY_ROCKS TERRA_BIOME_MOTES
for scene in connected-river picker-plains; do
    env TERRA_BIOME_SCENE="$scene" \
        timeout --signal=TERM --kill-after=10s 210s "$client_bin" --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
    printf 'Completed %s\n' "$scene"
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys,tomllib
root=Path(sys.argv[1]);captures=[]
world=tomllib.loads((root/'configs/world.toml').read_text())
assert (world['landscape_revision'],world['river_terrain_revision'],world['ecology_revision'])==(7,6,11)
for scene in ('connected-river','picker-plains'):
    log=re.sub(r'\x1b\[[0-9;]*m','',(root/'captures'/f'{scene}.log').read_text(errors='replace'))
    assert 'Native biome fixture passed:' in log if scene=='connected-river' else 'Native biome preview passed:' in log,scene
    assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log),scene
    # The standard biome picker takes one panorama; routed-water scenes take
    # both an overview and a close view through their normal capture path.
    for view in (('overview','close') if scene=='connected-river' else ('overview',)):
        path=root/'captures'/f'{scene}-{view}.png';data=path.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080),path
        captures.append(dict(file=str(path.relative_to(root)),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
river=json.loads((root/'captures/connected-river-source.json').read_text())
assert river['status']=='passed' and river['automatic_anchor'] and river['fixture_edits']==0
assert (river['landscape_revision'],river['river_terrain_revision'],river['ecology_revision'])==(7,6,11)
assert river['depth']>1 and river['level']>world['sea_level']+0.5
assert len(river['generator_probes'])==2 and river['generator_probes'][0][1]!=0 and river['generator_probes'][1][1]==0
(root/'verification.json').write_text(json.dumps(dict(status='passed',client_sha256=(root/'client.sha256').read_text().split()[0],
    landscape_revision=7,river_terrain_revision=6,ecology_revision=11,fixture_edits=0,river=river,captures=captures,
    scope='Real generated regional lowland relief and open routed river, with native loaded bed/air probes. Distance fog is disabled only in these detached inspection cameras. Captures are visual evidence, not an FPS benchmark.'),indent=2)+'\n')
print('Native fantasy landscape passed: regional lowland relief, real open river and three captures')
PY
