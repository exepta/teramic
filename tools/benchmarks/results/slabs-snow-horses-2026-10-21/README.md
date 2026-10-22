# Phase 96: Slabs, Snow, Horses And Retained HUD

## Scope

Targeted corrections for the supplied horse/block references, stair movement,
slab placement, snow coverage, chunk flicker and foreground UI layering.
No dependency upgrades, generator changes, replacement renderer or new save
system. The user explicitly excluded 10/15/30-minute tests for this phase.
Short functional/native probes do not establish long-term FPS or memory parity.

## Construction And Movement

- Slabs 82..89 can combine all material pairs in six split planes. Same-material
  doubles retain facing 9; mixed doubles use 10..57 in the existing facing byte.
  The original block ID stays unchanged. Survival consumes one extra slab and
  drops both materials. Existing region/object-state layouts are unchanged.
- Side-face bottom/top quarters select horizontal halves; the center selects
  vertical attachment. The authoritative server uses the actual ray hit point.
- The character controller attempts small stair steps before movement is
  substantially blocked. Render-only height smoothing follows the interpolated
  body without moving its collider or changing the authoritative aim ray.
- Horse jumping checks the ascending body arc before takeoff. Failed probes
  and successful jumps have bounded per-animal cooldowns; landing has a recovery
  interval. This prevents blocked animals cycling through Leap/Rest each tick.

## Art And Shader

- Horses share a continuous chamfered barrel, connected coat markings, stepped
  mane, muzzle and hoof details. All 16 coats use the existing eight shared parts
  and one creature material; no per-animal asset creation. Coat uniqueness and
  bounded geometry are covered by regression tests.
- Rock/earth templates have deeper grouped inward relief, while fine sand stays
  fine and snow remains softer. Existing template caching, closed cell shells,
  face culling and section/material batching are retained. The PBR extension
  adds broader irregular mineral seams and restrained rounded earth clods.
- This is an approximation of the reference art, not a certified 1:1 copy.
  Leaf geometry uses the existing Phase-95 species profiles, not a new redesign.
- A newly queued/fading section stays in the initial reveal path when a neighbor
  invalidates its first mesh. Replacements do not spawn a visible intermediate
  revision. No additional generation jobs or reduced quality are introduced.
- The wet/snow shader previously reset alpha to 1 after decoding vertex climate
  metadata, overriding the material fade. Material opacity is now read separately
  using Bevy's actual material bindings before alpha discard. The GPU probe
  `--smoke-test-wet-opacity` checks climate alpha 0/0.5/1 at actual material
  opacity 0/0.25/1 with pixel readback, not only CPU component values.

## Snow And Climate

- New Snow Layer ID 98 supplies states 1..7 with heights 0.125..0.875 m. State 8
  becomes full Snow ID 5. The same bounds drive rendering, picking and collision.
  Mammal ground/clearance probes use the same cap height instead of treating a
  partial layer as a full cube; cows, sheep, pigs and horses have regression
  coverage for all eight heights and rejected penetration.
- The existing bounded authoritative surface worker commits the caps through the
  shared block/object journal. Stable caps do not create new edits or mesh work.
  Weather only changes its own marked caps and never overwrites player blocks.
- Natural unedited canopies, including evergreen trees and branch logs, do not
  exclude snow on the ground. Rain retains its separate canopy roof rules.
  Saved player roofs are checked even when the roof section is not loaded.
- Winter retains snow. Cold winter ground receives a minimum first layer.
  Grass remains grass underneath, and its green rim is whitened. Highland's
  dominant biome converts rainfall/storm precipitation to snow in all seasons.
- Accumulation/melting visits already loaded columns using the existing work
  budget (one column per two seconds, at most 65,536 saved surface conditions).
  This is not global weather history or immediate regeneration of remote snow.
  Full snow remains an ordinary full voxel. Layer-specific shovel yields and
  player-controlled layer stacking are not introduced by this change.

## UI

