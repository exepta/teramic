#!/usr/bin/env bash
# Disposable native GPU fixture; uses an already-built client and no user saves.
# TERRA_PARTICLE_STRESS=1 selects the separate 8192-water-particle stress contract.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before running particle fluid inspection.\n' >&2; exit 1; }
stress_mode="${TERRA_PARTICLE_STRESS:-0}"
case "$stress_mode" in 0|1) ;; *) printf 'TERRA_PARTICLE_STRESS must be 0 or 1.\n' >&2; exit 2;; esac
run_prefix=run
verifier=verify-particle-fluids.py
if [[ "$stress_mode" == 1 ]]; then
    run_prefix=stress
    verifier=verify-particle-stress.py
fi
mkdir -p target/particle-fluids
run_dir=$(mktemp -d "$repo_dir/target/particle-fluids/$run_prefix.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
# Pin the executable throughout the run without duplicating its large contents.
ln -- "$client_source" "$run_dir/terra-client"
trap 'rm -f -- "$run_dir/terra-client"' EXIT
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir" "$stress_mode" <<'PY'
from pathlib import Path
import json
import re
import sys

root = Path(sys.argv[1])
stress = sys.argv[2] == '1'
p = root / 'configs/world.toml'
source = p.read_text()
for key, value in {
    'paused': 'true', 'start_hour': '10.0', 'trees': 'false', 'water': 'false',
    'caves': 'false', 'biomes': 'false', 'rivers': 'false',
    'structures_revision': '0', 'hill_amplitude': '0.0',
    'mountain_amplitude': '0.0', 'terrace_strength': '0.0',
}.items():
    source, count = re.subn(r'(?m)^' + key + r'\s*=.*$', key + ' = ' + value, source)
    assert count == 1, key
p.write_text(source)
manifest = {
    'fixture': 'materials_smoke_particles', 'readback_slots': 8192,
    'mode': 'native_gpu_manual_stress' if stress else 'native_gpu_manual_input',
    'world_water': False, 'stress': stress, 'fps_limit': 0 if stress else 60,
    'environment': {'TERRA_PARTICLE_INSPECT': '1', 'TERRA_PARTICLE_MANUAL_INPUT': '1'},
    'phases': ['airborne', 'impact', 'settled', 'drained'],
    'simulation_hz': 120, 'capture_seconds': [0.07, 0.75, 4.0, None],
}
if stress:
    manifest['environment']['TERRA_PARTICLE_STRESS'] = '1'
    manifest['active_counts'] = [8192, 0, 0, 0]
    manifest['target_mass'] = [128.0, 0.0, 0.0, 0.0]
    manifest['contract'] = 'maximum_particle_budget_mass_collision_clock'
else:
    manifest['settled_limits'] = {'centroid_y': [1.0, 1.5], 'maximum_y': 2.0}
(root / 'run.json').write_text(json.dumps(manifest, indent=2) + '\n')
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 3
vertical_view_distance = 3
fov_degrees = 48.0
fog_distance = 0.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 3
fog_distance = 0.0
fov = 48.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "high"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
TOML
if [[ "$stress_mode" == 1 ]]; then
    sed -i 's/^fps_limit = 60$/fps_limit = 0/' "$run_dir/configs/graphics.toml"
fi
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
# An inherited gallery flag must not replace this scene or its GPU inputs.
unset_flags=()
while IFS= read -r variable; do
    case "$variable" in
        TERRA_*_INSPECT|TERRA_*_SMOKE|TERRA_PARTICLE_MANUAL_INPUT|TERRA_PARTICLE_STRESS|TERRA_PARTICLE_FLUIDS_DISABLE|TERRA_BENCH_SAVE_DIR|TERRA_BENCH_WORLD_ID|TERRA_BENCH_UI|TERRA_SCREENSHOT|TERRA_MATERIALS_CASTLE)
            unset_flags+=(-u "$variable");;
    esac
done < <(compgen -e)
printf 'Native particle fluid inspection (%s): %s\n' "$run_prefix" "$run_dir"
status=0
env "${unset_flags[@]}" \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_CAPTURE_DIR="$run_dir/captures" TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_MODE=offline \
    TERRA_PARTICLE_INSPECT=1 TERRA_PARTICLE_MANUAL_INPUT=1 TERRA_PARTICLE_STRESS="$stress_mode" TERRA_PARTICLE_FLUIDS_DISABLE=0 \
    TERRA_COMPUTE_THREADS=4 RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 180s "$run_dir/terra-client" --smoke-test-materials \
    > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 "$repo_dir/tools/benchmarks/$verifier" "$run_dir" "$status"
