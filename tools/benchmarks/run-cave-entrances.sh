#!/usr/bin/env bash
# Real roof, daylight portal and cave-water exposure; uses disposable data only.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
mkdir -p target/cave-entrances
run_dir=$(mktemp -d "$repo_dir/target/cave-entrances/run.XXXXXX")
ln -- "$client_source" "$run_dir/terra-client"
trap 'rm -f -- "$run_dir/terra-client"' EXIT
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles" "$run_dir/captures"
cp configs/world.toml "$run_dir/configs/world.toml"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1280
height = 800
view_distance = 4
vertical_view_distance = 4
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1280
height = 800
render_distance = 4
vsync = false
fps_limit = 0
shadow_quality = "high"
cave_quality = "medium"
weather = "clear"
weather_intensity = 0.0
debug_overlay = false
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
printf 'Disposable cave entrance fixture: %s\n' "$run_dir"
env -u TERRA_BENCH_SAVE_DIR -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_MODE -u TERRA_BENCH_UI \
    -u TERRA_TORCH_INSPECT -u TERRA_CAVE_LIGHT_INSPECT -u TERRA_CAVE_LOD_SHADOW_INSPECT \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=4 \
    TERRA_CAVE_PORTAL_INSPECT=1 RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-caves \
    > "$run_dir/native.log" 2>&1
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
import hashlib,json,re,struct,sys
from pathlib import Path
root=Path(sys.argv[1])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
assert 'Cave showcase passed:' in log, 'Missing acceptance marker'
assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log)
images=[]
for stage in range(18,22):
    p=root/'captures'/f'cave-{stage}-Medium.png';data=p.read_bytes()
    assert data[:8]==b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II',data[16:24])==(1280,800)
    images.append({'file':p.name,'sha256':hashlib.sha256(data).hexdigest()})
proof={'status':'passed','client_sha256':(root/'client.sha256').read_text().split()[0],
       'scope':'Native 1280x800 cave fixture: real geometry/daylight portal and enclosed-water exposure; four settled poses, not a Spirit Forest mist density measurement.',
       'captures':images}
(root/'verification.json').write_text(json.dumps(proof,indent=2)+'\n')
print(f'Native cave entrance verification passed: {root}')
PY
