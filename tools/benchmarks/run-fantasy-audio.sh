#!/usr/bin/env bash
# Actual shared decoder and device sink; contacts are explicitly injected.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/fantasy-audio
run_dir=$(mktemp -d "$repo_dir/target/fantasy-audio/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 960
height = 640
view_distance = 2
vertical_view_distance = 2
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 960
height = 640
window_mode = "windowed"
render_distance = 2
fps_limit = 60
vsync = false
TOML
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
biomes = false
trees = false
water = false
caves = false
ecology_revision = 0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated native audio run: %s\n' "$run_dir"
env TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/local.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_COMPUTE_THREADS=4 RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 120s "$client_bin" --smoke-test-steps \
    > "$run_dir/native.log" 2>&1
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import json,re,sys
root=Path(sys.argv[1]);log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
assert 'Recorded steps passed:' in log,'real audio sink did not consume the contacts'
assert re.search(r'clips=132\b',log),'new shared audio bank was not decoded'
assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error',log)
report=dict(status='passed',client_sha256=(root/'client.sha256').read_text().split()[0],decoded_clips=132,
            scope='Actual game decoder/device sink and 27 injected material-contact messages. Biome mixing is independently exercised through the same decoder in the waveform test; this is not a recording of natural biome traversal.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Native shared audio sink passed: 132 decoded clips and 27 consumed contacts')
PY
