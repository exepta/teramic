"""Build the Forest trail, leaf litter, windfall and understory at block scale."""
import math
import random
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

DATA = [
    ("woodland_litter", "Woodland Litter", "Waldhumus",
     "Beech leaves and moss over dark forest loam.", "Buchenlaub und Moos auf dunklem Waldboden."),
    ("woodland_trail", "Woodland Trail", "Waldpfad",
     "A narrow woodland tread scattered with pale stones.", "Ein schmaler Waldpfad mit hellen Kieseln."),
    ("forest_bramble", "Woodland Bramble", "Waldbrombeere",
     "Arching canes bear clusters of red forest berries.", "An den gebogenen Ruten hängen rote Waldbeeren."),
    ("windfall_twigs", "Windfall Twigs", "Windbruchzweige", "A forked bough lies across a smaller broken branch.",
     "Ein gegabelter Ast liegt über einem abgebrochenen Zweig."),
    ("cushion_moss", "Cushion Moss", "Polstermoos", "Soft pads of lime-green moss spread over the forest floor.",
     "Weiche grüne Moospolster breiten sich auf dem Waldboden aus."),
    ("woodland_stones", "Woodland Stones", "Waldsteine", "Lichen crusts grow on a tumble of grey stones.",
     "Flechten wachsen auf einer Gruppe grauer Steine."),
    ("tawny_milkcaps", "Tawny Milkcaps", "Rotbraune Milchlinge",
     "Little ochre caps open above a bed of leaf litter.", "Ockerfarbene Pilzhüte öffnen sich über dem Laub."),
    ("buttress_stump", "Buttressed Stump", "Wurzelstock", "Old roots flare from a mossy, splintered stump.",
     "Vom bemoosten, gesplitterten Stumpf gehen kräftige Wurzeln aus."),
]


def floor_texture(slug, face, folder):
    side = 256
    rng = np.random.default_rng(210 if face == "side" else 211)

    def noise(size):
        raw = Image.fromarray(rng.integers(0, 256, (size, size), dtype=np.uint8))
        # Periodic padded sampling keeps neighbouring copies continuous.
        padded = np.pad(np.asarray(raw), 1, mode="wrap")
        border = side // size
        enlarged = Image.fromarray(padded).resize(
            (side + 2 * border,) * 2, Image.Resampling.BILINEAR
        )
        tile = np.asarray(enlarged)[border:border + side, border:border + side]
        return tile.astype(float) / 127.5 - 1

    texture = noise(8) * 15 + noise(32) * 9 + noise(128) * 4 + noise(256) * 2
    moss = np.clip(noise(8) * 1.25 + noise(32) * .38, 0, 1)
    trail = slug == "woodland_trail"
    base = [125, 100, 70] if trail else [91, 83, 43]
    if face == "side":
        base = [101, 74, 46]
        moss *= np.maximum(0, 1 - np.arange(side)[:, None] / 28)
    rgb = np.array(base) + texture[:, :, None] + moss[:, :, None] * np.array([-4, 20, -3])
    img = Image.fromarray(rgb.clip(0, 255).astype(np.uint8))
    draw = ImageDraw.Draw(img)
    art = random.Random(56 + int(trail))
    for _ in range(165 if trail else 215):
        x, y = art.randrange(side), art.randrange(side)
        width = art.randrange(2, 9)
        rise = art.randrange(-6, 7)
        if face == "surface" and not trail and art.random() < .65:
            color = art.choice([(143, 101, 49), (126, 105, 55),
                                (108, 97, 52), (95, 95, 41), (113, 124, 49)])
        else:
            color = art.choice([(120, 108, 84), (104, 86, 65), (145, 125, 94), (129, 112, 80)])
        for dx in (-side, 0, side):
            for dy in (-side, 0, side):
                a, b = x + dx, y + dy
                draw.polygon([(a - width, b - rise), (a, b - 1),
                              (a + width, b + rise), (a, b + 2)], fill=color)
    name = slug + "_" + face
    img.save(folder / (name + ".png"), optimize=True)
    return "textures/forest/" + name + ".png"


