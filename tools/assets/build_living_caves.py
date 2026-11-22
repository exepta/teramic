"""Build the lush and glacial cave palettes and their rooted modular set pieces.

The checked-in maps are production inputs. --lush and --ice resample the
1448 × 1086 artist boards with the same orthographic unwrap as the salt palette.
"""
import argparse
import json
import math
import random

from build_castle import BLOCKS, Mesh
from build_dripstone_cave import column
from build_salt_cave import sample_swatches, save_json, stone
from build_spirit_blossom import export

DIRECTORY = 'living_caves'
# Six corners in the 920 px board preview (left, back, right, front, front foot, right foot).
LUSH = {
    'rock': [(31,217),(77,197),(121,214),(77,236),(77,288),(121,270)],
    'overgrown': [(255,217),(300,196),(343,214),(301,232),(301,288),(343,269)],
    'roots': [(370,216),(414,199),(457,214),(413,232),(413,288),(457,269)],
    'turf': [(480,216),(522,201),(568,213),(526,232),(526,289),(566,269)],
    'moss': [(588,215),(633,197),(677,214),(633,233),(633,288),(675,270)],
    'glow': [(699,215),(741,196),(787,214),(744,234),(743,288),(784,269)],
}
ICE = {
    'clear': [(24,168),(86,143),(150,166),(86,190),(86,276),(150,247)],
    'packed': [(174,168),(236,142),(299,165),(237,192),(237,276),(298,250)],
    'rime': [(321,169),(384,143),(447,166),(384,192),(384,275),(446,249)],
    'frozen': [(468,170),(533,143),(597,166),(534,194),(536,275),(597,249)],
    'dust': [(618,169),(679,143),(747,166),(685,193),(685,276),(744,250)],
    'azure': [(771,170),(833,145),(898,169),(836,193),(837,275),(898,250)],
}
SURFACES = [
    ('lush_cave_stone','Lush Cave Stone','Grünes Höhlengestein','rock',.83,.20),
    ('overgrown_stone','Overgrown Cave Stone','Überwucherter Höhlenstein','overgrown',.83,.17),
    ('rooted_cave_dirt','Rooted Cave Dirt','Durchwurzelte Höhlenerde','roots',.91,.14),
    ('cave_grass','Cave Grass','Höhlengras','turf',.91,.14),
    ('cave_moss','Cave Moss','Höhlenmoos','moss',.92,.14),
    ('glow_moss','Glow Moss','Leuchtmoos','glow',.76,.19),
    ('clear_cave_ice','Clear Cave Ice','Klares Höhleneis','clear',.19,.47),
    ('packed_cave_ice','Packed Cave Ice','Verdichtetes Höhleneis','packed',.34,.38),
    ('frosted_cave_ice','Frosted Cave Ice','Bereiftes Höhleneis','rime',.50,.29),
    ('frozen_stone','Frozen Stone','Vereistes Gestein','frozen',.64,.29),
    ('snow_dust_stone','Snow-Dusted Stone','Schneebedecktes Gestein','dust',.84,.18),
    ('blue_crystal_block','Blue Crystal Block','Blauer Kristallblock','azure',.25,.46),
]
PROPS = [
    ('glow_vine','Glow Vine','Leuchtranke'),
    ('frost_vine','Frost Vine','Frostranke'),
    ('frost_cluster','Frost Cluster','Reifbüschel'),
    ('glacial_crystal_cluster','Glacial Crystal Cluster','Gletscherkristallgruppe'),
    ('cave_wildflowers','Cave Wildflowers','Höhlenblumen'),
]


