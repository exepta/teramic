# Armor And Equipment

The five sets are Leather, Iron, Gold, Diamond and Ruby. Each has a helmet,
chestplate, gloves, pants and boots, defined in its own item JSON. Earrings,
necklace, belt and socks activate the four existing accessory placeholders.
All equipment items stack to one, including in backpacks, chests, the cursor
and crafting output. Recipes live individually in `assets/crafting/`.

The original eight equipment indices remain unchanged: helmet, earrings,
chestplate, necklace, pants, belt, boots, socks. Gloves are appended as index 8.
Capes use index 9. Leather, Iron, Gold, Diamond and Ruby cloaks start white;
combine a cloak with one dye in a shapeless recipe to recolor its cloth.
All 13 dyes are supported, giving 65 material/color combinations. Existing
color indices remain stable; appended colors do not change old saved capes.
Recoloring keeps its material tier, protection and attribute bonuses.
Cape geometry is back-only. Two connected, smoothed panels respond to the
avatar's relative walking air and the existing weather wind; portraits remain
still and no cloth collisions or per-frame mesh rebuilds are introduced.
Each slot accepts only its corresponding equipment type. The persistent
loadout belongs to `PlayerStats`, not `Inventory`, so existing chest, furnace
and region inventory layouts are unchanged.

Each item JSON contains `equipment.slot`, `armor_points`, `health_bonus` and
`stamina_bonus`. Definitions are validated once; malformed overrides use the
embedded defaults. Slot identity cannot be changed to an incompatible piece.
The client/server handshake checks the canonical equipment catalog checksum.
Restart after JSON edits; live reload is not implemented.

Full-set armor points are Leather 10, Iron 20, Gold 16, Diamond 30 and Ruby 35.
Each point reduces animal attacks, lightning and falling-mineral damage by 2%,
up to 80% reduction at the hard cap of 40 points. Armor does not reduce falls,
fire, freezing, poison, bleeding, smoke, hunger, drowning or void damage.
Existing Shield Power and matching elemental resistances still apply to their
own damage causes. Armor reduction is calculated before temporary shield
absorption.

Health/stamina bonuses sum across the loadout, capped at +200 each. Base maxima
remain 100. Equipping never restores spent health/stamina; unequipping clamps
remaining values to the reduced maxima. Hunger, saturation and oxygen keep their
existing 100-point limits. Recovery and swimming respect the derived stamina
and health maxima.

New player records use `PLY7`. Explicit positional `PLY6` and `PLY5` readers preserve
status effects, remaining shield, food reserves, stats and inventory; earlier
player formats remain readable. Loading derives protection and attribute maxima
again from the validated loadout instead of trusting saved bonus values.
Existing block IDs, world IDs and inventory layouts are unchanged. Crusher
progress adds a separate `RGA1` region envelope, with explicit older readers.
