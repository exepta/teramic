# Inward Micro Surfaces And Missing-Triangle Fix

2026-10-14, native Linux / Bevy Vulkan, pinned workspace dependencies.
Seed 42042, generator v5, one-meter blocks and all existing IDs unchanged.

## Implementation

Optional `surface` in the existing block JSON registry selects turf, earth,
rock, sand, snow or wood. Absent fields preserve legacy cube/custom-mesh assets;
custom meshes cannot accidentally opt into cube tessellation. Existing textures,
material handles and face/grass-cap assignments remain shared.

The near mesh uses four deterministic variants with grouped small plateaus and
inward cuts. Turf tops cut at most 1.8 cm, earth/turf sides 3.5 cm, rock 4.5 cm,
wood 2.5 cm, sand/snow 1.2 cm. All vertices stay inside the unit cell. Outer
face borders, bevel outlines, bottom faces and thin grass side caps stay intact.
Closed step walls connect depths. There are no protruding panels, missing-base
decorations, new entities per piece, normal-map pipeline or extra GLBs.

Equal-height/equal-tone regions merge greedily; profile templates cache on
workers by face, variant and visibility mask. A few vertex tones multiply the
existing biome/condition color. Final geometry still joins the existing section
material batches. Middle/far terrain keeps its existing greedy geometry.

The user's screenshot exposed openings at wood joins. One concrete defect in
the new micro-surface path discarded an entire quad when its first triangle
collapsed during neighbor flattening. Its second triangle could still be valid.
Triangles are now independently checked for degeneracy, winding and normals.
The dedicated regression would fail with the whole-quad early return.

## Verification

All passed: `cargo fmt --all --check`, `cargo check --locked --workspace`,
`cargo test --locked --workspace` (181 tests),
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo build --locked --workspace`. Logs are alongside this file.

Added tests cover the collapsed-half case, every surface style/variant,
finite bounded geometry, valid indices/normals, bounded per-face geometry,
shared template cache, grass side caps, and schema compatibility. Dense border
rays compare detailed faces with the unchanged cube shell for seven ordinary
materials and all 63 nonempty visibility masks. Existing negative-coordinate,
concave, stepped and mixed-material/LOD seam tests also pass.

`--smoke-test-surface` makes a **session-only test fixture**, not a saved world
or new generator: connected wood, grass and rock across negative X/Z and
vertical section borders. It waits for current mesh epochs, captures the joined
fixture, mines one wood corner, waits for remeshing, and captures again.
`surface.log` passed; `surface-joined.png` and `surface-mined.png` were visually
inspected. No unintended see-through join was visible in these captures. The
second image intentionally has the single mined cell open, with neighboring
faces retained. This is not an exhaustive visual assertion for the entire world.

`botanical-final.log` passed native target/HUD checks and three close-ups;
`final-terrain.png` was inspected. `flight.log` passed its out-and-back route,
completed 12,971 LOD replacements and drained all jobs. `water.log` exercises
native water rendering over the updated terrain. `server.log` passed a headless
startup and generation of 27 sections without DISPLAY/WAYLAND_DISPLAY.

The earlier `detail-*.png`/`botanical.log` capture the first, overly regular
surface pattern. They are interim artifacts, not the final grouped geometry.
No claim of matching the reference image's full art direction is made.

## Performance Limitations

**This flight is not a valid isolated before/after benchmark.** A separate
user-started Terra client was discovered running during the native tests. It
was left untouched, including its session edits. Both clients shared the GPU.
The user's process still had an older executable mapping after the new build;
code corrections require restarting that client.

Raw flight measurements are retained for transparency, not as a claimed
isolated speedup or an isolated regression attributable to this change:

| Contended Run | Value |
|---|---:|
| Mean FPS | 41.110 |
| p95/p99 wall frame | 26.526 / 42.533 ms |
| Initial ready | 5.159 s |
| End RSS | 835952 KiB |
| Loaded/rendered sections | 4404 / 1988 |
| Mesh assets/visible mesh entities | 5181 / 1672 |
| Represented props/prop batches | 50605 / 721 |
| Terrain composition worker spans | 6.121 s |
| Prop composition worker spans | 1.387 s |
| Main opaque GPU pass mean | 1.752 ms |
| Early prepass GPU mean | 1.068 ms |

The detail adds real geometry and has a cost; an isolated replay is still
required to quantify it. GPU pass values are not total GPU frame time. Draw
calls and individually visible props are not instrumented. The pre-existing
tail-frame issue remains open. CPU generation/streaming benchmarks were not
rerun: this step changes renderer composition, not the voxel generator.

Flight settings: isolated `TERRA_CONFIG_DIR` with FOV 45, VSync, view distance
12, 1920x1200 physical pixels, 48-second route and render diagnostics. The
user's own configuration was not changed. Reproduce with only one Terra client
running after a workspace build:

```bash
TERRA_CONFIG_DIR=/tmp/terra-voxel-detail-benchmark-config TERRA_BENCH_SECONDS=48 TERRA_RENDER_DIAGNOSTICS=1 TERRA_SCREENSHOT=/tmp/terra-flight.png python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
TERRA_SCREENSHOT=/tmp/terra-surface.png target/debug/terra-client --smoke-test-surface
```
