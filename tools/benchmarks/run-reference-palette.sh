#!/usr/bin/env bash
# Disposable native fixture using the normal block/prop mesher and PBR shaders.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/reference-palette
run_dir=$(mktemp -d "$repo_dir/target/reference-palette/run.XXXXXX")
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
printf 'Native reference palette inspection: %s\n' "$run_dir"
status=0
env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_FOG_BLOCK_INSPECT \
    -u TERRA_FOLIAGE_INSPECT -u TERRA_MATERIALS_CASTLE -u TERRA_BENCH_SAVE_DIR \
    -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI -u TERRA_SCREENSHOT \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_REFERENCE_PALETTE_INSPECT=1 \
    TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys
root=Path(sys.argv[1]);errors=[];captures=[]
if int(sys.argv[2]):errors.append('Client exit '+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if 'Reference palette gallery passed:' not in log:errors.append('Missing gallery success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):errors.append('Runtime/GPU error in native log')
for i in range(4):
    p=root/'captures'/f'reference-palette-{i}.png'
    try:
        data=p.read_bytes()
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(1920,1080)
        captures.append(dict(path=str(p.relative_to(root)),sha256=hashlib.sha256(data).hexdigest()))
    except (OSError,AssertionError) as e:errors.append(f'{p.name}: {type(e).__name__}: {e}')
normal_paths=set(re.findall(r'texture=textures/block_palette/([a-z_]+_normal.png)',log))
if len(normal_paths)!=22:errors.append(f'Expected 22 linear PBR normal textures, found {len(normal_paths)}')
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,normal_map_count=len(normal_paths),
            client_sha256=hashlib.file_digest((root/'terra-client').open('rb'),'sha256').hexdigest(),
            scope='Actual native section models/materials, generated PBR maps, shader, placed shrubs and rooted dead bushes; four camera views.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Reference palette {report['status']}: {root/'verification.json'}")
for error in errors:print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
