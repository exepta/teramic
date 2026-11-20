#!/usr/bin/env bash
# Native F68 roads and physical water inspection, using only disposable saves.
# Usage: tools/benchmarks/run-roads-water-ai.sh [--partial-water]
set -euo pipefail
include_partial=0
case "${1:-}" in
    '') ;;
    --partial-water) include_partial=1 ;;
    --help|-h)
        printf 'Usage: %s [--partial-water]\n' "$0"
        printf 'Set TERRA_CLIENT_BINARY to inspect a different built client.\n'
        exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
esac
(( $# <= 1 )) || { printf 'Expected at most one option.\n' >&2; exit 2; }
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source=$(realpath -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}")
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/roads-water-ai
run_dir=$(mktemp -d "$repo_dir/target/roads-water-ai/run.XXXXXX")
client_bin="$run_dir/terra-client"
# Pin the inode before any scene: a concurrent later rebuild cannot replace it.
ln -- "$client_source" "$client_bin"
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated native roads/water inspection: %s\n' "$run_dir"

finish() {
    local process_status=$? verification_status
    trap - EXIT
    set +e
    python3 - "$run_dir" "$include_partial" "$process_status" <<'PY'
from pathlib import Path
import hashlib, json, re, struct, sys, tomllib

root = Path(sys.argv[1])
partial = sys.argv[2] == '1'
process_status = int(sys.argv[3])
scenes = [
    ('roads', 'F68 road gallery passed:', ['roads-0', 'roads-1', 'roads-2']),
    ('waterfalls', 'Native stepped waterfall fixture passed',
     ['waterfall-stepped-overview', 'waterfall-partial-step-close']),
]
if partial:
    scenes.append(('partial-water', 'Native partial water and pipe surrounds passed', [
        'slab-stair-fence-basins', 'waterlogged-partial-pool',
        'waterlogged-pipe-valve-fall', 'all-pipe-wall-roof-surrounds',
        'partial-basins-close', 'waterlogged-enclosed-pipes',
        'fence-inward-water-top', 'waterlogged-wall-parts', 'underwater-wall-pipes',
    ]))
errors = []
results = {}
expected_hash = (root / 'client.sha256').read_text().split()[0]
actual_hash = hashlib.file_digest((root / 'terra-client').open('rb'), 'sha256').hexdigest()
if actual_hash != expected_hash:
    errors.append('The pinned client changed during the inspection')
for scene, marker, expected in scenes:
    folder = root / scene
    scene_errors = []
    captures = []
    status_file = folder / 'exit-status'
    status = int(status_file.read_text()) if status_file.exists() else None
    if status != 0:
        scene_errors.append(f'Client exit status: {status!r}')
    log_path = folder / 'native.log'
    log = re.sub(r'\x1b\[[0-9;]*m', '', log_path.read_text(errors='replace')) if log_path.exists() else ''
    if marker not in log:
        scene_errors.append(f'Missing success marker: {marker}')
    if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost', log):
        scene_errors.append('Native log contains a runtime/render error')
    if re.search(r'failed to create (?:shader module|render pipeline|compute pipeline)', log, re.I):
        scene_errors.append('Native log contains a shader/pipeline creation error')
    if scene == 'waterfalls':
        if 'Actual waterfall receded after closing its upstream gate' not in log:
            scene_errors.append('The physical gate/recession check did not finish')
        if log.count('Waterfall screenshot has visible geometry and actual water coloration') < 2:
            scene_errors.append('Both normal waterfall captures must pass their geometry/color checks')
    try:
        client = tomllib.loads((folder / 'configs/client.toml').read_text())
        graphics = tomllib.loads((folder / 'configs/graphics.toml').read_text())
        world = tomllib.loads((folder / 'configs/world.toml').read_text())
        if (client['width'], client['height'], graphics['width'], graphics['height']) != (1920, 1080, 1920, 1080):
            scene_errors.append('Inspection configuration must use 1920x1080')
        if client['view_distance'] not in (3, 4) or graphics['render_distance'] not in (3, 4):
            scene_errors.append('Inspection view distance must be 3 or 4')
        if not 1 <= client['vertical_view_distance'] <= 4:
            scene_errors.append('Input vertical_view_distance must stay within 1..4')
        if client['fog_distance'] != 0.0 or graphics['fog_distance'] != 0.0:
            scene_errors.append('Both client and graphics distance fog must be disabled')
        if not world['water']:
            scene_errors.append('Physical water must be enabled')
    except (OSError, KeyError, ValueError) as exc:
        scene_errors.append(f'Invalid inspection configuration: {exc}')
        world = {}
    for name in expected:
        if not (folder / 'captures' / f'{name}.png').is_file():
            scene_errors.append(f'Missing capture: {name}.png')
    for path in sorted((folder / 'captures').glob('*.png')):
        data = path.read_bytes()
        dimensions = None
        if len(data) >= 33 and data[:8] == b'\x89PNG\r\n\x1a\n' and data[12:16] == b'IHDR':
            dimensions = struct.unpack('>II', data[16:24])
        if dimensions != (1920, 1080):
            scene_errors.append(f'Invalid PNG dimensions: {path.name}: {dimensions!r}')
        captures.append(dict(file=str(path.relative_to(root)), width=dimensions[0] if dimensions else None,
                             height=dimensions[1] if dimensions else None,
                             sha256=hashlib.sha256(data).hexdigest(), bytes=len(data)))
    results[scene] = dict(status='failed' if scene_errors else 'passed', exit_status=status,
                          success_marker=marker, log=str(log_path.relative_to(root)),
                          captures=captures, errors=scene_errors,
                          landscape_revision=world.get('landscape_revision'),
                          river_terrain_revision=world.get('river_terrain_revision'))
    errors.extend(f'{scene}: {message}' for message in scene_errors)
if process_status:
    errors.append(f'Inspection shell exited with status {process_status}')
report = dict(status='failed' if errors else 'passed', client_sha256=expected_hash,
              client_hash_unchanged=actual_hash == expected_hash, partial_water=partial,
              resolution=[1920, 1080], scenes=results, errors=errors,
              scope='Native road geometry and authoritative fluid flow/render inspection. '
                    'Separate disposable configs, worlds, profiles and captures per scene; '
                    'AI behavior is covered by the separate Rust regression suite.')
(root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(f"Native roads/water verification {report['status']}: {root / 'verification.json'}")
for error in errors:
    print(error, file=sys.stderr)
raise SystemExit(bool(errors))
PY
    verification_status=$?
    if (( process_status == 0 && verification_status != 0 )); then
        process_status=$verification_status
    fi
    exit "$process_status"
}
trap finish EXIT

prepare_scene() {
    local scene_dir="$run_dir/$1"
    mkdir -p "$scene_dir/configs" "$scene_dir/worlds" "$scene_dir/profiles" \
        "$scene_dir/captures" "$scene_dir/kingdom-save"
    cp configs/world.toml "$scene_dir/configs/world.toml"
    python3 - "$scene_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re, sys, tomllib
p = Path(sys.argv[1])
text = p.read_text()
text = re.sub(r'(?m)^paused\s*=.*$', 'paused = true', text)
text = re.sub(r'(?m)^start_hour\s*=.*$', 'start_hour = 10.0', text)
p.write_text(text)
assert tomllib.loads(text)['water'], 'Enable water in configs/world.toml before this inspection'
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
    local scene="$1" limit="$2" status scene_dir
    shift 2
    scene_dir="$run_dir/$scene"
    prepare_scene "$scene"
    printf 'Inspecting %s\n' "$scene"
    if env -u TERRA_ROADS_INSPECT -u TERRA_PARTIAL_WATER_SMOKE -u TERRA_PIPE_AIR_SMOKE \
        -u TERRA_FOG_BLOCK_INSPECT -u TERRA_FOLIAGE_INSPECT \
        -u TERRA_BENCH_SAVE_DIR -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI \
        -u TERRA_HYDRO_ANCHOR -u TERRA_BIOME_SCENE -u TERRA_REFERENCE_BIOMES \
        -u TERRA_REFERENCE_ANCHOR -u TERRA_BIOME_ATMOSPHERE -u TERRA_SCREENSHOT \
        TERRA_CONFIG_DIR="$scene_dir/configs" TERRA_MENU_CONFIG_DIR="$scene_dir/configs" \
        TERRA_WORLD_DIR="$scene_dir/worlds" TERRA_PROFILE_FILE="$scene_dir/profiles/inspection.toml" \
        TERRA_KINGDOM_SAVE_DIR="$scene_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
        TERRA_CAPTURE_DIR="$scene_dir/captures" TERRA_BENCH_GRAPHICS=1 \
        TERRA_COMPUTE_THREADS=6 TERRA_BENCH_MODE=offline RUST_LOG=info \
        "$@" > "$scene_dir/native.log" 2>&1; then
        status=0
    else
        status=$?
    fi
    printf '%s\n' "$status" > "$scene_dir/exit-status"
    (( status == 0 )) || { printf '%s failed; see %s/native.log\n' "$scene" "$scene_dir" >&2; return "$status"; }
    printf 'Client completed %s (timeout %ss); verification follows.\n' "$scene" "$limit"
}

run_scene roads 180 env TERRA_ROADS_INSPECT=1 \
    timeout --signal=TERM --kill-after=10s 180s "$client_bin" --smoke-test-materials
run_scene waterfalls 100 \
    timeout --signal=TERM --kill-after=10s 100s "$client_bin" --smoke-test-waterfalls
if (( include_partial )); then
    run_scene partial-water 100 env TERRA_PARTIAL_WATER_SMOKE=1 \
        timeout --signal=TERM --kill-after=10s 100s "$client_bin" --smoke-test-waterfalls
fi
