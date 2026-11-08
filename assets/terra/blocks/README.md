# Terra Block Library

Block JSON files are the single client material registry. They are discovered in
this directory and sorted by stable nonzero `u16` ID; 0 is air. Existing IDs 1
through 26, world IDs and logical one-meter cells retain their meanings.
The dedicated server does not load these rendering assets.

## Textiles, Pots And Connected Windows

White wool retains ID 103. Carpet IDs 106-118 and dyed wool IDs 119-130 use
the thirteen existing dye colors; only white wool/carpet appear in the creative
catalog. Recipes recolor the actual blocks. Carpets use a shared one-eighth-cell
profile, wool the existing cube mesher. Flower pot 131 stores empty, flower or
mushroom contents in the existing object-state byte and composes the canonical
plant meshes into its section batch. No separate entity or material per plant.

Window IDs 68-70 retain their saved states. Matching coplanar neighbors form
bounded 1x2, 2x1 or 2x2 groups with shared frames and outer hinges. Rendering,
picking, collision and atomic opening use the same group geometry. Different
woods, wall directions or saved open states are not silently merged.

The phase-124 report documents recipes, persistence, native galleries and limits:
[Village fields and decoration](../../../tools/benchmarks/results/village-fields-decor-2026-10-25/README.md).

Every shipped block's editable `description` lives in its own JSON definition.
Inventory tooltips read that text directly; changing a description needs no Rust
code changes. Collectible prop descriptions are in individual
`../props/*.json` files, keyed by their existing prop kind. An omitted description
on an older custom block remains compatible and displays no description.

## Kinetic Drives And Conveyors

F25 appends wooden, stone and iron small gears at IDs 141-143, matching shafts at
144-146, Water Wheel at 147 and Conveyor at 148. Large gears use IDs 149-151 in
the same wood, stone and iron order. All eleven parts have workbench
recipes in `assets/crafting` and appear in the existing creative catalog and
Too-Many-Items recipe browser. Crusher 99 and Hand Crank 100 keep their identities
and existing inventories and crushing recipes.

Rotary models share authored rotor meshes and material handles; teeth belong to
the rotor mesh rather than individual entities. Picking, collision and water
occupancy share authored selection volumes, including neighboring-cell and
section-boundary overhangs. Water-wheel player collision is the axle only;
its full rim remains selectable and still determines water-drive contact.
Gear and shaft `facing` values 0-5 select +X, -Z,
-X, +Z, +Y and -Y. Placement first matches the clicked mechanical connection
and usable neighboring ports, then the clicked support face, then the view.
New water wheels use horizontal axles only. Conveyor spans use horizontal
axles and can run horizontally, vertically or at 45 degrees. Clicking a shaft's
side or a machine face snaps to a free compatible
port instead of leaving the new part disconnected. Direct gear meshes keep the
clicked neighbor; ratio and right-angle pairs can snap diagonally. Obstructed
ports and unloaded clearances remain unavailable. Shafts and axial gear
connections retain speed. Adjacent parallel
small gears reverse rotation. A parallel small/large pair meshes diagonally
in the rotation plane: large-to-small doubles speed, small-to-large halves it.
Diagonally adjacent large gears with perpendicular axles transfer rotation
through a right angle at equal speed. Small and large pitch radii are 0.5 and
1.0 m; their eight and sixteen teeth are included in inventory previews.
Small and large gear rims may penetrate their supporting floor slightly;
this does not permit burying their axles or passing through walls.

The removable hand crank still mounts on the crusher drive. It can also attach
directly to either axial face of a shaft or gear, including vertical shafts.
Holding the existing crank interaction supplies 30 RPM. A water wheel can
replace the crank as an automatic crusher drive, directly or through connected
gears and shafts. The wheel is 1 m wide along its axle and 2.5 m in diameter.
Sixteen rim contacts sample loaded river currents, actual falling liquid and
near-surface ocean motion. Dry and unloaded contacts supply no power;
opposing or centered falls cancel. Still lakes supply none, including during
storms. Ocean contacts require a prepared marine column, not merely water at
sea level. Liquid occupies the complement of authored mechanical collision
boxes, using the existing fluid simulation.

Water-drive torque scales independently of RPM. Calm-to-full-storm strength
factors are 1.0 to 1.2 for currents, 1.2 to 1.35 for falls, and 1.1 to 1.4 for
ocean contacts. Weather interpolates between these limits; the wheel's base
torque rating is 32. Stronger water therefore carries more load without
silently speeding up the same gear train.

The server solves signed RPM and load for each connected component at 10 Hz.
Sources supply 30 RPM; ratios preserve fractional speeds such as 7.5 RPM, up to
a 256 RPM limit. Reversing water or a meshing gear reverses signed rotation.
A crank supplies 128 units; a shaft or small gear uses 1, a large gear 2, a crusher
20 and a conveyor span 2 (once, not per intermediate block). Wood, stone and iron
do not impose the old hidden per-part torque ceilings. Power is evaluated as
torque times speed: doubling speed
halves available torque, while a reduction increases torque without generating
power. Source torque is stored in hundredths of a unit. Available source power
and all rotating loads limit the whole network.
Conflicting sources, inconsistent ratio cycles, overspeed or excess load stop
the component with an overload state. Fractional RPM and overload appear in
the target HUD and are replicated within bounded interest when changed;
the current protocol also validates endpoint-based conveyor construction.

Placements and axes persist through the existing section journal, alongside
existing crusher inventory/progress saves. Runtime power is reconstructed from
loaded sections; unloading or removing a source stops its drive, and unloaded
time does not advance machines. A section index excludes unloaded placement
history from network discovery. The simulation admits at most 1024 loaded nodes
per step and 256 nodes per component. Admission has a stable section order;
additional networks remain stopped, and incomplete components fail closed.

Use a Conveyor item on the first shaft and then on a second, parallel shaft.
One connector spans up to 32 intermediate cells. The entire path must be loaded
and clear; endpoints remain real independent shafts. Shift cancels the first
selection. Breaking any segment removes the owned span, returns one connector
and keeps the endpoint shafts. Connected shafts/spans reject wrench rotation;
remove the band before changing their axes. Historic single conveyors remain
valid. Oak is the default; Birch, Spruce and Cherry versions use the existing
wood-variant items, crafting and TMI paths, as do wooden gears and shafts.
Wood selects the casing, not the moving ribbon. The ribbon defaults to gray;
right-click it with a dye to color the complete connected band using one pigment.
Ground-supported horizontal sections have wooden side frames and legs; suspended
sections keep the bare ribbon. Color persists in the existing placement metadata.