def model(slug):
    mesh = Mesh("stone")
    pigments = {"twig": [.29, .16, .078], "bark": [.19, .109, .06], "cut": [.61, .39, .19],
                "rings": [.43, .26, .13], "moss": [.29, .43, .085], "moss_tip": [.46, .57, .13],
                "stone": [.45, .45, .40], "rock": [.34, .35, .32], "fern": [.21, .38, .08],
                "leaf_tip": [.40, .52, .13], "redberry": [.72, .09, .07], "wine": [.46, .055, .08],
                "cream": [.80, .69, .48], "cap": [.59, .30, .12], "ochre": [.77, .48, .23]}
    for name, rgb in pigments.items():
        mesh.materials[name] = (rgb, None, 0, .89)
    rng = random.Random(214)

    def pads(cx, cy, cz, r, n):
        for i in range(n):
            a = i * 2.39996
            d = r * (i / n)**.5
            w = r * (.34 + rng.random() * .17)
            ellipsoid(mesh, [cx + math.cos(a) * d, cy, cz + math.sin(a) * d],
                      [w, w * .34, w * .87], "moss_tip" if i % 3 == 0 else "moss", 7, 3)
    if slug == "forest_bramble":
        for i in range(7):
            a = i * 2.39996
            dx, dz = math.cos(a), math.sin(a)
            h = .44 + rng.random() * .16
            joint = [.5 + dx * .23, h, .5 + dz * .23]
            tip = [.5 + dx * .39, h - .13, .5 + dz * .39]
            cylinder(mesh, [.5 + dx * .09, 0, .5 + dz * .09], joint, .018, "twig", 5)
            cylinder(mesh, joint, tip, .016, "twig", 5, end_radius=.009)
            for j in range(5):
                t = j / 5
                side = (-1)**j * .08
                p = [joint[0] - dx * t * .19 + dz * side, h -
                     t * .27, joint[2] - dz * t * .19 - dx * side]
                # A folded blade has two coloured faces and a dark underside. Its ridge
                # points away from the cane, like the three leaflets of a bramble.
                center_x, y, center_z = p
                along_x, along_z = dz * (-1)**j, -dx * (-1)**j

                def blade_point(length, width, lift):
                    return [center_x + along_x * length - along_z * width, y + lift,
                            center_z + along_z * length + along_x * width]
                base = blade_point(-.065, 0, .008)
                peak = blade_point(.102, 0, -.018)
                ridge = blade_point(.013, 0, .036)
                for side_sign in [-1, 1]:
                    edge = blade_point(.006, side_sign * .051, 0)
                    tint = "leaf_tip" if (i + j) % 3 == 0 else "fern"
                    mesh.polygon([base, ridge, peak, edge], tint, [0, 1, 0])
                    mesh.polygon([base, edge, peak], "fern", [0, -1, 0])
            for j in range(3):
                ellipsoid(mesh, [tip[0] + .026 * math.cos(j * 2.1), tip[1] - .028 * j, tip[2] + .026 *
                                 math.sin(j * 2.1)], [.034, .036, .033], "redberry" if j != 2 else "wine", 6, 3)
    elif slug == "windfall_twigs":
        for a, b, w in [([.06, .043, .32], [.89, .071, .68], .045), ([.46, .06, .48], [.68, .17, .20], .028),
                        ([.61, .07, .57], [.77, .091, .8], .020), ([.20, .046, .77], [.78, .06, .37], .033)]:
            cylinder(mesh, a, b, w, "twig", 7, end_radius=w * .63)
            cylinder(mesh, b, [b[0] + .006, b[1] + .006, b[2]], w * .61, "cut", 7)
    elif slug == "woodland_stones":
        for i, (x, z, r) in enumerate([(.36, .48, .25), (.68, .58, .19), (.54, .25, .098), (.14, .72, .07), (.80, .29, .08)]):
            ellipsoid(mesh, [x, r * .68, z], [r, r * .68, r * .81],
                      "stone" if i % 2 == 0 else "rock", 7, 4)
            if i < 2:
                pads(x, r * 1.18, z, r * .50, 4)
    elif slug == "cushion_moss":
        for x, z, r in [(.35, .41, .21), (.70, .60, .16), (.33, .77, .11)]:
            pads(x, .038, z, r, 8)
    elif slug == "tawny_milkcaps":
        for i, (x, z, h) in enumerate([(.26, .36, .26), (.57, .59, .39), (.77, .38, .23), (.40, .74, .17)]):
            cylinder(mesh, [x, 0, z], [x + .018, h, z], h * .1, "cream", 7, end_radius=h * .084)
            cylinder(mesh, [x + .018, h - .024, z], [x + .018, h + .008, z],
                     h * .42, "cream", 9, end_radius=h * .50)
            cylinder(mesh, [x + .018, h + .008, z], [x + .006, h + .072, z],
                     h * .50, "ochre" if i % 2 else "cap", 9, end_radius=h * .19)
    else:
        cylinder(mesh, [.5, 0, .5], [.49, .57, .52], .23, "bark", 9, end_radius=.19)
        cylinder(mesh, [.49, .57, .52], [.49, .584, .52], .185, "cut", 9)
        cylinder(mesh, [.49, .584, .52], [.49, .59, .52], .102, "rings", 8)
        for i in range(7):
            a = i * math.tau / 7
            dx, dz = math.cos(a), math.sin(a)
            cylinder(mesh, [.5 + dx * .13, .32, .5 + dz * .13],
                     [.5 + dx * .42, .032, .5 + dz * .42], .10, "twig", 6, end_radius=.022)
            if i % 2 == 0:
                pads(.5 + dx * .23, .074, .5 + dz * .23, .1, 3)
    # Place the lowest point of the tilted branches and root flare on the block plane.
    floor = min(p[1] for part in mesh.parts.values() for p in part["position"])
    for part in mesh.parts.values():
        part["position"] = [[x, y - floor, z] for x, y, z in part["position"]]
        if any(not 0 <= coordinate <= 1 for point in part["position"] for coordinate in point):
            raise ValueError(f"{slug} extends outside its block")
    return export_voxels(mesh, slug)


def build():
    destination = BLOCKS / "textures" / "forest"
    destination.mkdir(exist_ok=True)
    entries = []
    for ident, (slug, en, de, desc, translated) in enumerate(DATA, 519):
        definition = dict(schema_version=1, id=ident, name=en, description=desc, creative_category="nature",
                          hardness=.42, resistance=.9, required_tool=None, mining_level=0)
        if ident < 521:
            definition.update(render_kind="cube", bevel_width=0, face_materials=["side", "side", "side", "surface", "side", "side"],
                              textures={face: floor_texture(slug, face, destination)
                                        for face in ("surface", "side")},
                              materials={face: dict(roughness=.93) for face in ("surface", "side")})
        else:
            boxes = [dict(minimum=[.27, 0, .28], maximum=[.70, .58, .72])] if ident == 526 else []
            definition.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                              textures=model(slug), collision_boxes=boxes)
            if not boxes:
                definition["struct_block"] = True
        save_json(BLOCKS / f"{slug}.json", definition)
        for source, german in [(en, de), (desc, translated)]:
            entries.append(dict(source=source, en=source, de=german))
    save_json(Path(__file__).resolve().parents[2] / "src-ui/locales/source/forest.json", entries)


if __name__ == "__main__":
    build()
