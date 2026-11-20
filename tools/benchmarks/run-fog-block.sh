#!/usr/bin/env bash
# Real fog-emitter edits and a roofed Spirit Stone comparison. No build, no user saves.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before this fixture.\n' >&2; exit 1; }
mkdir -p target/fog-block
run_dir=$(mktemp -d "$repo_dir/target/fog-block/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles" "$run_dir/captures"
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
base_height = 0.0
hill_amplitude = 0.0
mountain_amplitude = 0.0
sea_level = -64.0
biomes = false
trees = false
water = false
rivers = false
caves = false
ecology_revision = 0
landscape_revision = 0
structures_revision = 0
spirit_blossom_revision = 0
[time]
start_hour = 10.0
paused = true
TOML
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated fog block run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
export TERRA_FOG_BLOCK_INSPECT=1 TERRA_CAPTURE_DIR="$run_dir/captures"
unset TERRA_FOLIAGE_INSPECT TERRA_EYE_INSPECT TERRA_FOLIAGE_REFERENCE TERRA_REFERENCE_BIOMES
unset TERRA_MATERIALS_CASTLE TERRA_BIOME_SCENE TERRA_BIOME_ATMOSPHERE TERRA_SPIRIT_MIST
unset TERRA_BENCH_SAVE_DIR TERRA_BENCH_WORLD_ID TERRA_BENCH_MODE TERRA_BENCH_UI
if timeout --signal=TERM --kill-after=10s 210s "$client_bin" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1; then
    printf 'Completed real fog block and stone comparison\n'
else
    status=$?
    printf 'Fog block fixture failed (%s); log: %s/native.log\n' "$status" "$run_dir" >&2
    exit "$status"
fi
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys
root=Path(sys.argv[1])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
assert 'Fog block acceptance passed:' in log,'missing acceptance marker'
assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log),'native error'
proof=json.loads((root/'captures/fog-block-report.json').read_text())
assert proof['status']=='passed' and proof['biomes'] is False and proof['source_id']==430
assert proof['radius']==8 and proof['floor_size']==[21,21] and proof['captures']==4
phases={m['phase']:m for m in proof['measurements'] if 'phase' in m}
assert set(phases)=={'exposed_on_floor','buried_a','buried_b','removed'}
for phase in ['exposed_on_floor','buried_a','buried_b']:
    state=phases[phase]
    assert state['source_material']==430 and state['sources']==1 and state['volumes']==1
    assert state['radius']==8 and 0<state['sampled_cells']<=41*41
    assert state['ambient_biome_volumes']==0
for phase in ['buried_a','buried_b']:
    assert phases[phase]['above_source_material']==1
assert phases['buried_a']['drift']!=phases['buried_b']['drift']
assert phases['buried_a']['generation']==phases['buried_b']['generation']
assert phases['removed']['source_material']==0
assert phases['removed']['sources']==0 and phases['removed']['volumes']==0
assert phases['removed']['sampled_cells']==0
stone=next(m for m in proof['measurements'] if 'spirit_stone' in m)
assert stone['spirit_stone']==308 and stone['control_stone']==3 and stone['emission']==[0,0,0]
assert stone['roof'] and stone['active_point_lights_near_chamber']==0
captures=[]
expected=['fog-buried-a.png','fog-buried-b.png','fog-removed.png','spirit-stone-roofed-comparison.png']
assert {p.name for p in (root/'captures').glob('*.png')}==set(expected)
for name in expected:
    path=root/'captures'/name
    data=path.read_bytes()
    assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080)
    captures.append({'file':'captures/'+name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
assert captures[0]['sha256']!=captures[1]['sha256'],'animated fog captures are identical'
assert captures[0]['sha256']!=captures[2]['sha256'],'fog removal did not change capture'
report={'status':'passed','client_sha256':(root/'client.sha256').read_text().split()[0],
        'config_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((root/'configs').glob('*.toml'))},
        'captures':captures,'fixture':proof}
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Native fog block acceptance passed: buried animated circle, removal, roofed non-emissive Spirit Stone')
PY