Powered conveyors move existing world-drop stacks along the same continuous
surface as their visible band, reversing with signed RPM, while preserving drop
IDs and item counts. Carried stacks are centered, upright and protected from
automatic proximity pickup and ordinary falling physics. Right-click the actual
stack to take it into the player inventory; authority checks reach and visibility.
Horizontal and 45-degree
bands carry items; vertical bands transmit rotation only. The horizontal surface
is 0.625 m high, with axial ports at the shared 0.5 m shaft height. Each 10 Hz step processes at
most 128 active on-belt stacks; unrelated off-belt drops do not consume those
slots. More than 128 active stacks share the budget in round-robin order and
move more slowly. There is no second transport inventory or item registry.
Items resting on a verified carrying surface do not age out, including when a
band stops or its region unloads. Leaving/removing the band resumes ordinary
world-drop physics, pickup and lifetime. Snapshot interpolation follows positions
even when the carried stack's lifetime does not advance.

### Tunnel And Funnel

The existing IDs remain compatible: `teramic::item_hopper` (152) is named Tunnel,
and `teramic::item_importer` (153) is the open Funnel. Both attach
to a chest, furnace or crusher at the band level or one cell above/below it.
Solid obstructions still block a diagonal inventory connection. A visible arrow
shows the item direction; right-click with the Wrench to reverse it without
moving the attachment or its inventory. New chest attachments default to intake;
new machine attachments default to output. Existing low-facing legacy modes keep
their previous behavior.

Output mode takes at most four items every half second from a chest (including
its loaded partner) or only the finished output slot of a machine. Input mode
stores passing stacks in chest slots or valid machine input/fuel slots, never
machine outputs or upgrades. A blocked discharge, stopped output belt, full world
drop budget or unloaded inventory pauses extraction without consuming anything.
Full targets leave unaccepted items on the band; there is no hidden extra storage. Transfers update
inventory and drop state atomically, so an open chest cannot overwrite an
automation change with an old revision. Both ports have workbench recipes/TMI
entries, reusable custom meshes and authored non-cubic collision.

A free Funnel at a band end forwards the existing visible stack through its
clear outlet and lets it fall; it does not create a duplicate item. Suspended
Funnels have no floor supports, in either rendering or collision.

New crushers occupy one cell and keep their input, output and processing state
in the existing machine inventory. Dropped items entering the actual top opening
fill the input when accepted. Incompatible or excess items remain on the model
until explicitly collected, including across unloading. Existing two-cell saved
crushers remain readable and keep their inventories and mounted cranks.

### Water Wheel Setup

1. Place a wheel with its horizontal axle across the current, not along it.
2. Let the lower paddles dip into a flowing river, or direct a waterfall onto
   one side of the rim. Opposing contacts can cancel each other's torque.
3. Attach a shaft directly to the hub and connect it to gears, a crusher or a
   conveyor endpoint. Matching axle directions must meet without a gap.
4. For a band, place the second parallel shaft and connect both with the Conveyor
   item. For output storage, use machine -> Tunnel -> band -> Funnel -> chest.

Still lake water and dry/unloaded wheels do not supply power. The rim and
paddles do not block players; the thin hub shaft does. Selection and water
motion still use the complete 3 m wheel, not the reduced player collider.
The wheel is 1 m wide and rotates at 20 RPM; its increased torque preserves
the previous available power. Twelve scoops visibly hold and drain water only
after contact with actual loaded fluid, without a second fluid simulation.
The target HUD reports network stress/capacity and remaining capacity along
the source path. Rigidly coupled shafts retain the same RPM; gear ratios change
RPM and torque normally, rather than inventing slippage at each shaft.

## Wrench And Reorientation

`teramic::wrench` is a non-stackable tool with the same held-item, pickup and
inventory-preview path as the other tools. Its workbench recipe uses three
iron ingots and one stick; it appears in the Tools category and TMI recipes.
Use the place/use action on a directional block to advance its pose; hold
Shift to step backward. Ordinary terrain, fences and flower-pot contents are
not rotated.

Stairs retain all 24 poses and slabs all six split planes, preserving both
materials of a combined slab. Gears and shafts cycle six axial directions;
new water wheels remain horizontal, while historic unlinked conveyors use four
yaw poses. Connected spans/endpoint shafts must be disconnected before rotation.
Door/window open state, wood species and bed color are preserved. Paired beds,
benches, chests and crushers keep their primary position and rotate their
second part around it; vertical furnace/door pairs stay in their cells. Mounted
cranks follow their owner. Chest contents and machine progress are not mined,
dropped or reset. Occupied beds cannot be rotated.

The authority checks the held wrench, inventory and section revisions, real
target ray, placement clearance and player bounds. Blocked or unloaded
footprints fail without partial edits. The transaction preflights only the
small affected footprint and neighboring sparse section metadata, never a
copy of world history. Existing save IDs and generator versions are unchanged.

## Cube Definitions

### Construction Placement And Surface Detail (Phases 95/96)

Construction IDs 74..97 retain their existing definitions. Stairs 74..81 now
use all 24 proper cube rotations in the existing object-state `facing` field.
The clicked support face selects floor, ceiling or wall placement; the rise
direction follows the player's view so the treads face the approaching player.
Adjacent perpendicular stairs can produce inner/outer corners. The shared
`terra_core::construction::placed_boxes` profile supplies rendering, picking
and physics, including wall moulding corners.

Slabs 82..89 retain legacy lower-half facings 0..3. Facing 4 is the upper half,
5/6 the -X/+X half, 7/8 the -Z/+Z half, and 9 an identical-material combined
pair. Clicking the inner cut face consumes another slab and completes the cell.
Different materials use facing `10 + (second_material - 82) * 6 + split_index`;
the split indices refer to `[0, 4, 5, 6, 7, 8]`. The original material remains
the cell ID, preserving both materials without changing the saved object-state
layout. Survival drops both halves with their respective material IDs.
Side-face clicks in the bottom/top quarter place horizontal halves; the middle
band attaches vertical halves. Top/bottom faces allow ordinary stacking.
Sand fence 95 remains readable for saved worlds but has no
recipe, creative catalog entry or authorized new placement.

Snow Layer appends ID 98. Its boxes definition supplies the default 1/8 m
inventory shape; live rendering, picking and physics use the shared
`construction::snow_bounds` profile. Saved facings 1..7 select 1/8..7/8 m.
State 8 becomes ordinary Snow (ID 5), with facing 8 marking weather ownership.
Weather edits only its own marked caps above a support and never overwrites
player blocks. The existing region journal, replication and surface-condition
storage retain the states. Client and server require protocol 54 together;
IDs 1..97, generator 5 and existing region formats retain their meanings.

