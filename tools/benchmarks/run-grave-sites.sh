#!/usr/bin/env bash
# Real generated cemetery and three tomb variants; requires a built client.
# Optional args: discovered-sites.jsonl [cemetery|grave-knight|grave-priestess|grave-scholar].
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
sites_file=${1:-tools/benchmarks/results/structure-terrain-revision-10/grave-sites.jsonl}
selected_scene=${2:-all}
mkdir -p target/grave-sites
run_dir=$(mktemp -d "$repo_dir/target/grave-sites/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
python3 - "$sites_file" "$selected_scene" "$run_dir" <<'PY'
from pathlib import Path
import json,re,sys,tomllib
source,selected,out=Path(sys.argv[1]),sys.argv[2],Path(sys.argv[3])
required=('cemetery','grave-knight','grave-priestess','grave-scholar')
sites={entry['scenario']:entry for line in source.read_text().splitlines() if line.strip() for entry in [json.loads(line)]}
assert all(scene in sites for scene in required), 'Finder must supply all four genuine generated sites'
assert selected=='all' or selected in required, selected
world=Path('configs/world.toml').read_text()
world=re.sub(r'(?m)^spirit_blossom_revision\s*=.*$', 'spirit_blossom_revision = 2', world)
world=re.sub(r'(?m)^structures_revision\s*=.*$', 'structures_revision = 10', world)
world=re.sub(r'(?m)^paused\s*=.*$', 'paused = true', world)
config=tomllib.loads(world)
assert config['spirit_blossom_revision']==2 and config['structures_revision']==10
assert all(sites[scene]['seed']==config['seed'] for scene in required), 'Finder and world seed differ'
(out/'configs/world.toml').write_text(world)
(out/'sites.json').write_text(json.dumps(sites,indent=2)+'\n')
with (out/'scenes.tsv').open('w') as commands:
    for scene in required:
        if selected=='all' or selected==scene:
            commands.write(scene+'\t'+json.dumps(sites[scene]['region'],separators=(',',':'))+'\n')
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
title = "Teramic Native Grave Acceptance"
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
printf 'Isolated native grave run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
export TERRA_CAPTURE_DIR="$run_dir/captures"
unset TERRA_BIOME_ATMOSPHERE
while IFS=$'\t' read -r scene region; do
    env TERRA_BIOME_SCENE="$scene" TERRA_GRAVE_REGION="$region" \
        timeout --signal=TERM 300s "${TERRA_CLIENT_BINARY:-target/debug/terra-client}" --smoke-test-biomes \
        > "$run_dir/captures/$scene.log" 2>&1
done < "$run_dir/scenes.tsv"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,struct,sys
root=Path(sys.argv[1]);results={}
for line in (root/'scenes.tsv').read_text().splitlines():
    scene,region=line.split('\t'); log=(root/'captures'/f'{scene}.log').read_text(errors='replace')
    assert 'Native biome fixture passed:' in log, scene
    for failure in ('Shader processing error','failed to process shader error','failed to create shader module','Validation Error','panicked at','Native grave inspection failed'):
        assert failure not in log,(scene,failure)
    report=json.loads((root/'captures'/f'{scene}-inspection.json').read_text())
    assert report['status']=='passed' and report['region']==json.loads(region)
    assert report['fixture_edits']==0 and report['spirit_blossom_revision']==2
    assert report['native_views']==['overview','close'] and report['verified_voxel_probes']>0
    for view in ('overview','close'):
        lighting=report['lighting'][view]
        assert 0<len(lighting['local_lights'])<=lighting['pool_budget'],(scene,view,lighting)
        assert all(light['intensity_lumens']>0 and light['range_m']>0 and not light['volumetric'] for light in lighting['local_lights'])
    if scene!='cemetery':
        levels=3 if scene=='grave-knight' else 4
        assert len(report['levels'])==len(report['loot'])==levels
        assert report['route_points']>100 and report['close_eye'][1]<report['center'][1]-8
        assert all(any(stack['count']>0 for stack in entry['inventory']['slots']) for entry in report['loot'])
        lighting=report['lighting']['close']; floor=report['levels'][-1][1]
        assert lighting['cave_blend']>.95 and 5<lighting['exposure_ev100']<8,(scene,lighting)
        assert any(floor<=light['position'][1]<floor+8 and light['distance_to_camera_m']<light['range_m'] for light in lighting['local_lights']), (scene,'No active local lamp reaches the camera on its dungeon floor')
    screens={}
    for view in ('overview','close'):
        path=root/'captures'/f'{scene}-{view}.png'; data=path.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n'
        width,height=struct.unpack('>II',data[16:24]); assert width>=1536 and height>=768
        screens[view]=dict(width=width,height=height,sha256=hashlib.sha256(data).hexdigest())
    results[scene]=dict(inspection=report,screens=screens)
(root/'verification.json').write_text(json.dumps(dict(status='passed',sites=results),indent=2)+'\n')
print('Generated grave sites, streamed voxel/loot probes and native captures passed:', ', '.join(results))
PY
