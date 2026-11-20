"""Shared recipe locations for asset generators; recipe IDs never depend on paths."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRAFTING = ROOT / "assets/crafting"


def category(slug):
    if slug.startswith("train_"):
        return "transport/trains"
    if slug.endswith("_station"):
        return "stations"
    if slug.startswith(("rail_", "ballast")):
        return "transport/rails"
    if slug.startswith(("castle_", "bergfried")):
        return "building/castle"
    if "_bed" in slug:
        return "building/furniture/beds"
    for suffix, group in [("_door", "doors"), ("_window", "windows"), ("_fence", "fences")]:
        if slug.endswith(suffix):
            return f"building/{group}"
    if "_cape" in slug:
        return "equipment/capes"
    if slug.endswith(("_boots", "_chestplate", "_gloves", "_helmet", "_pants")):
        return "equipment/armor"
    if slug in {"earrings", "necklace", "belt", "socks"}:
        return "equipment/accessories"
    if slug.endswith("_sword"):
        return "equipment/weapons"
    for tool in ["pickaxe", "axe", "shovel", "hoe", "scythe", "watering_can"]:
        if slug.endswith("_" + tool):
            return f"equipment/tools/{tool}"
    if slug in {"bucket", "wrench"}:
        return "equipment/tools/general"
    if slug.endswith("_dye") and not slug.startswith(("conveyor", "wool")):
        return "textiles/dyes"
    if slug.endswith("_carpet") or slug == "carpet":
        return "textiles/carpets"
    if slug.endswith("_wool") or slug == "wool":
        return "textiles/wool"
    if slug.startswith("wool_"):
        return "textiles/shapes"
    if slug.endswith(("_slab", "_stairs")):
        return "building/shapes"
    if slug in {"conveyor", "conveyor_dye", "belt_distributor", "belt_merger", "filtered_funnel", "filtered_tunnel", "item_hopper", "item_importer"}:
        return "machines/logistics"
    if slug.endswith(("_gear", "_large_gear", "_shaft")) or slug.startswith("gearbox_") or slug in {"hand_crank", "water_wheel"}:
        return "machines/kinetics"
    if slug.endswith("_pipe") or slug in {"basin", "pump", "valve", "faucet", "hold_block", "fluid_infuser"}:
        return "machines/plumbing"
    if slug.endswith("_bellows") or slug in {"furnace", "chimney_straight"}:
        return "machines/smelting"
    if slug.endswith("_spoon") or slug in {"cauldron", "glass_bottle"}:
        return "machines/brewing"
    if slug in {"crusher", "mixer"}:
        return "machines/processing"
    if slug in {"torch", "lantern", "chain", "campfire"}:
        return "building/lighting"
    if slug in {"chest", "workbench", "flower_pot"}:
        return "building/furniture"
    if "sapling" in slug or slug.endswith("_seeds") or slug in {"farmland", "scarecrow"}:
        return "farming"
    if slug == "bread":
        return "food"
    if slug.endswith("_planks") or slug == "sticks":
        return "materials/wood"
    if slug in {"sandstone", "path"}:
        return "materials/terrain"
    raise ValueError(f"Choose a recipe category for {slug}")


def recipe_path(slug):
    return CRAFTING / category(slug) / f"{slug}.json"


def write_builtins():
    sources = crafting_sources()
    target = ROOT / "crates/terra-core/src/crafting/builtins.rs"
    target.write_text("[\n" + "".join(
        f'    include_str!("../../../../assets/crafting/{path.relative_to(CRAFTING).as_posix()}"),\n'
        for path in sources) + "]\n")


def crafting_sources():
    process_roots = {("processes", kind) for kind in ["smelting", "crushing", "brewing"]}
    return sorted(path for path in CRAFTING.rglob("*.json")
                  if tuple(path.relative_to(CRAFTING).parts[:2]) not in process_roots)
