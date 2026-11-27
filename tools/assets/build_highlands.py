"""Build the firn, moraine, snowy canopy and wind-scoured undergrowth of the Highlands."""
from pathlib import Path
import math
import random

import numpy as np
from PIL import Image, ImageDraw
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from build_spirit_blossom import png
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

DATA = [
    ("firn", "Firn", "Firn", "Dense, pale grains of old mountain snow.", "Dichte, helle Körner aus altem Bergschnee."),
    ("icy_snow", "Icy Snow", "Eisiger Schnee", "Blue ice lenses shine through wind-packed snow.", "Blaue Eislinsen schimmern durch windgepressten Schnee."),
    ("alpine_granite", "Alpine Granite", "Alpengranit", "A jointed grey cliff face flecked with quartz.", "Eine zerklüftete graue Felswand mit Quarzeinschlüssen."),
    ("snow_covered_sod", "Snow-covered Sod", "Verschneite Grasnarbe", "A thin blanket of snow covers the alpine meadow.", "Eine dünne Schneedecke liegt auf der Bergwiese."),
    ("snowy_spruce_leaves", "Snowy Spruce Boughs", "Verschneite Fichtenzweige", "Snow rests on tiers of dark evergreen needles.", "Schnee liegt auf den Etagen dunkelgrüner Fichtennadeln."),
    ("winter_birch_branches", "Winter Birch Branches", "Winterbirkenzweige", "Rime traces each fork of the bare birch crown.", "Raureif zeichnet jede Gabel der kahlen Birkenkrone nach."),
    ("alpine_conifer", "Young Alpine Conifer", "Junge Bergtanne", "A little conifer carries its first winter snow.", "Eine kleine Bergtanne trägt ihren ersten Winterschnee."),
    ("dead_winter_bush", "Dead Winter Bush", "Kahler Winterbusch", "A fan of bare, weathered stems.", "Ein Fächer kahler, verwitterter Triebe."),
    ("frozen_shrub", "Frozen Shrub", "Bereifter Strauch", "White rime and a few red berries cling to the twigs.", "Weißer Reif und einzelne rote Beeren hängen an den Zweigen."),
    ("alpine_icicles", "Alpine Icicles", "Alpen-Eiszapfen", "Tapered blue icicles hang from a sheltered branch.", "Blaue Eiszapfen hängen an einem geschützten Ast."),
    ("moraine_rocks", "Moraine Rocks", "Moränengeröll", "A cluster of grey, snow-capped boulders.", "Eine Gruppe grauer, schneebedeckter Felsbrocken."),
    ("snow_mound", "Snow Mound", "Schneewehe", "A wind-rounded mound with soft overlapping edges.", "Eine vom Wind gerundete Wehe mit weichen Rändern."),
    ("pine_cones", "Pine Cones", "Tannenzapfen", "Copper-brown cone scales gather under the firs.", "Kupferbraune Zapfen liegen unter den Tannen."),
    ("frosted_tussock", "Frosted Grass Tuft", "Bereifter Grasbüschel", "Hoarfrost whitens the tips of golden alpine grass.", "Raureif färbt die Spitzen des goldenen Berggrases weiß."),
    ("mountain_flower_remnants", "Mountain Flower Remnants", "Bergblumenreste", "A few ochre and violet seed heads survived the frost.", "Ockerfarbene und violette Samenstände überdauern den Frost."),
]


