#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
output="${1:-/tmp/teramic-hydrology-suite}"
mkdir -p "$output"
for test in forest-1 forest-2 offline integrated river-1 river-2; do
    mode=offline
    case "$test" in
        forest-*) flag=--smoke-test-forest ;;
        offline) flag=--smoke-test-benchmark ;;
        integrated) flag=--smoke-test-benchmark; mode=integrated ;;
        river-*) flag=--smoke-test-river-benchmark ;;
    esac
    env -u TERRA_SCREENSHOT TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 \
        TERRA_BENCH_MODE="$mode" TERRA_BENCH_FULL_MICRO=0 \
        python3 tools/benchmarks/measure.py target/debug/terra-client "$flag" \
        > "$output/$test.log" 2>&1
done
