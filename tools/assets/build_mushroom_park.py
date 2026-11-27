"""Build the Mushroom Park woodland materials and metre-scale props."""
import json
import math
from pathlib import Path

from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from mushroom_surfaces import build_textures
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid, leaf

DIRECTORY = "mushroom_park"
# id, slug, English, German, English description, German description
BLOCKS_DATA = [
    (488, "mushroom_gill", "Parasol Gills", "Schirmlamellen",
     "Pleated ivory tissue beneath a giant cap.", "Helle gefaltete Lamellen unter einem Riesenpilzhut."),
    (489, "golden_spore_pods", "Golden Spore Pods", "Goldene Sporenkapseln",
     "Honey-colored lanterns on forked stalks.", "Honigfarbene Leuchtkapseln an gegabelten Stielen."),
    (490, "pond_blossoms", "Pond Blossoms", "Teichblüten",
     "Pink water lilies on a pair of notched pads.", "Rosa Seerosen auf eingekerbten Schwimmblättern."),
    (491, "mossy_nurse_log", "Hollow Nurse Log", "Bemoostes Totholz",
     "A hollow log bearing moss and little amber shelf fungi.", "Ein hohler Stamm mit Moos und bernsteinfarbenen Baumpilzen."),
    (504, "brown_mushroom_cap", "Chestnut Mushroom Cap", "Kastanienpilzhut",
     "Chestnut skin from a giant woodland mushroom.", "Kastanienbraune Haut eines Riesenpilzhuts."),
    (505, "mycelium_turf", "Mycelium Turf", "Myzelrasen",
     "Lilac mycelium intertwined with woodland turf.", "Fliederfarbenes Myzel im Waldboden."),
    (506, "spore_soil", "Wet Spore Soil", "Feuchte Sporenerde",
     "Dark, damp humus flecked with rose spores.", "Dunkler, feuchter Humus mit rosigen Sporen."),
    (507, "glow_mushroom_cap", "Luminous Lilac Cap", "Leuchtender Fliederhut",
     "Soft amber light glimmers between lilac scales.", "Zwischen fliederfarbenen Schuppen schimmert warmes Licht."),
    (508, "woodland_toadstools", "Woodland Toadstools", "Waldpilzgruppe",
     "A mixed cluster of spotted red caps and small brown boletes.", "Eine Gruppe roter Fliegenpilze und kleiner brauner Röhrlinge."),
    (509, "amber_shelf_stump", "Shelf Mushroom Stump", "Baumpilzstumpf", "Amber brackets climb an old mossy stump.",
     "Bernsteinfarbene Baumpilze wachsen an einem alten, bemoosten Stumpf."),
    (510, "exposed_mycelial_roots", "Exposed Mycelial Roots", "Freiliegende Myzelwurzeln",
     "Buttressed roots break through the moss.", "Kräftige Wurzeln durchbrechen die Moosdecke."),
    (511, "mushroom_moss_tuft", "Flowering Moss Tuft", "Blühendes Moospolster",
     "Tiny daisies and fern shoots spring from a moss carpet.", "Kleine Gänseblümchen und Farntriebe auf einem Moospolster."),
    (512, "park_cedar_shingles", "Weathered Cedar Shingles", "Verwitterte Zedernschindeln",
     "Overlapping cedar shakes from mushroom-grove lookouts.", "Überlappende Zedernschindeln von den Pavillons im Pilzwald."),
    (513, "park_timber", "Grove Timber", "Hainbalken", "Weathered timber from the bridges and pavilions of Mushroom Park.",
     "Verwittertes Bauholz der Brücken und Pavillons im Pilzpark."),
]
COLORS = {
    "cream": [0.97, 0.81, 0.55], "ivory": [1, 0.94, 0.78], "stalk": [0.74, 0.57, 0.34],
    "scarlet": [0.85, 0.13, 0.13], "ruby": [0.56, 0.046, 0.035], "chestnut": [0.56, 0.26, 0.10],
    "honey": [1, 0.65, 0.15], "spore": [1, 0.83, 0.33], "plum": [0.74, 0.38, 0.88],
    "bark": [0.30, 0.155, 0.067], "bark_light": [0.48, 0.28, 0.13],
    "petal": [1, 0.61, 0.75], "bud": [0.85, 0.91, 0.44],
}


