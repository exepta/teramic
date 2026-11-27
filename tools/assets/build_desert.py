"""Tileable dune strata, carved masonry and metre-scale oasis/ruin props."""
import math
from pathlib import Path

import numpy as np
from PIL import Image

from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

DATA = [
    ("dune_sand", "Dune Sand", "Dünensand", "Pale wind-rippled sand over compact amber sediment.", "Heller, gerippter Sand über verdichtetem, bernsteinfarbenem Sediment."),
    ("red_dune_sand", "Red Dune Sand", "Roter Dünensand", "Iron-rich ochre grains at the foot of desert mesas.", "Eisenreicher Ockersand am Fuß der Tafelberge."),
    ("banded_sandstone", "Banded Sandstone", "Geschichteter Sandstein", "Wind-cut beds of honey limestone and reddish sandstone.", "Windgeschliffene Schichten aus honigfarbenem Kalk und rötlichem Sandstein."),
    ("carved_sunstone", "Carved Sunstone", "Verzierter Sonnenstein", "A carved sandstone course from a desert gateway.", "Verziertes Sandsteinmauerwerk eines Wüstentors."),
    ("oasis_mosaic", "Oasis Mosaic", "Oasenmosaik", "Blue glazed diamonds set into pale sandstone tesserae.", "Blau glasierte Rauten zwischen hellen Sandsteinfliesen."),
    ("desert_agave", "Desert Agave", "Wüstenagave", "A rosette of thick blue-green pointed leaves.", "Eine Rosette aus dicken, blaugrünen Blattspitzen."),
    ("flowering_yucca", "Flowering Yucca", "Blühende Yucca", "Cream and amber bells crown a dryland rosette.", "Creme- und bernsteinfarbene Blüten über einem Kranz spitzer Blätter."),
    ("prickly_pear", "Flowering Prickly Pear", "Blühender Feigenkaktus", "Rose blossoms and pale spines on fleshy cactus pads.", "Rosa Blüten und helle Dornen auf fleischigen Kaktusgliedern."),
    ("painted_amphora", "Painted Amphora", "Bemalte Amphore", "A fired clay vessel with handles and indigo bands.", "Ein gebranntes Tongefäß mit Henkeln und indigoblauen Bändern."),
    ("sun_banner", "Sun Banner", "Sonnenbanner", "A gold sun embroidered on wind-folded crimson cloth.", "Eine gestickte goldene Sonne auf windgefaltetem, karminrotem Stoff."),
] + [
    ("palm_frond_" + axis, "Palm Frond (" + en + ")", "Palmwedel (" + de + ")",
     "Arching palm foliage with tapered, folded leaflets.", "Ein gebogener Palmwedel mit spitz zulaufenden Fiederblättern.")
    for axis, en, de in [("east", "East", "Ost"), ("northeast", "Northeast", "Nordost"),
                         ("north", "North", "Nord"), ("northwest", "Northwest", "Nordwest")]
]


def tile(slug, face, output):
    size = 256
    y, x = np.mgrid[0:size, 0:size] / size
    rng = np.random.default_rng(2641)
    def noise(nx, ny):
        grid = rng.standard_normal((ny, nx))
        ix, iy = (x * nx).astype(int), (y * ny).astype(int)
        tx, ty = (x * nx) % 1, (y * ny) % 1
        return ((1 - tx) * ((1 - ty) * grid[iy, ix] + ty * grid[(iy + 1) % ny, ix])
                + tx * ((1 - ty) * grid[iy, (ix + 1) % nx] + ty * grid[(iy + 1) % ny, (ix + 1) % nx]))
    grain = noise(61, 62) * 2.1 + noise(13, 11) * 1.8
    wave = noise(5, 4)
    base = [228, 188, 128]
    relief = grain.copy()
    color = np.broadcast_to(base, (size, size, 3)).astype(float).copy()
    if slug in ("dune_sand", "red_dune_sand"):
        if slug == "red_dune_sand":
            color[:] = [183, 111, 70]
        if face == "side":
            # A cut bank exposes shaded, compacted grains throughout.
            # The periodic mottling continues across stacked sand blocks.
            color -= [12, 15, 12]
            relief += noise(11, 13) * 1.8 + noise(45, 39) * 1.2
        else:
            relief += np.cos(math.tau * (y * 4 + .22 * wave)) * 2.2
    elif slug == "banded_sandstone":
        color[:] = [199, 150, 105]
        if face == "side":
            strata = (y + .036 * wave + .012 * noise(11, 4)) % 1
            tint = np.where(strata < .25, 13, np.where(strata < .33, -22, np.where(strata < .63, -7, 7)))
            color += tint[:, :, None] * [1.04, 1.42, 1.32]
            relief += noise(24, 8) * 3.5
    else:
        n = 4 if slug == "oasis_mosaic" else 2
        row = np.floor(y * n)
        gx = (x * n + (row % 2) * .5 + .014 * wave) % 1
        gy = (y * n + .012 * noise(8, 4)) % 1
        relief -= np.exp(-np.minimum.reduce([gx, 1 - gx, gy, 1 - gy]) * 55) * 39
        diamond = abs(gx - .5) + abs(gy - .5)
        if slug == "oasis_mosaic":
            color[diamond < .31] = [67, 123, 145]
            color[diamond < .16] = [192, 158, 96]
        else:
            relief -= 21 * np.exp(-abs(diamond - .29) * 85)
    name = f"{slug}_{face}"
    Image.fromarray(np.uint8(np.clip(color + relief[:, :, None], 0, 255))).save(output / f"{name}.png")
    normals = np.stack([(np.roll(relief, 1, axis=1) - np.roll(relief, -1, axis=1)) / 59,
                        (np.roll(relief, 1, axis=0) - np.roll(relief, -1, axis=0)) / 59,
                        np.ones_like(x)], axis=2)
    normals /= np.linalg.norm(normals, axis=2)[:, :, None]
    Image.fromarray(np.uint8((normals * .5 + .5) * 255)).save(output / f"{name}_normal.png")
    return "textures/desert/" + name


