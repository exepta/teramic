# Rail builder, smoke collision and character editor

The locomotive attachment uses block 296 and the existing train cargo, placement,
claim, drop and replication paths. Survival reserves rails and ballast before
committing sparse terrain/rail edits. Creative has no material cost. The work
mode caps speed at 3 m/s and allows one freight wagon no longer than 15 m.
Its facing selects the work direction, including reversed assembly orientations.

## Automated coverage

World tests extend both free ports of all four starter orientations, preserve
canonical owner/wood records, level one-block cuts/fills and reject stale terrain,
large gaps and machinery. Authority tests compare Survival and Creative cargo
accounting and verify that missing material and invalid consists make no changes.

Smoke tests sweep actual puff radii through thin glass, the real chimney lip and
open frame gaps in transformed vehicle coordinates. Vehicle geometry is cached
locally rather than reconstructed for every puff.

Storage uses ID18 with frozen positional ID15–ID17 motion records. Regression
coverage retains old pose, fuel, steam, cargo and names, then round-trips the new
active work mode. Profile tests preserve added colors and cosmetic outfit choices.

Native startup exposed an additional renderer bounds check for the three-metre
scoop; the loader now permits the authored attachment width while retaining the
unit height and bounds of unrelated blocks.

## Scope

The clearer removes small props and bounded natural earthworks. Large trees,
buildings, water, unloaded sections, denied claims and excessive height changes
stop it. It extends level ends and does not invent bridges or new gradients.

Character meshes are authored reconstructions, not imported original assets from
the supplied illustration. The editor adds six sections, live hairstyle cards,
mouse rotation, skin/eye/clothing colors and cosmetic tunic, boots and cape.
Unseen surfaces are authored. Separate clothing material choice remains open.

Local runtime evidence is kept under ignored `target/` directories; private saves
and machine-specific configurations are not committed.

## Native editor and model inspection

The ordinary Vulkan client started with all 296 block templates. The isolated
`character_live` run passed 57 pointer-driven steps: drag rotation, clothes,
color edits, reset/cancel, persistence, per-server overrides and first-join cancel.
Control bounds passed at HD, FHD, QHD and UHD. Real GPU captures were inspected.
A subsequent visual correction fits each hairstyle into its atlas tile instead
of allowing long hair to spill into its neighbor. NPC gallery output was also
rendered and inspected with the finer head geometry.

Evidence: `target/live227/editor.log`, `character-ui-result.json`,
`captures/editor-{hd,fhd,qhd,uhd,reference}.png` and `gallery/npcs.png`.

## Build and regression suite

- Workspace library/binary tests: 4,549 passed, zero failed, ten existing ignored.
  The one filtered test needs the already absent original FIVERR GLB.
- All train assets/recipes passed lint (86 entries, 418 recipe keys).
- Workspace client/server binary build passed.

Logs: `target/phase228-tests-final-pass.log`,
`target/phase228-build-thumbnail-fit.log`, `target/phase228-assets-final.log`.

Final verification also passed strict workspace Clippy, formatting and diff
checks, and the focused regression for shared three-metre world/vehicle collision
shapes. The final native editor repeat passed all 57 steps with fitted thumbnails.
The current Test world loaded successfully using the read-only inspector with
the ID17 migration; no repair/save operation was performed. All 17 original
world/profile/configuration files retained their baseline SHA-256 values.

Further logs: `target/phase228-clippy-complete.log`,
`target/phase228-shape-regression.log`,
`target/phase228-original-world-readonly-pass.log` and
`target/phase228-original-hashes-verified.json`.
