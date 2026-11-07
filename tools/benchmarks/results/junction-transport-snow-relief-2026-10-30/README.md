# Junction Transport, Snow And Recessed Tracks

Phase 162, locked Bevy 0.20 workspace. Existing block IDs, logical meter cells,
16-cube sections, world generation and dependencies remain unchanged.

## Junctions And Custody

Merger 158 now moves items continuously through the existing world conveyor
transport, not by inventory-tick position jumps. Its three active inputs receive
round-robin turns, exactly one item per successful admission. Splitting an old
stack preserves the remainder's ID and position and assigns a new monotone ID
only after capacity, revision and route checks. Full drop budgets and blocked,
stopped, reversed or unloaded outputs do not consume the source. Existing spatial
occupancy and collision checks run at movement substeps of at most 5 cm.

Waiting and turning junction items remain carried: gravity, lifetime aging and
automatic pickup cannot consume them. Explicit visible, loaded right-click
collection remains available. A runtime restart reconstructs the route from the
real item position, including an already departing side leg. Unmoved custody
clears stale velocity; repair of a changed output span permits a fresh route.

Discovery uses the existing sparse per-section logistics index, intersected
with loaded sections near indexed drops. A persistent cursor admits at most
128 raw objects and 64 router candidates per update, before expensive router
validation. Active routes have a separate fair 64-route movement budget. No
second registry, virtual belt inventory or transport engine is introduced.

## Ports And Categories

- `159`, `filtered_tunnel`: Tunnel (Filtered), using the existing single ghost
  selection and Allowed/Blocked filter. Legacy Tunnel 152 remains unchanged.
- `160`, `belt_distributor`: one rear input and three independently filtered
  outputs: index 0 left, 1 forward, 2 right, relative to its yaw.
- Empty filters pass every item. Exact RGB and wood item identities remain
  distinct. Configuration requires the actual open, visible, nearby authority
  snapshot; ghost controls never own or transfer inventory stacks.
- Region wrapper `RGA9` stores the three private output filters around unchanged
  RGA8 data. Older region readers/migrations remain available; removed objects
  clear their configuration. The network schema is versioned as protocol 90.
- Dyes, including dynamically mixed RGB dyes, return to Materials. The bundled
  Resources/Rohstoffe category is removed; there is no arbitrary RGB catalogue.

## Actors And Snow

Players, mounted horses and active mobs receive belt surface motion through
existing swept movement, including slopes. Airborne bodies and unloaded or
stopped belts do not contribute belt motion. Wall tests retain a powered, intact
belt and verify the complete body stops before an above-deck obstruction.
Inclined player collision uses continuous, section-clipped belt triangles in
the existing terrain collider, not a staircase of profile AABBs. No internal or
section-boundary caps are added; shaft collision remains. Picking, prospective
clearance and mob navigation retain their conservative shared profile boxes.
Ground advection never becomes a stored jump; XZ retains real world velocity.

Natural snow is capped at two eighth-height layers. Manual snow can reach all
eight layers, becoming the existing full snow block. Disjoint manual poses
preserve old stored poses and distinguish player snow from weather-owned caps.
Stairs, slabs, fences and paths can receive snow on their actual geometry.
Budgeted shape visits include generated village paths. Eligible shape coating
targets full coverage after 210 seconds of actual local snowfall, leaving time
for weather transitions and sparse visits before five minutes. This requires
real local snow, suitable temperature and no fire; winter alone creates no dry
snow. Ordinary terrain accumulation is not accelerated by this shape deadline.

## Recessed Footprints

Sand and snow tracks now have real floors below the surface and connecting side
walls in one bounded shared mesh. The original surface is cut out in both color
and material depth/prepass paths, including dry ground beneath a roof. A fixed
GPU lookup uses 128 spatial buckets and four entries per bucket, not a scan of
all footprints per fragment. Geometry is published only when all touched lookup
cells can represent its matching cutout; bucket pressure cannot leave an empty
hole. Support, exact surface height, negative coordinates, neighboring edits and
unload are checked. Fading decreases depth and size. No terrain remesh job or
entity per footstep is added.

## Validation Status

Final workspace validation passed **3359 tests**, with **0 failures** and
**8 ignored**, including a fresh rerun after the final HTML/CSS correction.
Successful commands, serially with one Cargo graph:

```sh
cargo fmt --all --check
cargo check --locked --workspace --all-targets -j1
cargo test --locked --workspace --no-fail-fast -j1
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

All 13 real Avian movement tests passed, including powered belts, walls,
step-up world velocity, uphill/downhill movement, counterwalking and jumping.
The three continuous-profile/section tests passed. Their area check tests the
shared f64 clipper strictly before conversion and bounds f32 area error from
the actual vertex quantization, rather than masking a geometry gap.

Native client and server checks, without a compiler running concurrently:

- `--smoke-test-rgb-logistics`: seven captures; actual RGB crafting/world state,
  saved filtered tunnel, three independent distributor ghosts and real UI
  switches without inventory mutation. The final direction labels were visually
  checked after replacing a nonmatching `h3` selector with a direct label class.
- `--smoke-test-materials`: all 160 block previews ready and 15 captures. Manual
  snow heights and surface-only winter shapes were visually inspected. An
  earlier run timed out at stage 13; separating setup (120 seconds) from capture
  time (90 seconds) allowed the complete rerun without skipping any view.
- `--smoke-test-tracks`: four captures and verified recessed-floor pixels for
  sand and snow, visually inspected. This is an injected-contact rendering and
  fading fixture, not a long-duration walking soak.
- `terra-server --smoke-test`: headless startup and 27 generated sections,
  successful exit. Native logs contain no errors or panics.

Selected local captures:

- [Three distributor filters](rgb-final/belt-distributor-three-filters.png)
- [Filtered tunnel](rgb-final/filtered-tunnel-ghost.png)
- [All eight manual snow heights](materials-final/materials-snow-layers.png)
- [Winter shapes](materials-final/materials-snow-shapes.png)
- [Recessed sand tracks](tracks-verified/tracks-1.png)
- [Recessed snow tracks](tracks-verified/tracks-2.png)

No FPS gain, universal frame-rate floor or new long-run stability result is
claimed for this phase. No new 10/15/30-minute native soak was performed. External
visual, audio, multi-machine and additional-GPU acceptance remains open.