def rosette(mesh, radius, height):
    for i in range(14):
        a = i * 2.39996
        dx, dz = math.cos(a), math.sin(a)
        reach = radius * (1 if i < 9 else .58)
        h = height * (.57 + .38 * (i % 5) / 4 + (.29 if i >= 9 else 0))
        def point(along, side, y):
            return [.5 + dx * along - dz * side, y, .5 + dz * along + dx * side]
        foot, tip = point(.015, 0, .025), point(reach, 0, h)
        left, right = point(reach * .62, .052, h * .35), point(reach * .62, -.052, h * .35)
        ridge = point(reach * .54, 0, h * .49)
        for points, material in (([foot, left, ridge], "sage"), ([left, tip, ridge], "jade"),
                                 ([ridge, tip, right], "sage"), ([ridge, right, foot], "jade"),
                                 ([foot, right, tip, left], "shade")):
            mesh.polygon(points, material)


def blossom(mesh, x, y, z, radius):
    for petal in range(5):
        a = petal * math.tau / 5
        center = [x + math.cos(a) * radius * .57, y + .009, z + math.sin(a) * radius * .57]
        ellipsoid(mesh, center, [radius * .48, .025, radius * .48], "rose", 6, 3)
    ellipsoid(mesh, [x, y + .025, z], [.018, .014, .018], "ochre", 6, 3)