The existing terrain PBR extension provides world-anchored stone seams, earth
clods, turf, fine sand ripples and fine snow grain. Analytic bump normals,
roughness and restrained variation augment the existing mipmapped textures;
no logical material or block identity changes. Near relief and leaf volumes
remain section/material batches, not per-detail entities. Contact tints read
a temporary neighbor halo and do not modify saved block data.

`configs/graphics.toml` contains the validated `[blocks]` settings:
`detail_quality`, `micro_geometry`, `micro_geometry_distance`, `material_detail`,
`normal_mapping`, `surface_variation`, and `leaf_geometry_quality`. Quality is
`low`, `medium`, or `high`; default is `high` with a 48 m micro-detail distance.
Apply invalidates a render-only revision using the existing bounded job queues.
Oak, birch and natural spruce crowns share a finite, lazy species/LOD cache and
retain alpha clipping, wind and seasonal shadow behavior. Spruce species uses
the existing tree-appearance metadata, not a replacement block ID.

Architecture, native captures and measured limitations:
[Phase 95](../../../tools/benchmarks/results/construction-materials-horses-2026-10-21/README.md).
Mixed slabs, physical snow layers, stronger relief and opacity regressions:
[Phase 96](../../../tools/benchmarks/results/slabs-snow-horses-2026-10-21/README.md).

### Rooted Segments (Phase 80)

IDs 50-56 append seagrass, salt crystal, dripstone tip, ice crystal, cave vine,
cave root and amber cave mushroom. Their individual JSON definitions use
`"render_kind": "segment"`. These are ordinary one-meter world cells with
procedural sub-cell geometry, not GLBs or per-plant ECS entities. Cube and static
custom-model loading remain unchanged. The segment kind currently accepts only
these native IDs; adding an unrelated custom procedural species requires a real
shape implementation, not just changing a JSON string.

`terra-world::segments` supplies the same canonical triangles to section batches,
accurate picking and the fracture effect. Neighbor profiles keep taper and rooted
water movement continuous across section boundaries. Crystal collision uses four
clipped tapered bounds per cell instead of a full cube. Plant segments do not
block movement. The JSON `collision_boxes` on crystals opt into shaped colliders;
their final dimensions come from the shared live support profile.

Seagrass remains waterlogged and is supported from below; vines and roots hang
from solid ceilings. Mineral chains can attach at either end. Breaking a segment
only removes unsupported continuations, while detached mineral cells use existing
3D world-drop physics. Salt host IDs 30/38 drop nothing; harvested or landed salt
crystals yield the existing salt resource. None of these rules adds another
registry, save format, entity per detail voxel or renderer to the dedicated server.

Cold ocean generation no longer emits ice. ID 49 remains available for explicit
building and existing saved player edits. Highland waterfall-cave crystals use
the separate ID 53; they are not ocean ice floes.

Ordinary full blocks use `"render_kind": "cube"` for their shared inventory and
terrain geometry. Fluid Water (27) is rendered by the existing fluid surface
pipeline. Furniture and torches use static custom models; the native plants and
mineral segments above use procedural segment geometry.
They use the same procedural face generator and existing section mesher. Only
exposed faces are emitted, then triangles are batched by material per section.
No GLB geometry, per-block entity or per-block draw call is needed.

```json
{
  "schema_version": 1,
  "id": 60001,
  "name": "New Cube",
  "render_kind": "cube",
  "bevel_width": 0.06,
  "textures": { "Painted block": "textures/new-block.png" },
  "materials": { "Painted block": {} },
  "face_materials": [
    "Painted block", "Painted block", "Painted block",
    "Painted block", "Painted block", "Painted block"
  ]
}
```

Face order is West (-X), East (+X), Down (-Y), Up (+Y), North (-Z), South (+Z).
`face_materials` explicitly binds the primary material of each face. Grass has a
green top and earth below; snow is a complete white block on all faces. Optional
`side_cap: { "material": "Painted block", "height": 0.1 }` adds a narrow band to
vertical faces. Both the near geometry and coarse geometry retain this binding.
This removes reliance on arbitrary GLB primitive ordering, which previously
selected earth for some coarse grass surfaces.

Cube material definitions accept `base_color` (linear RGBA, default white),
`roughness` (default 0.92), `metallic` (default 0), and `double_sided` (default
false). Texture keys refer to these material names. A material without a texture
binding is a plain color, such as snow. Complete identical descriptors share one
Bevy `StandardMaterial` handle across blocks; grass and earth therefore
share their soil handle. Snow has a separate whole-snow material.
The shared plant palette remains separate.
`coarse_face: { "block": 1, "side": 2 }` instead reuses an authored block's
tiled PBR material, including its exact albedo, normal and ORM image handles.
The source must provide validated coarse faces; side indices use the face order
above. Dirt and its stairs, slab and fence share the grass model's soil face this
way. Their projected tangent frames follow each target cube face's positive-axis
UVs, including near earth relief and placed construction profiles.
`texture_pattern` optionally selects a generated raster texture instead of a
PNG binding. A material cannot specify both sources. `frame_material` names an
additional opaque material for the connected glass perimeter.

The generator rounds the exterior of the cube within [0,1]. Neighbor exposure
flattens its bevel bands to exact cell boundaries, including section edges.
When a vertex touches a culled neighbor interface, every coordinate inside a
bevel boundary band snaps to the canonical cell outline, not only the interface's
normal axis. Normal-only flattening left open wedges at concave three-cell
junctions with different bevel widths or LODs. Free exterior edges retain their
rounded shape. This changes only rendered geometry, not block occupancy.
Collision and editing still use cubic occupancy. There is no global resolution
change and no new voxel storage system.

The ten old Cube GLBs and their JSON `model` references have been removed.
This removes 15,875,208 bytes of redundant authored geometry and embedded images
from the asset directory; runtime Cube loading already made no GLB requests.
Only special authored geometry needs a `.glb` or `.gltf` model. External legacy
Cube definitions containing a `model` field remain readable, but that field is
ignored for Cube rendering. The ten shipped material IDs remain unchanged.

## Textures And Batching

