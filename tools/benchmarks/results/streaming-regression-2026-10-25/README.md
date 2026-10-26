# Phase 130: Loading And Rendering Regressions

## Scope

Native Vulkan tests on the Ryzen 9 5900X / Radeon RX 6950 XT (RADV), using
the existing optimized development profile. No quality, view-distance, shadow,
vegetation-density or frame-limit reduction. No Cargo compilation overlapped
the native measurements. Sampling windows are 8 seconds stationary and
24 seconds moving, following the user's restriction against new long soaks.

These are short regression measurements, not a claim that every FPS spike or
the historical long-term collapse has been eliminated. CPU scope totals which
include startup are not presented as sampled-frame stalls. Process RSS is
not total VRAM; allocator capacities are not total VRAM either.

## Proven Regressions And Corrections

- The phase-129 quota counted material batches rather than sections. Splitting
  dense material meshes also created extra entities, buffers and bookkeeping.
  Terrain and props now reserve once per complete section and retain their
  shared material batches. Independent terrain/prop/water quotas and atomic
  visible swaps remain. One oversized first section can progress without
  starving forever; additional sections still obey the per-frame quotas.
- In the identical 4K camera view, merging distant sections reduced entity
  count but hurt culling and increased frame time. Ordinary gameplay therefore
  keeps per-section culling with no additional far-mesh cache. The experimental
  path requires both smoke-test arguments and `TERRA_FAR_BATCHES=1`.
- Stale terrain LOD/detail results are rejected before mesh asset allocation
  and before reserving the upload budget, not after creating GPU-bound assets.
- Diagnostic-only depth/shadow/UI draw wrappers are installed only when draw
  diagnostics are explicitly requested. Color draw feedback remains active
  because chunk reveal animation depends on actual draws.
- Exact timing found another 1.7 ms/frame in shadow bin-list queue writes.
  A same-version local `bevy_render` patch skips byte-identical uploads of the
  two CPU-owned bin-list buffers. It does not skip transforms, culling,
  per-frame unpacking/indirect metadata, lighting or shadow rendering. Regular
  writes and buffer reallocations invalidate reuse; the additional CPU copy
  is capped at 4 MiB per buffer and drops with its existing owner. See
  [patch details](../../../../vendor/bevy_render/TERAMIC_PATCH.md).

## Native Comparisons

The night fixture uses seed 42042, 1280 x 800, view distance 12, an integrated
server and real HUD, eight shadowed point lights, paused 22:00 time, and
camera `-574,39,962` looking at `-574,33,970`. Its existing configuration is
`../cherry-ai-light-2026-10-25/configs-night-lights`.

| Night loading / shared batches | Before | Corrected section admission |
|---|---:|---:|
| Initial ready time | 41.675 s | 25.413 s |
| Average FPS | 61.42 | 62.37 |
| p95 frame time | 19.942 ms | 20.007 ms |
| p99 frame time | 21.533 ms | 21.488 ms |
| Process RSS | 1,965.7 MiB | 1,807.8 MiB |
| Prop material batches | 1,255 | 1,188 |
| Represented props | 80,732 | 80,732 |

This fixes loading overhead; it does not demonstrate a stationary FPS gain.

The separate same-binary far-batch A/B uses seed 42042, 3840 x 2160, view
distance 20, rain, the integrated server and HUD, and exactly the same actual
camera position/rotation (`974,74.630005,-557`). Configuration:
`../cherry-water-transport-2026-10-25/configs-4k`.

| 4K rain / far batching | Merged sections | Per-section culling |
|---|---:|---:|
| Average FPS | 42.31 | 51.03 |
| p95 frame time | 39.975 ms | 28.835 ms |
| p99 frame time | 47.162 ms | 40.455 ms |
| Initial ready time | 83.148 s | 80.984 s |
| Loaded / rendered sections | 16,137 / 7,486 | 16,137 / 7,486 |
| Far groups / source batches | 240 / 761 | 0 / 0 |
| Extra far CPU geometry | 12.9 MB | 0 |

