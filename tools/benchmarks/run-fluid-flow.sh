#!/usr/bin/env bash
# Isolated replica fixture: discrete saved fluid targets, real meshes/materials,
# timed GPU interpolation. World-flow/source simulation is verified separately.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/fluid-flow
run_dir=$(mktemp -d "$repo_dir/target/fluid-flow/run.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
ln -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}" "$run_dir/terra-client"
trap 'rm -f -- "$run_dir/terra-client"' EXIT
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]);text=p.read_text()
for key,value in {'seed':'42042','paused':'true','start_hour':'10.0','trees':'false','water':'true','caves':'false','biomes':'true','rivers':'false','hill_amplitude':'0.0','mountain_amplitude':'0.0','terrace_strength':'0.0'}.items():
 text,count=re.subn(r'(?m)^'+key+r'\s*=.*$',key+' = '+value,text)
 assert count==1,key
p.write_text(text)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 3
vertical_view_distance = 3
fov_degrees = 45.0
fog_distance = 0.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 3
fog_distance = 0.0
fov = 45.0
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
unset_flags=()
while IFS= read -r variable; do
 case "$variable" in TERRA_*_INSPECT|TERRA_*_SMOKE|TERRA_PARTICLE_MANUAL_INPUT|TERRA_PARTICLE_FLUIDS_DISABLE|TERRA_BENCH_MODE|TERRA_BENCH_SAVE_DIR|TERRA_BENCH_WORLD_ID|TERRA_BENCH_UI|TERRA_SCREENSHOT|TERRA_MATERIALS_CASTLE) unset_flags+=(-u "$variable");; esac
done < <(compgen -e)
printf 'Native four-fluid interpolation inspection: %s\n' "$run_dir"
status=0
env "${unset_flags[@]}" TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
 TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/local.toml" \
 TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
 TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_FLOW_INSPECT=1 \
 TERRA_FLOW_TWEEN_SECONDS=1.6 TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
 timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
 > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
if [[ "${TERRA_BASIN_WIND_SEQUENCE:-}" == "1" ]]; then
 python3 - "$run_dir" "$status" <<'BASIN'
from pathlib import Path
import hashlib,json,re,sys
from PIL import Image,ImageChops,ImageDraw,ImageStat
root=Path(sys.argv[1]);captures=[];pairs=[];errors=[]
try:
 assert sys.argv[2]=='0','native exit '+sys.argv[2]
 log=(root/'native.log').read_text(errors='replace')
 assert 'Captured successive windy basin frames' in log and not re.search(r'\bERROR\b|panicked at|Validation Error',log)
 frames=sorted((root/'captures').glob('wind-basin-*.json'))
 assert len(frames)==12,len(frames)
 previous=None
 for path in frames:
  state=json.loads(path.read_text())
  png=path.with_suffix('.png')
  image=Image.open(png).convert('RGB')
  mask=Image.new('L',image.size,0)
  ImageDraw.Draw(mask).polygon([tuple(p) for p in state['polygon']], fill=255)
  box=mask.getbbox()
  image=image.crop(box);mask=mask.crop(box)
  assert state['volume']==8 and mask.histogram()[255]>4000
  if previous:
   old,old_image=previous
   assert state['frame']==old['frame']+1 and state['seconds']>old['seconds']
   assert state['publication']==old['publication'] and state['polygon']==old['polygon'] and state['wind']!=old['wind']
   change=ImageChops.difference(old_image,image)
   mean=sum(ImageStat.Stat(change,mask=mask).mean)/3
   tail=max(sum(c.histogram(mask)[32:])/mask.histogram()[255] for c in change.split())
   pairs.append(dict(mean_rgb_delta=mean,fraction_changing_over_32=tail))
   assert mean<3.5 and tail<0.025,(path.name,mean,tail)
  captures.append(dict(path=str(png.relative_to(root)),sha256=hashlib.sha256(png.read_bytes()).hexdigest(),**state))
  previous=state,image
 assert max(pair['mean_rgb_delta'] for pair in pairs)>0.01,'frozen surface'