The TiltUI application shell retains the same game/HUD tree while pause/options
are layered above it. Inventory, chat, target HUD, minimap, labels and hotbar no
longer disappear just because a foreground UI opens. The background hotbar uses
presentation-only `hud-slot` components, avoiding duplicate interactive slot IDs.
Debug stays above the vital HUD but below foreground menus. The native inventory
probe verifies that the same hotbar entity survives pause/options/back/resume.

## Compatibility

Client and server require protocol 54 together because older clients do not
understand mixed facing states or material 98. Existing IDs 1..97, world IDs,
seeds, generator 5, RGA0 region layout and PLY4 player layout are unchanged.
Existing single-material slabs and saved construction remain readable.

## Validation

All five commands pass on the final source:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

1,652 workspace tests passed; two existing tests remain ignored. Final logs:
[tests](tests-final-green.log), [check](check-final.log),
[clippy](clippy-final.log), [build](build.log).
Regressions cover all slab material pairs/split planes, placement hit edges,
real Avian stair traversal, camera smoothing at four render rates, blocked
horse arcs/cooldowns, eight snow heights, winter retention, roofs, Highland
precipitation, region roundtrip, stable snow no-op updates and queued mesh
replacement across successive revisions. Existing mesh-shell, asset-budget,
network, UI, physics and save tests also pass.

Native Vulkan probes used the RX 6950 XT/RADV, seed 42042 and isolated copies
of the [material](materials-config/graphics.toml) and
[visual](visual-config/graphics.toml) configuration. Each final probe took less
than two minutes; none was a 10/15/30-minute soak or FPS comparison.

| Probe | Observed Result | Evidence |
|---|---|---|
| Material gallery | 98 real block previews, construction orientations, six mixed splits and eight snow heights | [log](materials.log), [terrain](materials/materials-reference-terrain.png), [mixed slabs](materials/materials-mixed-slabs.png), [snow](materials/materials-snow-layers.png) |
| Horses | 16 rendered coats; idle, walking and sleeping presentation poses | [log](horses.log), [overview](horses/horses-0.png), [close view](horses/horses-1.png) |
| Seasonal shading | Seven nonblank captures; white ground and grass rims under natural trees, seasonal color and melt | [log](seasons.log), [winter](seasons/season-3-winter.png), [snowfall](seasons/season-4-winter-snow.png) |
| Actual wet-material opacity | Green pixel samples at climate alpha 0/0.5/1: `[0,0,0]` at opacity 0, `[137,137,137]` at 0.25, `[255,255,254]` at 1 | [log](opacity.log), [partial opacity](opacity/wet-opacity-1.png) |
| Fresh chunk/re-entry | First generated section starts invisible, reaches partial opacity and final alignment; FadeSlow/Bounce/Rise/Slide plus terrain prepared in eight directions | [log](animations.log), [fresh partial fade](animations/animation-4.png) |
| Real OS inventory input | Catalog grant, MouseTweaks, crafting drag, outside drop, scroll, trash and server acknowledgements; identical hotbar entities retained through pause/options/resume | [log](inventory.log), [inventory](inventory/inventory-open.png), [pause](inventory/hud-pause-overlay.png), [options](inventory/hud-options-overlay.png) |
| Headless server | Started without DISPLAY/WAYLAND_DISPLAY, bound a real loopback socket and generated 27 sections | [log](headless.log) |

The seasonal fixture deliberately pauses the logical clock to isolate shader
transitions; it is not proof of real-time physical snowfall accumulation. That
path is covered by the authoritative snow/region tests and the eight-state
material gallery. Horse gallery poses are inspection inputs, not an autonomous
navigation soak. The animation fixture observes real generated sections, while
the separate wet-opacity test specifically covers the material extension.

One OS-pointer attempt failed to reach its compositor target; the successful
final repeat is linked above, with the interrupted attempt retained in
[its log](inventory-pointer-interrupted.log). The short animation fixture emits
one screenshot-channel error during shutdown while a preview readback is still
pending; its animation checks and exit status pass. This is not hidden or
reported as a completely error-free run. The final opacity probe waits for
preview completion and exits without that callback error.

No before/after FPS, p95/p99, meshing-time or VRAM comparison is claimed for this
phase. Further GPU/art approval, exact reference shapes and large-scale climate
history remain outside the completed functional checks.
