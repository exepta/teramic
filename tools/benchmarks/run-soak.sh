#!/usr/bin/env bash
set -euo pipefail
# Run alone on the native desktop. All gameplay and rendering remain enabled.
binary=${1:?client binary}
label=${2:?output label}
scenario=${3:-idle}
root=${TERRA_SOAK_RESULT_DIR:-"$(pwd)/tools/benchmarks/results/progressive-collapse-2026-10-20"}
configs=${TERRA_SOAK_CONFIG_DIR:-"$root/configs"}
options=()
unset TERRA_SOAK_ROUTE TERRA_SOAK_ROUTE_DELAY_SECONDS TERRA_SOAK_EDITS
case "$scenario" in
    idle) seconds=900 ;;
    explore) seconds=900; options+=(TERRA_SOAK_ROUTE=explore) ;;
    walk) seconds=180; options+=(TERRA_SOAK_ROUTE=walk) ;;
    delayed) seconds=1800; options+=(TERRA_SOAK_ROUTE=explore TERRA_SOAK_ROUTE_DELAY_SECONDS=900) ;;
    edits) seconds=180; options+=(TERRA_SOAK_EDITS=1) ;;
    *) printf 'Unknown soak scenario: %s\n' "$scenario" >&2; exit 2 ;;
esac
env "${options[@]}" \
    TERRA_CONFIG_DIR="$configs" \
    TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
    TERRA_BENCH_WEATHER="${TERRA_SOAK_WEATHER:-rain}" \
    TERRA_BENCH_SECONDS="${TERRA_SOAK_SECONDS:-$seconds}" \
    TERRA_SOAK_REPORT=1 TERRA_CPU_SCOPES=1 TERRA_RENDER_DIAGNOSTICS=1 \
    TERRA_DRAW_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 TERRA_MEASURE_TIMELINE=1 \
    TERRA_SCREENSHOT="$root/$label.png" WGPU_BACKEND=vulkan \
    python3 tools/benchmarks/measure.py "$binary" --smoke-test-benchmark --gender=f \
    > "$root/$label.log" 2>&1
python3 tools/benchmarks/summarize_soak.py "$root/$label.log" --json > "$root/$label.json"
python3 tools/benchmarks/summarize_soak.py "$root/$label.log" --csv > "$root/$label.csv"
