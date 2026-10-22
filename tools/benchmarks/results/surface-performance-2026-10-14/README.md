# Micro-Surface Performance

2026-10-14, Linux/Vulkan, RX 6950 XT / RADV Mesa 26.2.4, existing debug build
profile and pinned dependencies. Seed 42042, generator v5, block IDs unchanged.

## Identified Cost And Changes

Micro-surface templates duplicated vertices per triangle and merged plateaus
but not their vertical step walls. Consequently, each grid cell on a continuous
ridge emitted another wall segment. This added index/vertex work, transfers
and repeated vertex processing in depth, color and shadow rendering.

- Merge consecutive walls with identical depth pairs into a single quad.
- Index cached templates by exact position, normal and UV attributes. Distinct
  UVs/hard normals never merge; no approximate welding or smoothing is used.
- Keep the partial-degenerate-triangle fix: valid halves are still retained.
- Perform this work once per cached template on existing async workers.

No relief heights, patterns, density, material factors, FOV, visibility, terrain
LOD thresholds or shadow settings were reduced. No separate renderer, new
entities/materials, dependencies, shaders or world-generation changes.

## Comparable Native Flight

48-second out-and-back flight, current user settings: FOV 90, VSync **off**,
view distance 16 (256 m), vertical distance 3, terrain 96/160 m with 12 m
hysteresis, 1920x1200 physical pixels, render diagnostics enabled. Same camera
path and world, with a before build, optimized build, and optimized repeat.
Only one Terra client ran at a time; no concurrent build/benchmark. Unlike
the preceding contended report, these are isolated runs. Configuration was
not edited. Raw logs: `before.log`, `indexed.log`, `repeat.log`.

| Metric | Before | Optimized | Repeat |
|---|---:|---:|---:|
| Mean FPS | 68.800 | 93.504 | 93.366 |
| p95 wall frame | 18.231 ms | 12.855 ms | 13.073 ms |
| p99 wall frame | 24.560 ms | 17.144 ms | 17.739 ms |
| Initial ready | 5.952 s | 4.287 s | 4.280 s |
| End RSS | 911636 KiB | 835336 KiB | 852644 KiB |
| Wrapper peak RSS | 931328 KiB | 870048 KiB | 871084 KiB |
| Terrain worker composition spans | 6.236 s | 4.566 s | 4.547 s |
| Terrain composition jobs | 19526 | 19526 | 19532 |
| Prop worker spans | 1.459 s | 1.446 s | 1.470 s |
| Loaded/rendered sections | 7704/3531 | 7704/3531 | 7704/3531 |
| Mesh assets/visible mesh entities | 9170/4734 | 9170/4734 | 9170/4734 |
| Represented props/prop batches | 89866/1277 | 89866/1277 | 89866/1277 |
| Opaque GPU pass mean | 2.753 ms | 2.139 ms | 2.141 ms |
| Early prepass GPU mean | 1.864 ms | 1.319 ms | 1.320 ms |

Main-pass vertex invocations average 17.478 million before and 12.015 million
after (about 31% less); clipper primitives 3.001 million -> 2.580 million.
Mean FPS improves about 36% in this route. These results do **not** promise
100 FPS everywhere or eliminate every frame below 60 FPS: p99 remains above
16.67 ms, and the existing tail-frame issue is still open. The small composition
job-count difference on repeat is retained rather than hidden.

GPU values describe instrumented passes, not total GPU frame time. Exact draw
calls/individually visible props are not instrumented. Summed worker spans are
not main-thread frame costs. Uncapped faster rendering also uses more total
CPU time over the fixed window, so wrapper CPU seconds are not treated as an
isolated meshing benchmark. CPU voxel-generation/streaming benchmarks were not
rerun because those code paths are unchanged.

## Verification

Passed: `cargo fmt --all --check`, `cargo check --locked --workspace`,
`cargo test --locked --workspace` (183 tests),
`cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo build --locked --workspace`.

New tests verify exact indexed triangle attributes, retained hard normals/UV
seams, and a maximum of ten triangles for the continuous bark wall direction
instead of one wall per grid cell. All existing surface masks, cell bounds,
winding, valid-half, negative-coordinate, mixed-LOD and concave-seam tests pass.

Native `surface.log` passed connected wood/grass/stone across section boundaries
before/after mining. `surface-mined.png` and `indexed-1.png` were inspected:
the intentional mined cell has closed neighboring faces, not see-through joins.
`botanical.log` passed close-up/HUD checks; `closeup-terrain.png` was inspected.
`server.log` passed the headless 27-section startup without display variables.
All three flights completed their LOD replacements and drained jobs.

Reproduce with the settings above and one client only:

```bash
TERRA_BENCH_SECONDS=48 TERRA_RENDER_DIAGNOSTICS=1 TERRA_SCREENSHOT=/tmp/terra-flight.png python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
TERRA_SCREENSHOT=/tmp/terra-surface.png target/debug/terra-client --smoke-test-surface
```
