# Pigments And Tool Tiers

The complete dye palette is red, yellow, blue, orange, green, purple, white,
black, lime, pink, gold, silver and light blue. Existing flower variants retain
their original pigments. The six new colors do not introduce recolored flowers.

Crafting routes, in addition to the existing primary and mixed pigments:

| Output | Ingredients | Output Count |
| --- | --- | --- |
| Black | Coal | 2 |
| White | Salt crystal | 2 |
| Lime | Green dye + yellow dye | 2 |
| Pink | Red dye + white dye | 2 |
| Gold | Yellow dye + orange dye | 2 |
| Silver | White dye + black dye | 2 |
| Light blue | Blue dye + white dye | 2 |

The manual crusher additionally extracts black pigment from coal dust and white
pigment from salt dust. Existing coal/salt dust recipes are unchanged. A mixer is
not implemented. All 13 dyes recolor all three wooden beds and all five cape
tiers; recoloring preserves the cape material and its attributes.

Sword, axe and pickaxe variants now cover wood, stone, iron, gold, diamond and
ruby. The two existing wood/stone shovels remain available. Shaped workbench
recipes consume the matching wood, stone, ingot or gemstone and sticks.

| Material | Harvest Level | Matching-Block Efficiency |
| --- | --- | --- |
| Wood | 1 | 2x |
| Stone | 2 | 4x |
| Iron | 3 | 6x |
| Gold | 3 | 10x |
| Diamond | 4 | 8x |
| Ruby | 5 | 12x |

Axes do not substitute for pickaxes. Existing ore requirements are unchanged.
The first seven serialized dye colors and first nine tool variants keep their
original enum positions. Old bed poses 0 through 27 are unchanged; appended
colors occupy poses 28 through 51. Existing RGA1/PLY7 saves remain readable.
Network protocol 64 requires clients and servers to be updated together.

## Farming Tools

Watering cans, hoes and scythes each have wood, iron, gold, diamond and ruby
variants. Their individual definitions and shaped workbench recipes use the
existing item and crafting directories, including Too Many Items support.
All 15 variants are appended after the existing tool tags.

Use the configured Place/Use action to till exposed dirt/grass, refill a can
from actual water, water a crop, or harvest ripe crops with the scythe. Can
capacities are 8/12/16/20/24; water reserves currently belong to the player
session and reset on reconnect. Crop moisture is persistent. Scythes cover
3x3 support cells, or 5x5 for diamond/ruby, on one height level. Each crop must
be visible and reachable; unripe crops remain intact. These tools do not
substitute for mining tools.

See [Farming Tools And HUD Corrections](../../../tools/benchmarks/results/farming-tools-2026-10-23/README.md)
for models, animations, placement rules, test evidence and known limits.

## Calibrated RGB Dye

The kinetic Mixer (`teramic::mixer`, block 156) produces `teramic::mixed_dye#RRGGBB`.
Choose a target in its color picker; the six input slots show missing base dyes
as transparent recipe ghosts, not real inventory items. The recipe summary shows
available/required quantities even when the slots contain other colors.

Recipes fit the 13 existing pigments in linear RGB with bounded integer-dose
refinement. A calibrated batch normally consumes 16 pigment units and returns
16 dyes carrying the byte-exact chosen RGB value. Colors matching a standard
pigment need one dose. The machine calibrates pigment concentration; this is a
game recipe model, not a physical paint-chemistry simulation. At 30 RPM a batch
takes four seconds. Inputs are consumed only when the complete batch can be
committed into its output slot. No power, an unloaded station or a full/mismatched
output pauses production without losing ingredients. Changing the target resets
only progress, never the inventory.

Mixed dyes stack only with identical RGB codes, survive save/load and travel on
belts through the same Tunnel/Funnel inventory path. A cape, bed, wool, wool
stairs, wool slab, carpet or conveyor and an RGB dye in the crafting grid recolor that item without adding
millions of static recipes. Conveyor dye changes only the belt, not its wooden
frame; bed dye changes only the fabric. Too
Many Items lists only the original pigments; it does not enumerate RGB values.
Press R over an existing mixed-dye stack to inspect its on-demand mixing recipe.
The recipe's apply action sets the target only when an actual Mixer inventory is
open; it neither supplies free ingredients nor bypasses server checks.
Beds, wool, wool stairs, wool slabs, carpets and belts can also be dyed directly
with the normal Use action, just like a standard-palette dye. Placement,
middle-click copying and breaking retain
the exact color and belt wood. Applying a standard dye replaces the old RGB color.
Press R over an existing RGB fabric item to see its matching dye recipe. RGB
variants are generated on demand, not added to the static Too Many Items list.
Cutting colored wool into stairs, slabs or carpets preserves its RGB value.
In a mixed-material slab cell, only its wool half receives the tint. The two
halves can still be mined separately. Two wool halves with different RGB values
cannot occupy one cell: the existing cell metadata stores one RGB color, so this
combination is rejected without consuming either slab.

The RGB enum tag is appended after all existing colors. Regions containing a
Mixer use the versioned RGA6 extension. Regions with placed RGB fabrics or funnel
filters use RGA7, preserving the RGA6 data inside it; older regions retain their
existing formats and remain readable. Mixed pigment metadata never changes an
existing block ID. Client and dedicated server use protocol 88 and must be updated
together. Dynamic fabric icons and dropped/held-model caches are bounded.
