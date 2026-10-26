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