except (AssertionError,ValueError,KeyError,OSError) as e:errors.append(str(e))
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,pairs=pairs,
 client_sha256=(root/'client.sha256').read_text().split()[0],
 scope='Fixed camera, filled 4 by 4 basin, steady volume and mesh, rotating wind and shader animation. Successive frames compared inside the projected water surface.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(report['status'],root/'verification.json',errors)
raise SystemExit(bool(errors))
BASIN
 exit $?
fi
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
from PIL import Image,ImageChops,ImageStat
import hashlib,json,math,re,struct,sys
root=Path(sys.argv[1]);errors=[];captures=[];flows=[]
if int(sys.argv[2]):errors.append('Client exit '+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if 'Temporal fluid gallery passed:' not in log:errors.append('Missing native gallery success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):errors.append('Runtime/GPU error')
for name,maximum,source in [('water',7,8),('galle',56,64),('tar',48,64),('blood',7,8)]:
 try:
  states=[];images=[]
  for stage in range(5):
   p=root/'captures'/f'flow-{name}-{stage}.png';raw=p.read_bytes()
   assert raw[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',raw[16:24])==(1920,1080)
   capture=dict(path=str(p.relative_to(root)),sha256=hashlib.sha256(raw).hexdigest())
   states.append(json.loads(p.with_suffix('.json').read_text()))
   s=states[-1]
   assert s['fluid']==name and s['stage']==stage and s['replica']
   assert s['authoritative_units']==[1,maximum,maximum,0,0][stage]
   assert s['source_units']==source
   assert all(math.isfinite(s[key]) for key in ['tween_fraction','duration','simulation_seconds','wall_seconds'])
   assert abs(s['duration']-1.6)<.01
   assert len(s['roi'])==4 and 0<=s['roi'][0]<s['roi'][2]<=1920 and 0<=s['roi'][1]<s['roi'][3]<=1080
   if stage in (1,3):assert .38<=s['tween_fraction']<=.90,(name,stage,s['tween_fraction'])
   if stage in (2,4):assert s['tween_fraction']>=1.15,(name,stage,s['tween_fraction'])
   if stage:assert s['simulation_seconds']>states[-2]['simulation_seconds'] and s['wall_seconds']>states[-2]['wall_seconds']
   captures.append(capture);images.append(Image.open(p).convert('RGB'))
  assert states[1]['publication']>states[0]['publication']
  assert states[3]['publication']>states[2]['publication']
  if name=='blood':assert states[0]['origin']!=[0,0]
  changes=[]
  for before,after in [(0,1),(1,2),(2,3),(3,4)]:
   roi=tuple(states[after]['roi'])
   difference=ImageChops.difference(images[before].crop(roi),images[after].crop(roi))
   mean=sum(ImageStat.Stat(difference).mean)/3
   changed=sum(max(p)>3 for p in difference.get_flattened_data())
   assert mean>.025 and changed>10,(name,before,after,mean,changed)
   changes.append(dict(before=before,after=after,mean_rgb_difference=mean,changed_pixels=changed,roi=roi))
  flows.append(dict(fluid=name,states=states,roi_changes=changes))
 except (OSError,AssertionError,ValueError,KeyError) as e:errors.append(f'{name}: {type(e).__name__}: {e}')
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,flows=flows,
 client_sha256=hashlib.file_digest((root/'terra-client').open('rb'),'sha256').hexdigest(),
 scope='Four discrete authoritative replica targets in a real glass-sided floor/step fixture. Five views per fluid prove timed interpolation and drainage within unchanged target volumes. Blood uses an actual Blood Forest site. This is renderer interpolation validation; authoritative source spread/withdrawal are verified separately.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Four-fluid interpolation {report['status']}: {root/'verification.json'}")
for error in errors:print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
