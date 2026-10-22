#!/usr/bin/env bash
set -euo pipefail
# Additional real-graphics/wet-weather acceptance, not the legacy A/B matrix.
binary=${1:?client binary}
label=${2:?log label}
root=tools/benchmarks/results/performance-weather-2026-10-20
for scenario in water12 water20 user12 user20 snow12 snow20
do
    args=(--smoke-test-lod)
    case "$scenario" in
        water*) weather=clear; args+=(--smoke-test-water) ;;
        user*) weather=rain; args+=(--smoke-test-water) ;;
        snow*) weather="snow 3" ;;
    esac
    printf '%s %s %s\n' "$label" "$scenario" "$weather"
    timeout 240s env \
        TERRA_CONFIG_DIR="$root/$scenario" \
        TERRA_BENCH_GRAPHICS=1 \
        TERRA_BENCH_WEATHER="$weather" \
        TERRA_BENCH_SECONDS=30 \
        TERRA_CPU_SCOPES=1 \
        TERRA_RENDER_DIAGNOSTICS=1 \
        WGPU_BACKEND=vulkan \
        "$binary" "${args[@]}" > "$root/logs/$scenario-$label.log" 2>&1
done
