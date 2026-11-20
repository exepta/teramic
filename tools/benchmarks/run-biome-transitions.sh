#!/usr/bin/env bash
# Real climate/turf ID boundary views in a disposable offline fixture.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/biome-transitions
run_dir=$(mktemp -d "$repo_dir/target/biome-transitions/run.XXXXXX")
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
ln -- "$client_source" "$run_dir/terra-client"
trap 'rm -f -- "$run_dir/terra-client"' EXIT
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 8
vertical_view_distance = 4
fov_degrees = 65.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 8
fov = 65.0
vsync = false
fps_limit = 0
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
printf 'Disposable biome transition fixture: %s\n' "$run_dir"
status=0
env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_FOG_BLOCK_INSPECT \
    -u TERRA_FOLIAGE_INSPECT -u TERRA_MATERIALS_CASTLE -u TERRA_BENCH_SAVE_DIR \
    -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI -u TERRA_SCREENSHOT \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 \
    TERRA_REFERENCE_PALETTE_INSPECT=1 TERRA_BIOME_TRANSITION_GALLERY=1 \
    TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 240s "$run_dir/terra-client" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1 || status=$?
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys
root=Path(sys.argv[1]);errors=[];captures=[]
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if int(sys.argv[2]):errors.append('Client exit '+sys.argv[2])
if 'Biome transition gallery passed:' not in log:errors.append('Missing native success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):errors.append('Native GPU/runtime error')
for name in ['spirit','blood']:
    try:
        p=root/'captures'/f'biome-transition-{name}.png'
        data=p.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n'
        assert struct.unpack('>II',data[16:24])==(1920,1080)
        report=json.loads(p.with_suffix('.json').read_text())
        assert report['status']=='passed' and report['real_climate']
        assert report['weight_min']<.42<report['weight_max'],report
        assert report['fixture_cells']==1728
        captures.append(str(p.relative_to(root)))
    except Exception as e:errors.append(f'{name}: {e!r}')
report=dict(status='failed' if errors else 'passed',errors=errors,captures=captures,
    client_sha256=(root/'client.sha256').read_text().split()[0],
    scope='Real generated Spirit/Blood climate, interleaved IDs1/306/400, actual section mesher/PBR shader; disposable sky platforms only.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Biome transitions {report['status']}: {root/'verification.json'}")
for error in errors:print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