def toadstool(mesh, x, y, z, height, radius, skin):
    cylinder(mesh, [x, y, z], [x + 0.12 * radius, y + height, z],
             radius * 0.22, "cream", 7, end_radius=radius * 0.16)
    x += 0.12 * radius
    cylinder(mesh, [x, y + height - radius * 0.13, z], [x, y + height, z],
             radius * 0.8, "ivory", 12, end_radius=radius)
    cylinder(mesh, [x, y + height, z], [x, y + height + radius * 0.34, z],
             radius, skin, 12, end_radius=radius * 0.72)
    cylinder(mesh, [x, y + height + radius * 0.34, z], [x, y + height + radius * 0.48, z],
             radius * 0.72, skin, 12, end_radius=radius * 0.27)
    if skin in ("scarlet", "plum"):
        for i in range(5):
            angle = i * math.tau / 5 + 0.3
            spot = [x + math.cos(angle) * radius * 0.56, y + height + radius * 0.38,
                    z + math.sin(angle) * radius * 0.56]
            ellipsoid(mesh, spot, [radius * 0.13, radius * 0.047, radius * 0.12], "ivory", 6, 3)


def bracket(mesh, x, y, z, radius):
    for dy, scale, color in [(0, 1, "ivory"), (0.025, 0.94, "chestnut"), (0.040, 0.67, "honey")]:
        ellipsoid(mesh, [x, y + dy, z], [radius * scale, 0.029, radius * 0.76 * scale], color, 10, 4)


def foliage(mesh):
    for i in range(23):
        angle = i * 2.399
        radius = 0.35 * (i / 22)**0.5
        x, z = 0.5 + math.cos(angle) * radius, 0.5 + math.sin(angle) * radius
        ellipsoid(mesh, [x, 0.065, z], [0.13, 0.077, 0.11], "moss" if i % 3 else "leaf_light", 6, 4)
        if i % 3:
            continue
        tip = [x + 0.07 * math.cos(angle), 0.20 + 0.13 * (i % 7) / 6, z + 0.08 * math.sin(angle)]
        cylinder(mesh, [x, 0.05, z], tip, 0.012, "leaf", 5, end_radius=0.006)
        for j in range(4):
            petal_angle = j * math.tau / 4
            petal = [tip[0] + 0.034 * math.cos(petal_angle), tip[1],
                     tip[2] + 0.034 * math.sin(petal_angle)]
            ellipsoid(mesh, petal, [0.032, 0.021, 0.029], "ivory", 6, 3)
        ellipsoid(mesh, tip, [0.021, 0.024, 0.021], "honey", 6, 3)


