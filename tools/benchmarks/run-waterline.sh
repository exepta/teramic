#!/usr/bin/env bash
# Disposable native fixture using the normal block/prop mesher and PBR shaders.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/waterline
run_dir=$(mktemp -d "$repo_dir/target/waterline/run.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
ln -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}" "$run_dir/terra-client"
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]); text=p.read_text()
for key,value in {'paused':'true','start_hour':'10.0','trees':'false','water':'true','caves':'false','biomes':'false','rivers':'false',
                  'hill_amplitude':'0.0','mountain_amplitude':'0.0','terrace_strength':'0.0'}.items():
    text,count=re.subn(r'(?m)^'+key+r'\s*=.*$',key+' = '+value,text)
    assert count==1,key
p.write_text(text)
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 4
vertical_view_distance = 4
fov_degrees = 45.0
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
printf 'Native unified water inspection: %s\n' "$run_dir"
status=0
env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_FOG_BLOCK_INSPECT \
    -u TERRA_PARTICLE_MANUAL_INPUT -u TERRA_PARTICLE_FLUIDS_DISABLE -u TERRA_PARTICLE_INSPECT \
    -u TERRA_FOLIAGE_INSPECT -u TERRA_MATERIALS_CASTLE -u TERRA_BENCH_SAVE_DIR \
    -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI -u TERRA_SCREENSHOT \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_REFERENCE_PALETTE_INSPECT=1 TERRA_WATERLINE_INSPECT=1 \
    TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
import hashlib,json,math,re,struct,sys
root=Path(sys.argv[1]);errors=[];captures=[];bodies=[]
if int(sys.argv[2]):errors.append('Client exit '+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if 'Waterline gallery passed:' not in log:errors.append('Missing gallery success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):errors.append('Runtime/GPU error in native log')
for i in range(6):
    p=root/'captures'/f'waterline-{i}.png'
    try:
        data=p.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080)
        captures.append(dict(path=str(p.relative_to(root)),sha256=hashlib.sha256(data).hexdigest()))
        body=json.loads((root/'captures'/f'waterline-{i}.json').read_text())
        assert body['stage']==i and body['water_ready']
        assert body['quality'].lower()==['low','medium','high'][i//2]
        assert body['underwater']==bool(i%2)
        assert math.isfinite(body['underwater_blend']) and 0<=body['underwater_blend']<=1
        assert body['body_surfaces']>=body['published_patches']>0
        assert body['visible_tiles']>0
        assert body['source_count']==1 and body['water_units']==17*17*2*8
        assert body['active_particles']==0 and body['particle_water_seeds']==0
        assert body['particle_mass'][:2]==[0,0]
        assert body['legacy_surfaces']==0 and body['legacy_spray']==0
        assert body['body_patches']>=body['published_patches']
        assert body['focus'][0]==(20.5 if i//2==1 else 16.5)
        assert abs(body['focus'][1]-body['pool_surface_y']-(.03 if i%2==0 else -.03))<.001
        assert body['focus'][2]==12
        vertices=body['published_vertices']
        assert len(vertices)>0 and len(vertices)%3==0
        assert all(len(v)==3 and all(math.isfinite(c) for c in v) for v in vertices)
        assert all(abs(v[1]-body['pool_surface_y'])<.002 for v in vertices)
        area=0.
        for j in range(0,len(vertices),3):
            a,b,c=vertices[j:j+3]
            area+=abs((b[0]-a[0])*(c[2]-a[2])-(b[2]-a[2])*(c[0]-a[0]))*.5
        assert abs(area-body['surface_area'])<.02
        assert abs(area-17*17)<.5,('missing or duplicate reservoir surface',area)
        body['vertex_sha256']=hashlib.sha256(json.dumps(vertices,separators=(',',':')).encode()).hexdigest()
        if bodies:
            # Camera position/quality may change optics, never topology, source
            # amounts or main-thread voxel sampling/upload work for this pool.
            for key in ['geometry_hash','source_hash','vertex_sha256','sampled_cells','body_patches']:
                assert body[key]==bodies[0][key],('camera changed settled body',i,key,body[key],bodies[0][key])
        bodies.append(body)
    except (OSError,AssertionError,ValueError,KeyError) as e:errors.append(f'{p.name}: {type(e).__name__}: {e}')
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,water_bodies=bodies,
            client_sha256=hashlib.file_digest((root/'terra-client').open('rb'),'sha256').hexdigest(),
            scope='Real saved 17x17x2 reservoir above/below at Low/Medium/High; focus moves4m and returns. Stable published surface topology, water level and source volume; zero camera-local water particles, zero repeat reservoir sampling or uploads, zero legacy spray.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Unified water {report['status']}: {root/'verification.json'}")
for error in errors:print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
