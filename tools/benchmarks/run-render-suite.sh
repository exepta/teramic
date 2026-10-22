#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
output="${1:-/tmp/teramic-render-suite}"
mkdir -p "$output"
binary="$PWD/target/debug/terra-client"
for run in 1 2 3; do
    for mode in full optimized; do
        detail=0
        if [[ "$mode" == full ]]; then detail=1; fi
        env TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_FULL_MICRO="$detail" \
            python3 tools/benchmarks/measure.py "$binary" --smoke-test-forest \
            > "$output/forest-$mode-$run.log" 2>&1
    done
done
for run in 1 2; do
    for mode in offline integrated; do
        env TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_FULL_MICRO=0 TERRA_BENCH_MODE="$mode" \
            python3 tools/benchmarks/measure.py "$binary" --smoke-test-benchmark \
            > "$output/controller-$mode-$run.log" 2>&1
    done
done
for mode in full optimized; do
    detail=0
    if [[ "$mode" == full ]]; then detail=1; fi
    env TERRA_BENCH_SECONDS=24 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_FULL_MICRO="$detail" \
        python3 tools/benchmarks/measure.py "$binary" --smoke-test-lod \
        > "$output/lod-$mode.log" 2>&1
done