Runtime textures combine shared standalone PNGs for neutral grass, dirt, bark
and canopy with small generated raster patterns: `sand`, `sandstone`,
`stone`, `smooth_stone`, `mossy_stone`, `planks`, `clay`, `mushroom_stem`,
`mushroom_gills`, `birch_bark` and six colored mushroom cap patterns. Patterns are generated
once as deterministic 64x64 RGBA textures, shared across blocks and sections,
and have a complete mip chain. Plank variants share one grain image with separate
linear material tints. Fine sand grain and sediment bands are distinct from the
stone seams; smooth stone has no generated rock relief. No new GLBs or external
texture dependencies are required. Existing PNG sources are retained. Leaves share
canopy-stylized while green, orange autumn and cool green birch use different
linear tints; bark uses wood. Compatible materials reuse handles; the expanded
palette does not imply one draw call per placed block.
Greedy quads use repeating coordinates rather than stretching one image over a
whole merged face. No atlas is introduced, so there are no adjacent atlas tiles
to bleed across UVs or mip levels.

The PNG loader supplies only mip level zero. Startup workers generate the full
chain down to 1x1 once per shared image, using linear-light premultiplied color
averaging, then replace the original image under the same handle. Terrain becomes
ready only after these jobs finish. Four-times anisotropic filtering reduces
oblique surface aliasing. Initial PNGs stay CPU-only until that chain is complete;
publishing the image also invalidates dependent material bindings, because Bevy
does not automatically rebuild a material when its image receives a new GPU view.
Mipmap creation leaves the source PNGs intact; smaller levels add roughly
one third to each uncompressed image's storage. Flattened near-face UVs are
projected from their final connected positions, matching the greedy LOD mapping.

A texture atlas would require padded tiles, mip-safe gutters, and shader-aware
repetition of greedy rectangles. A texture array avoids tile bleeding and would
support one terrain material, but requires array image preparation, a layer
attribute and a custom PBR shader path. The current change keeps Bevy's existing
PBR pipeline and deduplicates compatible handles. Draws remain per visible
material per section, rather than one draw per section across all materials.

## Surface Appearance

Grass top and side-cap vertices receive linear biome colors from the existing
six blended climate weights. Four shared section-corner samples avoid discrete
biome borders; LODs sample the same color field. Triangle interpolation inside
a greedy rectangle is an approximation to the finer near mesh's interpolation.
Prop templates mark foliage/stems explicitly so biome tinting preserves flower
colors. No extra entities, material per biome, or texture atlas is needed.
The sixth tint belongs to Mushroom Park. Dry oasis grass uses straw/olive
material variants rather than the green foliage palette. Dead desert bushes
are connected bare twigs with less wind flexibility than leafy bushes. Both
reuse the existing grass/bush identities, section batches and picking bounds;
variants 128 through 135 select these desert forms. Small mushrooms use six
cap colors matching their giant counterparts and retain color under biome tint.

Clear/rain/snow presentation changes the shared grass/prop materials at runtime.
Rain also reduces grass roughness. This is not a precipitation, snow accumulation
or fire simulation. Local wetness and charring are stored in the existing session
EditJournal. A separate appearance epoch invalidates only the owning section's
render jobs, not voxel data or colliders. Cached geometry is recomposed on workers;
stale appearances cannot replace the current batch. Sparse changes partition only
intersecting greedy faces by condition: one changed cell on a 16x16 face needs at
most five rectangles, not 256. Unloading/reloading preserves the state in-session.
Neither an atlas nor a texture array has been implemented; further reduction
requires a measured draw-call bottleneck to justify that pipeline complexity.

### LOD Replacement

Terrain and prop sections retain their current batch while an asynchronous
replacement is pending. A successful result with current epochs and the expected
LOD replaces the old entities in one deferred command commit. There is no timed
overlap of old and new mesh stages; `LodSwapStatus.completed` counts replacements.
Material bindings and selection hysteresis remain unchanged.

The previous terrain/prop dithering was removed. In the active Bevy 0.19
depth-only prepass, dithering alone does not enable a fragment shader: full depth
could be written where the main pass later discarded fragments, and shadows
were not masked consistently. The replacement does not add render targets,
passes or a custom shader pipeline. Props use `VisibilityRange::abrupt` to the
configured view distance, normally 192 m, behind the fully opaque fog around
179 m. No prop distance-fade mask remains.

This deliberately trades smooth transitions for a consistent opaque path:
geometric LOD changes can still be visible. Historical 48-second dither-flight
measurements do not validate this fix. Its final native and workspace validation
is recorded separately in the root README and PATCH; TODO contains only open work.

## Legacy And Custom Models

`"render_kind": "custom_mesh"` retains the authored GLB path. Missing
`render_kind` defaults to `custom_mesh`, so existing schema-v1 definitions load
without migration. A custom model requires a safe relative `.glb` or `.gltf` path and can
still use named mesh groups, multiple materials and authored detail meshes.

```json
{
  "schema_version": 1,
  "id": 60002,
  "name": "Authored Block",
  "render_kind": "custom_mesh",
  "model": "custom.glb",
  "bevel_width": 0.0,
  "textures": { "Surface": "textures/custom.png" },
  "faces": ["West", "East", "Down", "Up", "North", "South"],
  "details": [{ "mesh": "TopDecoration", "probability": 0.1 }]
}
```

The default `model_layout` is `face_groups`, preserving the original contract:

- glTF 2.0, Y up, indexed finite triangles with normals, UV0 and explicit materials.
- Six named surface groups with baked transforms; surfaces fit inside [0,1].
- Groups partition the authored surface and can contain multiple primitives.
- JSON `bevel_width` describes the band flattened against solid neighbors.
- JSON texture bindings override the named imported PBR materials' textures.
- Optional named details may extend above their supporting cell.
- The group meshes use baked transforms. Full node hierarchies are available in
  the optional scene layout below.

The renderer preserves custom authored surfaces at distance; it must not replace
an arbitrary custom face with the first primitive's material. The dominant face
material metadata is selected by triangle surface area, never primitive order.

### Whole Static Models

Non-cubic special blocks can opt into the entire default glTF scene:

```json
{
  "schema_version": 1,
  "id": 65000,
  "name": "Carved Pedestal",
  "render_kind": "custom_mesh",
  "model_layout": "scene",
  "model": "fixtures/pedestal.gltf"
}
```

The importer uses the default scene, or the first scene when no default is
specified. It recursively bakes translations, rotations and nonuniform scales
from all mesh nodes. Mirrored transforms reverse winding and normals use the
inverse-transpose transform. Multiple primitives, shared mesh instances and
imported PBR materials are preserved. No six named groups are required. Texture
overrides are optional; untextured models and glTF's default material work too.
Imported source meshes are CPU assets only; final section batches are uploaded.

