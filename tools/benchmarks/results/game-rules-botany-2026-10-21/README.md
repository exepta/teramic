# Phase 98: Botany Detail, Animal Contacts And World Rules

## Scope

The requested fixes extend existing canonical prop meshes, animal navigation,
construction targeting, crafting, calendar, TiltUI tooltips and minimap.
No dependency upgrades, block-ID changes, generator changes or second settings
backend. Region/player binary formats remain unchanged; protocol is now 55.

## Changes

- Near grass adds small square shoots, lateral leaf segments, bud tips and tiny
  flowers while retaining rooted Middle/Far stalks and open companion corners.
  Bushes add layered small leaves, species tints, paired berries and flowers.
  Flowers use three stems with attached segmented leaves, cupped poppies,
  radial daisies/cornflowers, open-bottom bells and tulip cups. Mushrooms use
  stepped cap segments, gills, pale stems and spots, including red/orange variants.
  CPU picking and rendering still consume the same bounded templates.
- Animal collision uses actual partial-block profiles instead of unit cubes.
  Long bodies use oriented rectangles, SAT contact rejection and rotated hoof
  probes instead of a square with their longest dimension. Water is not a floor
  when just one hoof touches a bank. Falling/jumping horses are not turned into
  grounded resting animals just because the jump timer expired.
  Heading changes are checked against the new oriented volume; a long body
  cannot turn into unknown terrain while resting at a loading edge.
- `/gamerule <rule> <value>` supports `sleep_percentage`,
  `season_swap_interval`, `game_time_speed`, plus `list [page]` (eight per page).
  Chat/console share parsing, validation and completion. Authority checks edits,
  replicates settings and saves them into existing world metadata. Sleep votes
  round up; even 0% needs one mature vote. The day-length multiplier does not
  advance the clock at application. Season-length edits retain the season/hour
  and never rewind the calendar.
- Placement body checks share the proposed half-slab volume. The center 70% of
  side faces selects vertical halves; 15% edges select horizontal halves. Top or
  bottom clicks on an existing vertical slab continue its plane into the next
  cell. Existing mixed-material merging is retained.
- Base beds are white; colored beds remain valid carried/saved items but do not
  duplicate catalog entries. Eighteen separate shapeless JSON recipes dye/re-dye
  the three woods in the ordinary 2x2 or workbench grid. Sleeping poses are raised
  above the quilt instead of intersecting it.
- Seasonal blending starts during the final day, with continuous smoothstep
  weights at the boundary. Natural snow accumulation stops at full snow blocks;
  player snow placement remains independent.
- Tooltips remain TiltUI widgets but are portaled outside scrollable ancestry.
  Owner tracking cleans them up when their originating UI disappears. Pointer
  positioning, viewport clamping, bindings, hover and text wrapping are retained.
- Minimap uses a detailed seasonal iron/gold frame, transparent map opening,
  centered runtime cardinal text and a dynamic calendar needle. Frame/cardinals
  follow map rotation together. Waypoint/label clipping matches the smaller
  ornamental opening.

## Geometry Budgets

Near vertex ceilings: grass 1168, ordinary bush 2112, flowers 1836,
mushrooms 1128. Middle/Far geometry remains inexpensive and bounded. Spawn
density is unchanged; there are no individual entities or draw calls per leaf.
These higher Near counts are not free GPU/meshing work or evidence of FPS parity.

## Validation

All five Cargo commands completed successfully with `CARGO_BUILD_JOBS=1`:

| Command | Evidence |
| --- | --- |
| `cargo fmt --all --check` | [fmt-final.log](fmt-final.log), empty on success |
| `cargo check --workspace` | [check-final.log](check-final.log) |
| `cargo test --workspace` | [tests-final.log](tests-final.log): 1666 passed, two existing ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | [clippy-final.log](clippy-final.log) |
| `cargo build --workspace` | [build-verified.log](build-verified.log), client and dedicated server |

Regressions cover gamerule grammar/completion/limits, real UDP replication to
two clients, console authority without an actor, survival permission denial,
metadata save/reload in both layouts, exact time multipliers and pause,
sleep percentages and mature votes, season continuity, all 18 dye/re-dye
recipes, actual slab placement volume, continuation of vertical slab planes,
mixed-material authority, natural snow caps versus manual stacking, tooltip
ownership/cleanup, LOD dimensions, plant mesh budgets and material identity.
The authoritative horse test checks takeoff, ledge landing and settling onto
a real half slab. Both positive and negative unloaded edges are tested with
horses and other mammals, including rotation, then loading and resumed movement.

### Native Checks

Short native probes ran on RX 6950 XT / RADV / Vulkan and Ryzen 9 5900X.
Botany, bed and calendar captures use 2880 x 1620; actual inventory OS-pointer
tests also resize to 1280 x 800 and 960 x 640. Configuration copies and temporary
fixture worlds isolate these tests from user saves.