def texture(slug, face):
    size = 256
    y, x = np.mgrid[0:size, 0:size]
    rng = np.random.default_rng(3184)
    grain = rng.normal(0, 4, (size, size))
    broad = 4 * np.cos(2 * math.pi * (x + y) / size) + 3 * np.sin(4 * math.pi * x / size)
    tint = np.ones((size, size, 3)) * [228, 235, 242]
    if slug == "alpine_granite":
        tint[:] = [111, 115, 129]
        seams = np.sin(2 * math.pi * (x * 3 + y) / size + np.sin(4 * math.pi * y / size) * 1.8)
        broad += np.where(seams > .94, -28, 0) + np.where(seams < -.92, 18, 0)
        grain *= 2.5
    elif slug == "icy_snow":
        veins = np.cos(2 * math.pi * (x + 2 * y) / size + np.sin(6 * math.pi * x / size))
        tint += np.clip(veins, 0, 1)[:, :, None] * [-46, -23, -5]
    elif slug == "snow_covered_sod":
        def turf_tile(name):
            with Image.open(BLOCKS / f"textures/{name}.png") as sheet:
                return np.asarray(sheet.convert("RGB").resize((size, size), Image.Resampling.LANCZOS))

        soil = turf_tile("dirt")
        grass = turf_tile("grass") * [.89, .86, .80] + [12, 12, 16]
        drift = (np.sin(2 * math.pi * (2*x + y) / size)
                 + .55 * np.cos(2 * math.pi * (x - 2*y) / size)
                 + .25 * np.cos(2 * math.pi * (5*x + y) / size))
        tint += drift[:, :, None] * [3.8, 3.2, 2.3]
        if face == "side":
            # Both the detailed cube and the greedy mesh use world Y for V:
            # the *last* PNG scanlines meet the snow cap.
            depth = size - 1 - y
            rim = size * (.17 + .029 * np.sin(4 * math.pi * x / size)
                          + .028 * np.cos(10 * math.pi * x / size)
                          + .012 * np.sin(18 * math.pi * x / size))
            turf_depth = rim + 15 + 9 * np.sin(6 * math.pi * x / size)
            tint[depth > rim] = grass[depth > rim]
            tint[depth > turf_depth] = soil[depth > turf_depth]
            # A cool shadow just under the snow lip gives the cap a readable edge.
            tint[(depth >= rim - 3) & (depth <= rim)] *= [.83, .88, .93]
            grain *= .35
        else:
            # Sparse short blades break through the thinnest powder.
            tips = (drift > 1.58) & (grass[:, :, 1] > 116)
            tint[tips] = .56 * tint[tips] + .44 * grass[tips]
    else:
        broad += 4 * np.sin(2 * math.pi * (y * 6 + x) / size) + (grain < -4) * -5
    rgb = np.clip(tint + (grain + broad)[:, :, None], 0, 255).astype(np.uint8)
    destination = f"textures/highlands/{slug}_{face}.png"
    Image.fromarray(rgb).save(BLOCKS / destination, optimize=True)
    return destination


