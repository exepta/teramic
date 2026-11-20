#!/usr/bin/env bash
# Three sequential native inspections, using only disposable configs and saves.
# Requires a built client; this script never builds or modifies product assets.
set -euo pipefail
case "${1:-}" in
    '') ;;
    --help|-h)
        printf 'Usage: %s\nSet TERRA_CLIENT_BINARY to inspect another built client.\n' "$0"
        exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
esac
(( $# <= 1 )) || { printf 'Expected at most one option.\n' >&2; exit 2; }
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source=$(realpath -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}")
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/facing-ambience-fixes
run_dir=$(mktemp -d "$repo_dir/target/facing-ambience-fixes/run.XXXXXX")
client_bin="$run_dir/terra-client"
# Keep the exact final executable even when a later build replaces its pathname.
ln -- "$client_source" "$client_bin"
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated facing/ambience inspection: %s\n' "$run_dir"

finish() {
    local process_status=$? verification_status
    trap - EXIT
    set +e
    python3 - "$run_dir" "$repo_dir" "$process_status" <<'PY'
from pathlib import Path
import hashlib, json, re, struct, sys, tomllib

root, repo = Path(sys.argv[1]), Path(sys.argv[2])
process_status = int(sys.argv[3])
errors, scenes, assets = [], {}, {}
sha = lambda p: hashlib.file_digest(p.open('rb'), 'sha256').hexdigest()
expected_hash = (root/'client.sha256').read_text().split()[0]
actual_hash = sha(root/'terra-client')
if actual_hash != expected_hash:
    errors.append('Pinned executable changed during inspection')
before = json.loads((root/'assets-before.json').read_text()) if (root/'assets-before.json').is_file() else {}
if len(before) != 5:
    errors.append('Expected four biome side PNG hashes and the environment leaves WAV hash')
for name, expected in before.items():
    path = repo/name
    actual = sha(path) if path.is_file() else None
    assets[name] = dict(before_sha256=expected, after_sha256=actual, unchanged=actual == expected)
    if actual != expected:
        errors.append(f'Asset changed or disappeared: {name}')
definitions = [
    ('spirit-grass', 'Native biome preview passed:', [
        'picker-spirit-blossom-overview', 'picker-spirit-blossom-animated', 'picker-spirit-blossom-close']),
    ('roads', 'F68 road gallery passed:', ['roads-0', 'roads-1', 'roads-2']),
    ('audio', 'Recorded steps passed:', []),
]
image_count = 0
for name, marker, expected in definitions:
    folder = root/name
    failures, captures = [], []
    status_file = folder/'exit-status'
    status = int(status_file.read_text()) if status_file.is_file() else None
    if status != 0:
        failures.append(f'Client exit status: {status!r}')
    log_file = folder/'native.log'
    log = re.sub(r'\x1b\[[0-9;]*m', '', log_file.read_text(errors='replace')) if log_file.is_file() else ''
    if marker not in log:
        failures.append(f'Missing success marker: {marker}')
    if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost', log):
        failures.append('Native log contains a runtime/render error')
    if re.search(r'failed to (?:create (?:shader module|render pipeline|compute pipeline)|process shader)', log, re.I):
        failures.append('Native log contains a shader/pipeline failure')
    try:
        client = tomllib.loads((folder/'configs/client.toml').read_text())
        graphics = tomllib.loads((folder/'configs/graphics.toml').read_text())
        world = tomllib.loads((folder/'configs/world.toml').read_text())
        if (client['width'], client['height'], graphics['width'], graphics['height']) != (1920,1080,1920,1080):
            failures.append('All scene configurations must use 1920x1080')
        if (client['view_distance'], graphics['render_distance'], client['vertical_view_distance']) != (4,4,4):
            failures.append('Scene view/render/vertical distances must all be four')
        if client['fog_distance'] != 0.0 or graphics['fog_distance'] != 0.0:
            failures.append('Client and graphics distance fog must both be disabled')
        if world['landscape_revision'] != 7:
            failures.append('Copied world must retain current landscape revision seven')
        if world['time']['start_hour'] != 10.0 or world['time']['paused'] is not True:
            failures.append('World clock must be paused at hour ten')
        if name == 'audio' and (any(world[flag] for flag in ('trees','water','caves','biomes','rivers')) or any(world[field] != 0.0 for field in ('hill_amplitude','mountain_amplitude','terrace_strength'))):
            failures.append('Only the audio scene must use the explicit flat, dry, empty fixture')
    except (OSError, KeyError, ValueError) as exc:
        world = {}
        failures.append(f'Invalid isolated configuration: {exc}')
    for stem in expected:
        if not (folder/'captures'/f'{stem}.png').is_file():
            failures.append(f'Missing screenshot: {stem}.png')
    pngs = sorted((folder/'captures').glob('*.png'))
    if len(pngs) != len(expected):
        failures.append(f'Expected {len(expected)} screenshots, found {len(pngs)}')
    for path in pngs:
        data = path.read_bytes()
        dimensions = struct.unpack('>II', data[16:24]) if len(data) >= 33 and data[:8] == b'\x89PNG\r\n\x1a\n' and data[12:16] == b'IHDR' else None
        if dimensions != (1920,1080):
            failures.append(f'Incorrect screenshot dimensions: {path.name}: {dimensions!r}')
        captures.append(dict(file=str(path.relative_to(root)), width=dimensions[0] if dimensions else None,
                             height=dimensions[1] if dimensions else None, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    image_count += len(pngs)
    details = {}
    if name == 'spirit-grass':
        try:
            atmosphere = json.loads((folder/'captures/picker-spirit-blossom-atmosphere.json').read_text())
            if atmosphere['status'] != 'passed' or atmosphere['first_banks'] <= 0 or atmosphere['later_banks'] <= 0 or atmosphere['first_drift'] == atmosphere['later_drift']:
                failures.append('Actual Spirit atmosphere/animation inspection did not pass')
            details['atmosphere'] = atmosphere
        except (OSError, KeyError, ValueError) as exc:
            failures.append(f'Missing or invalid atmosphere report: {exc}')
        hashes = {Path(c['file']).stem: c['sha256'] for c in captures}
        if hashes.get('picker-spirit-blossom-overview') == hashes.get('picker-spirit-blossom-animated'):
            failures.append('Overview and animated captures must contain different GPU pixels')
    if name == 'audio':
        summary = next((line for line in log.splitlines() if marker in line), '')
        contacts, played = re.search(r'\bcontacts=(\d+)\b', summary), re.search(r'\bplayed=(\d+)\b', summary)
        if not re.search(r'\bclips=124\b', log):
            failures.append('Actual shared decoder did not report all 124 clips')
        if contacts is None or int(contacts[1]) != 27 or played is None or int(played[1]) < 27:
            failures.append('Actual audio sink did not consume the 27 injected contacts')
        emitted = set(map(int, re.findall(r'\bcontact=(\d+)\b', log)))
        if emitted != set(range(1,28)):
            failures.append('The explicit contact sequence 1..27 is incomplete')
        details.update(decoded_clips=124 if re.search(r'\bclips=124\b', log) else None,
                       injected_contacts=int(contacts[1]) if contacts else None,
                       consumed_contacts=int(played[1]) if played else None)
    scenes[name] = dict(status='failed' if failures else 'passed', exit_status=status,
                       success_marker=marker, log=str(log_file.relative_to(root)), captures=captures,
                       landscape_revision=world.get('landscape_revision'), errors=failures, **details)
    errors.extend(f'{name}: {error}' for error in failures)
if image_count != 6:
    errors.append(f'Expected six screenshots in total, found {image_count}')
if process_status:
    errors.append(f'Inspection shell exited with status {process_status}')
report = dict(status='failed' if errors else 'passed', client_sha256=expected_hash,
              client_hash_unchanged=actual_hash == expected_hash, resolution=[1920,1080],
              screenshot_count=image_count, assets=assets, scenes=scenes, errors=errors,
              scope='Native Spirit turf and road geometry inspection plus the actual shared audio decoder/device sink. '
                    'The 27 material contacts are explicitly injected; this does not record natural biome traversal or measure FPS.')
(root/'verification.json').write_text(json.dumps(report, indent=2)+'\n')
print(f"Facing/ambience inspection {report['status']}: {root/'verification.json'}")
for error in errors:
    print(error, file=sys.stderr)
raise SystemExit(bool(errors))
PY
    verification_status=$?
    if (( process_status == 0 && verification_status != 0 )); then process_status=$verification_status; fi
    exit "$process_status"
}
trap finish EXIT

python3 - "$repo_dir" "$run_dir/assets-before.json" <<'PY'
from pathlib import Path
import hashlib,json,sys
repo = Path(sys.argv[1])
paths = ['assets/terra/blocks/textures/'+name+'_side.png' for name in (
    'spirit_blossom_grass','blood_grass','blood_moss','crimson_mycelium')]
paths.append('assets/audio/environment/leaves.wav')
Path(sys.argv[2]).write_text(json.dumps({name:hashlib.file_digest((repo/name).open('rb'),'sha256').hexdigest() for name in paths},indent=2)+'\n')
PY

prepare_scene() {
    local scene_dir="$run_dir/$1"
    mkdir -p "$scene_dir/configs" "$scene_dir/worlds" "$scene_dir/profiles" \
        "$scene_dir/captures" "$scene_dir/kingdom-save"
    cp configs/world.toml "$scene_dir/configs/world.toml"
    python3 - "$scene_dir/configs/world.toml" "$1" <<'PY'
from pathlib import Path
import re,sys,tomllib
p = Path(sys.argv[1]); text = p.read_text()
values = {'paused':'true','start_hour':'10.0'}
if sys.argv[2] == 'audio':
    values.update({name:'false' for name in ('trees','water','caves','biomes','rivers')})
    values.update({name:'0.0' for name in ('hill_amplitude','mountain_amplitude','terrace_strength')})
for key,value in values.items():
    text,count = re.subn(r'(?m)^'+re.escape(key)+r'\s*=.*$',key+' = '+value,text)
    assert count == 1, f'Expected one {key} in copied world configuration'
p.write_text(text)
assert tomllib.loads(text)['landscape_revision'] == 7
PY
    cat > "$scene_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 4
vertical_view_distance = 4
fov_degrees = 55.0
fog_distance = 0.0
vsync = false
debug_overlay = false
TOML
    cat > "$scene_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 4
fog_distance = 0.0
fov = 55.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "high"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
TOML
    cat > "$scene_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
}

run_scene() {
    local scene="$1" status scene_dir
    shift
    scene_dir="$run_dir/$scene"
    prepare_scene "$scene"
    printf 'Inspecting %s\n' "$scene"
    if env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_PIPE_AIR_SMOKE \
        -u TERRA_FOG_BLOCK_INSPECT -u TERRA_FOLIAGE_INSPECT -u TERRA_MATERIALS_CASTLE \
        -u TERRA_BENCH_SAVE_DIR -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI \
        -u TERRA_HYDRO_ANCHOR -u TERRA_BIOME_SCENE -u TERRA_BIOME_MOTES \
        -u TERRA_REFERENCE_BIOMES -u TERRA_REFERENCE_ANCHOR -u TERRA_BIOME_ATMOSPHERE \
        -u TERRA_SCREENSHOT -u TERRA_BENCH_WEATHER -u TERRA_BENCH_CAMERA -u TERRA_BENCH_LOOK_AT \
        TERRA_CONFIG_DIR="$scene_dir/configs" TERRA_MENU_CONFIG_DIR="$scene_dir/configs" \
        TERRA_WORLD_DIR="$scene_dir/worlds" TERRA_PROFILE_FILE="$scene_dir/profiles/inspection.toml" \
        TERRA_KINGDOM_SAVE_DIR="$scene_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
        TERRA_CAPTURE_DIR="$scene_dir/captures" TERRA_BENCH_GRAPHICS=1 \
        TERRA_COMPUTE_THREADS=4 TERRA_BENCH_MODE=offline RUST_LOG=info \
        "$@" > "$scene_dir/native.log" 2>&1; then status=0; else status=$?; fi
    printf '%s\n' "$status" > "$scene_dir/exit-status"
    (( status == 0 )) || { printf '%s failed; see %s/native.log\n' "$scene" "$scene_dir" >&2; return "$status"; }
}

run_scene spirit-grass env TERRA_BIOME_SCENE=picker-spirit-blossom TERRA_BIOME_ATMOSPHERE=1 \
    timeout --signal=TERM --kill-after=10s 180s "$client_bin" --smoke-test-biomes
run_scene roads env TERRA_ROADS_INSPECT=1 \
    timeout --signal=TERM --kill-after=10s 180s "$client_bin" --smoke-test-materials
run_scene audio timeout --signal=TERM --kill-after=10s 120s "$client_bin" --smoke-test-steps
