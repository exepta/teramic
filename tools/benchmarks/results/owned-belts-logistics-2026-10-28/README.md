# Owned Conveyor Spans, Stress And Item Logistics

## Implementation

- Hand crank capacity: 128 at 30 RPM. Small gears and shafts consume 1,
  large gears 2, crusher 20, and a complete belt 2 once at its controller.
  Hidden material-specific torque ceilings are removed. Existing signed
  ratios, fractional RPM, overload protection and bounded graph solving remain.
- Conveyor items connect two parallel horizontal shafts in two clicks, with
  up to 32 intermediate cells. Horizontal and 45-degree spans carry real item
  stacks; vertical spans transmit rotation. End shafts remain separate blocks.
  Shift cancels selection; breaking a segment returns one connector and removes
  its owned span. Connected endpoints/spans reject independent wrench rotation.
- Oak remains the default. Birch, Spruce and Cherry wooden gears, large gears,
  shafts and conveyors use existing wood metadata, recipes, Creative and TMI.
- `teramic::item_hopper` (new ID 152) extracts only finished furnace/crusher
  output onto a powered band below it. `teramic::item_importer` (153) attaches
  to a chest and collects passing band stacks. Four items per extraction,
  half-second scheduling, loaded-only ports and bounded budgets are enforced.
  Output removal and drop creation, or drop removal and chest insertion, commit
  atomically. Partial/full chests preserve unaccepted items and drop identity.
- Water-wheel player collision is only its 0.186 m thick, cell-length axle.
  The full 2.5 m rim remains visible/selectable and samples water motion.
- Small teeth extend to radius 0.55 m; their 0.05 m radial floor penetration is
  permitted. Large gears similarly permit their 0.55 m under-cell overhang.
  Side walls and buried axles remain invalid; floor contact is not blanket
  collision disabling.
- Sand has warm multi-scale fine grain that survives normal mip levels and
  a granular PBR normal response. It has no gray pebbles. Existing 28-vertex
  face budget, 4 mm inner relief and planar section-border rim are unchanged.

## Water Wheel And Transport Setup

1. Place the wheel with its horizontal axle perpendicular to a real current.
2. Immerse the lower paddles in a river, or direct falling water onto one side
   of the rim. Centered/opposing flows can cancel; still lakes produce no power.
3. Attach a shaft directly to its hub, then connect gears and the consumer.
4. Place two parallel belt shafts and use one Conveyor item on each endpoint.
5. For storage, attach a Hopper to a furnace/crusher over the belt, then an
   Importer to the destination chest over the same transport path.

The pre-existing wheel capacity is 32 times its water-strength factor. Current,
fall and ocean factors remain 1.0/1.2/1.1 when calm and 1.2/1.35/1.4 at full
storm; stronger water increases capacity, not RPM. A hand crank can therefore
drive a larger network than a single wheel under these requested/custom values.

Create's primary implementation informed the two-shaft connector and
controller-owned belt model, not a second simulation or copied engine:
[BeltConnectorItem](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/src/main/java/com/simibubi/create/content/kinetics/belt/item/BeltConnectorItem.java),
[BeltBlockEntity](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/src/main/java/com/simibubi/create/content/kinetics/belt/BeltBlockEntity.java).
The numeric loads are Teramic's requested values, not Create's defaults.

## Architecture And Bounds

Persistent ownership reuses `BlockObjectState.peer` and the existing section
journal; no second item, prop or inventory registry exists. Endpoint validation,
clearance, selection, collider profiles, surface heights and transport share
`terra_world::conveyors`. Protocol 81 validates the connector action, held slot,
section/inventory revisions, player reach and occupied world volumes.

One continuous ribbon replaces repeated standalone belt frames. Cached meshes
are keyed by bounded length/slope combinations; UV motion does not rebuild
geometry. Gears and shafts share wood-specific materials while metal stays
unchanged. Each belt ribbon owns one material for its independent UV offset.
Hopper/importer meshes have 288/264 authored vertices and use normal
section batching. Transfers do not invalidate terrain meshes.

