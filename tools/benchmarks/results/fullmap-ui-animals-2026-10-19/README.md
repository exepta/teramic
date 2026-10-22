# Phase 70: FullMap, Map Cadence, Animal Support And HUD Frames

Implementation and final validation are recorded separately below. Native
captures were inspected on KDE Wayland, Vulkan, AMD Radeon RX 6950 XT,
Mesa 26.2.4. No controlled before/after FPS improvement or memory parity is
inferred from these functional tests.

## Implemented Behavior

`M` opens the TiltUI FullMap and `N` collapses/expands the MiniMap. Controls remain
rebindable and old default MiniMap bindings migrate without replacing custom
bindings. The FullMap shares the existing exploration atlas and supports drag
panning, cursor-relative wheel zoom, centering, surface/cave layers and unknown
or still-loading areas. It releases the mouse and blocks gameplay input.

Named, colored waypoints can be created, edited and deleted after confirmation.
They are stored per world/player/dimension beside the map tiles in
`configs/maps/{world_id}-{seed}-v{generator}/dimensions/overworld/{uuid}/waypoints.toml`.
The validated book is limited to 256 entries, reads are size-limited and failed
writes preserve the previously applied in-memory annotations. Pins do not imply
server teleport permissions or a new world-save backend.

Terrain rasterization runs asynchronously, at most five times per second.
An affine transform reprojects the cached terrain through Bevy's existing UI
material support on TiltUI image elements. A transparent overlay updates live
markers, waypoints and the local direction arrow at the configured 24/30/40/60 Hz
(default 30). The MiniMap popup adds this cadence and a 1-2.5x animal-icon slider.
The latter leaves remote players and the own direction arrow unchanged.
The bottom MiniMap control strip is removed; the remaining readouts are shorter
and centered.

Land-animal support checks reject water-only floors and deep air gaps between
feet. Mammals float with their lower body immersed and share current/bank-exit
handling. Close-range flight rules and resting gaze are separated so ordinary
nearby players do not make every animal continually turn away. Bear rendering
and dimensions increase together by 35 percent.

Shared transparent stone/gold/emerald frames cover menus, inventories, storage,
recipe dialogs, item browser, FullMap and hotbar through one TiltUI component
and Bevy nine-slice image rendering. F1, target info and chat are excluded.
The stats container has no background or outer border. A larger heart and red
health bar sit above a shorter yellow stamina bar and the hunger card. Existing
oxygen remains conditional. Hunger art uses the five existing atlas cells:
full at >=90%, first bite at >=75%, half at >=50%, last piece above 10% and bare
bone at <=10%; the 30% example therefore retains the last piece.

## Work And Memory Bounds

- Existing 16x16-metre color tiles and live height/material samples are reused.
  No extra world camera or world-generation pass is introduced.
- At most 4096 full color tiles and 256 resident detail tiles. A bounded set of
  65536 shared 4x4-RGBA overview tiles retains distant explored colors; coarse
  pixels represent four metres and do not preserve full height detail.
- MiniMap uses 320x320 and FullMap 640x640 terrain/overlay images. Each pair has
  about 800 KiB or 3.125 MiB of RGBA payload, respectively, before driver and
  asset bookkeeping. CPU/GPU ownership and old queued uploads can add overhead.
- Terrain jobs borrow immutable visible snapshots; camera interpolation and
  icon movement do not rerun full terrain rasterization at 60 Hz.
- Marker glyphs use 13x13 stack buffers and nearest scaling. Circle clipping is
  limited to glyph pixels instead of clearing the entire image a second time.
- The frame texture is shared. Nine-slicing can produce multiple UI slices;
  this is not a claim of zero additional draw work.

These are source-level budgets, not controlled runtime measurements. Long
streaming, dense mobs, two clients, multiple GPUs and upload costs remain part
of the performance backlog.

## Final Validation

All Cargo commands used the unchanged workspace dependencies; compilation was
limited to two jobs to reduce link-time memory pressure.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo test --workspace` | 1065 passed, one existing UDP soak ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo build --workspace` | Passed |
| Headless dedicated server | Passed with DISPLAY and WAYLAND_DISPLAY unset, 27 sections |
| Native FullMap/MiniMap | 22 stages plus scaled waypoint editor passed |
| Native Gameplay options | Real widgets, Apply/Reset/Back and TOML reload passed |
| Native survival HUD | Food, stamina, death/respawn, layout drag/save/reload and game-mode roundtrip passed |
| Native inventory frames | Layout-only captures at 1280x800 and 3840x2160 passed |
| Bear model inspection | Five rest/hunt/attack/sleep/climb poses captured; model-only fixture |
| Controlled FPS/p95/p99 and memory comparison | Not measured for this update |