DESCRIPTIONS = {
    "lush_cave_stone": ("Jointed grey stone from underground galleries.", "Graues, zerklüftetes Gestein aus unterirdischen Galerien."),
    "overgrown_stone": ("Trailing leaves and moss follow the joints in this rock.", "Ranken und Moos wachsen in den Fugen dieses Steins."),
    "rooted_cave_dirt": ("Layers of cave soil bound by thick roots.", "Dicke Wurzeln halten die Schichten dieser Höhlenerde zusammen."),
    "cave_grass": ("Soft grass thrives on the moist banks of underground streams.", "Weiches Gras gedeiht an feuchten unterirdischen Bachufern."),
    "cave_moss": ("A deep mat of green cave moss.", "Ein dichtes Polster aus grünem Höhlenmoos."),
    "glow_moss": ("Warm flecks glow among the moss leaves.", "Zwischen den Moosblättchen leuchten goldene Punkte."),
    "clear_cave_ice": ("Clear glacier ice veined with frozen air bubbles.", "Klares Gletschereis mit eingeschlossenen Luftblasen."),
    "packed_cave_ice": ("Dense blue ice pressed into layers.", "Dichtes blaues Eis aus zusammengepressten Schichten."),
    "frosted_cave_ice": ("A white crust of rime covers the blue ice.", "Eine weiße Raureifkruste bedeckt das blaue Eis."),
    "frozen_stone": ("Frost fills the joints in this dark stone.", "Frost füllt die Fugen dieses dunklen Gesteins."),
    "snow_dust_stone": ("Powder snow rests on a shelf of frozen stone.", "Pulverschnee liegt auf einem Absatz aus gefrorenem Stein."),
    "blue_crystal_block": ("A seam of deep blue crystalline ice.", "Eine Ader aus tiefblauem kristallinem Eis."),
    "glow_vine": ("Hanging cave vines carry golden buds.", "Goldene Knospen wachsen an herabhängenden Höhlenranken."),
    "frost_vine": ("Slender frost filaments hang from the cave roof.", "Feine Reiffäden hängen von der Höhlendecke."),
    "frost_cluster": ("Short blades of rime grow on glacial ledges.", "Kurze Reifklingen wachsen auf vereisten Vorsprüngen."),
    "glacial_crystal_cluster": ("A fan of pale blue glacial crystals.", "Ein Fächer aus hellblauen Gletscherkristallen."),
    "cave_wildflowers": ("Pink, white and violet flowers grow on lit moss beds.", "Rosa, weiße und violette Blüten wachsen auf hellen Moospolstern."),
}


def swatches(table):
    return {
        key: [[round(component * 1448 / 920) for component in point] for point in corners]
        for key, corners in table.items()
    }


def crystals(mesh, slug, rng):
    tall = slug == "glacial_crystal_cluster"
    for i in range(18):
        angle = i * 2.39996
        r = 0.37 * math.sqrt(i / 18)
        width = 0.105 if i == 0 else rng.uniform(0.040, 0.10)
        height = (0.94 if tall else 0.44) * (1 - 1.65 * r) * rng.uniform(0.72, 1.0)
        column(
            mesh, 0.5 + math.cos(angle) * r, 0.5 + math.sin(angle) * r,
            width, width * 0.74, height, angle,
            (math.cos(angle) * r * 0.20, math.sin(angle) * r * 0.20),
            ("core", "rime") if tall else ("snow", "rime"),
        )


def wildflowers(mesh, rng):
    for i in range(15):
        angle = 2.40 * i
        r = 0.41 * math.sqrt(i / 15)
        x, z = 0.5 + r * math.cos(angle), 0.5 + r * math.sin(angle)
        h = rng.uniform(0.16, 0.46)
        mesh.box([x - 0.012, 0, z - 0.012], [x + 0.012, h, z + 0.012], "stem", 0)
        key = ["petal white", "petal purple", "petal pink"][i % 3]
        for j in range(4):
            b = j * math.pi / 2 + 0.45
            dx, dz = 0.035 * math.cos(b), 0.035 * math.sin(b)
            mesh.box(
                [x + dx - 0.023, h - 0.007, z + dz - 0.023],
                [x + dx + 0.023, h + 0.017, z + dz + 0.023], key, 0,
            )
        mesh.box([x - 0.017, h, z - 0.017], [x + 0.017, h + 0.02, z + 0.017], "amber", 0)


