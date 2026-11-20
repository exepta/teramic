#!/usr/bin/env bash
# Fresh ecology-nine biome panoramas plus naturally generated plant close-ups.
# Requires a built client; does not compile or alter a player world.
# Optional arguments select individual scenes, e.g. reference-cactus picker-desert.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before this fixture.\n' >&2; exit 1; }
mkdir -p target/reference-biomes
run_dir=$(mktemp -d "$repo_dir/target/reference-biomes/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]);s=p.read_text()
s=re.sub(r'(?m)^ecology_revision\s*=.*$','ecology_revision = 9',s)
s=re.sub(r'(?m)^paused\s*=.*$','paused = true',s)
s=re.sub(r'(?m)^start_hour\s*=.*$','start_hour = 10.0',s)
p.write_text(s)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
title = "Teramic Native Biome References"
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
printf 'Isolated native reference run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=4 RUST_LOG=info
export TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_REFERENCE_BIOMES=1
unset TERRA_BIOME_ATMOSPHERE TERRA_ECOLOGY_ANCHOR TERRA_BLOOD_VIEW
if (( $# )); then scenes=("$@"); else
    scenes=(picker-plains picker-desert picker-cherry-forest picker-highlands
        picker-blood-forest picker-spirit-blossom picker-ocean picker-forest
        reference-cactus reference-plains reference-desert reference-spirit reference-birch)
fi
printf '%s\n' "${scenes[@]}" > "$run_dir/scenes.txt"
for scene in "${scenes[@]}"; do
    case "$scene" in
        picker-plains|picker-desert|picker-cherry-forest|picker-highlands|picker-blood-forest|picker-spirit-blossom|picker-ocean|picker-forest|reference-cactus|reference-plains|reference-desert|reference-spirit|reference-birch) ;;
        *) printf 'Unsupported reference scene: %s\n' "$scene" >&2; exit 2 ;;
    esac
    env TERRA_BIOME_SCENE="$scene" \
        timeout --signal=TERM 300s "$client_bin" --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,struct,sys
root=Path(sys.argv[1]);results={}
for scene in (root/'scenes.txt').read_text().splitlines():
    picker=scene.startswith('picker-')
    log=(root/'captures'/f'{scene}.log').read_text(errors='replace')
    marker='Native biome preview passed:' if picker else 'Native biome fixture passed:'
    assert marker in log,scene
    for failure in ('panicked at','Validation Error','Shader processing error','failed to process shader error','failed to create shader module','Native reference flora inspection failed','Native reference biome inspection failed'):
        assert failure not in log,(scene,failure)
    report=json.loads((root/'captures'/f'{scene}-{"reference" if picker else "inspection"}.json').read_text())
    assert report['status']=='passed' and report['fixture_edits']==0 and report['ecology_revision']==9,scene
    assert report['source_cells'],scene
    if not picker:
        assert set(report['captures'])=={'overview','close'},scene
        assert all(v['camera_in_loaded_air'] and v['clear_loaded_voxel_sightline'] and v['verified_voxel_probes']>=3 for v in report['captures'].values()),scene
    if scene=='reference-cactus':
        cactus=report['cactus'];assert 2<=cactus['height']<=8
        assert cactus['voxel_count']>cactus['height'] and cactus['face_connected'] and cactus['sand_supported']
        assert sum(m==422 for _,m in report['source_cells'])==cactus['voxel_count']
    screens={}
    for view in (('overview',) if picker else ('overview','close')):
        path=root/'captures'/f'{scene}-{view}.png';data=path.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n'
        width,height=struct.unpack('>II',data[16:24]);assert width>=1920 and height>=1080
        screens[view]=dict(width=width,height=height,sha256=hashlib.sha256(data).hexdigest())
    results[scene]=dict(inspection=report,screens=screens)
(root/'verification.json').write_text(json.dumps(dict(status='passed',client_sha256=(root/'client.sha256').read_text().split()[0],ecology_revision=9,fixture_edits=0,
    world='fresh WorldConfig revisions, requested seed/time, current biome JSON snapshots',scenes=results),indent=2)+'\n')
print('Native reference biome/plant captures and loaded voxel inspections passed:', ', '.join(results))
PY
