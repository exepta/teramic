# Blocks, Materials, Inventory Previews And Mining

## Implementation

The existing palette now has 19 blocks. IDs 1..10 are unchanged; appended IDs
11..19 are Sandstone, Smooth Stone, Mossy Stone, Solid Water, Glass, Oak Planks,
Birch Planks, Spruce Planks and Clay. Sand and Stone received distinct procedural
surfaces; Dirt keeps its existing texture. All ordinary blocks use Cube JSONs,
the existing section mesher and shared materials, not individual GLBs/entities.
Seven small repeatable 64x64 texture patterns have complete mip chains and are
cached once. Four existing PNGs remain shared. There are 20 base materials,
32 including the existing prop palette; neither figure is a draw-call count.

Desert strata contain sandstone, submerged sandy ground has patchy clay, and
damp daylight cave walls occasionally receive moss. These change material IDs,
not terrain density or geometry. Smooth Stone, Glass, Planks and Solid Water
are building materials, not new natural terrain. Existing seeds, block IDs,
world format 1 and generator 5 are retained. New deposits can appear in unedited
parts of existing worlds; saved edits remain authoritative. Protocol 9 prevents
older clients from joining a server whose extended palette they cannot display.

Glass uses 0.1-alpha panes with thin wood borders only at connected group edges.
Neighbor padding determines frames across section boundaries, at every LOD.
Internal faces are culled. This uses ordinary transparent rendering, not an
order-independent transparency pipeline. Solid Water is a collidable, non-flowing
building cube with the existing animated water shader; it blocks fluid flow and
does not count as swimming water, underwater audio/fog or a fluid source.

Inventory and material HUD render the actual block mesh/material combinations
once into 128x128 GPU images, including multi-face textures, glass and foliage.
A GPU readback checks for nonblank output before publishing each image. One
temporary isolated camera/light is removed after the queue; there is no permanent
inventory scene render. TiltUI owns all UI elements, with a small generated-img
handle adapter. Harvested plant resources still use their resource symbol.

Debug defaults off. F1 toggles the overlay on release; F1+B toggles the placement
box and enables debug. Menus, rebinding and inventory suppress debug shortcuts
and overlay visibility without discarding the toggle. The player hand and held
viewmodel are removed. Twelve cosmetic damage stages grow irregular dark
Voronoi fractures and small material flakes. Only exposed faces are affected;
the real block remains intact until server confirmation. Creative still removes
immediately. Particles remain capped at 48; no zero-vertex mesh is uploaded.

## Validation

All five workspace commands passed: fmt --check, check, test, Clippy --all-targets
with -D warnings, and build. **394 tests passed, zero failed; one existing long
network soak remains ignored.** The separate server build and smoke start with
DISPLAY/WAYLAND_DISPLAY unset passed. Logs are in `native/`.

Coverage includes material classification, old IDs and custom journal IDs,
deterministic deposits, no generated building-only blocks, disk/inventory reload,
real UDP edits for every added material, solid collision and water exclusion,
glass edge masks and section padding, valid mesh indices, monotonic fracture
stages, cancellation, transparent neighbors, empty meshes, preview source meshes,
debug defaults/chords/menu gates and reserved-key rebinding.

## Native Inspection

Native Vulkan/RX 6950 XT tests use the actual renderer and local UDP Integrated
Server. The material fixture places all 19 blocks using the existing edit path,
including a 2x2 glass group crossing both X and Y section boundaries. Its cameras
capture all materials, joined glass and Solid Water. Gizmo selection outlines
are disabled only in this fixture so they cannot be mistaken for glass borders.

Survival captures verify default-hidden placement, debug opt-in, cancellation,
progressive damage, authoritative removal, pickup, actual inventory preview
handles, replacement and input locks at 1920x1200 and 960x640. Creative checks
immediate removal and no resource consumption. Captures are in `captures/`.
These are automated native input/render tests and inspected screenshots, not
manual multiplayer testing on a second machine or complete art-direction approval.
An isolated copy of the asset directory enables the existing CustomMesh 65000
fixture: three imported hierarchical models beside a Stone cube rendered and
uploaded with the correct edit epochs. This is an offline renderer test, not
an extension of the authoritative multiplayer palette to arbitrary custom IDs.
Final observed grass-block removal was 601 ms in Survival (0.55 s authority
duration) and 28 ms in Creative. These local observations are not network-latency
guarantees. All final native logs are free of ERROR/WARN.

Problems found during validation were corrected rather than hidden: an initial
fixture incorrectly expected a section mesh after the renderer consumed it;
early empty fracture stages triggered Bevy allocator warnings; the first fracture
pattern was too regular; and F1+B also activated the default B placement binding.
Final logs are archived separately from these exploratory failures.

## Measurements

One current 18-second forest sample: seed 42042, 1920x1200, FOV 90, VSync off,
radius 16/3, same established forest camera. No parallel Cargo process or second
Terra client ran. Config snapshots and raw logs are included. No immediately
preceding old-binary run was captured, so these are current values, not a proven
speedup or performance-parity result.

| Metric | Current |
|---|---:|
| Average FPS | 109.056 |
| p95 / p99 wall frame | 10.685 / 14.115 ms |
| Initial load | 3.550 s |
| Process RSS at sample | 886972 KiB |
| Loaded / rendered sections | 7704 / 3531 |
| Mesh assets / visible mesh entities | 9315 / 4848 |
| Represented props / batches | 89866 / 1277 |
| Terrain composition summed worker time | 0.732 s |
| Prop composition summed worker time | 0.286 s |
| Main opaque GPU pass mean | 1.970 ms |

Worker sums are not wall latency; one GPU pass is not total GPU frame time.
Exact draw calls were not measured. `/proc` sample RSS and the process wrapper's
peak RSS are different accounting sources; both raw values are retained.

The existing frame-paced CPU-only streaming example reports cold 4375 sections
in 2.626 s, one-column move in 0.117 s, eviction to 5273 sections in 4.934 s and
reload of 4375 in 2.626 s. Its historical cold baseline was also 2.626 s. The
job/frame budget limits this measurement; it is not a pure CPU throughput result
and does not include GPU upload. Earlier foliage and frame-tail costs remain
open in TODO.md.

## Limits And Reproduction

Crafting is still open: building-only blocks are available in Creative, while
natural blocks can be harvested in Survival. Drop markers are still simple
shared geometry, not the inventory's exact preview renderer. No animated custom
model pipeline, fluid behavior for Solid Water, or new external dependency was
added. Long-running stress tests and final visual acceptance remain separate.

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo build -p terra-server
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --bind 127.0.0.1:0 --no-save --smoke-test
TERRA_SCREENSHOT=/tmp/materials.png target/debug/terra-client --smoke-test-materials
TERRA_CAPTURE_DIR=/tmp/survival target/debug/terra-client --smoke-test-building
TERRA_BUILD_CREATIVE=1 TERRA_CAPTURE_DIR=/tmp/creative target/debug/terra-client --smoke-test-building
cargo run --locked -p terra-world --features bevy --example streaming_benchmark
TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_MODE=offline TERRA_BENCH_FULL_MICRO=0 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```
