# Living Rivers, Foliage And Cell Water

## Architecture

- River sampling retains the existing deterministic catchment paths. Width,
  depth, asymmetric portal erosion and occasional rejoining side channels are
  additional samples of those paths, not another terrain generator.
- Portal overburden blends over 8-23 m; cataract ceilings taper over 28 m.
  Full waterfalls and their carved shafts remain connected. Beds vary between
  shallow riffles and deeper pools; side channels are generally shallower.
- Seeds, world/block IDs and generator version 5 are unchanged. Regenerating an
  existing `rivers = true` world changes its base river geometry; edits retain
  their coordinates. Compare the change in a new world. `rivers = false` keeps
  its existing generated blocks.

## Dynamic Water

`FluidCell` belongs to the existing `EditJournal` and `SectionDelta`: amount
0-8, horizontal flow and source status. No terrain material IDs, cube GLBs or
per-cell ECS entities are added. Natural water is sampled as an implicit
unlimited reservoir; escaped water has finite volume and cannot create sources.
Returning liquid can be absorbed by a natural reservoir. `FluidStep` accounts
for transfers, sourced volume and absorbed volume separately.

The integrated/dedicated authority processes at most 256 queued cells every
0.1 seconds. Downward flow precedes lateral equalization. Mining wakes nearby
water, placed solids block it, and unloaded borders suspend work until reload.
Placing a solid into a finite cell removes that cell's liquid, rather than
teleporting it elsewhere. There is no pressure solver, erosion, global ocean
update or simulation outside loaded regions. Finite puddles settle at eighth-
block resolution; this is a bounded gameplay solver, not a fluid dynamics model.

Fluid-only revisions do not invalidate terrain, props or colliders. Two bounded
asynchronous jobs rebuild affected section water batches. Water uploads retain
the old mesh until a current replacement exists. Shared corners match the
existing river surface, and captured block edits override stale neighbor data.
Idle queue processing does not mark saved state dirty. The existing save path
persists fluid changes, and old saves without fluid fields load through serde
defaults. Network protocol 7 carries the fluid revision: both endpoints must
use this build. An older executable should not be used to overwrite a new save.

Swimming, fog and player-position audio read the same loaded cell levels.
Nearby dynamic audio candidates are scanned from local section deltas without
cloning the complete journal. Quiet river mixing and the existing audio options
are retained.

## Foliage

The three existing leaf variants share masked extended materials. The existing
canopy PNG keeps its RGB artwork and gains sparse cutouts with coverage-preserving
mips in the existing texture worker. Wind deformation is shared by forward,
depth and shadow vertex paths; bounds include its maximum displacement.
Logical blocks, collision, mining and tree generation remain unchanged.
Closed crowns use backface culling; the flat falling particles remain double-
sided. The final mask has approximately 12% openings. Meshing distinguishes
visible faces behind masked leaves from genuinely empty neighbors, preserving
closed bevel/micro interfaces. The face record stays 48 bytes on this target;
only rectangles with mixed leaf/air contacts need splitting.

Up to 48 cosmetic falling leaves share one reusable mesh/material/entity.
Updates run at 30 Hz; source searches are bounded and use loaded leaf blocks.
Leaves stop at real solids or unloaded space and are cleared at session changes.
They do not remove logical foliage or create gameplay drops.

## Validation

All five requested workspace commands pass. `cargo test --workspace` reports
302 passed, zero failed and one explicitly ignored network soak. Existing mixed
material/LOD seam tests were not weakened. Separate `cargo build -p terra-server`
and a native start without DISPLAY/WAYLAND_DISPLAY pass. Its feature tree has no
renderer, PBR, winit, audio, TiltUI or wgpu dependency. Logs are under `native/`.

New checks cover bed/width variation, connected side channels and a solid island,
natural portal samples, finite volume, source/absorption accounting, fractional
sources, negative section boundaries, block barriers, unload/resume, persistence,
legacy saves and invalid data. Real UDP tests verify two replicas, successive
fluid-only revisions, late joining and removal without terrain invalidation.
Further tests check contiguous swimming levels, stale async results, shared mesh
corners/indices, idle change detection, foliage masking and bounded particles.

The native basin fixture uses real block edits and the installed authority tick:
240 finite units remain conserved after opening its section-boundary gate;
44 units had crossed in the first recorded flowing frame. Reclosing the gate
blocks water without liquid inside the solid. Terrain revisions stay unchanged
during fluid-only updates. The screenshots show a deliberately constructed test
basin, not a claim about procedural river appearance.

