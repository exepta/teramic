# Voxel Vegetation, Closed Blocks And HUD

Native Linux/Vulkan runs on 2026-10-14, AMD RX 6950 XT / RADV Mesa 26.2.4.
Seed 42042, generator version 5, block IDs and one-meter cells unchanged.
Runs were serial, without concurrent compilation or another benchmark.

## Geometry Correction

The additional physical face relief was rejected after visible holes and raised
block surfaces. It has been removed, including its template cache. Ordinary
blocks use the shared closed cube-face generator again: planar main faces,
small exterior bevels inside the cell, and canonical neighbor joins. No new
GLBs, render pipeline, or per-block entities are introduced.

The new regression test checks seven ordinary block materials, every nonempty
visibility mask (63), and all three LODs. Every vertex must remain in its cell;
sampled main faces must lie on the logical block plane. All seam ray tests now
reject backfaces, so incorrect winding cannot masquerade as a closed surface.
Existing negative-coordinate, mixed-material, concave and mixed-LOD seam tests
also pass. Foliage microgeometry and custom meshes retain their intended shapes.

`cubic-terrain.png` and `cubic-flight-1.png` were visually inspected after this
correction. They show restored planar terrain faces. This is not a claim of
exhaustively inspecting every world coordinate or matching the reference image.

## Other Changes Retained

Grass has upright variable-height voxel blades and stepped tips; flowers have
stalks, leaves, petals and centers, retaining the original color variants.
Rendering remains section-batched. Natural props are filtered against original
support/occupancy before edits; covering a loaded plant or changing its support
retires it in the existing session journal. Mining tree parts cannot create new
plants, including across a section ceiling. Dedicated tests cover generation,
individual removal, neighboring plants and eviction/reload.

Water has stronger turquoise tint and absorption while preserving shallow-bed
visibility and quality controls. Reflections remain the existing Fresnel/sky
approximation, not new planar reflections or SSR.

The square-framed HUD shows FPS, process CPU/RSS, chunk, XYZ and the actual
looked-at block/prop. Linux process metrics are sampled at 1 Hz. CPU is normalized
to available logical cores; unsupported platforms display `--`. The voxel-style
settings menu retains its existing FOV/view-distance and other working bindings.

## Verification

All five workspace checks passed after the geometry correction:

- `cargo fmt --all --check`
- `cargo check --locked --workspace`
- `cargo test --locked --workspace`: 177 tests
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace`

`cubic.log` passed native grass/flower/terrain captures and actual HUD target,
CPU and RSS checks. `cubic-mining.log` passed mining/remeshing; `cubic-flight.log`
completed 12,997 LOD replacements and drained all mesh jobs. `server.log` passed
generation of 27 sections without DISPLAY/WAYLAND_DISPLAY. Earlier native runs
in this folder passed water, individual prop removal, physics and settings,
including normal and 640x480 logical settings captures. Water shader compilation
was exercised natively. Settings smoke checks actual handlers/widget values;
physical pointer dragging remains unverified, as documented in the preceding
player-settings report.

`botanical-*.png`, `flight.log` and `flight*.png` are **interim artifacts with
the now-removed relief**, not the final geometry. Final captures/logs use the
`cubic` prefix. The water/settings captures are still applicable.

## Comparable Native Flight

Same 48-second out-and-back camera path, 1920x1200 physical pixels, VSync,
FOV 45, view distance 12, GPU pass diagnostics enabled. An isolated temporary
configuration preserves the user's current FOV 90, VSync-off, distance-16 file.
Previous baseline: `../player-settings-2026-10-14/flight-final.log`.

| Metric | Previous | Final Closed Cubes |
|---|---:|---:|
| Mean FPS | 59.997 | 59.924 |
| p95 wall frame | 18.286 ms | 19.764 ms |
| p99 wall frame | 29.424 ms | 30.011 ms |
| Initial ready | 3.310 s | 3.297 s |
| Terrain composition worker spans | 3.128 s | 3.191 s |
| Prop composition worker spans | 1.147 s | 1.316 s |
| End RSS | 668812 KiB | 664292 KiB |
| Loaded/rendered sections | 4404/1988 | 4404/1988 |
| Mesh assets/visible mesh entities | 5181/1672 | 5181/1672 |
| Represented props/section batches | 50605/721 | 50605/721 |
| Main opaque GPU pass mean | 1.244 ms | 1.273 ms |
| Early prepass GPU mean | 0.501 ms | 0.493 ms |

Final opaque GPU p95/p99: 1.413/1.565 ms. Wrapper peak RSS: 681584 KiB.
GPU passes are not summed into a claimed total GPU frame time. Exact draw calls
and individually visible props are not instrumented. The pre-existing frame-tail
regression remains open; this run's p95/p99 and prop worker spans are worse than
the baseline, not a performance improvement. CPU composition spans are summed
worker elapsed time, not main-thread frame costs.

## CPU Generation

Four workers, 625 columns / 4,375 sections, one warmup and five measured runs:

| Worker throughput | Previous Mean | Current Mean |
|---|---:|---:|
| Headless generation | 0.204690 s | 0.214267 s |
| Render preparation | 0.363971 s | 0.388039 s |

Current throughput is about 5%/7% slower in these runs; no isolated cause is
claimed. These benchmarks do not include renderer-specific composition/GPU
uploads, and removing render relief does not affect their code path. Checksums
remain 22,955,883, with 50,506 props and 450,427 greedy quads.
Streaming: cold 2.626 s, one-column move 0.117 s, eviction 4.933 s,
reload 2.626 s; peak RSS 167092 KiB. Raw output: `workers.log`, `streaming.log`.

Reproduce the native flight after building with the same isolated settings:

```bash
TERRA_CONFIG_DIR=/tmp/terra-voxel-detail-benchmark-config TERRA_BENCH_SECONDS=48 TERRA_RENDER_DIAGNOSTICS=1 TERRA_SCREENSHOT=/tmp/terra-flight.png python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
```
