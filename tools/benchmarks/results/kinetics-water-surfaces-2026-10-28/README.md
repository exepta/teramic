# Kinetic Connections Water Power And Surface Colors

## Behavior

Small gears (141-143), shafts (144-146), Water Wheel (147) and Conveyor (148)
keep their IDs. Wooden, Stone and Iron Large Gear append IDs 149-151, with
workbench recipes in the existing TMI registry. The client and server use
protocol 79 together. World generation and existing world IDs are unchanged.

Parallel small/large gears mesh diagonally in their rotation plane with a 2:1
ratio. Perpendicular large gears mesh across a diagonal spanning both axes.
Axial shaft connections retain physical rotation, and ordinary small gears
reverse direction at adjacent radial contacts. These connection principles
follow the [Create rotation implementation](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/src/main/java/com/simibubi/create/content/kinetics/RotationPropagator.java).
Teramic retains its bounded authoritative graph, safe overload stops and
existing machine inventories rather than importing a second engine.

RPM is now floating point, retaining 15, 7.5 and 3.75 RPM through reductions.
The maximum is 256 RPM. Loads are compared as power: faster outputs consume
proportionally more capacity; reduction trades speed for torque without
creating energy. Large gears have three load points versus one for small
gears. Contradictory ratios, sources or overspeed stop the component.
Large gear torque ratings are 32/96/256 for wood/stone/iron, twice the small
gear ratings. Their doubled radius and reduced RPM do not create more power.
The existing safety budgets remain: 1024 loaded nodes per tick, 256 per
component, and at most 128 active conveyor stacks processed per tick. Visual
replication includes at most 256 nearby parts within 64 m. Unloaded assemblies
do not produce offline; an incomplete component safely stops. This implements
the requested connection behavior, not every machine or mechanic of Create.

Placement shares one neighbor/face-aware axis selector between preview and
authority. Shafts reach both axial cell boundaries. Small gears have a 0.5 m
pitch radius and large gears a 1 m pitch radius; their teeth extend slightly
beyond that pitch. Picking, collision, placement clearance and water clipping
include indexed neighboring overhangs, including negative section borders.
Moving rotor bounds use the real geometry rather than a one-cell placeholder.
Ordinary authoritative block placements cannot bypass that clearance when a
stale request misses its exact placement plan. Crank frames, moving parts,
placement ghosts and collision/picking share the same coaxial mounting offset
on gears/shafts, preserving the older two-cell crusher attachment.

The old inventory gear images omitted the rotor and displayed only identical
bearings. Previews now include the complete moving mesh and its own wood,
stone or iron PBR material, with framing for larger parts. The bounded shared
moving library contains 31 meshes, not one entity per tooth or paddle.

## Water Wheel And Current

The wheel measures **1 m axially and 2.5 m across both radial axes**, with a
segmented round rim and paddles. New placement keeps its axle horizontal.
Sixteen loaded-liquid contacts sample the actual rim. No liquid means no
drive; a still inland lake has no drive either.
These are sixteen discrete contact samples, not a continuous hydrodynamic
solver. Sources nominally turn at 30 RPM; the factors below increase supported
load, not speed. Ocean transport is sampled only near the marine surface.

| Contact | Calm strength | Full storm strength |
|---|---:|---:|
| Flowing current | 1.0 | 1.2 |
| Falling water | 1.2 | 1.35 |
| Marine wave transport | 1.1 | 1.4 |

Storm strength blends continuously with the existing authoritative wind. Source
strength is independent of RPM and stored in hundredths for the graph solve.
Opposing paddle forces cancel. Marine power requires a real prepared ocean
column and near-surface loaded water, not merely a lake at sea level. The wave
transport field uses world position, authoritative time and wind, independent
of camera or client graphics settings. It does not create fluid cells.

Natural river and underground-river currents have 20% higher physical response;
ordinary canal current speed is unchanged. Inland surfaces retain subtle
wave displacement and normal ripples while marine surfaces keep full swell
and storm whitecaps. Actor swimming wakes remain independent of that calming.

Running nearby gears and shafts join the existing bounded stereo mechanical
mix. Contact cadence and pitch respond to RPM; reversing does not silence
them. Stopped, overloaded, distant or unknown parts contribute no gear sound.
Audio uses the existing mute/volume controls and a single shared source with
at most twelve Foley voices, not a separate emitter for every tooth.

