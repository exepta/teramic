# Train turntable, model widths and workshop UI — 2026-11-09

Phase 220 implements the eight train repair, placement performance, turntable,
model width, catalogue and UI requests. Client/server protocol is 122; train
save index remains ID16. Both executables must be rebuilt together.

## Behavior

- Restoring track beneath an existing train rebinds its cursor at its current
  position, including replacement modules with different anchors. Repair checks
  actual vehicle shapes rather than rejecting an entire occupied voxel.
- Unchanged rail completeness checks are cached per journal/epoch. Terrain
  collider activation is limited to two completed results per frame.
- The new craftable turntable uses one item and a 25 m track. Empty platforms
  and stopped, braked, uncoupled trains turn by quarter turns from the workshop.
  Authority, loaded terrain, permissions, people, vehicles and swept clearance
  are checked before either track or vehicle changes.
- Templates explicitly select 1× or 3× body width, retaining the shared gauge.
  Wide boilers, chimney/dome attachment and complete assembly/dismantling are
  covered end to end. Automatic building validates the wide boiler overhang
  before spending inventory or changing the world.
- The Trains catalogue includes track, ballast, parts and their wood variants.
- The reference workshop layout has a real model render, compact cargo/fuel
  panels, standard inventory slot sizing and bounded controls. The lower
  Close/ESC action has been removed; E/ESC and the top-right X close the UI.

## Verification

The full workspace library/binary suite passed: **4,474 passed, 0 failed,
10 ignored**, across 12 suites. `tests.log` records the individual tests.
The additional focused assembly test also passed (`assembly-integration.log`).
After the final cargo heading correction, all 569 UI library tests passed again
(`ui-final-tests.log`).

Native workshop captures use actual rendering, layout and DOM geometry with
an explicit synthetic UI-only vehicle. Native fuel/storage pointer tests use
actual picking and controlled authoritative acknowledgements. These checks
must not be confused with a manual drive in the reported user world. Real UDP
authority, repair, assembly and turntable behavior are covered by the workspace
suite. Native building measurements use an isolated test world and the real
integrated authoritative server; they do not establish a performance bound for
every existing save or chunk-streaming workload.

The workspace build, strict Clippy across all targets, formatting and whitespace
checks passed. Workshop and driving views were captured at 1920×1080 and
1280×720; the runtime checked named controls against the painted frame bounds
and verified that the model image had a real render target. Fuel/storage tests
confirmed four actual native pointer transactions and their inventory results.

The isolated native creative building test passed real mining, inventory
selection, authoritative placement, visible world replacement and render
particles. In its Place/Placed stages, 139 sampled frames had a 6.84 ms median,
9.98 ms nearest-rank P95 and 48.44 ms maximum; none reached 100 or 400 ms.
`placement-metrics.json` documents the scope and calculation. This is one run,
with no before/after comparison in the user's world, and does not establish
that all intermittent streaming stalls are gone.

The 110 changed existing JSON/glTF documents parse, and the generated rail
profile manifest matches the production rail sources (SHA256
`71666c4c7c1195666f68054d39692fcbc7a907ea19b04718fe2a078d596afd9c`).
See `verification.txt` for commands, statuses and artifact checksums.


## Captures

| View | Capture |
| --- | --- |
| Workshop, 1080p | [Native capture](train-workshop-1080.png) |
| Workshop, 720p | [Native capture](train-workshop-720.png) |
| Driving controls, 1080p | [Native capture](train-driving-1080.png) |
| Driving controls, 720p | [Native capture](train-driving-720.png) |
| Fueled engine | [Native capture](engine-filled.png) |
| Loaded storage | [Native capture](storage-filled.png) |
| Confirmed world placement | [Native capture](building-restored.png) |