Four props differ between the two runs (128,350 versus 128,346); they are
not claimed to have bit-identical whole-scene contents. Camera, section counts,
quality, resolution, weather and five shadowed lights match. Per-section
culling improves average FPS by 20.6% and p95 frame time by 27.9% here.

The seed-7 moving/rain test improves initial ready time from 11.370 to 8.802 s,
sampled p99 from 19.205 to 17.902 ms, and the worst sampled frame from 80.040
to 27.496 ms. Average FPS changes only from 93.90 to 94.32. Its asynchronously
changing LOD population is not a fixed-scene FPS comparison.

## Final Bin-Upload Patch And Combined Results

The same new binary, with `TERRA_BIN_UPLOAD_CACHE=0` as the explicit reference,
confirms the additional CPU bottleneck in the night fixture:

| Night bin-list A/B | Unconditional writes | Identical-byte reuse |
|---|---:|---:|
| Average FPS | 62.09 | 72.88 |
| p95 frame time | 19.794 ms | 17.227 ms |
| p99 frame time | 21.262 ms | 19.129 ms |
| Shadow bin-list system CPU time | 1.731 ms | 0.199 ms |
| Initial ready time | 24.357 s | 24.059 s |
| Process RSS | 1,857.7 MiB | 1,843.1 MiB |
| Loaded / rendered sections | 5,625 / 2,878 | 5,625 / 2,878 |
| Represented props / batches | 80,732 / 1,188 | 80,732 / 1,188 |
| Shadowed point lights | 8 | 8 |

The physics-adjusted camera height differs by 0.113 m between these two runs,
with a small corresponding pitch difference; raw logs preserve both poses.
The exact system-body measurement, unchanged section/prop/light populations
and the reference's agreement with the preceding 62.98-FPS run support the
CPU correction. This is not claimed as a pixel-identical image comparison.

Final 4K rain, with per-section culling and bin reuse, has the same exact
camera pose and the same 30,261 mesh assets, 128,346 represented props,
3,986 prop batches, 420 water tiles and five shadowed lights as the previous
per-section run:

| 4K rain | Phase-130 starting far batches | Final corrections |
|---|---:|---:|
| Average FPS | 42.31 | 56.84 |
| p95 frame time | 39.975 ms | 27.301 ms |
| p99 frame time | 47.162 ms | 40.480 ms |
| Initial ready time | 83.148 s | 77.023 s |
| Process RSS | 2,513.8 MiB | 2,515.0 MiB |

The combined FPS increase is 34.3%; p95 decreases 31.7%. The 4K p99 remains
about 40 ms and the worst frame is 55.856 ms, so this is **not** a complete
elimination of spikes. Those high frames do not coincide with terrain/prop
generation or geometry uploads in the preceding-frame context. Full GPU time
and the remaining main-schedule / render handoff costs are not isolated by
these CPU scopes; no stronger causal claim is made.

Final moving/rain seed 7: **99.78 FPS**, p95 **14.941 ms**, p99 **17.971 ms**,
worst **31.690 ms**, initial ready **8.671 s**, RSS **1,183.5 MiB**. This is an
offline moving-camera streaming test, not the integrated-server/HUD workload.
Final snow-3 seed 104729 at 1280 x 800/view 12: **158.14 FPS**, p95 **8.179 ms**,
p99 **9.249 ms**, initial ready **6.046 s**. There is no new same-session
pre-fix snow baseline, so no snow improvement percentage is asserted.

The final 4K rain and snow screenshots were opened and inspected. Terrain,
foliage, shadows, precipitation and (in the integrated rain view) HUD render.
The only native warning is the existing borderless monitor-selection warning
in the 4K case. No slab allocator, shader, generation or other runtime errors
were logged in the final four native runs. These short tests do not establish
long-term memory stability, and no new 10/15/30-minute soak was performed.

## Reproduction

