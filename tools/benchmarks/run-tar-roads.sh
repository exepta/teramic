#!/usr/bin/env bash
# Disposable native fixture. Uses normal fine flux and two-second curing.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/tar-roads
run_dir=$(mktemp -d "$repo_dir/target/tar-roads/run.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
ln -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}" "$run_dir/terra-client"
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]); text=p.read_text()
for key,value in {'paused':'true','start_hour':'10.0','trees':'false','water':'false','caves':'false','biomes':'false','rivers':'false',
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
printf 'Native tar inspection: %s\n' "$run_dir"
status=0
env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_FOG_BLOCK_INSPECT \
    -u TERRA_FOLIAGE_INSPECT -u TERRA_MATERIALS_CASTLE -u TERRA_BENCH_SAVE_DIR \
    -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI -u TERRA_SCREENSHOT \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_TAR_INSPECT=1 \
    TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys
root=Path(sys.argv[1]); errors=[]; phases=[]
if int(sys.argv[2]): errors.append('Client exit '+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if 'Native tar inspection passed:' not in log: errors.append('Missing final native success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):
    errors.append('Runtime or GPU error in native log')
for index,name in enumerate(['tar-0-path','tar-1-poured','tar-2-flowing','tar-3-dried']):
    try:
        phase=json.loads((root/'captures'/f'{name}.json').read_text())
        data=(root/'captures'/f'{name}.png').read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080)
        assert phase['units']==(0 if index==0 else 192)
        if index==0: assert phase['wet_cells']==phase['dried_cells']==0
        if index==2:
            assert phase['wet_cells']>3 and phase['elapsed_seconds']>=10
            assert len(phase['bile_cells'])==9
            acid=[c['liquid'] for c in phase['bile_cells']]
            assert all(1<=c['volume']<=64 and c['age']==128 for c in acid)
            assert sum(c['source'] for c in acid)==1
            assert max(c['volume'] for c in acid)==64
        if index==3: assert phase['wet_cells']==0 and phase['dried_cells']>3 and phase['elapsed_seconds']>=58
        phases.append(dict(name=name,**phase,png_sha256=hashlib.sha256(data).hexdigest()))
    except (OSError,ValueError,AssertionError,KeyError) as e:
        errors.append(f'{name}: {type(e).__name__}: {e}')
report=dict(status='failed' if errors else 'passed',errors=errors,phases=phases,
            client_sha256=hashlib.file_digest((root/'terra-client').open('rb'),'sha256').hexdigest(),
            scope='Actual native PBR rendering and authoritative finite64-unit flow/drying and bounded bile basin. Fixture invokes shared world actions; player input and server inventory checked separately.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Tar inspection {report['status']}: {root/'verification.json'}")
for error in errors: print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