## Map And Materials

Surface cartography samples the real authored grass top, applies its painted
palette and then relative biome tint. It no longer multiplies the atlas mean
by a second dark absolute tint. Cave coloring retains its prior path.

New map cache records retain exact samples plus a stable palette fingerprint.
Palette changes recolor reconstructible distant tiles, and stale worker output
remains distinguishable. Cache/job limits are unchanged. **Old RGBA-only cells
cannot be reconstructed exactly and keep their old color until revisited.**
They are not deleted; explored areas and separate waypoint files are preserved.
Legacy climate subtraction can produce tiny negative endpoint roundoff. The
writer canonicalizes only finite endpoint residues within 64 * f64::EPSILON;
the reader still requires strict [0, 1] samples. NaN, infinity and genuine
out-of-range weights remain errors, now with tile/cell/field diagnostics.

Grass, foliage, stems and green crops use a coordinated brighter green family
and the same relative biome factor as the authored ground. Flower petals,
dry plants, branches and ripe straw retain their own colors.

Sand now uses a deterministic 512px golden grain texture with six sparse gray
pebbles. Those pebble positions also receive real, shallow, batched geometry.
The ground remains closed and inside its cell, with a planar 4 cm perimeter
before its shallow inset; the full detail face budget is 148 vertices.
The fine grain stays in the texture rather than creating one
cube per grain. This is a procedural interpretation of the provided sand
reference, not a pixel-identical copy.

## Validation

Short native fixtures and final command logs are retained with this report.
No 10/15/30-minute performance soak is part of this change, and it does not
claim to solve the previously documented overall FPS-drop problem.