def winter_birch_atlas():
    """Forking bare branches with a crown of hoarfrost, one whole spray per UV cell."""
    atlas = Image.new("RGBA", (512, 512), (116, 106, 102, 0))
    for motif in range(4):
        cell = Image.new("RGBA", (256, 256), (116, 106, 102, 0))
        pen = ImageDraw.Draw(cell)
        rng = random.Random(9001 + motif)
        lean = rng.uniform(-16, 16)

        def branch(start, end, width):
            pen.line((start, end), fill=(105, 88, 85, 255), width=width)
            frost = [(x - 1, y - 2) for x, y in (start, end)]
            pen.line(frost, fill=(208, 220, 227, 255), width=max(2, width - 4))

        def within_crown(x, y):
            # The leaf card has a pointed tip; keep an entire bud inside either sloping edge.
            return (x, max(y, 12 + .50 * abs(x - 128)))

        def axis(y):
            return 128 + lean * (226 - y) / 200 + 3 * math.sin(y / 42)

        branch((axis(226), 226), (axis(25), 25), 11)
        for tier, y in enumerate((200, 172, 143, 115, 86, 60)):
            for side in (-1, 1):
                base = (axis(y), y + rng.uniform(-7, 7))
                spread = 82 * math.sin((tier + 1) * math.pi / 7) + rng.uniform(4, 16)
                tip = within_crown(base[0] + side * spread, base[1] - rng.uniform(30, 46))
                branch(base, tip, 10)
                for fraction in (.36, .59, .79):
                    fork = tuple(a + (b - a) * fraction for a, b in zip(base, tip))
                    reach = 27 if tier < 4 else 15
                    end = within_crown(fork[0] + side * rng.uniform(4, 17), fork[1] - reach)
                    branch(fork, end, 8)
                    # Just a few plump winter buds remain on the ends.
                    x, y_tip = end
                    pen.ellipse((x - 4, y_tip - 5, x + 3, y_tip + 4), (170, 158, 150, 255))
        atlas.paste(cell, (motif % 2 * 256, motif // 2 * 256))
    return atlas


def far_crown(atlas, fir, face):
    """Layer individual sprays into a full cube face, preserving the gaps between branches."""
    rng = random.Random(724 + (face == "top"))
    base = Image.new("RGBA", (512, 512), (90, 104, 91, 0))
    for y in range(-50, 660, 96):
        for x in range(-30, 650, 120):
            motif = rng.randrange(2 if fir else 4)
            left, top = motif % 2 * 256, motif // 2 * 256
            twig = atlas.crop((left, top, left + 256, top + 256))
            angle = rng.uniform(-45, 45) + (rng.randrange(4) * 90 if face == "top" else 0)
            twig = twig.rotate(angle, resample=Image.Resampling.BICUBIC)
            base.alpha_composite(twig, (x - 110 + rng.randrange(-22, 23), y - 110))
    return base


def snow_crown(slug, face):
    fir = slug == "snowy_spruce_leaves"
    if fir:
        atlas = Image.open(BLOCKS / "textures/foliage/spruce_cluster.png").convert("RGBA")
    else:
        atlas = winter_birch_atlas()
    image = atlas if face == "cluster" else far_crown(atlas, fir, face)
    pixels = np.array(image)
    h, w = pixels.shape[:2]
    y, x = np.mgrid[:h, :w]
    rng = np.random.default_rng(406 if fir else 711)
    grain = rng.normal(0, 2.8, (h, w, 3))
    if fir and face != "cluster":
        # The far mesh is a solid bough mass. Tiny needle gaps and isolated
        # white flecks alias against sky when a whole cube covers a few pixels.
        # Keep individual needles on the near spray; paint broad bough tiers
        # and settled shelves of snow into the distant mass instead.
        u, v = x / w, y / h
        relief = (np.sin(math.tau * (2*u + v)) + .45*np.cos(math.tau * (u - 3*v))) / 1.45
        tier = np.cos(math.tau * (2*v + .18*np.cos(math.tau*u) + .09*relief))
        if face == "top":
            drift = np.clip(.72 + .28*relief + .12*tier, 0, 1)
        else:
            drift = np.clip((tier + .30*relief - .43) * 1.6, 0, .86)
        evergreen = np.array([48,76,65]) + relief[:,:,None] * [10,14,10]
        colours = evergreen + (np.array([210,224,228]) - evergreen) * drift[:,:,None]
        pixels[:,:,3] = 255
        grain *= .42
    elif fir:
        value = pixels[:, :, :3].mean(2) / 255
        light = np.clip((value - .17) * 2.7, 0, 1)
        sweeps = np.cos(y / h * math.tau * 3 + 1.8 * np.sin(x / w * math.tau * 2))
        snow = np.clip(light + .38 * sweeps + (.30 if face == "top" else -.15), 0, 1)
        dark = np.array([30, 59, 45])
        colours = dark + (np.array([226, 235, 244]) - dark) * snow[:, :, None]
    else:
        colours = pixels[:, :, :3].astype(float)
    pixels[:, :, :3] = np.clip(colours + grain, 0, 255).astype(np.uint8)
    # Keep pigment in the transparent gutter to avoid dark edges after texture filtering.
    pixels[pixels[:, :, 3] == 0, :3] = [88, 115, 99] if fir else [116, 106, 102]
    dest = f"textures/foliage/{slug}_{face}.png"
    png(BLOCKS / dest, pixels.reshape(-1, 4).tolist(), w)
    return dest


def model(slug):
    mesh = Mesh("stone")
    for key, rgb in {
        "snow": [.91, .946, .985], "shade": [.63, .75, .84], "needles": [.075, .20, .13],
        "twig": [.32, .22, .16], "rime": [.78, .87, .93], "berry": [.58, .15, .15],
        "ice": [.35, .68, .88], "stone": [.39, .41, .46], "scale": [.54, .33, .17],
        "straw": [.60, .54, .38], "ochre": [.76, .57, .23], "purple": [.56, .35, .50]
    }.items():
        mesh.materials[key] = (rgb, None, 0, .3 if key == "ice" else .90)
    rng = random.Random(3184)

    def snowy_rock(x, z, r, h, material):
        ellipsoid(mesh, [x, h * .48, z], [r, h * .60, r * .83], material, 7, 4)
        ellipsoid(mesh, [x - .01, h * .84, z], [r * .82, h * .30, r * .71], "snow", 7, 3)
    if slug == "alpine_conifer":
        cylinder(mesh, [.5, .01, .5], [.5, .88, .5], .035, "twig", 6, end_radius=.012)
        for i in range(4):
            y = .13 + i * .17
            r = .33 - i * .063
            cylinder(mesh, [.5, y, .5], [.5, y + .31, .5], r, "needles", 7, end_radius=.008)
            cylinder(mesh, [.5, y + .066, .5], [.5, y + .316, .5], r * .8, "snow", 7, end_radius=.005)
    elif slug in ("dead_winter_bush", "frozen_shrub", "mountain_flower_remnants"):
        frosted = slug == "frozen_shrub"
        for i in range(9):
            a = i * 2.39996
            dx, dz = math.cos(a), math.sin(a)
            height = .27 + rng.random() * .39
            tip = [.5 + dx * (.15 + rng.random() * .19), height, .5 + dz * (.15 + rng.random() * .19)]
            mid = [.5 + dx * .11, height * .51, .5 + dz * .11]
            fork = [mid[0] - dz * .12, height * .83, mid[2] + dx * .12]
            cylinder(mesh, [.5, .012, .5], mid, .02, "twig", 5, end_radius=.013)
            for end in [tip, fork]:
                cylinder(mesh, mid, end, .013, "rime" if frosted else "twig", 5, end_radius=.004)
                if slug == "mountain_flower_remnants" or (frosted and i % 3 == 0):
                    color = ("purple" if i % 3 == 0 else "ochre") if not frosted else "berry"
                    ellipsoid(mesh, end, [.034, .046, .031], color, 6, 3)
    elif slug == "alpine_icicles":
        ellipsoid(mesh, [.5, .94, .5], [.37, .068, .19], "rime", 8, 3)
        for i in range(8):
            x = .19 + .087 * i
            z = .50 + .07 * math.sin(i * 1.7)
            cylinder(mesh, [x, .95, z], [x, .14 + rng.random() * .57, z], .041 + rng.random() * .036, "ice", 6, end_radius=.002)
    elif slug in ("moraine_rocks", "snow_mound"):
        for x, z, r, h in [(.33, .42, .27, .39), (.65, .60, .27, .27), (.20, .71, .13, .16), (.72, .28, .12, .14)]:
            snowy_rock(x, z, r, h, "stone" if slug == "moraine_rocks" else "shade")
    elif slug == "pine_cones":
        for x, z, h in [(.29, .42, .25), (.64, .60, .34), (.62, .27, .17)]:
            ellipsoid(mesh, [x, h * .50, z], [h * .23, h * .5, h * .23], "twig", 6, 4)
            for i in range(13):
                a = i * 2.4
                t = (i + 1) / 14
                rr = h * .26 * math.sin(t * math.pi)
                ellipsoid(mesh, [x + rr * math.cos(a), h * t, z + rr * math.sin(a)], [h * .115, h * .14, h * .11], "scale", 5, 3)
    else:
        for i in range(17):
            a = i * 2.39996
            dx, dz = math.cos(a), math.sin(a)
            h = .21 + rng.random() * .28
            mid = [.5 + dx * .16, h * .71, .5 + dz * .16]
            tip = [.5 + dx * .34, h, .5 + dz * .34]
            cylinder(mesh, [.5 + dx * .055, .01, .5 + dz * .055], mid, .012, "straw", 4, end_radius=.008)
            cylinder(mesh, mid, tip, .008, "rime", 4, end_radius=.001)
    # Ground props rest on the block plane; icicles touch the ceiling instead.
    vertices = [p for part in mesh.parts.values() for p in part["position"]]
    offset = 1 - max(p[1] for p in vertices) if slug == "alpine_icicles" else -min(p[1] for p in vertices)
    for part in mesh.parts.values():
        part["position"] = [[x, y + offset, z] for x, y, z in part["position"]]
    return export_voxels(mesh, slug)


def build():
    (BLOCKS / "textures/highlands").mkdir(exist_ok=True)
    locale = []
    for ident, (slug, en, de, description, translation) in enumerate(DATA, 581):
        block = dict(schema_version=1, id=ident, name=en, description=description,
                     creative_category="nature", mining_level=0, required_tool=None)
        if ident <= 586:
            granite = slug == "alpine_granite"
            foliage = ident >= 585
            block.update(render_kind="cube", bevel_width=0.0,
                         hardness=1.7 if granite else .4, resistance=2 if granite else .6)
            draw = snow_crown if foliage else texture
            textures = {face: draw(slug, face) for face in ("side", "top")}
            bottom = "side"
            if slug == "snow_covered_sod":
                bottom = "soil"
                textures[bottom] = "textures/dirt.png"
            if foliage:
                textures["spray"] = snow_crown(slug, "cluster")
                block["foliage_cluster"] = {"material": "spray"}
                if slug == "snowy_spruce_leaves":
                    block["foliage_cluster"]["needles"] = "spray"
            materials = {
                key: dict(base_color=[1, 1, 1, 1], roughness=.91, reflectance=.08)
                for key in textures
            }
            block.update(textures=textures, materials=materials,
                         face_materials=["side", "side", bottom, "top", "side", "side"])
        else:
            block.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                         collision_boxes=[], struct_block=True, hardness=.25, resistance=.4,
                         textures=model(slug))
        save_json(BLOCKS / f"{slug}.json", block)
        for source, german in [(en, de), (description, translation)]:
            locale.append(dict(source=source, en=source, de=german))
    root = Path(__file__).resolve().parents[2]
    save_json(root / "src-ui/locales/source/highlands.json", locale)


if __name__ == "__main__":
    build()
