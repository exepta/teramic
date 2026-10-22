# Phase 68: MiniMap, Daylight Transitions And Animal Support

## Scope

The MiniMap uses the existing TiltUI component tree, authoritative client pose,
prepared terrain columns and storage crate. No additional render camera, terrain
generator or network protocol was introduced. World, seed, block IDs, generator
version and dependency versions are unchanged.

Map tiles contain natural surface data, not player constructions or dynamic
water edits. The cache is client cartography, separate from authoritative region
files. Its namespace includes world ID, seed, generator, overworld and player UUID.
Files have a fixed binary size, coordinates, a version marker and checksum.

The night transition now compensates sun, moon and ambient intensity for exposure.
This prevents eye adaptation from amplifying the receding sun. The horizon
shader evaluates the twilight/night mix continuously, including when the moon
disc is below the horizon. Existing noon/night endpoints and cave lighting remain.

Land animals previously mixed a four-foot support query with a center-only fall
check. A shared swept support resolver now handles partial ledges and unknown
terrain consistently. Sleeping animals adopt the validated ground height after
edits. Bears preserve downward velocity instead of resetting gravity every tick.

## Bounded Work

- One background map task, at most eight 16x16 tiles per task.
- At most 4096 cached tiles: 4 MiB pixel payload plus map/bookkeeping overhead.
- One 192x192 RGBA map image: 144 KiB CPU pixel payload.
- At most five raster updates/uploads per second, only on changed map content,
  location, direction or zoom. No uploads while the expanded map is hidden.
- Exploration radius 96 m; zoom spans 96/192/384/768 m. Unexplored cells stay dark.
- Animal descent uses bounded 0.125 m steps and a short collision refinement,
  not a global navigation rebuild.

These are implementation budgets, not measured FPS or memory parity. Native
smoke tests and compiler jobs ran during the same development session. There is
no controlled before/after performance pair for this change. Long travel,
multiple-GPU and dense-herd stress tests remain in TODO.md.

## Reproduction

```bash
CARGO_BUILD_JOBS=2 cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --workspace
CARGO_BUILD_JOBS=2 cargo test --workspace
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --workspace

TERRA_CONFIG_DIR=/tmp/terra68-configs TERRA_SCREENSHOT_DIR=/tmp/terra68-minimap \
  target/debug/terra-client --smoke-test-minimap
TERRA_CONFIG_DIR=/tmp/terra68-configs TERRA_CAPTURE_DIR=/tmp/terra68-daylight \
  target/debug/terra-client --smoke-test-daylight
env -u DISPLAY -u WAYLAND_DISPLAY TERRA_CONFIG_DIR=/tmp/terra68-configs \
  target/debug/terra-server --smoke-test
```

Native machine: Linux KDE Wayland, AMD Radeon RX 6950 XT, RADV Mesa 26.2.4.
The map fixture uses a real integrated server. It dispatches actual TiltUI button
events for zoom/collapse and checks the image dimensions, nonblank terrain pixels,
square computed layout and scaled layout. This is not an OS-mouse interaction test.
The daylight fixture captures 25 daylight, twilight, night, underwater and moving
shadow views. Screenshots were also inspected, rather than relying only on exit
codes. A first map run exposed an inherited button size and the Bevy default white
image handle; both were corrected before final capture.

Regression coverage includes negative map coordinates, teleport ranges, atlas
eviction, stable terrain colors, tile persistence/corruption/world-player isolation,
compatible rebinding migration, exposed sunlight continuity, partial animal
support, unknown-section barriers and sleeping support edits.

## Results

- `cargo fmt --all --check`: passed.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: 1032 passed, zero failed, one existing UDP soak ignored.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo build --workspace`: passed, including the dedicated server.
- Headless server smoke without DISPLAY/WAYLAND_DISPLAY: passed, 27 sections.
- Native MiniMap: four captures passed. Computed square image bounds were about
  202x201 logical pixels normally and 193x193 at UI scale 1.5. The backing texture
  remained 192x192; 28,387/7,203 terrain-colored pixels were detected at the two zooms.
- Native daylight: 25 captures passed, no shader errors. Dusk and night views
  were manually inspected; night remains deliberately dark rather than daylight.

Selected captures: [MiniMap](minimap/map-0.png), [scaled MiniMap](minimap/map-3.png),
[dusk](daylight/dusk.png), [night](daylight/night.png).

The attached logs and captures record final validation. General horizon art
direction, unknown-area loading during very long travel and complete animal
navigation quality are not claimed solved by these focused fixes.
