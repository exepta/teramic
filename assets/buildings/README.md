# Editable construction templates

The server discovers real JSON templates recursively under `buildings/`
inside `terra_core::asset_directory()`. `TERRA_ASSET_DIR` selects a deployment's
asset root. The validated catalog loads at server bootstrap. A kingdom manager
can explicitly reload it with the building catalog's refresh action after saving
editor output or repairing files; the server rate limits reloads to one per two
seconds. HUD inspection reads only the cached snapshot. Reload validates the
complete directory atomically; partial or malformed files produce a visible
catalog error until a successful reload. Invalid files are never silently
replaced by a procedural fallback. Clients select catalog IDs and cannot submit
filesystem paths, voxel plans or material prices.

The shipped residential choices are:

| ID | Display name | Dimensions X/Y/Z | Atomic steps / cells | Material kinds |
| --- | --- | --- | --- | --- |
| `village-house-temperate-v1` | Temperate village house | 9 / 11 / 11 | 390 / 392 | 17 |
| `spruce-cottage-v1` | Spruce cottage | 9 / 11 / 11 | 388 / 390 | 16 |

The default remains the furnished temperate village house. The cottage has an
open timber porch with solid spruce canopy columns and a slab bench instead of side guard rails. It uses oak plank infill and spruce beams,
floors, roof, openings and bed, and has green rugs. Its geometry and material
requirements differ from the village house. Neither template grants free chest
contents, tools, population or building resources.

Catalog summaries contain the display name, residential category, dimensions,
step count and exact **permanent** material totals from each JSON. These counts
are aggregate quantities and can exceed one inventory stack. A queued job also
includes its real, paid site scaffolds and any additional supports; its material
list counts the inputs for the remaining steps, including materials already
in chests or craftsmen's bags. Dismantled scaffold planks enter the craftsmen's
bags and are physically returned to storage. Natural excavation does
not create free building materials.

The version 1 schema is implemented by
`terra_core::building_jobs::template::BuildingTemplate`:

| Field | Meaning |
| --- | --- |
| `schema_version` | Currently `1`; unsupported versions fail validation |
| `id` | Unique safe ID, 1–64 lowercase ASCII letters/digits/hyphens/underscores, starting with a letter |
| `display_name` | Trimmed display text, at most 80 UTF-8 bytes and no control characters |
| `category` | Currently `Residential` |
| `anchor` | Local foot coordinate corresponding to the player's selected house origin |
| `bounds.min`, `bounds.max` | Inclusive local bounds containing the anchor and every cell |
| `steps[].phase` | `Foundation`, `Floor`, `Walls`, `Roof`, or `Furnishing` |
| `steps[].cells[]` | Relative position, material ID, facing/state byte, orientation behavior and paired-object state |
| `steps[].costs[]` | Real input items consumed by this atomic step |
| `materials[]` | Exact aggregate of all step inputs, including both combined-slab halves and flower-pot contents |

Legacy version 1 JSON without metadata defaults to the residential category and
`Temperate village house` display name. New templates should set both fields.
The directory catalog is deterministic by ID and bounded to 32 templates,
8 MiB total, 256 directory entries and four nested directory levels. Each JSON
is limited to 2 MiB and 8,192 steps, with at most 64 material kinds. Symlinked
entries, duplicate IDs and malformed files fail validation.

The shipped anchor is `[0,0,0]`; the normal ground click places it one block above
the selected support. `rotate_facing` distinguishes cardinal orientation from
fixed state bytes such as lantern mounts and pot contents. All 24 stair
orientations and vertical/combined slabs rotate through the real construction
state APIs when enabled. Double or mixed slabs cost both material halves.
Paired doors/beds have reciprocal `peer` positions, one primary/one secondary
half, and one real object item in their shared step. Paired chests use lateral
neighbors matching their facing and cost two chest items; their real inventories
remain separate physical halves of one logical container. Invalid or duplicate cells, unmatched
pairs, unsupported poses, free placements and inconsistent material totals are
rejected.

These choices use the current residential construction site envelope: local X
`-4..4`, Y `-2..9`, Z `-6..4` relative to the anchor. The world planner reserves
the surrounding working ring and access ramp. This is not yet a planner for
arbitrary larger buildings or arbitrary interior layouts. Walls/roof cells below
local Y zero are rejected. The builder's interior temporary stair and working
deck are reserved; non-furnishing permanent cells cannot overlap them. Each
shipped house is tested through actual movement, all four orientations, roof construction,
furnishing and scaffold recovery with exact material conservation.

Manual placement resolves the selected template. The residential area's saved
selection controls future automatic houses. Missing selections never silently
change to another building. Each existing job persists its complete resolved
steps and costs, so editing or removing an asset cannot rewrite or invalidate
that already planned job. Frozen ID24/ID25 decoders preserve existing plans,
progress, selections and inventories; ID24 defaults only its missing selection.
The next save writes ID26 with the authoritative Creative funding state and
funding breakpoint. Old jobs start with paid funding until the server grants a
Creative owner's policy. Paid scaffolds still refund their real materials when
that policy starts midway through a project; freely funded scaffolds do not
mint inventory items. Losing the owner's Creative policy pauses free jobs until
it is restored. New free jobs need no resource chest or material transport;
Survival jobs retain actual costs and transport.


Directory labels become the server summary's `category_path`, using validated
relative slash segments (at most four, 128 UTF-8 bytes total). The first legacy
`templates/` segment is hidden, so `buildings/templates/village/` still appears as
`village`. New editor output may use `buildings/Kingdom/Wohnhäuser/` directly.
These labels come from filesystem traversal, not a field supplied in template
JSON or a filesystem path accepted from a client.

Town resource snapshots count only the assigned area's loaded, authorized real
chest inventories. Both halves of a double chest must be in the selected area;
its items count once and its logical chest count is one. Sampling is bounded and
cached for at most one second per viewer; `complete=false` marks unloaded or
budget-limited inventory rather than presenting a partial total as exact.
Preview affordability uses exact item counts; HUD wood/stone/iron/coal are grouped
physical units, with raw ore and refined iron counted together in the iron group.
Creative infinity is a separate server-authorized policy flag, never a fabricated
warehouse count.

To deliberately re-export the unchanged procedural village reference:

```sh
cargo run --locked -p terra-world --features bevy --example export_house_template
```

This authoring command writes the actual default JSON file; it has no game
window and is not used during ordinary job execution. An optional output path
can be passed after `--`. The separate visual JSON building editor remains TODO
**F82**.
