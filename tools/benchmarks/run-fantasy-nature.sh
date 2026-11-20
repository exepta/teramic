#!/usr/bin/env bash
# Genuine revisioned arch and cactus, generated and streamed without edits.
# Requires a built client. Uses isolated worlds/configs and a pinned executable.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before this fixture.\n' >&2; exit 1; }
mkdir -p target/fantasy-nature
run_dir=$(mktemp -d "$repo_dir/target/fantasy-nature/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
revision="${TERRA_FANTASY_REVISION:-9}"
[[ "$revision" == 9 || "$revision" == 10 ]] || { printf 'Use fantasy revision 9 or 10.\n' >&2; exit 1; }
python3 - "$run_dir/configs/world.toml" "$revision" <<'PY'
from pathlib import Path
import re,sys
path=Path(sys.argv[1]);text=path.read_text()
text=re.sub(r'(?m)^ecology_revision\s*=.*$',f'ecology_revision = {sys.argv[2]}',text)
text=re.sub(r'(?m)^paused\s*=.*$','paused = true',text)
text=re.sub(r'(?m)^start_hour\s*=.*$','start_hour = 10.0',text)
path.write_text(text)
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
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated native fantasy run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
export TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_REFERENCE_BIOMES=1
unset TERRA_BIOME_ATMOSPHERE TERRA_ECOLOGY_ANCHOR TERRA_BLOOD_VIEW TERRA_REFERENCE_ANCHOR
for scene in reference-cactus rock-formations; do
    export TERRA_BIOME_SCENE="$scene"
    if [[ "$scene" == rock-formations ]]; then export TERRA_FANTASY_ROCKS=1; else unset TERRA_FANTASY_ROCKS; fi
    timeout --signal=TERM --kill-after=10s 240s "$client_bin" --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
    printf 'Completed %s\n' "$scene"
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" "$revision" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys,tomllib
root=Path(sys.argv[1]);captures=[]
revision=int(sys.argv[2])
assert tomllib.loads((root/'configs/world.toml').read_text())['ecology_revision']==revision
for scene in ['reference-cactus','rock-formations']:
    log=re.sub(r'\x1b\[[0-9;]*m','',(root/'captures'/f'{scene}.log').read_text(errors='replace'))
    assert 'Native biome fixture passed:' in log,scene
    assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log),scene
    for view in ['overview','close']:
        path=root/'captures'/f'{scene}-{view}.png';data=path.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080),path
        captures.append(dict(file=str(path.relative_to(root)),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
cactus=json.loads((root/'captures/reference-cactus-inspection.json').read_text())
assert cactus['status']=='passed' and cactus['ecology_revision']==9 and cactus['fixture_edits']==0
assert 5<=cactus['cactus']['height']<=8 and cactus['cactus']['face_connected'] and cactus['cactus']['sand_supported']
assert cactus['cactus']['voxel_count']>cactus['cactus']['height']
assert all(c['camera_in_loaded_air'] and c['clear_loaded_voxel_sightline'] and c['verified_voxel_probes']>=3 for c in cactus['captures'].values())
assert sum(m==422 for _,m in cactus['source_cells'])==cactus['cactus']['voxel_count']
arch=json.loads((root/'captures/rock-formations-source.json').read_text())
assert arch['ecology_revision']==revision and arch['fixture_edits']==0
extra=arch['extra'];assert extra['fantasy_arch'] and extra['clear_generator_sightlines']
assert extra['span'][1]>=10 and max(extra['span'][0],extra['span'][2])>=13
if revision == 10:
    assert extra['span'][1]>=18 and max(extra['span'][0],extra['span'][2])>=20
if revision==10:
    assert extra['span'][1]>=18 and max(extra['span'][0],extra['span'][2])>=20
assert extra['formation_voxels']>=70 and len(extra['root_probes'])==2
assert sum(m==0 for _,m in extra['voxel_probes'])==9
assert sum(m!=0 for _,m in extra['voxel_probes'])>=5
report=dict(status='passed',client_sha256=(root/'client.sha256').read_text().split()[0],
            ecology_revision=revision,fixture_edits=0,captures=captures,cactus=cactus,arch=arch,
            scope='Real generated voxels and loaded probes; pictures show cactus spines and arch geometry. Damage is checked by authoritative tests, not this visual fixture.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Native fantasy nature passed: genuine open arch and spined cactus, four captures')
PY
