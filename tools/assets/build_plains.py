"""Author the metre-scale meadow flowers and tileable materials of Plains settlements."""
import math
from pathlib import Path

import numpy as np
from PIL import Image

from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid, leaf

DATA = [
    ("meadow_sunflowers", "Meadow Sunflowers", "Wiesensonnenblumen", "Ochre seed heads surrounded by golden ray florets.", "Goldgelbe Blütenblätter umrahmen die ockerfarbenen Samenstände."),
    ("rose_foxgloves", "Rose Foxgloves", "Rosa Fingerhut", "Rose and cream bells climb each side of the slender stems.", "Rosafarbene und cremeweiße Glocken reihen sich an den schlanken Stängeln."),
    ("blue_slate_shingles", "Blue Slate Shingles", "Blaue Schieferschindeln", "Overlapping blue slate from the roofs of Plains farms.", "Überlappender blauer Dachschiefer von den Höfen der Ebenen."),
    ("bound_hay_bale", "Bound Hay Bale", "Gebundener Heuballen", "Dried golden stalks cinched tight with two rope bands.", "Goldgelbes Heu, von zwei Seilen fest zusammengehalten."),
    ("meadow_limestone", "Meadow Limestone", "Wiesenkalkstein", "Pale weathered limestone from ledges above the river.", "Heller, verwitterter Kalkstein von den Felsbänken über dem Fluss."),
]


def flower_mesh(slug):
    mesh = Mesh("stone")
    colors = {"honey": [.98, .53, .042], "saffron": [1, .78, .06], "seeds": [.29, .13, .044],
              "pink": [.89, .24, .45], "rose": [.98, .53, .64], "cream": [1, .9, .77]}
    for name, color in colors.items():
        mesh.materials[name] = (color, None, 0, .85)
    for i, (x, z, height) in enumerate([(.34, .40, .78), (.71, .64, .63), (.60, .23, .48)]):
        cylinder(mesh, [x, 0, z], [x, height, z], .021, "leaf", 6, end_radius=.013)
        for j in range(4):
            y = .12 + j * .083
            side = (-1) ** (j + i)
            joint = [x + side * .082, y + .03, z + .031]
            cylinder(mesh, [x, y, z], joint, .012, "leaf", 5)
            leaf(mesh, joint, .11, "leaf_light")
        if slug == "meadow_sunflowers":
            radius = .116 - .012 * i
            angle = (i - 1) * 1.45
            tangent = math.cos(angle), -math.sin(angle)
            for j in range(13):
                a = math.tau * j / 13
                dx, dy = math.cos(a), math.sin(a)
                def petal_point(radial, lateral):
                    s = radial * dx - lateral * dy
                    return [x + tangent[0] * s, height + radial * dy + lateral * dx, z + tangent[1] * s]
                mesh.polygon([petal_point(radius * .3, 0), petal_point(radius * 1.06, .028),
                              petal_point(radius * 1.51, 0), petal_point(radius, -.028)],
                             ("honey", "saffron")[j % 2])
            ellipsoid(mesh, [x, height, z], [radius * .61, radius * .64, radius * .57], "seeds", 9, 5)
        else:
            for j in range(9):
                a = i + j * 2.4
                radius = .038 * (1 - j * .05)
                y = height - (8 - j) * .038
                bell = [x + .075 * math.cos(a), y + .02, z + .075 * math.sin(a)]
                cylinder(mesh, [x, y, z], bell, .012, "leaf", 5)
                lip = [bell[0] + .026 * math.cos(a), y - .042, bell[2] + .026 * math.sin(a)]
                cylinder(mesh, bell, lip, radius * .54, ("rose", "pink")[j % 2], 7, end_radius=radius)
                ellipsoid(mesh, lip, [radius * .74, .008, radius * .74], "cream", 7, 3)
    return export_voxels(mesh, slug)


