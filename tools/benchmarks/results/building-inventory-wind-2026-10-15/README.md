# Building, Inventory And Rooted Prop Wind

## Implementation

The former TODO point 1 is implemented through the existing authoritative
Integrated/Dedicated session path. Explicit Creative mode removes a target on
the initial input without a mining timer. Survival sends a held mining intent;
the server owns elapsed time and rechecks the target, revision, permissions,
reach and occlusion each tick. Release, target change, menus and heartbeat loss
cancel progress. Raw instant-removal commands cannot bypass Survival timing.
Legacy vegetation-density hints cannot hide logical props from server picking.

Six progressive crack stages, a first-person hand swing, a placement pulse and
at most 48 short-lived break particles provide feedback. Rejected edits do not
emit break effects. A red/green placement ghost checks inventory and local/remote
player overlap, and hides while mining so it cannot cover cracks. Hand geometry
fits FOV/aspect/near-plane constraints inside horizontal player clearance,
without a second camera or render pass. Two shared cube meshes and existing
material handles serve feedback, held items and drops; neutral grass receives
a green vertex tint instead of appearing white.

Targeted Survival block/prop removal creates one resource. Server-side pickup
within 2 m requires clear loaded air and inventory capacity. Successful placement
consumes one matching block only after its edit succeeds. Inventories have 27
slots of up to 64 items, keyed by player UUID. Inventories and loose resources
are persisted in the existing EditJournal/save path. Old saves lacking gameplay
data load with defaults; no competing store or destructive migration was added.
Creative grants all existing building materials and neither consumes nor drops
resources. The TiltUI inventory opens with rebindable E, blocks gameplay input,
supports material selection, and waits for held mouse actions to end before
recapturing gameplay. Escape closes the inventory before opening Pause.

Grass now forms varied radial groups of broad, blunt-ended blades. Bushes use
irregular, layered leaf clusters and small branches instead of a solid crown.
Near/Middle/Far grass geometry remains 280/160/80 vertices; bush geometry remains
312/144/96. All kinds, deterministic IDs, picking bounds and section batches
remain. A shared extended material reuses UV0 for squared root-relative height
and flexibility. GPU wind anchors roots, uses 0.026 m sway plus 0.007 m flutter,
is reduced in Middle LOD and disabled in Far LOD. Forward, depth, shadow and
motion-vector positions agree; conservative bounds include the displacement.
Stones and mushrooms remain still. No per-plant or per-blade entities were added.

## Validation

All workspace commands passed: fmt --check, check, test, Clippy --all-targets
with -D warnings, and build. **348 tests passed, zero failed; one existing
long network soak remains ignored.** Logs are in `native/`.

New coverage includes Creative immediate block/prop removal, Survival timing,
release and same-target rehold, stale server progress, input gating, invalid
placement without consumption, real UDP pickup/reconnect, full inventories/drop
budgets, wall occlusion, individual prop removal, compatible old saves, controls
migration, TiltUI state/CSS, effect caps and cleanup, all-corner viewmodel checks
at FOV 45/90/120 and narrow/wide aspects, wind bounds and geometry budgets.

The separate headless server build and smoke start with DISPLAY/WAYLAND_DISPLAY
unset passed. No mandatory rendering, UI or audio dependency was added to it.

Native Vulkan/RX 6950 XT captures were inspected for grass, flowers, bushes,
Survival/Creative inventory, cracks, unavailable/valid placement and the hand at
45-degree FOV in a 960x640 window. Both building fixtures use real UDP Integrated
sessions, natural terrain and actual input. No fixture preloads inventory or
writes terrain deltas. A foreground plant is genuinely mined, followed by a
ground block. Survival verifies cancellation, pickup, inventory selection and
replacement with the collected block. Creative verifies the absence of timed
mining and resource consumption. Actual icon bitmaps must load before captures.
Both final logs are free of ERROR/WARN. These are automated native input tests,
not a claim of manual mouse testing or a second-machine multiplayer session.

The final Survival run confirmed its 0.55-second grass block in 571 ms; the final
Creative run confirmed its block in 54 ms. These are local UDP smoke
observations, not latency guarantees. Both observed six break particles. Native
grass/bush frozen-camera images one second apart show small motion; central-crop
mean absolute pixel differences were 0.00212049 and 0.000896821 respectively.
Shadows also animate, so image differences alone are not a geometric proof;
shader execution, stable picking IDs and CPU bound tests provide additional checks.

The old-binary botanical images in `before/` are geometry references only:
its older UI catalog did not know the new inventory component. They are not a
clean old-UI acceptance test. The six before benchmark logs contain no such
error and completed before that UI change.

## Measurements

