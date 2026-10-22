# Phase 69: Spatial MiniMap And Gameplay Settings

The circular TiltUI MiniMap now samples live loaded blocks, including canopies,
constructions and confirmed water changes. Its material colors come from actual
top-face materials and texture means captured by the renderer before CPU pixels
are released. Turf and water use the renderer's existing biome palettes. Short
height strips and directional shading give the map its 2.5D relief.

Animal portraits and remote-player diamonds use existing replicated, interpolated
states. Dead animals and bees inside nests are excluded. A vertical loaded-cell
visibility check prevents markers leaking through opaque roofs between storeys.
Leaves and water do not suppress visible animals. A cyan direction arrow marks
the server-confirmed local position. Compass labels follow the same projection.

Automatic cave mode checks the player's depth below natural terrain and an
opaque overhead roof. Cave tiles trace continuous open columns near the current
four-metre height band. Unknown cells and solid walls remain dark. Surface and
cave views can also be selected explicitly.

## Settings

`Options > Gameplay` contains HUD Layout and MiniMap customization. The latter
opens an existing TiltUI modal with size, default zoom, automatic/surface/cave
view, player rotation, relief, animal/player markers, coordinates, biome and time.
Changes are staged until Options Apply, with Reset, discard and display rollback.
Preferences are stored in `configs/gameplay.toml`. Existing HUD positions remain
compatible in `graphics.toml`, while their command moves into Gameplay.

## Work And Memory Bounds

- Up to four changed tile snapshots per asynchronous job; no world regeneration
  and no extra rendering camera.
- One loaded-section revision scan per scheduling tick. Mesh LOD and animation
  do not invalidate cartography.
- Maximum 4096 cached color entries and 256 separately allocated detail tiles.
  Empty entries hold pointers, not inline eleven-KiB arrays. Color payload is
  at most 4 MiB; detail payload is approximately 3 MiB plus allocator/bookkeeping.
- One 320x320 RGBA map texture (400 KiB pixel payload). Rasterization/uploads
  occur at most five times per second and only when the relevant state changes.
- Collapsed maps explore at most once per second. Exploration stays within
  96 m of the authoritative player; zoom spans are 96/192/384/768 m.
- Texture means sample at most 256 alpha-weighted base-level texels per texture
  once, storing only three linear color components.
- Palette revisions reject stale worker results; world and cave-layer keys remain
  separate. Existing checksummed map tile files stay compatible.

These are implementation budgets. No controlled FPS parity comparison or
multiple-GPU performance claim is made for this update.

## Validation

Commands use `CARGO_BUILD_JOBS=2` to limit simultaneous large link jobs:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace

TERRA_CONFIG_DIR=/tmp/terra69-configs TERRA_SCREENSHOT_DIR=/tmp/terra69-map \
  target/debug/terra-client --smoke-test-minimap
TERRA_CONFIG_DIR=/tmp/terra69-configs TERRA_MENU_SMOKE_DIR=/tmp/terra69-menu-gameplay \
  TERRA_MENU_SCREENSHOTS=/tmp/terra69-menu/captures \
  target/debug/terra-client --smoke-test-menu-gameplay --smoke-test-menu-small
env -u DISPLAY -u WAYLAND_DISPLAY TERRA_CONFIG_DIR=/tmp/terra69-configs \
  target/debug/terra-server --smoke-test
```

The map smoke starts a real integrated server and a second headless network
client with its own UUID. It checks live surface pixels, zoom/collapse, UI scale,
real animal and remote-player markers, then teleports through the production
chat command to a roofed cave found in loaded terrain and checks automatic cave
selection. It does not inject fake replica tracks. The menu smoke dispatches
actual TiltUI handlers and checks staged changes, Apply/TOML reload, Reset, Back
and popup/footer bounds at 960x640. These event-driven tests are not OS-pointer
interaction tests.

The first full menu run stopped in the older world-mode dropdown assertion. Its
global label query included hidden unrelated labels; the check now scopes to the
actual ChoiceBox popup and allows bounded layout completion. The new isolated
Gameplay flow also exposed a test sequence that tried clicking the underlying
Apply button before popup closure had been processed; the test now waits for
the real page transition. Neither failure is counted as a passed native run.

Native machine: KDE Wayland, RX 6950 XT, RADV Mesa 26.2.4. Captures were inspected
for the actual map, cave silhouette, compass frame and small-window popup layout.

Final results:

- Format, workspace check/build and all-target Clippy with denied warnings passed.
- Workspace tests: 1043 passed, zero failed, one existing UDP soak ignored.
- Native map smoke: six captures passed, including 50 real animal markers and one
  remote player in the wide surface view. The final cave view uses the 96 m zoom.
- Native Gameplay smoke at 960x640 passed: real widgets, draft, Apply, TOML reload,
  Reset, Back and popup/footer bounds.
- The full small-window menu smoke also passed after the scoped dropdown check.
- Dedicated server starts without DISPLAY/WAYLAND_DISPLAY and generates 27 sections.

Selected captures: [surface](map/map-0.png), [cave](map/map-5.png),
[Gameplay](menu/gameplay-options.png), [MiniMap settings](menu/minimap-settings.png).

## Boundaries

Saved remote tiles retain their colors, but height/material samples are currently
resident data rebuilt from loaded sections. Fine tree/prop appearance, full-map
waypoints, extensive cave exploration and long travel/IO stress remain follow-ups.
The cave view is a local vertical slice, not a global connected-cave solver.
No world-generation, seed, block ID, protocol or dependency-version change.

## Frame Asset

The new project asset is `src-ui/assets/map-compass-ring.png`, generated with the
built-in imagegen tool and transparent alpha. It contains only the decorative
frame; terrain and markers are computed game data. Generation prompt:

> Production transparent UI frame for TERAMIC: a perfectly circular compass ring,
> square canvas, transparent circular map opening and exterior, dark charcoal
> hammered stone/iron, restrained warm brass/gold beveled trim, four small angular
> compass diamonds with green amber gems, top-down orthographic low-poly fantasy
> art, subtle bevel depth. No map, landscape, background, letters, numbers or labels.

The full exact prompt is preserved in `frame-prompt.txt` next to this report.