def vine(mesh, icy, rng):
    for i in range(5):
        angle = i * 2.40
        x, z = 0.5 + 0.28 * math.cos(angle), 0.5 + 0.28 * math.sin(angle)
        length = (1 - i * 0.13) * rng.uniform(0.87, 0.98)
        for j in range(7):
            t = j / 7
            y = 1 - t * length
            cx = x + 0.058 * math.sin(4 * t + angle)
            cz = z + 0.055 * math.cos(5 * t + angle)
            mesh.box(
                [cx - 0.024, y - length / 7, cz - 0.024], [cx + 0.024, y, cz + 0.024],
                "rime" if icy else "stem", 0,
            )
            dx, dz = 0.069 * math.cos(j * 2.4 + angle), 0.069 * math.sin(j * 2.4 + angle)
            leaves = ("leaf", "amber") if j % 4 == 1 else ("moss", "leaf")
            column(
                mesh, cx + dx, cz + dz, 0.045, 0.027, 0.1, j, (dx * 0.5, dz * 0.5),
                ("snow", "rime") if icy else leaves, hanging=True, base=t * length,
            )


def mesh_prop(slug, number):
    mesh = Mesh("stone")
    rng = random.Random(number)
    textures = {"rime": "rime", "core": "clear", "snow": "dust", "moss": "moss", "leaf": "turf"}
    for key, tile in textures.items():
        roughness = 0.30 if key == "core" else 0.76
        mesh.materials[key] = ([1, 1, 1], f"{DIRECTORY}/{tile}_side.png", 0, roughness)
    pigments = {
        "stem": [0.22, 0.37, 0.11], "amber": [1, 0.65, 0.12], "petal white": [1, 0.95, 0.84],
        "petal pink": [0.93, 0.48, 0.65], "petal purple": [0.74, 0.46, 0.91],
    }
    for key, rgb in pigments.items():
        mesh.materials[key] = (rgb, None, 0, 0.72)
    crystal = slug in ("frost_cluster", "glacial_crystal_cluster")
    if crystal:
        crystals(mesh, slug, rng)
    elif slug == "cave_wildflowers":
        wildflowers(mesh, rng)
    else:
        vine(mesh, slug == "frost_vine", rng)
    emission = {"glow_vine": ("amber",), "glacial_crystal_cluster": ("core",)}
    maps = export(mesh, slug, emission.get(slug, ()))
    return dict(
        render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb", textures=maps,
        collision_boxes=[dict(minimum=[0.15, 0, 0.15], maximum=[0.85, 0.25, 0.85])] if crystal else [],
    )


def build():
    text = []
    for number, (slug, name, de, tile, rough, reflection) in enumerate(SURFACES, 471):
        description, translated = DESCRIPTIONS[slug]
        stone(number, slug, name, description, tile, rough, reflection,
              alpha=0.90 if tile == "clear" else 1, directory=DIRECTORY)
        path = BLOCKS / f"{slug}.json"
        definition = json.loads(path.read_text())
        definition.update(name=name, description=description)
        if tile in ("glow", "azure"):
            tint = [0.09, 0.058, 0.009] if tile == "glow" else [0.014, 0.045, 0.105]
            for mat in definition["materials"].values():
                mat["emission"] = tint
        save_json(path, definition)
        text.extend((dict(source=name, en=name, de=de),
                     dict(source=description, en=description, de=translated)))
    for number, (slug, name, de) in enumerate(PROPS, 483):
        description, translated = DESCRIPTIONS[slug]
        definition = dict(
            schema_version=1, id=number, name=name, description=description, creative_category="nature",
            struct_block=True, hardness=0.35, resistance=1, mining_level=0,
        )
        definition.update(mesh_prop(slug, number))
        if slug in ("glow_vine", "glacial_crystal_cluster"):
            tint = [1, 0.69, 0.19] if slug == "glow_vine" else [0.33, 0.73, 1]
            definition["light"] = dict(
                color=tint, range=7, intensity=640, falloff="soft_core",
                shadows=False, offset=[0.5, 0.48, 0.5],
            )
        save_json(BLOCKS / f"{slug}.json", definition)
        text.extend((dict(source=name, en=name, de=de),
                     dict(source=description, en=description, de=translated)))
    save_json(BLOCKS.parents[2] / "src-ui/locales/source/living-caves.json", text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lush")
    parser.add_argument("--ice")
    args = parser.parse_args()
    for path, table in [(args.lush, LUSH), (args.ice, ICE)]:
        if path:
            sample_swatches(path, BLOCKS / "textures" / DIRECTORY, swatches(table))
    build()