def prop(slug):
    mesh = Mesh("stone")
    for name, color in COLORS.items():
        mesh.materials[name] = (color, None, 0, 0.79)
    if slug == "woodland_toadstools":
        for x, z, height, radius, skin in [(0.33, 0.40, 0.52, 0.24, "scarlet"), (0.67, 0.61, 0.36, 0.19, "scarlet"),
                                           (0.26, 0.77, 0.20, 0.14, "chestnut"), (0.72, 0.28, 0.19, 0.12, "chestnut")]:
            toadstool(mesh, x, 0, z, height, radius, skin)
    elif slug == "golden_spore_pods":
        for x, z, height, radius in [(0.33, 0.35, 0.53, 0.135), (0.53, 0.59, 0.77, 0.16), (0.76, 0.39, 0.39, 0.11)]:
            base = [0.50, 0.025, 0.48]
            joint = [x - 0.035, height * 0.62, z + 0.045]
            tip = [x, height, z]
            cylinder(mesh, base, joint, 0.028, "stalk", 7, end_radius=0.022)
            cylinder(mesh, joint, tip, 0.022, "cream", 7, end_radius=0.015)
            ellipsoid(mesh, tip, [radius, radius * 1.12, radius], "honey", 10, 5)
            for i in range(7):
                angle = i * math.tau / 7
                offset = [x + radius * 0.85 * math.cos(angle), height + 0.03, z + radius * 0.85 * math.sin(angle)]
                ellipsoid(mesh, offset, [radius * 0.34, radius * 0.52, radius * 0.34], "spore", 6, 4)
            leaf(mesh, [joint[0], joint[1] - 0.015, joint[2] - 0.04], 0.13, "leaf_light")
    elif slug == "pond_blossoms":
        for x, z, radius in [(0.33, 0.46, 0.27), (0.69, 0.65, 0.23)]:
            for i in range(1, 14):  # leave a radial notch revealing the water
                angle, next_angle = i * math.tau / 15, (i + 1) * math.tau / 15
                petal_color = "leaf_light" if i % 4 == 0 else "leaf"
                mesh.polygon([[x, 0.023, z], [x + radius * math.cos(angle), 0.016, z + radius * math.sin(angle)],
                              [x + radius * math.cos(next_angle), 0.016, z + radius * math.sin(next_angle)]],
                             petal_color, [0, 1, 0])
        for i in range(8):
            angle = i * math.tau / 8
            petal = [0.35 + 0.064 * math.cos(angle), 0.067, 0.46 + 0.064 * math.sin(angle)]
            ellipsoid(mesh, petal, [0.050, 0.060, 0.052], "petal", 7, 4)
        ellipsoid(mesh, [0.35, 0.082, 0.46], [0.045, 0.050, 0.045], "honey", 8, 4)
    elif slug == "mossy_nurse_log":
        # Separate weathered staves leave a visible opening through the trunk.
        for i in range(11):
            angle = i * math.tau / 11
            y, z = 0.22 + 0.155 * math.cos(angle), 0.5 + 0.195 * math.sin(angle)
            cylinder(mesh, [0.03 + 0.018 * (i % 3), y, z], [0.97 - 0.021 * (i % 4), y + 0.04, z + 0.04],
                     0.078, "moss" if i in (0, 1, 10) else ("bark", "bark_light")[i % 2], 6, end_radius=0.081)
        for i in range(4):
            bracket(mesh, 0.18 + 0.18 * i, 0.23 + 0.045 * (i % 2), 0.31, 0.09)
    elif slug == "amber_shelf_stump":
        for i in range(9):
            angle = i * math.tau / 9
            x, z = 0.50 + 0.17 * math.cos(angle), 0.50 + 0.15 * math.sin(angle)
            cylinder(mesh, [x, 0, z], [x, 0.70 + 0.12 * math.sin(3 * angle), z], 0.11,
                     "bark" if i % 2 else "bark_light", 6, end_radius=0.09)
            ellipsoid(mesh, [x, 0.69 + 0.12 * math.sin(3 * angle), z], [0.095, 0.06, 0.09], "moss", 7, 4)
        for i in range(6):
            angle = i * 2.2
            bracket(mesh, 0.5 + 0.24 * math.cos(angle), 0.13 + 0.09 * i,
                    0.5 + 0.23 * math.sin(angle), 0.19)
    elif slug == "exposed_mycelial_roots":
        cylinder(mesh, [0.5, 0, 0.5], [0.52, 0.39, 0.5], 0.19, "bark_light", 8, end_radius=0.15)
        for i in range(7):
            angle = i * math.tau / 7
            joint = [0.5 + 0.24 * math.cos(angle), 0.19, 0.5 + 0.24 * math.sin(angle)]
            end = [0.5 + 0.47 * math.cos(angle + 0.17), 0.029, 0.5 + 0.47 * math.sin(angle + 0.17)]
            cylinder(mesh, [0.52, 0.34, 0.5], joint, 0.090, "bark", 7, end_radius=0.061)
            cylinder(mesh, joint, end, 0.067, "bark_light", 7, end_radius=0.025)
            leaf(mesh, [end[0], 0.05, end[2]], 0.09, "moss")
    else:
        foliage(mesh)
    # Tilted cylinders and faceted leaves extend past their centre lines. Seat the
    # finished geometry on the voxel floor and fit overhanging roots inside the cell.
    points = [vertex for primitive in mesh.parts.values() for vertex in primitive["position"]]
    minimum = [min(vertex[i] for vertex in points) for i in range(3)]
    maximum = [max(vertex[i] for vertex in points) for i in range(3)]
    scale = min(1.0, 1.0 / max(maximum[i] - minimum[i] for i in range(3)))
    offset = [max(-minimum[i] * scale, min(0., 1. - maximum[i] * scale)) for i in range(3)]
    offset[1] = -minimum[1] * scale
    for primitive in mesh.parts.values():
        primitive["position"] = [
            [vertex[i] * scale + offset[i] for i in range(3)] for vertex in primitive["position"]
        ]
        assert all(-1e-8 <= c <= 1 + 1e-8 for vertex in primitive["position"] for c in vertex)
    return export_voxels(mesh, slug, ("spore",))


