# Crafting recipes

All finite crafting/process recipes live below this directory, one JSON file per
recipe. Existing IDs, patterns, ingredients, output quantities and timings remain
unchanged when a file moves. The directory path is never part of a recipe's ID.

| Directory | Contents |
| --- | --- |
| `building/` | Castle blocks, shapes, doors, windows, fences, beds, furniture and lighting |
| `equipment/` | Tools, weapons, armor, capes and accessories |
| `farming/`, `food/`, `materials/`, `textiles/` | Seeds, saplings, bread, ordinary materials, dyes and fabrics |
| `machines/` | Recipes **for constructing** kinetic, logistics and processing machines |
| `stations/` | Nine profession stations |
| `transport/rails/`, `transport/trains/` | Railway and vehicle parts, including the assembly station |
| `processes/smelting/` | Nine furnace transformations; also shared by campfire cooking |
| `processes/crushing/` | Nineteen crusher transformations |
| `processes/brewing/` | Twelve cauldron recipes with their actual potion inputs/effects |

The ordinary recipe schema (`id`, `name`, `station`, `pattern`, `output`) contains
447 recipes. `terra_core::crafting::RecipeBook::load_directory` reads category
subfolders recursively in a deterministic order. A malformed file or duplicate
ID rejects the complete attempted load; it never replaces an existing recipe.
Files include their path in load errors. Flat legacy/mod recipe folders remain
supported. Symbolic links are rejected to avoid recursion loops.

The three explicitly named `processes/` directories have their existing separate
smelting/crushing/brewing schemas and real machine loaders. They are excluded
from workbench parsing; unknown process directories are still validated as
ordinary crafting data. These 40 process files and their embedded fallbacks
preserve all previous definitions. Fuel durations remain under `assets/fuels/`;
effect definitions remain under `assets/effects/`. RGB dye mixing is computed
from color inputs and is not represented as fabricated finite recipes.

The server loads this asset tree through the shared core plugin and sends its
authoritative ordinary recipe snapshot to clients. `configs/recipes/` still adds
ordinary namespaced mod recipes, subject to the same atomic duplicate checks.

Asset generators use `tools/assets/recipe_paths.py` for their output paths.
`python3 tools/assets/organize_crafting.py` can migrate legacy flat files and
refresh the embedded ordinary recipe list. It does not change recipe bytes and
refuses to overwrite an existing destination. Machine process files retain
their individual embedded definitions. Regenerating meshes is unnecessary when
only moving recipes.