def model(slug):
    mesh = Mesh("stone")
    for name, color in {"sage": [.40, .59, .39], "jade": [.25, .45, .31], "shade": [.15, .32, .23],
                        "rose": [.98, .29, .43], "ochre": [.97, .72, .27], "ivory": [1, .88, .63],
                        "copper": [.63, .29, .14], "indigo": [.20, .33, .45], "wine": [.60, .12, .095],
                        "pad": [.36, .55, .16], "cloth": [.78, .22, .13]}.items():
        mesh.materials[name] = (color, None, 0, .86)
    if slug.startswith("palm_frond_"):
        # The same folded frond along four voxel-grid axes. Two leaf surfaces share
        # the game's existing matte foliage palette. Negative winding closes each blade.
        angles = {"east": 0, "northeast": 1, "north": 2, "northwest": 3}
        angle = angles[slug.removeprefix("palm_frond_")] * math.pi / 4
        dx, dz = math.cos(angle), math.sin(angle)
        def point(along, across, height):
            return [.5 + .87 * (dx * along - dz * across), height, .5 + .87 * (dz * along + dx * across)]
        cylinder(mesh, point(-.47, 0, .62), point(.47, 0, .66), .023, "leaf_light", 4)
        for i in range(5):
            t = -.40 + i * .18
            for side in [-1, 1]:
                width = (.29 + .13 * math.sin((i + .5) * math.pi / 5)) * side
                foot = point(t, 0, .64)
                leading = point(t + .004, width * .58, .56)
                trailing = point(t + .15, width * .46, .55)
                tip = point(t + .13, width, .31)
                ridge = point(t + .071, width * .52, .60)
                mesh.polygon([foot, leading, tip, ridge], "leaf", [0,1,0])
                mesh.polygon([foot, ridge, tip, trailing], "leaf_light", [0,1,0])
                mesh.polygon([foot, trailing, tip, leading], "leaf", [0,-1,0])
    elif slug in ("desert_agave", "flowering_yucca"):
        rosette(mesh, .43, .48 if slug == "desert_agave" else .26)
        if slug == "flowering_yucca":
            cylinder(mesh, [.5, .05, .5], [.5, .91, .5], .021, "sage", 6, end_radius=.012)
            for i in range(11):
                a = i * 2.39996
                y = .45 + i * .042
                r = .14 * (1 - i / 14)
                end = [.5 + r * math.cos(a), y + .05, .5 + r * math.sin(a)]
                cylinder(mesh, [.5, y, .5], end, .012, "sage", 5)
                cylinder(mesh, end, [end[0], y - .022, end[2]], .018, "ivory", 7, end_radius=.043)
    elif slug == "prickly_pear":
        for x, y, z, w, h in [(.49,.25,.51,.20,.25), (.31,.47,.48,.135,.19),
                                (.67,.49,.52,.139,.206), (.49,.68,.52,.145,.20)]:
            ellipsoid(mesh, [x,y,z], [w,h,w*.48], "pad", 9, 6)
            for k in range(5):
                a = k * 2.4
                pin = [x + .58*w*math.cos(a), y + .62*h*math.sin(a), z + .065]
                cylinder(mesh, pin, [pin[0]+.012, pin[1]+.028, pin[2]+.035], .009, "ivory", 4, end_radius=.001)
        blossom(mesh, .49, .87, .52, .091)
        blossom(mesh, .32, .65, .48, .064)
    elif slug == "painted_amphora":
        profile = [(.01,.14),(.075,.16),(.20,.245),(.40,.275),(.51,.25),(.54,.25),
                   (.61,.16),(.69,.107),(.78,.104),(.80,.137),(.85,.135),(.844,.094),(.75,.09)]
        for i, ((y,r),(yy,rr)) in enumerate(zip(profile, profile[1:])):
            cylinder(mesh,[.50,y,.50],[.50,yy,.50],r, "indigo" if i in (4,9) else "copper",12,end_radius=rr)
        for side in [-1,1]:
            path = [[.5+side*x,y,.5] for x,y in [(.10,.75),(.23,.74),(.285,.67),(.28,.60),(.23,.51)]]
            for a,b in zip(path,path[1:]):
                cylinder(mesh, a, b, .027, "copper", 6)
    else:
        cylinder(mesh,[.10,.93,.5],[.90,.93,.5],.036,"oak",8)
        for i in range(8):
            def cloth_edge(n, top):
                x = .16 + n * .085
                return [x, .91 if top else .24 - .14*(1-abs(n-4)/4), .5 + math.cos(n*.9)*.036]
            corners = [cloth_edge(i,False),cloth_edge(i+1,False),cloth_edge(i+1,True),cloth_edge(i,True)]
            mesh.polygon(corners, "wine" if i % 4 < 2 else "cloth")
            mesh.polygon(corners[::-1], "wine")
        for side in [-1,1]:
            z = .5 + side*.048
            mesh.polygon([[.50,.77,z],[.64,.59,z],[.50,.41,z],[.36,.59,z]], "ochre",[0,0,side])
            for i in range(8):
                a=i*math.tau/8
                def ray(r, d):
                    return [.5+r*math.cos(a)-d*math.sin(a),.59+r*math.sin(a)+d*math.cos(a),z]
                mesh.polygon([ray(.19,-.014),ray(.245,0),ray(.19,.014)],"ochre",[0,0,side])
    return export_voxels(mesh, slug)


def build():
    output = BLOCKS / "textures/desert"
    output.mkdir(exist_ok=True)
    locale = []
    for i, (slug, name, de, desc, translated) in enumerate(DATA, 527):
        block = dict(schema_version=1, id=i, name=name, description=desc, creative_category="terrain" if i <= 531 else "nature",
                     hardness=.55 if i < 529 else 1.4 if i <= 531 else .25,
                     resistance=2 if i<=531 else .4, mining_level=0, required_tool=None)
        if i <= 531:
            faces = ["side","side","top","top","side","side"] if i<=529 else ["surface"]*6
            textures = {f:tile(slug, f, output) for f in sorted(set(faces))}
            block.update(render_kind="cube", bevel_width=0, face_materials=faces,
                         textures={k:v+".png" for k,v in textures.items()},
                         materials={k:dict(roughness=.94, reflectance=.14, normal_texture=v+"_normal.png") for k,v in textures.items()})
        else:
            block.update(render_kind="custom_mesh", model_layout="scene", model=f"models/{slug}.glb",
                         collision_boxes=[], struct_block=True, textures=model(slug))
        save_json(BLOCKS / f"{slug}.json", block)
        for s, t in ((name,de),(desc,translated)):
            locale.append(dict(source=s, en=s, de=t))
    save_json(Path(__file__).resolve().parents[2] / "src-ui/locales/source/desert.json", locale)


if __name__ == "__main__":
    build()
