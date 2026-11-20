#!/usr/bin/env bash
# Regression for the real Test save: copied terrain/profile, actual client input.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/saved-construction
run_dir=$(mktemp -d "$repo_dir/target/saved-construction/run.XXXXXX")
source_world=${1:-"$repo_dir/worlds/Test"}
source_profile=${2:-"$repo_dir/configs/profile.toml"}
python3 - "$source_world" "$source_profile" "$run_dir" <<'PYSETUP'
from pathlib import Path
import hashlib, json, shutil, sys, tomllib
source = Path(sys.argv[1]).resolve(strict=True)
profile = Path(sys.argv[2]).resolve(strict=True)
root = Path(sys.argv[3])
world = tomllib.loads((source / 'world.toml').read_text())
world_id = world['manifest']['world']['id']
shutil.copytree(source, root / 'worlds/Test', ignore=shutil.ignore_patterns('session.lock'))
shutil.copytree('configs', root / 'configs')
shutil.copyfile(profile, root / 'configs/saved-profile.toml')
(root / 'captures').mkdir()
(root / 'world-id').write_text(str(world_id))
hashes = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest()
          for p in source.rglob('*') if p.is_file() and p.name != 'session.lock'}
(root / 'source.json').write_text(json.dumps({'source': str(source), 'world_id': world_id, 'hashes': hashes}, indent=2) + '\n')
(root / 'configs/client.toml').write_text('''title = "Teramic Saved Construction Acceptance"
width = 1600
height = 900
view_distance = 2
vertical_view_distance = 3
fly_speed = 4.0
vsync = false
debug_overlay = false
''')
(root / 'configs/graphics.toml').write_text('''width = 1600
height = 900
window_mode = "windowed"
render_distance = 2
vsync = false
fps_limit = 60
ui_scale = 1.0
ui_fps = 60
shadow_quality = "low"
water_reflections = false
weather = "clear"
weather_intensity = 0.0
debug_overlay = false
''')
(root / 'configs/server.toml').write_text('''natural_mob_limit = 0
simulation_distance = 2
lan_discovery = false
require_verified_accounts = false
''')
PYSETUP
world_id=$(< "$run_dir/world-id")
printf 'Isolated saved-world construction run: %s\n' "$run_dir"
test -x target/debug/terra-client
env \
    TERRA_CONFIG_DIR="$run_dir/configs" \
    TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_PROFILE_FILE="$run_dir/configs/saved-profile.toml" \
    TERRA_WORLD_DIR="$run_dir/worlds" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" \
    TERRA_CAPTURE_DIR="$run_dir/captures" \
    TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_BENCH_MODE=integrated \
    TERRA_BENCH_UI=1 \
    TERRA_BENCH_SAVE_DIR="$run_dir/worlds/Test" \
    TERRA_BENCH_WORLD_ID="$world_id" \
    TERRA_SAVED_CONSTRUCTION_SMOKE=1 \
    TERRA_NATIVE_OS_POINTER="${TERRA_NATIVE_OS_POINTER:-0}" \
    TERRA_BENCH_GRAPHICS=1 \
    TERRA_PREVIEW_WARMUP=1 \
    TERRA_COMPUTE_THREADS=4 \
    RUST_LOG=info \
    timeout --signal=TERM 210s target/debug/terra-client --smoke-test-benchmark \
    > "$run_dir/native.log" 2>&1
python3 - "$run_dir" <<'PYVERIFY'
from pathlib import Path
import hashlib, json, os, struct, sys
root = Path(sys.argv[1])
log = (root / 'native.log').read_text(errors='replace')
for marker in [
    'Saved construction is using the real saved player and kingdom',
    'Saved construction real placement and authoritative QueueHouse acknowledgement received',
    'Saved construction acknowledged save and disk reload passed',
    'Saved construction passed:',
]:
    assert marker in log, marker
for failure in ['Saved construction acceptance failed', 'Cannot save integrated world', 'World autosave failed']:
    assert failure not in log, failure
screens = {}
for name in ['saved-construction-preview', 'saved-construction-queued']:
    data = (root / f'captures/{name}.png').read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    size = struct.unpack('>II', data[16:24])
    assert size == (1600, 900), (name, size)
    screens[name] = {'width': size[0], 'height': size[1], 'bytes': len(data)}
source = json.loads((root / 'source.json').read_text())
source_dir = Path(source['source'])
unchanged = all((source_dir / p).is_file() and hashlib.sha256((source_dir / p).read_bytes()).hexdigest() == digest
                for p, digest in source['hashes'].items())
(root / 'verification.json').write_text(json.dumps({
    'status': 'passed', 'world_id': source['world_id'], 'source': source['source'],
    'original_save_unchanged': unchanged,
    'origin': [int(v) for v in os.environ.get('TERRA_SAVED_CONSTRUCTION_ORIGIN', '16,16,24').split(',')],
    'facing': int(os.environ.get('TERRA_SAVED_CONSTRUCTION_FACING', '0')),
    'confirmation': os.environ.get('TERRA_SAVED_CONSTRUCTION_CONFIRM', 'mouse'),
    'os_event_source': 'Linux uinput and KWin' if os.environ.get('TERRA_SAVED_CONSTRUCTION_CONFIRM') == 'os-mouse' or os.environ.get('TERRA_SAVED_CONSTRUCTION_CONFIRM') == 'button' and os.environ.get('TERRA_NATIVE_OS_POINTER') == '1' else None,
    'template': 'village-house-temperate-v1', 'screens': screens,
    'workflow': 'real saved identity, TAB, HTML template card, selected-site confirmation, authoritative QueueHouse ACK, Save ACK and disk reload',
}, indent=2) + '\n')
print(f"Saved-world construction passed: confirmation={os.environ.get('TERRA_SAVED_CONSTRUCTION_CONFIRM', 'mouse')}, server job, save/reload; original save unchanged={unchanged}")
PYVERIFY