Six before/six after runs used the same benchmark script, 18-second samples,
seed 42042, camera anchors, 1920x1200, VSync off, FOV 90, radius 16/3 and job
budgets. No compilation or second Terra client ran during measurements.
Configuration snapshots and full timing/process logs are included.

| View | FPS Before / After | p95 ms Before / After | p99 ms Before / After |
|---|---:|---:|---:|
| Forest 1 | 109.405 / 109.526 | 10.623 / 10.375 | 14.672 / 14.515 |
| Forest 2 | 108.267 / 108.533 | 11.068 / 11.014 | 14.665 / 14.702 |
| Offline | 141.323 / 141.642 | 8.875 / 8.555 | 10.082 / 9.798 |
| Integrated | 141.266 / 141.558 | 8.816 / 8.729 | 9.898 / 9.801 |
| Fixed river 1, shared-load caveat | 142.595 / 142.248 | 8.658 / 8.856 | 11.020 / 10.763 |
| Fixed river 2, shared-load caveat | 142.892 / 121.089 | 8.749 / 10.779 | 11.036 / 13.097 |
| Additional river repeat, shared load | - / 68.539 | - / 18.931 | - / 26.327 |

Forest mean is 108.836 -> 109.029 FPS. Forest and spawn show no meaningful
additional average-frame regression in these short samples, not a proven speedup.
The earlier foliage regression documented in phase 20 remains open.

The river slowdown was not discarded or silently averaged away. Another 3D
application started at **19:11:01 UTC / 21:11:01 local**, during the end of the
first river sample and before the second. It remained active during a diagnostic
repeat; `after/background-process.txt` records its start and >100% CPU use.
Individual GPU pass timings changed less than CPU wall-frame time. This is a
confounding workload, not proof of the entire slowdown's cause. These river
runs cannot establish performance parity or an implementation regression.
A fresh isolated before/after river comparison remains open. No unrelated
process was terminated or reconfigured to obtain favorable measurements.

| First Run | Initial Load s Before / After | Process RSS KiB Before / After | Prop Worker s Before / After |
|---|---:|---:|---:|
| Forest | 3.361 / 3.288 | 855392 / 851004 | 0.252 / 0.265 |
| Offline | 3.223 / 3.175 | 810316 / 827604 | 0.250 / 0.250 |
| Integrated | 3.156 / 3.218 | 844056 / 840476 | 0.235 / 0.266 |
| Fixed river, shared load | 3.176 / 3.004 | 798180 / 818708 | 0.196 / 0.190 |

Loaded sections remain 7704 / 7623 / 7635, represented props 89866 / 86727 /
63169, and prop batches 1277 / 1248 / 1231 for forest / spawn / river.
Mesh assets increase by exactly two shared feedback cubes: 9313 -> 9315,
9316 -> 9318, 9085 -> 9087. Counts are not exact draw calls. First forest summed
terrain-composition worker time is 0.849 -> 0.755 s; prop worker time rises slightly
as wind weights/bounds are prepared. Worker seconds are not wall latency.
First forest main opaque GPU pass is 1.966 -> 1.962 ms; this is not total GPU
frame time. Full pass data and peak RSS/user/system CPU times are in raw logs.
The separate 4375-section CPU-generation example was not rerun; generation itself
was not changed in this step.

## Compatibility And Limits

Protocol 8 requires updating client and server together. World format 1,
generator 5, seeds, world/block IDs and dependency versions remain unchanged.
Existing gameplay-less saves remain readable. Drop state is capped at 2048
objects; exhaustion rejects further resource-producing mining rather than losing
items. Each client receives at most 256 nearby markers within 48 m.

Drops have visual bob/rotation, not rigid-body gravity or TTL. Held/drop cubes
are representative top-textured markers, not exact multi-face/custom models.
Crafting, slot rearrangement and planting collected props are separate work.
Destroying support terrain retires its plants without extra loot; only the
targeted object is harvested. Remote clients see edits and drops, but another
player's hand/crack progression is not animated yet. Extremely close ceilings
can still depth-occlude the first-person hand. Long resource/streaming stress
tests, isolated river remeasurement and final art-direction approval remain open.

## Reproduction

```bash
TERRA_CAPTURE_DIR=/tmp/survival target/debug/terra-client --smoke-test-building
TERRA_BUILD_CREATIVE=1 TERRA_CAPTURE_DIR=/tmp/creative target/debug/terra-client --smoke-test-building
TERRA_SCREENSHOT=/tmp/botanical.png target/debug/terra-client --smoke-test-botanical
TERRA_RIVER_BASELINE_VIEW=1 bash tools/benchmarks/run-hydrology-suite.sh /tmp/benchmarks
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --bind 127.0.0.1:0 --no-save --smoke-test
```
