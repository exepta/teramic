# Item Reference Pass

## Implemented

- The existing procedural item pipeline now builds rounded voxel solids with shallow face relief and deterministic color variation. Interior voxel faces are culled before mesh upload. A 32-unit design grid is sampled onto a 16-cell visual lattice; these cells are geometry, not gameplay blocks or ECS entities.
- Raw/cooked beef, pork and mutton retain separate shapes and palettes. Meat has fat rims, marbling, tapered edges and grill marks; mutton retains its protruding bone. Leather has folded edges and wool has uneven tufts with cut-back corners.
- Fish have volumetric bodies, visible eyes, fins, tails, pale bellies and distinct raw/cooked/clownfish coloration. They are not flat item cards.
- Honey has an octagonal belly, narrowing shoulder and neck, wooden lid, binding details and a separate transparent glass shell. The shell uses ordinary alpha blending, not a new screen-space refraction pass.
- The eight reference tools have shaped heads, wood grain, grip wraps, metal fasteners and rivets. The existing iron pickaxe and stick remain supported by the same generator.
- Inventory previews, equipped items and world drops use the same mesh generators. The existing held/drop cache reuses mesh handles per item kind. No individual GLB, bitmap icon, material or entity is created for each small voxel.

## Native Validation

`--smoke-test-items` renders the actual item geometry in an isolated native studio view. All five images from the 17:19 UTC run were inspected. After fitting the tilted tool heads uniformly within their unit bounds, the final run completed on 2026-10-18 at 17:35 UTC, with five refreshed nonblank 1920 x 1080 captures and no GPU validation error. The final swords/axe/shovel close-up was inspected again; mesh counts are unchanged:

- [Overview](items/items-0.png): all 21 reference items are present and framed. Wool has a broken tufted outline, and raw/cooked variants are distinguishable.
- [Meat and honey](items/items-1.png): the meat edges taper, mutton's bone is attached, and the honey container now has a visibly separate belly, shoulder, neck and lid.
- [Fish variants](items/items-2.png): both fish types and their cooked variants have visible body depth and fins.
- [Pickaxes, wood axe and shovel](items/items-3.png): complete grips, fasteners and head silhouettes are visible.
- [Swords, stone axe and shovel](items/items-4.png): all four handles, including their lower bindings, are intact and inside the viewport. The final image was rechecked individually at original resolution; an earlier grouped-image suspicion of missing lower handles was not confirmed.

The first fixture attempt resized the window after renderer startup while using MSAA 4 on a second HDR camera. At 150% desktop scaling this produced mismatched depth/color attachment sizes. The fixture now inherits the startup resolution and uses MSAA Off. Its lights use the existing `PreviewScene` exclusion so graphics settings cannot turn their shadows back on. A soft studio-only fill light makes the side faces inspectable; world lighting and ordinary inventory-preview lighting were not changed.

These are native geometry captures, not generated mockups. They do not independently validate every equipped-item pose, world-drop collision or lighting situation.

## Measured Geometry

Counts below come from the [final native fixture log](items/native.log), not estimates. Vertices include face-normal/color discontinuities and relief edges. Each opaque item is one batch; honey has one opaque and one transparent batch.

| Item | Vertices | Triangles | Batches |
| --- | ---: | ---: | ---: |
| Cow Beef | 5,072 | 2,536 | 1 |
| Cooked Cow Beef | 5,072 | 2,536 | 1 |
| Pig Beef | 5,232 | 2,616 | 1 |
| Cooked Pig Beef | 5,232 | 2,616 | 1 |
| Leather | 4,848 | 2,424 | 1 |
| Wool | 13,896 | 6,948 | 1 |
| Sheep Beef | 4,448 | 2,224 | 1 |
| Cooked Sheep Beef | 4,448 | 2,224 | 1 |
| Honey | 8,008 | 4,004 | 2 |
| Raw Fish | 5,632 | 2,816 | 1 |
| Cooked Fish | 5,632 | 2,816 | 1 |
| Raw Clownfish | 5,768 | 2,884 | 1 |
| Cooked Clownfish | 5,768 | 2,884 | 1 |
| Wood Pickaxe | 5,056 | 2,528 | 1 |
| Stone Pickaxe | 5,248 | 2,624 | 1 |
| Wood Axe | 5,232 | 2,616 | 1 |
| Wood Shovel | 3,768 | 1,884 | 1 |
| Wood Sword | 4,120 | 2,060 | 1 |
| Stone Axe | 6,152 | 3,076 | 1 |
| Stone Shovel | 3,768 | 1,884 | 1 |
| Stone Sword | 4,120 | 2,060 | 1 |
| **All 21 studio models** | **116,520** | **58,260** | **22** |

These models have more vertices than the earlier simple cuboid products/tools. Mesh generation is cached rather than repeated every frame. This table is not a draw-call measurement, GPU-memory measurement, FPS benchmark or a claim of performance parity. The studio displays all item types simultaneously; normal gameplay retains only the cached kinds actually used. Winding, valid indices, finite bounds, attribute lengths and per-model vertex/batch limits have focused tests; workspace results are recorded in the main report.

## Fidelity Limits

This is a bounded 3D reconstruction of the supplied reference, not a pixel-for-pixel recreation. The 16-cell lattice gives coarser contours than the illustration. Fine leather wrinkles, irregular meat marbling, highly polished food surfaces, wispy cooked-food steam and illustrated honey highlights are not reproduced exactly. Honey has a real transparent outer shell but an opaque colored liquid core and no optical liquid absorption/refraction simulation. The reference's warm rim lighting and painted shading remain different from gameplay lighting.

Further artistic refinement can adjust these existing models without introducing another rendering or item registry. The native captures establish visible geometry and resolved fixture rendering; they do not constitute final reference-quality sign-off.
