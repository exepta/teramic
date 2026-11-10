# Taller inventories, complete icon preload and shallow turnouts — 2026-11-10

Inventories using the shared item browser have 20% taller logical frames:
player 366 → 439.2 px, stations 400 → 480 px. Inventory/catalog cells retain
33.6 px logical squares (result cells 38.4 px). Responsive fitting reserves
space for the title and HUD; text and icons are not enlarged with the frame.

The normal frontend again retains **all 617 finite inventory previews before
the main menu**, together with the existing model, texture, font, UI and audio
preload. Sixteen spatially isolated render slots replace serial captures.
Admission is bounded by eight items and 3 ms per frame. Every image still passes
the existing GPU alpha-coverage check; blank images retry during shader startup.
Scenes, cameras and the shared light are released after completion. Dynamically
requested variants still use the existing serial queue after startup.

New left/right turnouts use a 20 m S-shaped branch with 4 m lateral displacement
and a maximum heading deviation of **about 17.1°**, below 20°. Both exits have
cardinal, parallel tangents, permitting ordinary straight continuations without
module ownership overlap. The main line and branch retain independent routing.
Canonical profiles are shared by collision, rendering, replication and motion.
Switching preserves the complete geometry marker and owned footprint.

**Existing saved 90° turnouts keep their old ports.** Rebuild them with sufficient
space and adapt their approach/exit tracks to use the shallow profile. No silent
change moves existing connected modules or edits a user's saved world.
Client and server require protocol **127**; persistent schemas are unchanged.

Rail construction rays and individual connection curves now allow **64 m**.
Adaptive geometry permits 513 samples while retaining quarter-metre subdivisions;
metric analysis retains its established 257 samples. Free ends within 1.5 m of
the actual aim can be snapped even when the ray hits the adjacent floor. Both
preview and authority reconstruct the same plan, including ownership checks
for the complete destination module. Real occlusion and unloaded cells still
block placement.

## Verification

**4512 tests passed, 10 ignored and one filtered** across 12 workspace suites.
The single filter excludes the separately deleted original FIVERR GLB fixture;
that external deletion is preserved. Strict all-target Clippy, locked build,
formatting, whitespace and the CPU train-asset lint pass. Native GLTF/profile
provenance was regenerated from production Core sources and its contract passes.

Two native galleries pass: 26 views at 1680×936/1024×640, and 26 at
3840×2160/1024×640. They check painted slot/picking bounds, title containment,
composite positioning, no HUD overlap, no content scrollbars and all twelve
station layouts. The 4K gallery uses the complete parallel icon preload.
The harness waits for the painted viewport after resize and removes desktop
window decorations for a true 3840×2160 render target. Gallery logs retain two
transient Bevy zero-size-text warnings while switching projected controls;
visible text geometry and content checks pass.

## Native startup comparison

Same executable and isolated, identical 1920×1080 windowed configs, Vulkan on
RX 6950 XT, vsync off, no FPS cap, warm OS/GPU caches:

| Complete preload | Icons published before menu | Whole smoke-process duration |
| --- | ---: | ---: |
| Serial diagnostic override | 617 | 19.148 s |
| Normal bounded parallel rendering | 617 | 9.032 s |

Both runs pass the native startup/fade/main-menu assertions with **1642 UI
assets and 1611 UI icon images**, no pending preview jobs and no world/server
creation. The observed process duration decreases by about 53%; these single
warm-cache runs include captures and shutdown and are not cold-start guarantees.
A preliminary first parallel run took 16.493 s, including a much slower initial
GPU initialization; a subsequent parallel run took 9.080 s. No icons are omitted
to achieve the reduction.

[4K player inventory](player-4k.png), [compact chest](chest-compact.png),
[linked workbench](workbench-4k.png), [startup fade](startup-fade.png).

## Regression coverage

Physics regressions advance fueled locomotives with 8 m and 15 m wheelbases
for 720 ticks through both routes of left and right shallow turnouts, checking
actual rail cursors and swept collision each tick. Other regressions cover
switch placement/toggle/replication, exact 64 m and rejected 65 m rays/curves,
real long-distance client preview/signature reconstruction, and snapping beside
an existing module without bypassing its building permissions.

Native frontend checks use disposable configs/world directories. They do not
modify the user's original Test world. The inventory gallery projects validated
container snapshots; it checks real layout/picking bounds but is not a
multiplayer container-transaction test. The train regression uses the real motion
and collision implementation with a controlled track fixture; it is not a
manual driving or long-duration streaming test.

```sh
cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --jobs 2
cargo fmt --all -- --check
python3 tools/assets/lint_train_parts.py
git diff --check
# Native startup: also set TERRA_MENU_SMOKE_DIR and TERRA_MENU_SCREENSHOTS;
# supply separate world/config directories and complete copied configs.
# Set TERRA_PREVIEW_SERIAL=1 only for the full-catalog serial comparison.
TERRA_STARTUP_ONLY=1 target/debug/terra-client --smoke-test-menu
# Native gallery: also set TERRA_CONFIG_DIR, TERRA_MENU_CONFIG_DIR,
# TERRA_WORLD_DIR and TERRA_CAPTURE_DIR to disposable directories.
TERRA_INVENTORY_REFERENCE_SMOKE=1 TERRA_SURFACES_SMOKE=1 TERRA_LAYOUT_ONLY_SMOKE=1 target/debug/terra-client --smoke-test-inventory
# For the 4K/full-preload gallery add TERRA_INVENTORY_4K_SMOKE=1 TERRA_PREVIEW_WARMUP=1.
```