```bash
cargo fmt --all --check
cargo check --locked --workspace --all-targets -j1
cargo test --locked --workspace -j1 -- --test-threads=2
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

The regression suite covers connection ratios, fractional RPM, load/torque,
cycle conflicts, adaptive placement, overhang clipping, water/current source
rules, map cache compatibility/recoloring, complete gear previews, bounded
sand geometry and plant palette consistency.

The kinetic render gallery uses the real core graph solve for its connected
gear train, including 7.5/3.75 RPM and a perpendicular pair, but injects its
source. It is a rendering/asset-lifetime check, not a native fluid-drive test.
Authoritative wet/dry/current/fall/storm behavior is covered separately by the
network/world regression tests. Audio signal tests do not replace a human
headphone/speaker listening review.

### Completed Command Checks

- [Full workspace suite](logs/teramic150-tests.log): **2,859 passed, 7 existing
  ignored**, no failures.
- After the final map-cache and fixture corrections, the
  [client/storage suite](logs/teramic150-targeted-tests.log) passed again:
  **243 passed, 1 existing ignored**, no failures.
- [Workspace formatting](logs/teramic150-fmt.log) and
  [all-targets check](logs/teramic150-check.log) passed.
- Latest [strict workspace/all-targets Clippy](logs/teramic150-clippy.log)
  passed with `-D warnings`.
- Latest [workspace build](logs/teramic150-build.log) passed, including the
  client and headless dedicated server.
- The subsequent Clippy-only edits collapse an equivalent condition, name an
  existing cache tuple and iterate the same three test coordinates. They do
  not change the tested behavior.

### Completed Native Checks

All three fixtures ran separately on the actual RX 6950 XT using Vulkan/RADV,
Mesa 26.2.5-arch3.1 and X11. No Cargo jobs ran concurrently with these checks.
The build was the existing optimized development profile, not a release FPS
benchmark. Initial gallery resolution was 1280 x 800; the map fixture also
tested its 1280 x 900 and 960 x 720 layouts. The integrated map world used seed
42042, generator 5, landscape 1, geology 4 and ecology 2, at view distance 12.

| Fixture | Result | Evidence |
|---|---|---|
| Materials | Passed; 151 block definitions and all GPU previews ready, terrain/foliage/water rendered | [Log](logs/teramic150-materials.log) |
| Kinetics | Passed; forward/reverse/stopped trains, perpendicular gears, fractional RPM, six distinct gear icon readbacks | [Log](logs/teramic150-kinetics.log) |
| MiniMap/FullMap | Passed all 52 stages, including caves, cache writes, pan/zoom, peers/mobs, settings, waypoint persistence and real held-input flight | [Log](logs/teramic150-minimap.log) |

The kinetic fixture retained 31 shared moving meshes and unchanged asset
counts during the stopped stage (731 meshes, 326 materials). This is a short
lifecycle check, not a general long-term resource-growth guarantee. The map
flight covered 142.56 m with a 29.70 m altitude change and three in-flight
readbacks. Final native logs contain no warnings or errors.

Reproduction entry points (use isolated config/world directories):

```bash
TERRA_SCREENSHOT=/tmp/teramic-materials/materials.png target/debug/terra-client --smoke-test-materials
TERRA_CAPTURE_DIR=/tmp/teramic-kinetics target/debug/terra-client --smoke-test-kinetics
TERRA_SCREENSHOT_DIR=/tmp/teramic-map target/debug/terra-client --smoke-test-minimap
```

On this Linux session, runs also used `WGPU_BACKEND=vulkan`,
`WINIT_X11_SCALE_FACTOR=1` and unset `WAYLAND_DISPLAY` / `WAYLAND_SOCKET`.
`TERRA_CONFIG_DIR` and `TERRA_WORLD_DIR` selected disposable test directories.

### Selected Captures

- [Terrain, grass and sand](captures/terrain-palette-sand.png)
- [Coordinated foliage](captures/foliage-palette.png)
- [Solid water shader](captures/solid-water.png)
- [Connected gears and rounded wheel](captures/kinetics-forward.png),
  [reverse](captures/kinetics-reverse.png), [stopped](captures/kinetics-stopped.png)
- Small gear icons: [wood](captures/kinetic-icon-141.png),
  [stone](captures/kinetic-icon-142.png), [iron](captures/kinetic-icon-143.png)
- Large gear icons: [wood](captures/kinetic-icon-149.png),
  [stone](captures/kinetic-icon-150.png), [iron](captures/kinetic-icon-151.png)
- [Brighter MiniMap](captures/map-surface-minimap.png),
  [surface FullMap](captures/map-surface-fullmap.png),
  [cave FullMap](captures/map-cave-fullmap.png),
  [flight](captures/map-flight.png), [live miniature](captures/map-live-miniature.png)

Terrain, foliage, both surface maps, cave map, connected machinery and the gear
icon captures were also visually inspected. The sand reference is an art
direction target, not a promise of exact image duplication.

### Native Test Corrections

The first kinetic icon check used `Screenshot::image` on dormant inventory
targets. Bevy redirects camera targets for that API, so this test replaced the
already rendered icon with an unwritten capture texture. The fixture now uses
`Readback::texture`, copies padded GPU rows without modifying the source and
saves only the copy. Production previews were not changed for this correction.

The first map run reached surface/cave/full-map checks through stage 22 and
captured the brighter grass. Its flight stage then waited for terrain readiness
before releasing synthetic flight input; continuing to fly kept invalidating
that readiness. Only this test-stage gate was corrected. That run also exposed
the real cache-writer endpoint-roundoff error described above. The original
failed logs are retained separately rather than counted as passed checks.

Original diagnostic runs:
[initial icon capture](logs/teramic150-kinetics-initial-screenshot.log),
[initial map timeout](logs/teramic150-minimap-initial-timeout.log).

## Main Code Paths

- `crates/terra-core/src/kinetic.rs`: shared connection graph, axes, ratios,
  power/torque and crank mounting.
- `crates/terra-network/src/server_kinetics.rs`: bounded authoritative drives
  and actual water-wheel contacts.
- `crates/terra-world/src/water/`: fluid space, motion and source classification.
- `crates/terra-world/src/streaming/water_jobs.rs`: indexed overhang clipping
  with loaded assembly ownership.
- `crates/terra-render/src/crushers.rs`, block models and previews: shared
  connected moving geometry and complete inventory icons.
- `apps/client/src/crusher_audio.rs` and `crusher_audio/source.rs`: bounded
  RPM-driven mechanical Foley.
- `crates/terra-core/src/map_palette.rs`, map raster/cache paths and
  `crates/terra-storage/src/maps.rs`: authored colors and persistent samples.
- `crates/terra-render/src/blocks/sand.rs`, appearance and prop/crop palettes:
  golden sand detail and coordinated greens.
- `assets/shaders/terra_water.wesl`: calm inland surface versus marine swell.
