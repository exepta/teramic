# Phase 97: Placement, Shadows, Horses, Botany And Calendar HUD

## Changes

- Terrain upload records the last composed local block revision, including empty
  sections. Confirmed building changes publish directly instead of re-entering
  the generated-chunk fade queue. Neighbor invalidations and first loads of
  saved constructions still retain their initial reveal. Revision records are
  evicted with unloaded sections. Replacing an active entrance also finishes its
  temporary materials/draw-readiness entries before retiring owned meshes.
- Directional shadows follow loaded view range, capped at 160/256/512 m for
  Low/Medium/High. Medium uses three cascades and High four; the near bound and
  map resolution stay unchanged. Far terrain remains a caster. Glass, water and
  torches retain their existing no-shadow rules. More distant casters and an
  extra cascade cost GPU work; this is not a free performance improvement.
- The horse barrel and leg spacing are 30% narrower, with corresponding authored
  width and mob JSON bounds. Head, height, length, coat identities and shared
  eight-part models are retained. Herd separation no longer projects a jumping
  horse onto the floor. A jump finishes before bedtime can ground the animal.
  Ballistic integration now takes place within collision substeps instead of
  subtracting a whole 10-Hz tick of gravity before any movement.
- Grass uses upright square shoots with stepped lateral leaves and real gaps.
  Bushes use connected cubic leaf cushions and optional berry/blossom details.
  Flowers use thicker square stems, attached leaves, cupped/radial petals and
  tiered purple bells. Mushrooms have broad stepped caps, thick stems, gills,
  spots and a smaller companion. Existing species/variant color assignments,
  spawn density, prop IDs, removal persistence and shared render/picking geometry
  are retained. Wind still uses the existing GPU path, not per-plant updates.
- TiltUI gains a shared calendar-ring material with four seasonal quarters,
  tick marks and a year-progress needle. Flower/sun/leaf/snowflake icons use the
  actual TiltUI catalog. Spring is west, summer north, autumn east, winter south.
  The saved authoritative SeasonSample determines progress, including custom
  season durations/start season. Disabling seasons removes the ring.
- Survival diving uses oxygen rather than stamina. The same movement path is
  used by prediction and authority: underwater stamina is full and exhaustion
  cannot block a dive. Surface swimming still consumes energy. The existing
  stamina rail switches to blue air and a breath icon, remains during refill,
  briefly shows full air, then returns to stamina. No second meter/resized HUD.

## Budgets And Compatibility

No new crates, generator versions, block IDs, wire fields or save formats.
The additional effort flag is not serialized. Dedicated server rendering
dependencies are unchanged. All ordinary props still use section/material
batches and shared bounded templates, not an entity per stalk/leaf/petal.

Near vertex ceilings: grass 476, ordinary bush 456, flower 480, mushroom 384.
Grass Middle/Far remain 160/32; flowers remain 144/48. Ordinary bush Middle
changes 144 -> 168, Far stays 96. Mushroom Middle/Far are 144/72. These geometry
counts are not FPS, GPU-memory or long-term performance-parity measurements.

## Validation

All commands completed with exit code 0, with `CARGO_BUILD_JOBS=1` for Cargo:

| Command | Evidence |
| --- | --- |
| `cargo fmt --all --check` | [fmt-final.log](fmt-final.log) (empty on success) |
| `cargo check --workspace` | [check-verified.log](check-verified.log) |
| `cargo test --workspace` | [tests-verified.log](tests-verified.log): 1656 passed, 2 existing ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | [clippy-verified.log](clippy-verified.log) |
| `cargo build --workspace` | [build-verified.log](build-verified.log), client and server |

Focused regressions include local placement versus first-load neighbor revisions,
first placement in an empty section, Far terrain/glass/torch shadow rules,
range/quality cascade bounds, all 16 horse coats and the complete authoritative
obstacle jump with ecology/separation enabled, blocky prop mesh/picking budgets,
one shared calendar material across recreated TiltUI nodes, underwater effort
and air consumption, surface energy use, air refill and stable HUD dimensions.

The native calendar/HUD fixture starts a fresh integrated Survival authority,
with a bounded saved fluid pool and real season/teleport commands. It tests
diving, stamina, air use/refill and the shared meter through replicated stats.
Shared survival rules and authoritative horse movement also have regression
tests. The building fixture uses real input, UDP authority and replicated world
state and checks its current visible mesh.

Native checks ran on RX 6950 XT / RADV / Vulkan, Ryzen 9 5900X, 2880 x 1620.
They used isolated copied configuration directories and temporary fixture saves,
not edits to a user world. Final logs contain no ERROR or WARN entries.

| Short native check | Result and evidence |
| --- | --- |
| `--smoke-test-botanical` | Blocky grass/flowers/bushes/mushrooms, wind frames and nonblank capture; [log](botany.log) |
| `--smoke-test-creatures` with `TERRA_HORSE_INSPECT=1` | 16 coat/model gallery with idle/walk/sleep poses; [log](horses.log). Autonomous jumping is checked by the authoritative CPU test, not this pose gallery. |
| `--smoke-test-forest` | View 12, High shadows, distant loaded terrain; [log](shadows.log) |
| `--smoke-test-building` with `TERRA_BUILD_CREATIVE=1` | Real break/catalog pickup/place flow, current visible section mesh, final capture without debug placement overlay; [log](building-verified.log) |
| `--smoke-test-calendar-hud` | Four real season commands, actual diving/surfacing, blue air in the existing rail, full stamina underwater, stable geometry; [log](calendar-verified.log) |
| `terra-server --smoke-test` without DISPLAY/WAYLAND_DISPLAY | Headless startup and 27 generated sections; [log](headless.log) |

The calendar shader was compiled natively, including correcting WGSL's reserved
`active` identifier and using the real TiltUI `flower` icon. The calendar fixture
waits for background inventory-preview readbacks before exiting; the final run
does not retain the screenshot-channel shutdown error from an earlier attempt.
The building harness follows the current single-item Creative catalog contract.

## Native Images

- [Grass](botany-grass.png), [bushes](botany-bush.png),
  [flowers](botany-flowers-detail.png), [mushrooms](botany-mushrooms.png).
- [Horse gallery](horses-1.png), [distant shadows](shadows.png),
  [confirmed placement](building-restored.png).
- [Spring](calendar-hud-0.png), [summer / submerged air](calendar-hud-1.png),
  [autumn / surface stamina](calendar-hud-2.png), [winter](calendar-hud-3.png).

Images were inspected for blocky forms, actual mesh visibility, shadow presence,
calendar icon/label placement and the single shared air/stamina rail. They are
not a claim that every reference detail is identical or all seeds are covered.

## Performance And Remaining Limits

The shadow check includes one **after-only**, three-second CPU-wall sample:
seed 42042, view 12, High shadows, clear weather, 2880 x 1620, 266 frames,
88.32 FPS, p95 12.959 ms, p99 13.229 ms, RSS 1,031,408 KiB.
It loaded 4426 sections, rendered 2097, held 7473 mesh assets and represented
59,318 props in 724 batches. All 833 admitted chunk entrances completed.
This single short sample has no matched baseline and is not a performance
improvement, GPU-time comparison, memory-parity or streaming-stability claim.

Only short native checks are intended for this phase, as requested. No new
10/15/30-minute soak or invented before/after FPS comparison. Native views can
show art direction but cannot certify an exact 1:1 copy of the references.
Dense foliage, farther shadow casters and new prop Middle-LOD shapes still need
broader GPU/art approval. Existing performance backlog is not marked complete.
Horse riding/taming/breeding are unchanged and remain separate work.
