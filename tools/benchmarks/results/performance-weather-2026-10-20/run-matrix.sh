#!/usr/bin/env bash
set -euo pipefail
# Run from the repository root, without other clients or compilers.
binary=${1:?client binary}
label=${2:?log label}
root=tools/benchmarks/results/performance-weather-2026-10-20
for scenario in user20 user12 snow12 snow20 water12 water20
do
    args=(--smoke-test-lod)
    if [[ $scenario != snow* ]]; then
        args+=(--smoke-test-water)
    fi
    printf '%s %s\n' "$label" "$scenario"
    timeout 240s env \
        TERRA_CONFIG_DIR="$root/$scenario" \
        TERRA_BENCH_SECONDS=60 \
        TERRA_CPU_SCOPES=1 \
        TERRA_RENDER_DIAGNOSTICS=1 \
        WGPU_BACKEND=vulkan \
        "$binary" "${args[@]}" > "$root/logs/$scenario-$label.log" 2>&1
done
