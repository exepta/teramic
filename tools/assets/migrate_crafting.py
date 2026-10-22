"""One-time structured migration of phase-45 bundled asset definitions."""
import json
import pathlib
import tomllib

root = pathlib.Path(__file__).resolve().parents[2]
assets = root / "assets"
old = assets / "terra/recipes/basic.toml"
if old.exists():
    recipes = tomllib.loads(old.read_text())["recipes"]
    target = assets / "crafting"
    target.mkdir(exist_ok=True)
    for recipe in recipes:
        name = recipe["id"].split("::")[1]
        ingredients = recipe["pattern"].get("ingredients", [])
        if recipe["station"] == "workbench":
            cells = [None] * 9
            if "chest" in name:
                pattern = [0, 0, 0, 0, None, 0, 0, 0, 0]
            elif "pickaxe" in name:
                pattern = [0, 0, 0, None, 1, None, None, 1, None]
            elif "axe" in name:
                pattern = [0, 0, None, 0, 1, None, None, 1, None]
            elif "shovel" in name:
                pattern = [None, 0, None, None, 1, None, None, 1, None]
            else:
                pattern = []
            for i, material in enumerate(pattern):
                if material is not None:
                    cells[i] = {"item": ingredients[material]["item"], "count": 1}
            recipe["pattern"] = {"kind": "shaped", "width": 3, "height": 3, "cells": cells}
        (target / f"{name}.json").write_text(json.dumps(recipe, indent=2) + "\n")
    old.unlink()
    old.parent.rmdir()

for path, key in [(assets / "terra/props/items.json", "kind"), (assets / "terra/items/tools.json", "item")]:
    if not path.exists():
        continue
    content = json.loads(path.read_text())
    for definition in content.get("items", []) if isinstance(content, dict) else content:
        identity = definition[key]
        slug = identity if isinstance(identity, str) else identity["tool"]
        (path.parent / f"{slug}.json").write_text(json.dumps(definition, indent=2) + "\n")
    path.unlink()

for path in (assets / "terra/blocks").glob("*.json"):
    definition = json.loads(path.read_text())
    material = definition["id"]
    rock = material in [3, 11, 12, 13]
    wood = material in [6, 10, 16, 17, 18, 28, 29]
    hardness = 1.25 if rock else 0.8 if wood else 0.55
    if material in [7, 8, 9]:
        hardness = 0.3
    definition["hardness"] = hardness
    definition["resistance"] = 6.0 if rock else 3.0 if wood else 0.5
    definition["required_tool"] = "pickaxe" if material in [12, 13] else None
    definition["mining_level"] = 1 if material in [12, 13] else 0
    path.write_text(json.dumps(definition, indent=2) + "\n")

for path in (assets / "crafting").glob("*.json"):
    recipe = json.loads(path.read_text())
    pattern = recipe["pattern"]
    if pattern["kind"] != "shaped":
        continue
    width = pattern["width"]
    occupied = [i for i, cell in enumerate(pattern["cells"]) if cell is not None]
    x0, x1 = min(i % width for i in occupied), max(i % width for i in occupied)
    y0, y1 = min(i // width for i in occupied), max(i // width for i in occupied)
    pattern["cells"] = [pattern["cells"][x + y * width] for y in range(y0, y1 + 1) for x in range(x0, x1 + 1)]
    pattern["width"], pattern["height"] = x1 - x0 + 1, y1 - y0 + 1
    path.write_text(json.dumps(recipe, indent=2) + "\n")

sources = sorted((assets / "crafting").glob("*.json"))
(root / "crates/terra-core/src/crafting/builtins.rs").write_text(
    "[\n" + "".join(f'    include_str!("../../../../assets/crafting/{path.name}"),\n' for path in sources) + "]\n"
)

categories = assets / "terra/inventory/categories.json"
if categories.exists():
    target = categories.with_suffix("")
    target.mkdir(exist_ok=True)
    for category in json.loads(categories.read_text())["categories"]:
        (target / f'{category["id"]}.json').write_text(json.dumps(category, indent=2) + "\n")
    categories.unlink()