After transforms, geometry must fit [0,1] on all axes. A scene model is one
editable occupied cell for collision, mining and placement, even if its visible
shape is smaller. It has **no render occlusion of neighbors**: the renderer restores
terrain surfaces next to it and emits the full model once per occupied cell,
including models surrounded by other non-cubic models. This rule also applies
across negative-coordinate section boundaries and unloaded neighbors represented
by session edits. Models remain complete at every LOD; their material triangles
share existing section batches, with no per-model entity or draw call.

The normal Cube meshing path is unchanged. Only a library containing scene models
enables additional occupancy snapshots, cached by the existing section mesh epoch.
Composition and supplemental face meshing happen on workers. The renderer does
not maintain a second block registry, modify generation, or change world IDs.
Final terrain, prop and water Mesh assets use RENDER_WORLD: vertex/index data
moves to the GPU, while Bevy retains the AABB and handle metadata. Rebuilds read
the existing cached source geometry, never the uploaded CPU mesh attributes.

Scene layout requires zero bevel width and no legacy `details` entries. It rejects
animations, skins, morph targets, singular transforms and geometry outside the
cell rather than pretending to animate or silently clipping it. Animated or
multi-cell objects still need a gameplay object renderer. Cameras and glTF lights
are not instantiated. Arbitrary per-placement rotation is not stored in existing
voxel data; author the desired orientation in the model's node hierarchy.

`fixtures/pedestal.gltf` is a real non-cubic hierarchical model used by tests.
Its accompanying JSON is intentionally inside `fixtures/`, so it is not
discovered as a new default palette entry and the shipped Cube IDs stay
unchanged. An opt-in rendering fixture places that definition in the block directory;
the model remains referenced from the fixture subdirectory.

The loader rejects duplicate IDs, unsafe paths, unknown JSON fields, invalid
schema versions, missing face/material bindings and non-finite material values.
Restart the client after changing block definitions. The generic block storage
and edit journal preserve arbitrary `u16` custom-model IDs, including the fixture's
65000. Adding a new gameplay block also requires registering its authoritative
`MaterialId`; dropping an arbitrary JSON into the directory does not grant remote
clients permission to place an unregistered ID.

## Plants And Tree Details

Cherry Forest appends `cherry_leaves` (132) and `white_cherry_leaves` (133).
Both use the existing shared, alpha-clipped foliage meshes and the neutral
procedural `blossom_canopy` pattern. The material tint supplies pink/white;
the green seasonal leaf-color filter is bypassed, but wind and seasonal
shedding still apply. Sparse material IDs use dense bounded provenance slots,
not raw-ID offsets. Existing IDs and logical block sizes are unchanged.
These ordinary leaf blocks need no separate GLB or entity per leaf.

The historical grass/sand `details` entries remain compatible authoring metadata.
For Cube definitions, these names do not load model assets or emit decorations
in addition to world props.
Grass, flowers, ferns, shrubs and shore plants use the existing deterministic
ground-cover generation extended with stable identities and section batches.
Small visual elements belong to their logical prop, not to separate ECS entities
or additional editable voxel cells. The prop renderer reuses the common palette.

For custom meshes, `details[].density_field` still selects grass, bush, flower or
fern density. `water_clearance` restricts details to an inclusive dry-bank height
range above the configured waterline. Submerged decorations are suppressed.

Version-5 trees retain editable wood and foliage cells, roots, branches and crown
clusters, including green, autumn and birch variants. Placement hashes global
coordinates; the generator seed and material IDs are unchanged. Visual crown
details do not modify the generated occupancy or world save data.

Flowing water has no block JSON or GLB. It uses the separate streamed surface
mesh and `shaders/terra_water.wesl`. The decorative solid-water block described
below reuses that shader without entering the fluid simulation.

## Building Palette And World Materials

IDs 1 through 10 retain their original meanings. The appended palette is:

| ID | Material | Natural occurrence |
| --- | --- | --- |
| 11 | Sandstone | Layered beneath desert sand, transitioning to deep stone |
| 12 | Smooth Stone | Building block only; never procedurally generated |
| 13 | Mossy Stone | Sparse damp cave walls near daylight and river-gallery mouths |
| 14 | Solid Water | Building block only; animated, translucent and colliding |
| 15 | Framed Glass | Building block only; connected translucent panes |
| 16 | Oak Planks | Building block only |
| 17 | Birch Planks | Building block only |
| 18 | Spruce Planks | Building block only |
| 19 | Clay | Irregular shallow deposits in submerged sandy beds and river beds |
| 20 | Mushroom Stem | Cream/brown fibrous stems of giant mushrooms |
| 21 | Purple Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 22 | Red Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 23 | Yellow Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 24 | Green Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 25 | Blue Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 26 | White Mushroom Cap | Giant mushroom cap in Mushroom Park |

Material deposits and giant mushroom structures use deterministic cached column
samples. Existing edits override generated materials. Unedited ground may have
new deposits and biome structures; no block ID or saved edit is remapped.
The wire protocol is 14 so clients with older material or biome contracts cannot
silently join an incompatible server.

Mushroom caps share one pale gill material on their underside while their top
and sides use one of six speckled textures. The stem has cream/brown fibers.
All are opaque, solid, individually editable blocks; the existing chunk mesher
culls hidden faces inside each giant structure. Their textures and material
handles are shared by every mushroom. The palette now has 28 terrain material
handles before adding the shared vegetation palette. No giant mushroom needs
a model instance, custom GLB or special collision implementation.

The shared authoritative enum defines opacity, collision, natural spawn support
and flammability. Clay, sandstone and moss are valid natural spawn supports;
glass and decorative water are not interpreted as dry land. New wood planks burn
through the existing surface-condition system. Inventories, timed mining,
creative instant mining, resource drops, placement, section deltas and disk saves
use the same stable IDs. No new inventory or persistence store is introduced.

Solid Water (14) has the same cubic collision and mining semantics as other
building blocks. Its visible faces use the existing animated water material,
with a fixed-cell shader tag and one-meter depth limit. Animation does not move
the logical cell or create a fluid source. Placing it displaces liquid in its
cell; removing it permits neighboring ordinary fluid to flow into the vacancy.
It is excluded from swimming, underwater fog and underwater audio classification.
It remains visible through the water material even if natural-water generation
is disabled for the world. Graphics water-quality settings also apply to it.