Native river bifurcation checks use actual generated channels with a dry, solid
island. Initial visual inspection also caught overly strong foliage perforation;
the mask was reduced and mixed leaf/trunk seam tests were retained during fixes.
Final Vulkan captures were inspected on an AMD RX 6950 XT / Mesa 26.2.4. The final
foliage fixture observed three variants, 12 active falling leaves, one batch and
20 emitted leaves. A fixed 580x540 foliage crop excluding the HUD differs by
2.09% between the stationary-camera wind captures (`magick compare -metric AE`);
this contains wind, moving shadows and particles, not an isolated shader unit
measurement. Portal, bifurcation, finite flow and 31.5856-m falls were also tested
natively. Audio and movement smoke tests pass. Audio output/filtering is tested
technically, not a new human listening/mix approval. Art-direction approval and
large/long-running flood tests remain open in TODO.md.

## Measurements

Same 18-second samples, physical 1920x1200, VSync off, FOV 90, horizontal/vertical
view 16/3, four generation jobs, sixteen composition jobs. Identical camera
anchors and seed 42042 before/after, with no concurrent compile or second native
client. `before/` and `after/` retain complete six-run suites. River geometry
changes intentionally, so matching cameras do not imply identical primitives.

| View | FPS Before | FPS After | p95 ms Before / After | p99 ms Before / After |
|---|---:|---:|---:|---:|
| Forest1 |114.608|105.445|10.588 /10.939|14.239 /15.320|
| Forest2 |113.785|104.758|10.672 /11.068|14.244 /14.824|
| Offline |140.642|136.030|8.722 /9.341|9.592 /10.630|
| Integrated |141.271|136.765|8.764 /9.229|9.504 /10.222|
| Fixed river1 |141.959|137.262|9.089 /9.101|11.524 /11.488|
| Fixed river2 |142.902|137.598|8.939 /8.870|11.204 /10.412|

There is a measurable regression, not performance parity: forest mean FPS falls
7.96%, fixed-river mean 3.51%. Two intermediate double-sided forest runs measured
96.436/95.366 FPS; backface culling recovers part of that cost. Those logs are in
`intermediate/`. The first forest GPU opaque pass rises from 1.537 to 1.983 ms
(intermediate 2.807 ms); prepass 0.963 to 0.996 ms. These are instrumented pass means,
not total GPU frame time. Actual draw-call count is unavailable; mesh/entity
counts below are not labeled draw calls.

| View1 | Initial Load s Before / After | RSS KiB Before / After | Composition Worker s Before / After |
|---|---:|---:|---:|
| Forest |3.179 /3.539|755864 /858772|0.821 /0.890|
| Offline |3.113 /3.263|780916 /812268|0.605 /0.702|
| Integrated |3.074 /3.261|791268 /811488|0.686 /0.744|
| Fixed river |3.129 /3.159|770188 /808124|0.601 /0.625|

| View | Loaded Sections | Mesh Assets Before / After | Visible Mesh Entities Before / After | Represented Props Before / After |
|---|---:|---:|---:|---:|
| Forest |7704|9171 /9313|4764 /4848|89866 /89866|
| Offline / Integrated |7623|9235 /9316|4615 /4656|86727 /86727|
| Fixed river |7635|9028 /9085|4591 /4617|63245 /63180|

Fixed-view water tiles change from 49 to 45 with the river profile. Forest process RSS
is about 100 MiB higher in run 1; the logs do not isolate CPU allocations from driver
memory. Composition worker time is summed job time, not initial-load wall time.

The separate frame-paced CPU streaming example still loads 4375 sections in
158 frames / 2.626 s, matching the previous phase's recorded 2.626 s. This comparison
uses the previous report for the CPU baseline, not a fresh old-binary CPU run.
Current move 117 ms, eviction 5273 sections / 4.934 s, reload 2.626 s; peak process RSS
201296 KiB. GPU uploads are excluded and the example's 60 Hz dispatch budget affects
these timings. Stationary native views do not benchmark large active floods.

## Reproduction

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
TERRA_CAPTURE_DIR=OUTPUT target/debug/terra-client --smoke-test-fluid
TERRA_CAPTURE_DIR=OUTPUT target/debug/terra-client --smoke-test-foliage
TERRA_RIVER_TUNNEL=1 TERRA_CAPTURE_DIR=OUTPUT target/debug/terra-client --smoke-test-rivers
TERRA_RIVER_BIFURCATION=1 TERRA_CAPTURE_DIR=OUTPUT target/debug/terra-client --smoke-test-rivers
TERRA_RIVER_BASELINE_VIEW=1 bash tools/benchmarks/run-hydrology-suite.sh OUTPUT
```

Visual fixtures use the normal presentation settings. The benchmark script uses
18-second samples, VSync off, 1920x1200 physical pixels, the same seed, camera
anchors and streaming/render budgets before and after. Background compilation
is excluded from timed runs. Stationary benchmarks do not establish performance
for large excavations, long-running floods or multiplayer WAN load.
