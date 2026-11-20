#!/usr/bin/env bash
# Generated ocean/river flight, immersion and return; no fixture terrain edits.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/water-bodies
run_dir=$(mktemp -d "$repo_dir/target/water-bodies/run.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
ln -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}" "$run_dir/terra-client"
trap 'rm -f -- "$run_dir/terra-client"' EXIT
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]);text=p.read_text()
for key,value in {'seed':'42042','paused':'true','start_hour':'10.0','trees':'false','caves':'false',
                  'water':'true','rivers':'true','structures_revision':'0'}.items():
 text,n=re.subn(r'(?m)^'+key+r'\s*=.*$',key+' = '+value,text)
 assert n==1,key
p.write_text(text)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 4
vertical_view_distance = 4
fov_degrees = 65.0
fog_distance = 0.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 4
fog_distance = 0.0
fov = 65.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "medium"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Generated water body inspection: %s\n' "$run_dir"
unset_flags=()
while IFS= read -r variable; do
 case "$variable" in
  TERRA_*_INSPECT|TERRA_*_SMOKE|TERRA_PARTICLE_MANUAL_INPUT|TERRA_PARTICLE_FLUIDS_DISABLE|TERRA_BENCH_SAVE_DIR|TERRA_BENCH_WORLD_ID|TERRA_BENCH_UI|TERRA_SCREENSHOT|TERRA_MATERIALS_CASTLE) unset_flags+=(-u "$variable");;
 esac
done < <(compgen -e)
status=0
env "${unset_flags[@]}" TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
 TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
 TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
 TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_MODE=offline \
 TERRA_WATER_BODIES_INSPECT=1 TERRA_COMPUTE_THREADS=4 RUST_LOG=info \
 timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
 > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
from PIL import Image
import hashlib,json,math,re,struct,sys
root=Path(sys.argv[1]);errors=[];captures=[];reports=[]
if int(sys.argv[2]):errors.append('Client exit '+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if 'Generated water bodies passed:' not in log:errors.append('Missing native success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):errors.append('Runtime/GPU error')
for i in range(8):
 p=root/'captures'/f'water-body-{i}.png'
 try:
  image=p.read_bytes();assert image[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',image[16:24])==(1920,1080)
  with Image.open(p) as screenshot:
   sample=screenshot.convert('RGB').resize((160,90))
   visible=sum(max(rgb)>8 for rgb in sample.get_flattened_data())/(160*90)
   assert visible>0.70, f'only {visible:.0%} of the frame rendered'
   if i in (0,3):
    ocean=sum(b-r>36 for r,g,b in sample.get_flattened_data())/(160*90)
    assert ocean>0.50, f'only {ocean:.0%} of the ocean view shows deep water'
  d=json.loads(p.with_suffix('.json').read_text())
  assert d['stage']==i and d['body']==('ocean' if i<4 else 'river') and d['phase']==i%4
  assert d['published_vertices']>0 and d['published_triangles']>0
  assert d['active_particles']==0 and d['target_mass'][:2]==[0,0]
  assert d['sampled_cells']==0 and d['body_patches']>0 and d['water_motion_events']==0
  assert d['submerged']==(i%4==2) and d['generated_without_edits']
  assert d['bounds_min'][1]-.05<=d['level']<=d['bounds_max'][1]+.05
  assert all(math.isfinite(v) for k in ['camera','bounds_min','bounds_max'] for v in d[k])
  if i%4==3:assert d['retained_return_mesh']
  if reports:assert d['particle_revision']==reports[0]['particle_revision']
  captures.append(dict(path=str(p.relative_to(root)),sha256=hashlib.sha256(image).hexdigest()))
  reports.append(d)
 except (OSError,AssertionError,ValueError) as e:errors.append(f'{p.name}: {e}')
if reports and reports[-1]['focus_domains']<=12:errors.append('Camera did not cross enough simulation/chunk boundaries')
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,body_views=reports,
 client_sha256=(root/'client.sha256').read_text().split()[0],
 scope='Real generated ocean and river; smoothly flying detached camera crosses4m/16m boundaries, goes underwater and returns. Native assertions verify retained published mesh bounds, no reservoir GPU particles, no camera reseeding, no water contact impulses and zero main-thread body voxel samples.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Generated water bodies '+report['status']+':',root/'verification.json')
for e in errors:print(e,file=sys.stderr)
raise SystemExit(bool(errors))
PY