def mapped(style, face):
    return f"textures/{DIRECTORY}/{style}_{face}"


def build():
    locale = []
    for number, slug, name, german_name, description, translation in BLOCKS_DATA:
        data = dict(schema_version=1, id=number, name=name, description=description, creative_category="nature",
                    hardness=0.65, resistance=1.2, required_tool=None, mining_level=0)
        if number in (488, 504, 505, 506, 507, 512, 513):
            style = {504: "chestnut", 505: "mycelium", 506: "spore",
                     507: "amethyst", 512: "shingles", 513: "timber"}.get(number)
            materials = {}
            textures = {}
            if style:
                for face in ("top", "side"):
                    key = mapped(style, face)
                    materials[face] = dict(roughness=0.84, normal_texture=key + "_normal.png")
                    if number == 507:
                        materials[face]["emission"] = [0.14, 0.066, 0.021]
                    textures[face] = key + ".png"
            if number in (488, 504, 507):
                gill = mapped("gill", "side")
                materials["gills"] = dict(roughness=0.91, normal_texture=gill + "_normal.png")
                textures["gills"] = gill + ".png"
            bottom = "gills" if number in (488, 504, 507) else "side"
            faces = ["side", "side", bottom, "top", "side", "side"] if style else ["gills"] * 6
            data.update(render_kind="cube", bevel_width=0, materials=materials, textures=textures, face_materials=faces,
                        surface="earth" if number in (505, 506) else ("wood" if number in (512, 513) else "mushroom"))
        else:
            textures = prop(slug)
            boxes = {491: [(0.035, 0, 0.24), (0.97, 0.38, 0.77)], 509: [(0.22, 0, 0.23), (0.77, 0.70, 0.78)]}
            collision = []
            if number in boxes:
                minimum, maximum = boxes[number]
                collision.append(dict(minimum=minimum, maximum=maximum))
            data.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                        textures=textures, collision_boxes=collision)
            if number == 489:
                data["light"] = dict(color=[1, 0.72, 0.26], intensity=120, range=5, falloff="soft_core",
                                     flicker=0.025, cave_intensity=0.75, shadows=False)
        save_json(BLOCKS / f"{slug}.json", data)
        for source, german in [(name, german_name), (description, translation)]:
            locale.append(dict(source=source, en=source, de=german))
    for slug, style, label, face in [("mushroom-stem", "stem", "Mushroom stem", "side"),
                                     ("red-mushroom-cap", "scarlet", "Mushroom cap", "top"),
                                     ("white-mushroom-cap", "cream", "Mushroom cap", "top")]:
        path = BLOCKS / f"{slug}.json"
        data = json.loads(path.read_text())
        base = mapped(style, face)
        data["textures"] = {label: base + ".png"}
        data["materials"][label].pop("texture_pattern", None)
        data["materials"][label]["normal_texture"] = base + "_normal.png"
        save_json(path, data)
    save_json(Path(__file__).resolve().parents[2] /
              "src-ui/locales/source/mushroom-park.json", locale)


if __name__ == "__main__":
    build_textures(BLOCKS / "textures" / DIRECTORY)
    build()
