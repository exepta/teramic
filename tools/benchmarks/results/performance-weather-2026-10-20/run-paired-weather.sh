#!/usr/bin/env bash
set -euo pipefail
# Same code/settings/assets except the atmosphere IBL source cubemap size.
# Pair each scenario immediately to reduce drift from background desktop work.
before=${1:?512 cubemap binary}
after=${2:?128 cubemap binary}
root=tools/benchmarks/results/performance-weather-2026-10-20
for scenario in water12 water20 user12 user20 snow12 snow20
do
    args=(--smoke-test-lod)
    case "$scenario" in
        water*) weather=clear; args+=(--smoke-test-water) ;;
        user*) weather=rain; args+=(--smoke-test-water) ;;
        snow*) weather="snow 3" ;;
    esac
    for variant in light512 final128
    do
        binary=$before
        if [[ $variant == final128 ]]; then binary=$after; fi
        printf '%s %s %s\n' "$variant" "$scenario" "$weather"
        timeout 240s env \
            TERRA_CONFIG_DIR="$root/$scenario" \
            TERRA_BENCH_GRAPHICS=1 \
            TERRA_BENCH_WEATHER="$weather" \
            TERRA_BENCH_SECONDS=30 \
            TERRA_CPU_SCOPES=1 \
            TERRA_RENDER_DIAGNOSTICS=1 \
            TERRA_SCREENSHOT="$root/$scenario-$variant.png" \
            WGPU_BACKEND=vulkan \
            "$binary" "${args[@]}" > "$root/logs/$scenario-$variant.log" 2>&1
    done
done