Run these sequentially, with no builds or other test clients in parallel.
Commands assume the repository root and the unchanged development profile:

```bash
cargo build --locked --workspace

env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-night-lights" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
  TERRA_BENCH_CAMERA=-574,39,962 TERRA_BENCH_LOOK_AT=-574,33,970 \
  TERRA_BENCH_WEATHER=clear TERRA_BENCH_SECONDS=8 TERRA_BENCH_LOAD_SECONDS=240 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan target/debug/terra-client --smoke-test-benchmark
# Repeat with TERRA_BIN_UPLOAD_CACHE=0 for the same-binary reference.

env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-water-transport-2026-10-25/configs-4k" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
  TERRA_BENCH_CAMERA=974,74.8,-557 TERRA_BENCH_LOOK_AT=974,74.1,-562 \
  TERRA_BENCH_WEATHER=rain TERRA_BENCH_SECONDS=8 TERRA_BENCH_LOAD_SECONDS=240 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan target/debug/terra-client --smoke-test-benchmark
# TERRA_FAR_BATCHES=1 restores only the rejected far-batch experiment.

env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/tree-structures-performance-2026-10-25/configs-seed7" \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_WEATHER=rain TERRA_BENCH_SECONDS=24 \
  TERRA_BENCH_LOAD_SECONDS=240 TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod

env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-water-transport-2026-10-25/configs-snow" \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_CAMERA=-1143.5,90,1176.5 \
  TERRA_BENCH_LOOK_AT=-1148,85,1160 TERRA_BENCH_WEATHER="snow 3" \
  TERRA_BENCH_SECONDS=8 TERRA_BENCH_LOAD_SECONDS=240 TERRA_CPU_SCOPES=1 \
  WGPU_BACKEND=vulkan target/debug/terra-client --smoke-test-benchmark
```

The raw `.log` files in this directory include the effective settings, actual
camera poses, resource counts and CPU-scope totals, rather than only selected
FPS lines. Inventory previews still run during part of the integrated sampling
window; their readback is drained before exit. This is deliberately not
described as a fully quiescent single-camera GPU benchmark.

## Validation

All final workspace commands succeeded:

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace --no-fail-fast -- --test-threads=2
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Workspace result: **2,515 passed, seven explicitly ignored, zero failed**.
New regressions cover full-section admission with 12 material batches,
preserving old geometry while deferred, rejecting stale LOD work without
allocations/budget consumption, dense shared prop batches and their unloading,
no extra far cache in ordinary gameplay, and diagnostic-only shadow wrappers.

The excluded, same-version Bevy patch is tested separately because dependency
dev-tests are not workspace members. Its three exact-byte/cache-limit tests
also passed, including signed zero, NaN payload bits, in-place edits, length
changes and the 4-MiB fallback. Its own test lockfile does not change production
dependency versions. Reproduce with:

```bash
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path vendor/bevy_render/Cargo.toml --locked \
  --lib cached_upload_tests -- --test-threads=2
rustfmt --edition 2024 --check \
  vendor/bevy_render/src/render_resource/buffer_vec.rs \
  vendor/bevy_render/src/batching/gpu_preprocessing.rs
```

Headless native `terra-server --smoke-test --no-save` with both `DISPLAY` and
`WAYLAND_DISPLAY` unset succeeds, generating 27 sections and opening/closing
its UDP endpoint. The final native client runs also exercise mesh uploads,
GPU bin-list creation, moving LOD populations, unloading and rendering with
the patch. Build/check/clippy/test logs are retained alongside the benchmarks.

Production changes are confined to the existing renderer and its same-version
local dependency patch. No networking/protocol, save-format, generator-version,
block-ID or gameplay-quality settings were changed for this correction.

## Rejected Experiments

Whole-render-schedule serialization, disabling wgpu validation, a larger mesh
slab growth factor and higher local renderer optimization did not deliver a
reliable gain. None is enabled in the shipped configuration. Buffer-capacity
telemetry and exact CPU scopes are opt-in diagnostics only.
