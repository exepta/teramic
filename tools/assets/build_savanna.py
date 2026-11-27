"""Build the savanna's placeable blocks, voxel props and acacia woodworking recipe."""
from pathlib import Path

from build_castle import BLOCKS, ROOT
from build_salt_cave import save_json
from savanna import surfaces
from savanna.catalog import CUBES, PROPS
from savanna.scenery import COLLIDERS, model


def cube(slug, index):
    if slug.endswith("_turf"):
        textures = {
            face: surfaces.write(slug, face, surfaces.turf(slug, face))
            for face in ("top", "side")
        }
        textures["soil"] = "textures/savanna/dry_savanna_soil_side.png"
        bottom, top = "soil", "top"
    elif slug.endswith("_leaves"):
        textures = surfaces.canopy(slug.removesuffix("_leaves"))
        textures["spray"] = textures.pop("cluster")
        bottom, top = "side", "top"
    elif slug.endswith(("_log", "_planks")):
        faces = ("side", "end") if slug.endswith("_log") else ("side",)
        textures = {face: surfaces.write(slug, face, surfaces.wood(slug, face)) for face in faces}
        bottom = top = faces[-1]
    else:
        textures = {"side": surfaces.write(slug, "side", surfaces.mineral(slug, index))}
        bottom = top = "side"

    materials = {face: {"roughness": .91, "reflectance": .09} for face in textures}
    block = dict(render_kind="cube", bevel_width=0, textures=textures, materials=materials,
                 face_materials=["side", "side", bottom, top, "side", "side"])
    if "spray" in textures:
        block["foliage_cluster"] = {"material": "spray"}
    else:
        for face, path in textures.items():
            path = Path(path)
            materials[face]["normal_texture"] = path.with_name(path.stem + "_normal.png").as_posix()
    return block


def build():
    locale = {
        "Savanna": "Savanne",
        "Savanna · Warm": "Savanne · Warm",
        "Acacia sapling": "Akaziensetzling",
        "An acacia sapling. Plant on open soil with room for its wide, flat crown. Grows during loaded daylight.":
            "Pflanze den Akaziensetzling auf offenem Boden mit Platz für seine breite Schirmkrone. "
            "Er wächst bei Tageslicht, solange das Gebiet geladen ist.",
    }
    for index, (slug, english, german, description, translation) in enumerate(CUBES + PROPS):
        block_id = 632 + index
        wood = slug.endswith(("_log", "_planks"))
        mineral = 636 <= block_id <= 642
        category = "nature"
        if wood:
            category = "wood"
        elif index < len(CUBES) and not slug.endswith("_leaves"):
            category = "terrain"
        block = dict(schema_version=1, id=block_id, name=english, description=description,
                     creative_category=category, mining_level=0, required_tool=None,
                     hardness=1.85 if wood or mineral else .5, resistance=2.2 if wood or mineral else .6)
        if index < len(CUBES):
            block.update(cube(slug, index))
        else:
            boxes = [dict(minimum=box[:3], maximum=box[3:]) for box in COLLIDERS.get(slug, ())]
            block.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                         struct_block=True, textures=model(slug, block_id), collision_boxes=boxes)
        save_json(BLOCKS / (slug + ".json"), block)
        locale[english] = german
        locale[description] = translation

    save_json(ROOT / "src-ui/locales/source/savanna-blocks.json",
              [dict(source=en, en=en, de=de) for en, de in locale.items()])
    folder = ROOT / "assets/crafting/biomes/savanna"
    folder.mkdir(parents=True, exist_ok=True)
    save_json(folder / "acacia_planks.json", dict(
        id="teramic::acacia_planks", name="Acacia Planks", station="hand",
        output=dict(item=dict(block=645), count=4),
        pattern=dict(kind="shapeless", ingredients=[dict(item=dict(block=643), count=1)]),
    ))


if __name__ == "__main__":
    build()