| Probe | Result |
| --- | --- |
| `--smoke-test-botanical` | Six natural-world close-ups plus fixed-camera grass/bush wind pairs; [log](botany.log) |
| `--smoke-test-calendar-hud` | Four replicated season commands, actual Survival diving/refill, same air/stamina rail and seasonal frame; [log](calendar.log) |
| `TERRA_FURNITURE_MINING_SMOKE=bed --smoke-test-building` | White paired bed, Survival dye consumption, lying avatar above the quilt, next-day 08:00 wake via real authority; [log](bed.log) |
| `TERRA_NATIVE_OS_POINTER=1 --smoke-test-inventory` | Creative pickup/split/drag, crafting drag/sweep, conservation, tooltips, trash, resize, scrolling and HUD overlays; [log](inventory.log) |
| Same inventory probe with `TERRA_INVENTORY_MODE=survival` | Survival browser tooltip, unclipped popup, resize/scrolling, pause/options over retained HUD; [log](inventory-survival.log) |
| `terra-server --smoke-test --no-save`, DISPLAY/WAYLAND_DISPLAY unset | Headless startup and 27 generated sections; [log](headless.log) |

Images inspected for actual geometry, open gaps, wind displacement, quilt
intersection, tooltip placement, cardinal alignment and frame/map composition:

- [Grass](native/botany-grass.png), [grass wind](native/botany-grass-wind.png),
  [flowers](native/botany-flowers-detail.png), [bush](native/botany-bush.png),
  [bush wind](native/botany-bush-wind.png), [mushrooms](native/botany-mushrooms.png).
- [Spring frame](native/calendar/calendar-hud-0.png),
  [winter frame](native/calendar/calendar-hud-3.png).
- [White bed](native/bed/bed-intact.png),
  [sleeping pose](native/bed/bed-sleeping-pose.png).
- [Creative tooltip](native/inventory/inventory-hotbar-tooltip.png),
  [Survival tooltip](native/inventory-survival/inventory-survival-browser-tooltip.png),
  [crafting drag preview](native/inventory/crafting-held-drag-preview.png).

### Preliminary Findings

The first full Creative pointer run stopped because a previously dropped stack
was picked up again between source-count and click acknowledgements. The probe
now validates the real conserved total, uses the acknowledged cursor amount
and parks an odd remainder through actual right-click input before testing an
even distribution. It does not grant replacement items or change pickup rules.
The final complete Creative run passes; the earlier result remains recorded
in [inventory-first.log](inventory-first.log).

The Survival fixture previously submitted two window captures in one frame;
the duplicate request was removed. One pre-Welcome `CreatureStates` warning
still occurred in the final Survival run. The same warning exists in the
[Phase-95 inventory log](../construction-materials-horses-2026-10-21/inventory-native.log).
It is not claimed fixed here; no accumulation or continuing warning stream was
observed. A quick render probe also exposed AppExit before background preview
readbacks finished; its shutdown now waits for them without extending the FPS
sampling interval.

### Remaining Limits

Only short native probes were run, no new 10/15/30-minute soak. There is no
controlled before/after performance or general GPU/memory parity claim.
Additional Near geometry has real cost. Dense scenes, further GPUs, rotating
map/DPI combinations and exact artistic reference matching need broader review.
The native plant images are a concrete implementation, not an assertion of
an exact 1:1 copy of lighting, materials or every reference silhouette.
Horse riding/taming/breeding and horizontal sleep capsules are unchanged.
World metadata and region writes use the existing atomic file operations,
not a new cross-file crash transaction.

### Short Render Measurement

One after-only probe used `--smoke-test-forest` (the existing fixture camera
views a mushroom-park hillside), seed 42042, view 12, High shadows/water/block
detail, LOD factor 1.6, no rain, 2880 x 1620, VSync/FPS cap off. This is the
offline render fixture, not an integrated-server/full-HUD benchmark. No water
tiles were visible. GPU pass timing and draw-call instrumentation were off.

| CPU-wall sample | Measured |
| --- | --- |
| Window / frames | 3 seconds / 376 frames |
| Average FPS | 125.35 |
| p95 / p99 | 9.447 / 10.427 ms |
| Reported RSS | 1,069,928 KiB |
| Loaded / rendered sections | 4426 / 2097 |
| Reported mesh assets | 7219 |
| Represented props / batches | 59,318 / 724 |
| Total prop worker CPU time | 0.442 seconds |

[Complete log](performance.log), [native image](native/performance.png).
The three-second sampling boundary is unchanged while shutdown waits for
background preview readbacks. Resource counters are reported at the final
summary after those readbacks drain. The earlier run's shutdown-channel error
remains visible in [performance-first.log](performance-first.log); it is absent
from the repeated final run. These values are not an old/new FPS comparison,
GPU time, peak VRAM, steady-state UI cost or long-term stability proof.

## Frame Asset

`src-ui/assets/season-compass-frame.png` was generated using the built-in
image-generation tool, with the user's minimap reference for art direction.
Both the exterior and the round center have verified zero alpha. Text, map,
markers and calendar progress are rendered at runtime, not baked screenshots.

Final prompt: create one centered transparent fantasy minimap frame, viewed
straight-on; black iron and fine warm-gold bevels; blank jewel-tipped cardinal
diamonds; pink spring blossoms left, lush green leaves and yellow/white flowers
top, orange/red autumn leaves right, blue snow/fir decorations bottom. Keep a
truly transparent circular opening and exterior. No terrain, characters, text,
numbers, coordinate bars or pointer. Crisp stylized voxel-fantasy ornament,
balanced geometry with varied seasonal details. Opening requested at 74% width;
actual generated artwork was measured and runtime clipping fitted to it.

Exact reference matching is assessed from actual native captures, not claimed
from generation or geometry counts alone.
