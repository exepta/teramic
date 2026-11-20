#!/usr/bin/env bash
# Native foliage and eye/eyelid galleries through the real rendering pipeline.
# Uses an already-built client and isolated files; never builds or opens a user save.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before running this gallery.\n' >&2; exit 1; }
mkdir -p target/foliage-creatures
run_dir=$(mktemp -d "$repo_dir/target/foliage-creatures/run.XXXXXX")
# Pin the executable inode so both scenes use the same binary even if Cargo
# replaces target/debug/terra-client. The SHA is checked again after both runs.
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 3
vertical_view_distance = 3
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 3
vsync = false
fps_limit = 60
shadow_quality = "high"
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
spirit_blossom_revision = 2
[time]
start_hour = 10.0
paused = true
TOML
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated foliage and eye run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
unset TERRA_FOREST_INSPECT TERRA_SPIRIT_INSPECT TERRA_SMOKE_VILLAGERS TERRA_HORSE_INSPECT
unset TERRA_AQUATIC_INSPECT TERRA_LIFE_INSPECT TERRA_CREATURE_INSPECT TERRA_BEE_INSPECT
unset TERRA_SPIDER_MODEL_INSPECT
unset TERRA_BEAR_INSPECT TERRA_BIOME_SCENE TERRA_BIOME_ATMOSPHERE TERRA_MATERIALS_CASTLE
unset TERRA_FOLIAGE_INSPECT TERRA_EYE_INSPECT TERRA_FOLIAGE_REFERENCE TERRA_REFERENCE_BIOMES
unset TERRA_BENCH_SAVE_DIR TERRA_BENCH_WORLD_ID TERRA_BENCH_MODE TERRA_BENCH_UI
for scene in foliage eyes; do
    mkdir -p "$run_dir/$scene"
    case "$scene" in
        foliage) scene_env=TERRA_FOLIAGE_INSPECT=1; scene_flag=--smoke-test-materials;;
        eyes) scene_env=TERRA_EYE_INSPECT=1; scene_flag=--smoke-test-creatures;;
    esac
    if env "$scene_env" TERRA_CAPTURE_DIR="$run_dir/$scene" \
        timeout --signal=TERM --kill-after=10s 180s "$client_bin" "$scene_flag" \
        > "$run_dir/$scene/native.log" 2>&1; then
        printf 'Completed %s\n' "$scene"
    else
        status=$?
        printf 'Gallery %s failed (%s); log: %s/%s/native.log\n' "$scene" "$status" "$run_dir" "$scene" >&2
        exit "$status"
    fi
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PYTHON'
from pathlib import Path
import hashlib
import json
import re
import struct
import sys

root = Path(sys.argv[1])
scenes = (
    ('foliage', [f'foliage-{i}.png' for i in range(7)],
     'Foliage gallery passed: ten leaf materials, seven native views'),
    ('eyes', ['eyes-0-open.png', 'eyes-1-half-blink.png', 'eyes-2-closed.png',
              'eyes-3-after-blink.png'],
     'Eye gallery passed: open, half-blink, closed, after-blink'),
)
failures = (
    'panicked at', 'Validation Error', 'Shader processing error',
    'failed to create shader module', 'failed to load shader',
    'screenshot is blank', 'DeviceLost', 'device lost',
    'Foliage gallery timed out', 'Foliage gallery capture invalid',
    'Creature model failed', 'Eye gallery failed',
)
report = {
    'status': 'passed',
    'scope': 'Native fixture rendering: ten leaf materials and four eyelid poses. '
             'Not a natural-world population, navigation or AI test.',
    'resolution': [1920, 1080],
    'client_sha256': (root / 'client.sha256').read_text().split()[0],
    'config_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in sorted((root / 'configs').glob('*.toml'))},
    'scenes': {},
}
for scene, expected, marker in scenes:
    folder = root / scene
    log = (folder / 'native.log').read_text(errors='replace')
    plain_log = re.sub(r'\x1b\[[0-9;]*m', '', log)
    assert marker in plain_log, (scene, 'missing completion marker')
    assert not re.search(r'\bERROR\b', plain_log), (scene, 'runtime ERROR in native.log')
    for failure in failures:
        assert failure.casefold() not in plain_log.casefold(), (scene, failure)
    actual = {p.name for p in folder.glob('*.png')}
    assert actual == set(expected), (scene, set(expected) - actual, actual - set(expected))
    captures = []
    for name in expected:
        path = folder / name
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n', path
        assert struct.unpack('>II', data[16:24]) == (1920, 1080), path
        captures.append({'file': name, 'bytes': len(data),
                         'sha256': hashlib.sha256(data).hexdigest()})
    if scene == 'foliage':
        assert len({c['sha256'] for c in captures}) == len(captures), (scene, 'identical captures')
    else:
        # A completed blink may correctly return to the exact open-eye image.
        assert len({c['sha256'] for c in captures[:3]}) == 3, (scene, 'blink poses unchanged')
    report['scenes'][scene] = {'count': len(captures), 'marker': marker, 'captures': captures}
report['total_captures'] = sum(scene['count'] for scene in report['scenes'].values())
(root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Native foliage and eye galleries passed: {report["total_captures"]} captures in {root}')
PYTHON