Fluid Water (27), `teramic::fluid_water`, creates an actual full source cell in
the existing fluid journal. Its selectable anchor is not solid and emits no
terrain cube faces; the shared fluid mesh renders the source and its finite
outflow. Removing it stops the infinite supply without deleting water already
released. The inventory preview shares Solid Water's animated material. No GLB,
new water renderer or independent persistence format is introduced.

Glass (15) uses alpha 0.1 for the pane, approximately 90 percent transmission for
one face, and an opaque 3.5 cm wood perimeter. Multiple separated panes can still
attenuate more light; this is alpha blending rather than a volumetric optical
model. Adjacent glass cells suppress their shared faces and their internal frame
edges. `BlockFace.edge_mask` stores U-/U+/V-/V+ edge visibility from the existing
one-cell meshing padding, including unloaded and negative-coordinate neighbors.
Glass faces remain cell-sized to preserve irregular joins and frame masks; other
materials retain greedy faces. The same pane/frame construction is used at every
LOD, and border edits remesh the affected neighboring sections. Panes and frames
are batched by material per section, not emitted as individual block entities.
Standard Bevy transparent-batch sorting still applies; intersecting transparent
surfaces are not handled by a new order-independent transparency renderer.
Both pane and frame batch entities explicitly carry `NotShadowCaster`.

Opaque faces behind leaves, glass and solid water remain visible. Transparent
neighbors never punch holes in the terrain, and same-material internal faces
remain culled. Render transparency does not weaken collider occupancy. No block
requires its own Cube GLB; custom non-cubic authored models retain their existing
scene/face-group pipeline.

## Mixer And Passive Inventory Ports

`teramic::mixer` (156) is a one-cell paddle mixer with a side axle, shared wood/
iron geometry, an open upper hopper and a front outlet. Attach a shaft or hand
crank to its horizontal axle, or drive it from a water wheel through the existing
kinetic network. Its load is 24 units at 30 RPM. The normal stress/capacity limits
apply; a stopped or overloaded network cannot manufacture pigment.