Logs are archived in [logs](logs/), selected inspected frames in
[captures](captures/). The native MiniMap fixture used real integrated authority
and a second real headless network client. It tested every FPS preset, 2.5x
icons, loaded cave teleport, M/Escape, drag/wheel/button zoom, centering and
waypoint create/edit/confirmed delete with TOML reload. The marker checks saw
50 real mobs and one remote player in the final run; this is not a population
or two-renderer performance benchmark.

Native inspection found and corrected early UI-material plugin registration,
an unsupported CSS shorthand, unsupported icon names, coordinate-input
overflow, over-thick compact-panel frames and an obscured health label. The
material now registers after the asset/render plugins through their finish
phase. Regression tests cover that ordering and actual TiltUI CSS parsing.
One Wayland cursor-warp error remains in the synthetic map-pointer fixture;
all pan/wheel/control assertions passed. No shader or stylesheet errors occurred
in the final run. Production pointer movement does not request those test warps.

The separate 37-test creature suite covers all four mammals entering the exact
water surface, immersed stability, bank exit, partial-fill water seams at
positive/negative section boundaries, falling after water drainage and land
movement resuming after section reload. Enlarged-bear body obstruction and
calm-player-facing versus fleeing regressions are included. The native bear
fixture checks presentation only, not navigation on real banks.

UI tests also cover frame-rate phase without drift at a simulated 100-Hz game
loop, affine reprojection under fractional/negative coordinates, rotation,
zoom/height offsets, circular versus rectangular clipping, selective mob-icon
scaling, waypoint validation/persistence/isolation, binding migration and exact
hunger thresholds. Native food frames show 18/36/54/72/90 percent; they do not
separately exercise the exact 75-percent boundary, which is unit-tested.

Reproduction uses isolated temporary configs and smoke storage, never normal
player worlds:

```bash
TERRA_CONFIG_DIR=/tmp/terra70-configs TERRA_SCREENSHOT_DIR=/tmp/terra70-minimap-final target/debug/terra-client --smoke-test-minimap
TERRA_CONFIG_DIR=/tmp/terra70-configs TERRA_MENU_SMOKE_DIR=/tmp/terra70-menu-runtime-final TERRA_MENU_SCREENSHOTS=/tmp/terra70-menu-final/captures target/debug/terra-client --smoke-test-menu-gameplay --smoke-test-menu-small
TERRA_CONFIG_DIR=/tmp/terra70-configs TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra70-survival-world-final TERRA_CAPTURE_DIR=/tmp/terra70-survival-final target/debug/terra-client --smoke-test-survival
TERRA_CONFIG_DIR=/tmp/terra70-configs TERRA_INVENTORY_LAYOUT_ONLY=1 TERRA_INVENTORY_4K=1 target/debug/terra-client --smoke-test-inventory
env -u DISPLAY -u WAYLAND_DISPLAY TERRA_CONFIG_DIR=/tmp/terra70-configs target/debug/terra-server --smoke-test
```

## Boundaries

The cave map is still a local height-band view over loaded cells, not a global
connected-cave solver. Distant saved colors and coarse overviews do not provide
persisted detailed relief. Props smaller than a block are not individually
modeled in every map pixel. Waypoints are private client annotations; shared
server maps and new dimensions are not implemented. No block/world IDs, seeds,
generator version or dependency versions are changed by the map/UI work.

Mammal collisions remain conservative voxel body bounds rather than individual
limb rigid bodies. Unloaded terrain deliberately remains a barrier, shore
climbing handles one-block steps, and the decorative SolidWater block remains
solid. Mutual creature rigid-body collisions and a global navmesh are not
introduced by the support fixes.

## Frame Asset

Production bitmap: `src-ui/assets/stone-gold-frame.png`, generated with built-in
imagegen, transparent background, then inspected and copied into the workspace.
The center alpha was checked as zero. The prompt and provenance are recorded in
`src-ui/assets/stone-gold-frame.txt`. Existing `survival-atlas.png` supplies the
heart, stamina, oxygen and five food states; no status-system artwork substitutes
for absent gameplay mechanics.
