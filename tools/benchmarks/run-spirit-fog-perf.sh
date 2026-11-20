#!/usr/bin/env bash
# Fixed revision-one grove: compare rendering without changing world generation.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/spirit-fog-perf
run_dir=$(mktemp -d "$repo_dir/target/spirit-fog-perf/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 5
vertical_view_distance = 4
fov_degrees = 55.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 5
fov = 55.0
vsync = false
fps_limit = 0
shadow_quality = "high"
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
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
spirit_blossom_revision = 1
[time]
start_hour = 10.0
paused = true
TOML
printf 'Isolated Spirit fog performance run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_ASSET_DIR="$repo_dir/assets" TERRA_CAPTURE_DIR="$run_dir/captures"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
export TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_SECONDS=30
export TERRA_BENCH_CAMERA='-1323.3406,57.1447,812.3406'
export TERRA_BENCH_LOOK_AT='-1279.5,27.1447,768.5'
timeout --signal=TERM 150s target/debug/terra-client --smoke-test-benchmark \
    > "$run_dir/native.log" 2>&1
rg 'Native smoke benchmark|GPU pass|Rendered.*resolution' "$run_dir/native.log"