Its private inventory has six base-dye inputs and one RGB output. Right-click
the body for the color-picker UI. See [Calibrated RGB Dye](../items/DYES_AND_TOOLS.md#calibrated-rgb-dye)
for recipes, persistence and current dye-application limits. Use the existing
workbench recipe, also visible in Too Many Items, to build the mixer.

A free Tunnel resolves inventory in front of its visible arrow as an insertion
target and inventory behind it as an extraction source. Same-height, one-above
and one-below inventories are supported only where the intermediate path is
loaded and unobstructed. The wrench reverses flow. Older explicitly attached
ports keep their saved extraction/insertion modes.
Ports attached to a machine's outward crank side resolve that one real crank
back to its inventory; unrelated shafts, gear trains and walls are not inventory
extensions.

A Funnel extracting from a machine reads only its real result slot. Its lower
mouth can drop onto a stopped belt or into open air. Dropping a loose stack into
a free or extracting Funnel's upper mouth sends that same drop out below; an
inserting Funnel routes accepted items into its connected inventory instead.
Full inventories, walls, unloaded space and
occupied outlets provide backpressure. All transfers share existing authority,
inventory revisions, world drops and bounded logistics budgets.

### Filtered Funnel And Belt Merger

`teramic::filtered_funnel` (157) follows the normal Funnel's placement, inventory
attachment and flow rules. Right-click opens one ghost selection and an
Allowed/Blocked switch (default Allowed). Copy an item from the cursor, or click
the ghost slot and select a bag/Too Many Items item. No item is consumed or
created. Right-click the selection or use the trash icon to clear it. An empty
filter passes all items in either mode. A selected filter compares the exact
item identity, including wood and RGB metadata. Allowed passes only that item;
Blocked passes everything except it. Filtering applies to machine results,
chest extraction, inventory insertion and loose drops at the mouth. Rejected
items stay at their source; the filter never deletes them. Use the wrench to
reverse an attached port's flow as before.
Loose stacks rest inside the mouth until a valid outlet accepts them, including
while a destination is full or unloaded. Gravity cannot bypass the filter.
Held mouth stacks use deliberate right-click collection rather than passive
pickup, just like items carried by belts. Items already below the outlet are
not caught again.

`teramic::belt_merger` (158) sits above a loaded conveyor segment. Its arrow is
the output direction; belts may feed it from behind, left and right. The output
belt and each contributing input must be running toward the correct connection.
The merger takes turns between available inputs and waits when its outlet is
occupied. Incoming items wait before the junction instead of bypassing it on a
straight belt. It reuses the existing carried stack ID/count and transport
simulation, so belt items remain manual-pickup only. The wrench rotates its
output in quarter turns. No additional independent motor or inventory is used.
Both blocks have workbench recipes in Too Many Items and shed natural snow.

RGB colors on wool, carpet, beds and conveyor bands are sparse appearance data,
not new block IDs. Pure recoloring updates render appearance without rebuilding
terrain, collision or fluid geometry. Filters and RGB survive region unload and
the atomic RGA7 save/load path; mixed old RGA6/ordinary regions remain readable.

### Random Tunnel Extraction

Both Tunnel (152) and Tunnel (Filtered) (159) expose a random-extraction switch.
The ordinary Tunnel has only these transport controls, without a filter slot or
filter mode. Ghost filters belong exclusively to the filtered Tunnel/Funnel
variants; obsolete ordinary-port filters are discarded when loading old saves.
Ordered extraction remains the default. Random mode chooses among occupied
chest slots, respecting an actual filtered variant's filter and both halves
of a connected double chest. Stack size does not weight the selection.
It still exports exactly one item per successful transfer and waits at a blocked
outlet. Machine ports continue to extract only the result slot. The switch is
saved with the world and removed when its Tunnel is removed.

## Alchemy And Campfires

`teramic::cauldron` (161) uses three ingredient slots, water, empty glass bottles,
a reusable spoon and a result slot. Its recipe selector and Too Many Items show
the actual ingredients and separate strength/duration of every potion effect.
The recipes include regeneration, strength, speed, night vision, fire
resistance, a regeneration/strength mixture, and the harmful effects poison,
slowness, freezing, bleeding, rotten and fire. Existing potion IDs 1..6 retain
their recipes; the harmful potions append IDs 7..12. Recipe JSON lives in
`assets/brewing/`; it reuses the existing effect definitions.

Fill a glass bottle by holding use while aiming at a visible water source or a
supplied, open faucet. The held water level rises before the checked transfer;
releasing use, changing slots or looking away interrupts filling. A water bottle
adds one water portion to the cauldron and returns its empty bottle; a filled
bucket adds three portions when the cauldron is empty and returns the bucket.
Smelt sand in the existing furnace (10 seconds per glass block), then craft
three glass blocks into three empty bottles. The full chain is available in
Survival and its recipes appear in Too Many Items.
Each finished potion requires one portion, one glass bottle and its ingredients.
Place a cauldron onto a Campfire, or use a Campfire on a cauldron, to combine them
inside one block cell. The real fire sits below the bowl within its wooden stand,
not on a meter-high pedestal. Both placement orders preserve existing fuel,
ingredients and results. The Campfire/Cauldron navigation button opens the two
separate real inventories. Breaking the assembly returns both blocks and their
contents once. Ignite the fire in its own view to heat the cauldron. Wooden, stone and
gold spoons multiply brewing speed by 1.0, 1.2 and 1.5 respectively; they do not
change the effect strengths or duplicate output. Hold the usual use/place action
to drink a potion. Drinking returns a glass bottle in Survival.
Damage does not restart drinking, so a potion can still be used while poisoned
or burning. Death, changing the held slot or releasing use still interrupts it.
Drinking a water bottle extinguishes the player's fire; it does not heal damage
or remove unrelated effects.
Hot liquid ripples, bubbles and steams even without a selected or ready recipe.
Cold liquid remains still. These effects share the existing two bounded FX
meshes; no entity is created for an individual bubble. Historic two-cell
arrangements remain valid and unchanged; rebuilding the cauldron onto their
Campfire converts the arrangement explicitly. Region RG11 preserves the
separate attached hearth state, while historical RG10 regions remain readable.
The current RG12 wrapper loads this historical state; clients and servers now
require protocol 96 together.

`teramic::campfire` (162) has raw-food, fuel and result slots. Its ignition control
spends real fuel; burning continues even without food. It reuses the existing
food-cooking recipes and fuel values, rather than smelting ores. Extinguishing
discards the currently burning fuel remainder, not the unused fuel stack. Fire
and brewing progress are saved; unloaded fires do not receive unlimited catch-up
simulation. The full ambient temperature system remains the separate F32 task.

Inventory previews preload the finite catalogue at application startup, before
an interactive world is joined. Afterwards the hotbar, station, recipe and Too
Many Items reuse those image handles. Arbitrary RGB variants remain generated
on demand with a 128-icon cache bound; unseen queued requests are discarded and
an active GPU readback finishes safely. Performance/native gameplay fixtures
normally skip startup warmup so their sampling windows do not include icon work;
explicit catalogue checks use `TERRA_PREVIEW_WARMUP=1`.

## Water Pipes, Faucets And Reservoirs

Wood, stone and iron pipes (163..165), plus Glass Pipe (172), share the existing
section model batching. Craft four glass pipes from three framed-glass blocks
and one iron ingot. Glass has transparent walls and opaque fittings; confirmed
network contents are visible inside its bore. Resting empty pipes show no liquid;
confirmed transit remains visible as moving inner pulses without adding stored
water. An interrupted or stopped route clears that transit activity.
Place adjacent pipe segments to form straight runs, elbows, T-junctions and
crosses, including vertical runs. Pipe ends remain round and open; elbows and
branch housings use round profiles. Connected mouths have no duplicated end
rings, and pipe flanges fit their bodies without gaps. External water flows
around pipes, valves, faucets and pumps without entering their separately stored
network contents. Placing, replacing or removing a pipe retains the existing
external water amount and source flag. Idle pipework never primes itself by
debiting the surrounding pool. Natural intake needs a directed route to a
running pump or a reachable open consumer/tank with sufficient hydrostatic
head. Closed valves, stopped pumps and unreachable higher outlets do not request
water. Stored finite basin supply retains its existing gravity rules. Finite
passing water stays finite; actual pumping still applies the small-pool
depletion rules below. Water surfaces use the visible round sweep and faceted junctions instead of square collision holes.
A dry open mouth clips only its annular rim, retaining water visible in its bore.
The shutoff valve (166) follows its shaft
axis; the faucet (167) receives water through its back connection. Use a valve
without a filling vessel to toggle it. Faucets have stages closed/1/2/3: left-click
closes one stage, right-click opens one stage; Shift + left-click mines the faucet.
The target HUD shows the confirmed stage. A held vessel retains its filling action.
Sneak retains adjacent block placement. The wrench changes orientation without
replacing saved water.

The kinetic pump (168) takes real mechanical RPM and stress capacity. Its intake
and discharge follow its facing, and reversing rotation reverses pumping. It
can draw from oceans, lakes and rivers through a submerged inlet pipe, including
a rising inlet leading to a pump above the surface. Keep the inlet connected
and rotate the drive in the intended pumping direction. External water enters
through a real open pipe mouth, rather than through the closed side of a pipe.
Confirmed intake movement produces bubbles at that mouth. Waterlogged plumbing
retains the surrounding water level, including fractional natural levels, so
placing an inlet does not cut a rectangular depression into the surface.
The bubbles remain below the surface and clear of the flange. They do not add
a second flat water sheet over the animated surface.

A connected body containing at least 27 wet cells replenishes intake without
losing its external water. An open water boundary into unloaded territory is
also preserved until its extent is known. Only a proven isolated body with fewer
than 27 wet cells is drained. Its remaining source cells become finite together
so they cannot regenerate the withdrawn quantity. Stored basin and pipe contents
always remain finite. The bounded connectivity check does not flood-fill a sea.
Closed valves stop supply, unloaded pipe connections are not silently traversed,
and an unpowered pump cannot lift water to a higher outlet.

Join basin modules (169) into rectangular reservoirs: 1x1, 1x2, 2x2, 3x3, 4x4
or 5x5 footprints, up to ten modules tall. Each actual module holds at most eight
existing fluid units; the largest reservoir holds 2000 units. Internal walls
are omitted in complete connected rectangles. Looking at the system shows its
real stored amount and flow, while pumps also use the usual kinetic HUD rows.
An open supplied faucet can fill a basin through a clear air gap of up to 32
loaded blocks. Stages discharge 1/2/4 actual units per hydraulic tick, constrained
by available water and receiving capacity. Outdoor basins collect rain slowly:
one unit per 30 seconds per exposed top square at full rain. Roofs block collection.
Reservoir water uses the same material, biome tint, transparency and underwater
effects as rivers and lakes. Its finite stored amount also drives swimming and
breath checks. Modules support Oak, Birch, Spruce and Cherry through the existing
wood identity; the plain Too Many Items basin is Spruce. Crafting inherits the
consumed planks, and mining/copying preserves the actual species.

Building underwater looks through the water at the actual solid target, even
when the player's eyes start inside a source cell. The target's adjacent cell
receives the placed block; the eye's water cell is not selected as the surface.
Vessel filling and underwater mining retain their separate targeting rules.

Right-click (place/use) a visible water source with an empty bucket or a
watering can to fill it immediately. Glass bottles and filling at a supplied
faucet still use the hold action. A source bucket transfer uses the existing
source-cell rules; a faucet spends finite network contents. A bottle takes one
unit, a bucket eight, and a can only its missing reserve. The water level is
visible in the held vessel during filling. Filled buckets retain their existing
source-placement action. Hold a watering can over farmland to tip its spout and
pour a visible stream; watering a dry planted plot consumes its real reserve in
Survival. Empty cans must be filled first.
Each can carries its own water amount through inventory moves, drops and saving;
changing the held slot cannot transfer water to another can.

## Resting Water And Source Formation

A fully supported, enclosed 2x2 terrain pool converts its two remaining cells
into full sources when the other two full sources occupy opposite diagonals.
One source or two adjacent sources do not create additional source flags.
Missing floor support, open sides, falling water and unloaded boundaries cannot
create floating or uncontrolled sources. Source formation uses the existing
fluid metadata and persists through section and region reloads.

Supported, enclosed shallow terrain pools of other shapes also settle, up to
64 interior cells. With no sources, their total finite amount is conserved and
distributed deterministically. The check has a shared budget of 256 cells per
fluid step; an incomplete proof waits without copying water into empty members.
Settled pools keep zero flow and stop changing their fluid and water mesh epochs.
Pumping a small body removes all its source flags together, so settling does not
regenerate the withdrawn amount. Stored wooden basin contents remain finite.

## Passive Belt Filling

The Hold Block (170) has the same upper surface as a horizontal conveyor.
Place it between two aligned belts at that height. Adjacent belt geometry sets
its axis, and actual belt motion sets the transport direction; the wrench can
adjust the preferred orientation when needed. The target HUD shows the confirmed
direction. Its belt surface continues to the neighboring rollers, and its stops
rise while a vessel is held. The oak frame, iron platform, brass straps, posts,
pipe flange and filling nozzle follow the detailed
[filling-machine reference](../../../docs/references/filling-machine-system.png).

Leave one complete air block above the Hold Block, then place the Fluid Infuser
(171) above that gap: Hold at Y, air at Y+1, Infuser at Y+2. The Infuser occupies
one full block including its nozzle and fittings, following the
[six-view reference](../../../docs/references/fluid-infuser-block.png).
Connect a pipe to an Infuser side or its top. The nozzle faces downward through
the clear gap toward the stationary vessel. Blocking that gap stops filling
without consuming water. The Infuser requires no kinetic drive; the belts provide
transport. Drive both adjoining belt sections in the same direction, connecting
their shafts with the usual shafts or gears around the station. The passive Hold
Block does not transmit torque. Both blocks are in Technic and have workbench recipes.

Incompatible and already full items pass together at the ordinary belt spacing
and speed, including across the roller transition. They do not reserve a filling
slot or raise the stops. When a fillable vessel arrives, preceding traffic clears
first; that vessel then enters alone and stops at the center. The Infuser spends
finite network contents: one water unit for a bottle,
eight for a bucket, and the missing amount for a watering can. Cans can fill over
several transfers. After filling, the stops release the vessel onto the outgoing
belt through the ordinary continuous transport and collision checks. Items do
not teleport across the station. Ordinary following items can continue closely
behind the released vessel; another fillable vessel waits for the filling slot.
Dry supply or a blocked output retains the admitted item; queued stacks separate
one vessel at a time without duplicating their contents. Removing the Infuser
makes the platform pass items onward. Unloaded stations receive no unlimited
catch-up. The current pipe system carries water; the conversion contract
explicitly identifies that liquid for future fluid types.

Running pumps and flowing supplied faucets use the existing positional device
soundscape, with distance, obstruction, volume settings and mute applied.

Plumbing cells and shapes extend the RG12 region wrapper; old RG11 and earlier
regions remain readable. New clients and servers use protocol 97 together.

## Validation And Authoring

Tests cover stable block IDs, legacy schema loading, missing bindings, unsafe
paths, material sharing, explicit grass/snow caps, closed procedural surfaces,
outward triangle winding and finite indices/coordinates. Cube asset tests verify
standalone PNG bindings, current JSON color factors, empty model references and
the absence of the ten obsolete GLBs.
Section seam tests additionally exercise the rendering mesh pipeline.
Static-scene tests run the real Bevy glTF loader, verify hierarchy transforms,
mirrored winding, full-model batching and neighbor-face restoration. They do not
require a renderer. The additional `--smoke-test-custom` native mode renders
three fixture models beside a regular cube; its checked result is recorded in
`screenshots/reference-custom.png`. The root README contains the opt-in command.

The obsolete `tools/assets/build_blocks.py` exporter has been removed so it cannot
regenerate per-material Cube GLBs. Author only non-cubic custom objects in a glTF
2.0 editor, following the scene or face-group constraints above; normal Cube
geometry is maintained in the shared Rust face generator. The small text-based
`fixtures/pedestal.gltf` remains the deterministic custom-model regression asset.
Texture provenance is recorded in `textures/prompt.txt`,
`textures/canopy-prompt.txt` and `textures/stylized-prompts.txt` (new built-in
image_gen textures and exact prompts). Performance measurements and remaining visual
limitations belong in the project overhaul report; material deduplication alone
is not evidence of an FPS improvement.

## Modular Parked Trains

Materials 210–216 are chassis, bogie, frame, coupler, cab, seat and private cargo;
217 is their assembly station. `tools/assets/build_train_parts.py` authors the
native scenes, collision profiles and workbench recipes together. These custom
objects share the existing machinery textures and wood material classification.
Bogies span the shared two-metre rail gauge; their authored boxes intentionally
extend into the neighboring cells. Other modules fit their owner cell.

An assembled vehicle uses the same part geometry with the common Core model
origin offset of -0.7375 m, bringing wheel bottoms onto the 0.3125 m rail head.
Ownership/restoration cells stay on the integer block grid. Renderer, physical
compound shapes, ray hits, clearance and seats use the shared physical origin.
Vehicles batch by authored material and retain their geometry through cargo or
coupling changes. F63 covers parked assembly; driving and arbitrary block payloads
remain separate features. Acceptance uses CPU scene/geometry checks only.