def tile(slug, face, directory):
    size = 256
    rng = np.random.default_rng(628)
    y, x = np.mgrid[0:size, 0:size] / size
    def grain(nx, ny):
        values = rng.standard_normal((ny, nx))
        ix, iy = (x * nx).astype(int), (y * ny).astype(int)
        fx, fy = (x * nx) % 1, (y * ny) % 1
        return ((values[iy, ix] * (1 - fx) + values[iy, (ix + 1) % nx] * fx) * (1 - fy) +
                (values[(iy + 1) % ny, ix] * (1 - fx) + values[(iy + 1) % ny, (ix + 1) % nx] * fx) * fy)
    pores = grain(115, 115)
    vein = grain(22, 13)
    if slug == "blue_slate_shingles":
        row = np.floor(y * 4)
        joint = (x * 4 + (row % 2) * .5 + .014 * vein) % 1
        border = np.minimum(joint, 1 - joint)
        overlap = (y * 4 + .025 * grain(14, 4)) % 1
        shade = (np.minimum(overlap * 2, 1) * 8 - np.exp(-border * 55) * 27 -
                 np.exp(-overlap * 24) * 22 + grain(8, 4) * 6 + pores * 2 + vein * 3)
        rgb = [74, 103, 129]
    elif slug == "bound_hay_bale":
        warp = .045 * grain(10, 7)
        fibre = np.sin(math.tau * (y * 33 + warp * 2)) + .5 * grain(5, 64)
        ties = (x * 2 + .5 + .009 * grain(4, 16)) % 1
        band = np.minimum(ties, 1 - ties) < .058
        shade = np.where(band, -38 + 8 * np.sin(math.tau * (y * 29 + ties * 9)),
                         fibre * 11 + vein * 6 + pores * 3)
        if face == "end":
            shade = grain(75, 63) * 14 + grain(9, 10) * 7
        rgb = [192, 151, 64]
    else:
        strata = grain(8, 6) * .7 + grain(17, 8) * .3
        seam = np.exp(-np.abs(strata - .17) * 21)
        shade = vein * 5 + pores * 4 + grain(6, 5) * 7 - seam * 18
        rgb = [169, 166, 140]
    texture = np.clip(np.asarray(rgb) + shade[:, :, None], 0, 255).astype("uint8")
    name = f"{slug}_{face}"
    Image.fromarray(texture).save(directory / f"{name}.png", optimize=True)
    nx = (np.roll(shade, 1, axis=1) - np.roll(shade, -1, axis=1)) / 46
    ny = (np.roll(shade, 1, axis=0) - np.roll(shade, -1, axis=0)) / 46
    normal = np.stack([nx, ny, np.ones_like(nx)], axis=2)
    normal /= np.linalg.norm(normal, axis=2)[:, :, None]
    Image.fromarray(np.round((normal * .5 + .5) * 255).astype("uint8")).save(directory / f"{name}_normal.png", optimize=True)
    return "textures/plains/" + name


def build():
    directory = BLOCKS / "textures" / "plains"
    directory.mkdir(exist_ok=True)
    locale = []
    for number, (slug, en, de, desc, translation) in enumerate(DATA, 514):
        definition = dict(schema_version=1, id=number, name=en, description=desc,
                          creative_category="nature", hardness=.15 if number < 516 else 1.5,
                          resistance=.3 if number < 516 else 3.0, required_tool=None, mining_level=0)
        if number < 516:
            definition.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                              textures=flower_mesh(slug), collision_boxes=[], struct_block=True)
        else:
            faces = ["surface"] * 6
            if number == 517:
                faces[0] = faces[1] = "end"
            patterns = {f: tile(slug, f, directory) for f in sorted(set(faces))}
            definition.update(render_kind="cube", bevel_width=0, face_materials=faces,
                              textures={key: path + ".png" for key, path in patterns.items()},
                              materials={key: dict(roughness=.87, normal_texture=path + "_normal.png") for key, path in patterns.items()})
        save_json(BLOCKS / f"{slug}.json", definition)
        for source, german in ((en, de), (desc, translation)):
            locale.append(dict(source=source, en=source, de=german))
    save_json(Path(__file__).resolve().parents[2] / "src-ui/locales/source/plains.json", locale)


if __name__ == "__main__":
    build()