Graph limits remain 1024 admitted nodes per step and 256 per component. Transport
processes at most 128 active on-band stacks per tick; larger populations share
the budget and move more slowly. Passive ports process at most 64 loaded ports
per half-second cycle. Incomplete/unloaded spans stop rather than simulating
unloaded terrain. Full storage does not delete items; unaccepted stacks can
continue to a belt end and fall as ordinary collectible world drops.
Verified on-band stacks do not age out while stopped or unloaded. This uses
three bounded persistent surface probes in the existing drop simulation, not
another inventory. Leaving/removing the band resumes normal drop expiration.
Drop interpolation also recognizes authoritative pose changes without an age
change, so transported items remain visibly mobile while lifetime is paused.
Identical snapshots do not restart interpolation. Authored conveyor assets keep
unit-cell bounds even though their runtime spans use extended spatial metadata.

Existing block IDs, generator identity and save format remain unchanged; new
IDs are appended. Legacy single-cell conveyors continue to load and operate.

## Validation

All commands below completed successfully on Linux with Rust 1.97.1:

| Check | Result | Log |
|---|---|---|
| `cargo fmt --all --check` | Passed | [fmt](logs/fmt.log) |
| `cargo check --locked --workspace --all-targets -j1` | Passed | [check](logs/workspace-check.log) |
| `cargo test --locked --workspace -j1 -- --test-threads=2` | 2932 passed, 7 existing ignored | [tests](logs/workspace-tests.log) |
| `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings` | Passed | [Clippy](logs/clippy.log) |
| `cargo build --locked --workspace -j1` | Client and dedicated server passed | [build](logs/workspace-build.log) |
| Native `--smoke-test-kinetics` | Passed, approximately 27 seconds | [kinetics](logs/native-kinetics.log) |
| Native `--smoke-test-materials` | Passed, approximately 107 seconds | [materials](logs/native-materials.log) |

Native runs used the RX 6950 XT, Vulkan/RADV Mesa 26.2.5, X11 and isolated
copies of the repository configuration. Neither native log contains warnings,
errors or panics. Four kinetic screenshots and six distinct gear icons were
captured. Forward/reverse/fractional RPM, stopped transforms, stable asset
counts, all three span profiles and actual item transport were checked. The
final fixture requires two real drop mesh entities whose rendered positions
match the transported authoritative positions; a journal-only assertion is not
accepted. The material run loaded 153 block models and 153 GPU previews.

The [belt/item close-up](captures/kinetics-stretched-belts-items-woods.png) and
[sand/material reference view](captures/materials-reference-terrain.png) were
visually inspected. The first offline fixture exposed a missing ready drop
source, leading to the stronger mesh checks and same-age interpolation fix.
The final run includes both corrections. These are short functional/rendering
checks, not an FPS benchmark, a full Create feature-equivalence claim, a
long-soak result or a two-machine multiplayer acceptance test.

Regression coverage includes the real cranked, owned conveyor delivering eight
furnace-output items into a real chest; output-only extraction and backpressure;
single/double-chest partial insertion; late partner revision overflow; exhausted
drop IDs; index rebuilding; and actual repeated save/load with active drops,
machine progress and source/target inventories across negative region borders.
Additional regressions cover same-age drop pose changes, identical snapshot
interpolation, stopped poses and strict authored conveyor model bounds.

## Main Code Paths

- `terra-core/src/kinetic.rs`, `gameplay.rs`, `crafting.rs`: loads and wood items.
- `terra-world/src/conveyors.rs`: span ownership, geometry and transport.
- `terra-network/src/server_conveyors.rs`: authoritative connector transaction.
- `terra-world/src/edits/automation.rs`, `terra-network/src/server_logistics.rs`:
  atomic output extraction and chest insertion.
- `terra-world/src/interaction_shapes.rs`, `terra-physics/src/terrain.rs`:
  physical axle separated from water-wheel picking.
- `terra-render/src/crushers/belts.rs`: continuous shared ribbon and UV motion.
- `terra-render/src/building/drops.rs`: visible same-age transported snapshots.
- `terra-render/src/blocks/sand.rs`, `assets/shaders/terra_wet_surface.wesl`:
  fine grain and granular surface response.
- `tools/assets/build_logistics.mjs`: reproducible port meshes/collision boxes.
