#!/usr/bin/env python3
"""Deterministic F35 art: individual pixel textures, portable GLBs and block JSON.

Original code-authored assets inspired by the supplied Spirit Blossom references.
No atlas, external images, dependencies or random runtime generation required.
"""
import json
import math
import struct
import zlib
from pathlib import Path
from build_castle import Mesh, BLOCKS, MODELS
from voxel_props.geometry import VoxelSculpt
from biome_surfaces import SIZE, rgb, mix, soil, turf, stone, wood, blossom as blossom_pixel

ROOT = Path(__file__).resolve().parents[2]
TEXTURES = BLOCKS / 'textures'
DEFINITIONS = [
    (306, 'spirit_blossom_grass', 'Spirit Blossom Grass', 'Turquoise turf over violet earth from the Spirit Blossom sanctuary.'),
    (307, 'spirit_blossom_dirt', 'Spirit Blossom Dirt', 'Violet soil threaded with pale blossom roots.'),
    (308, 'spirit_stone', 'Spirit Stone', 'Lilac stone with natural cyan mineral veins.'),
    (309, 'blossom_petal_block', 'Blossom Petal Block', 'A compact block of overlapping pink blossom petals.'),
    (310, 'pink_blossom_leaves', 'Pink Blossom Leaves', 'Dense pink blossoms from the Spirit Blossom trees.'),
    (311, 'white_blossom_leaves', 'White Blossom Leaves', 'Pearl-white blossoms with pale blue centers.'),
    (312, 'lavender_blossom_leaves', 'Lavender Blossom Leaves', 'Lavender flower clusters from the spirit grove.'),
    (313, 'spirit_blossom_log', 'Spirit Blossom Log', 'Twisted dark bark with rose-colored end grain.'),
    (314, 'spirit_blossom_planks', 'Spirit Blossom Planks', 'Pale purple timber sawn from spirit blossom logs.'),
    (315, 'glowing_lantern_stone', 'Glowing Lantern Stone', 'A violet stone lantern with a warm luminous blossom.'),
    (316, 'glowing_spirit_flowers', 'Glowing Spirit Flowers', 'Small cyan flowers that softly illuminate the sanctuary.'),
    (317, 'blossom_bush', 'Blossom Bush', 'A low shrub covered in five-petaled pink flowers.'),
    (318, 'spirit_lantern_post', 'Spirit Lantern Post', 'A mossy stepped stone lantern with a warm blossom window.'),
    (319, 'lotus_flower', 'Lotus Flower', 'A pink lotus resting on a broad green water lily leaf.'),
    (320, 'lily_pad', 'Lily Pad', 'A low green leaf for the calm waters of Spirit Blossom.'),
    (321, 'hanging_blossom_lantern', 'Hanging Blossom Lantern', 'A blossom paper lantern with a wooden cap and pink tassel.'),
    (322, 'petal_ground_cover', 'Petal Ground Cover', 'Loose pink and pearl petals scattered over the ground.'),
    (323, 'spirit_shrine_stone', 'Spirit Shrine Stone', 'A small mossy shrine sheltering a luminous spirit spiral.'),
]


def png(path, pixels, size=64):
    raw = b''.join(b'\0' + bytes(c for color in pixels[y * size:(y + 1) * size] for c in color) for y in range(size))
    def chunk(kind, value):
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value))
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def noise(x, y, salt=0):
    n = ((x * 73856093) ^ (y * 19349663) ^ salt * 83492791) & 0xffffffff
    n = ((n ^ (n >> 13)) * 1274126177) & 0xffffffff
    return (n & 255) / 255


# Full-resolution, code-authored art. The cube silhouette remains closed and the
# surface detail is inward-only; none of these settings affect collision.
PATTERNS = {
    306: ([80, 178, 166], 'grass'), 307: ([151, 109, 140], 'dirt'),
    308: ([160, 143, 181], 'stone'), 309: ([237, 119, 177], 'petal'),
    310: ([243, 157, 200], 'pink'), 311: ([224, 222, 248], 'white'),
    312: ([181, 138, 226], 'lavender'), 313: ([137, 83, 111], 'bark'),
    314: ([184, 137, 172], 'planks'), 315: ([156, 135, 168], 'lantern'),
}
SURFACES = {306: 'turf', 307: 'earth', 308: 'rock', 313: 'wood', 314: 'wood', 315: 'rock'}


def texture_pixel(x, y, color, pattern):
    if pattern in ('grass', 'grass_side'):
        return turf(x, y, [80, 178, 166], [151, 109, 140], side=pattern == 'grass_side')
    if pattern == 'dirt':
        return soil(x, y, color)
    if pattern in ('pink', 'white', 'lavender', 'petal'):
        return blossom_pixel(x, y, color, white=pattern == 'white', petals_only=pattern == 'petal')
    if pattern in ('bark', 'end', 'planks'):
        return wood(x, y, color, end=pattern == 'end', planks=pattern == 'planks')
    c = stone(x, y, color, vein=[121, 228, 247] if pattern == 'stone' else None)
    if pattern == 'lantern':
        dx, dy = x - 127.5, y - 127.5
        radius, angle = math.hypot(dx, dy), math.atan2(dy, dx)
        boundary = 24 + 20 * (.5 + .5 * math.cos(5 * angle))
        if radius < boundary:
            c = rgb(mix([255, 218, 139], [255, 241, 185], 1 - radius / boundary))
    return c


def texture(slug, color, pattern):
    pixels = [texture_pixel(x, y, color, pattern) + [255]
              for y in range(SIZE) for x in range(SIZE)]
    png(TEXTURES / f'{slug}.png', pixels, SIZE)


def cube_art_only():
    """Update the existing cube PNGs/surface metadata without touching any GLB."""
    for material, slug, _, _ in DEFINITIONS:
        if material not in PATTERNS:
            continue
        color, pattern = PATTERNS[material]
        texture(slug, color, pattern)
        if material == 306:
            texture(slug + '_side', [151, 109, 140], 'grass_side')
            texture(slug + '_bottom', [151, 109, 140], 'dirt')
        elif material == 313:
            texture(slug + '_end', [209, 142, 170], 'end')
        if material in SURFACES:
            path = BLOCKS / f'{slug}.json'
            definition = json.loads(path.read_text())
            definition['surface'] = SURFACES[material]
            path.write_text(json.dumps(definition, indent=2) + '\n')


def export(mesh, slug, luminous=()):
    textures = mesh.export(slug)
    source, binary = MODELS / f'{slug}.gltf', MODELS / f'{slug}.bin'
    doc = json.loads(source.read_text())
    doc['asset']['generator'] = 'Teramic Spirit Blossom deterministic asset builder'
    doc['buffers'][0].pop('uri')
    for material in doc['materials']:
        if material['name'] in luminous:
            material['emissiveFactor'] = material['pbrMetallicRoughness']['baseColorFactor'][:3]
            material['extensions'] = {'KHR_materials_emissive_strength': {'emissiveStrength': 2.0}}
            doc['extensionsUsed'] = ['KHR_materials_emissive_strength']
    encoded = json.dumps(doc, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4)
    data = binary.read_bytes(); data += b'\0' * (-len(data) % 4)
    result = struct.pack('<III', 0x46546c67, 2, 28 + len(encoded) + len(data))
    result += struct.pack('<I4s', len(encoded), b'JSON') + encoded
    result += struct.pack('<I4s', len(data), b'BIN\0') + data
    (MODELS / f'{slug}.glb').write_bytes(result)
    source.unlink(); binary.unlink()
    return textures


def palette(mesh):
    for name, color in {
        'spirit_rock': [.46, .36, .53], 'spirit_edge': [.69, .57, .71],
        'spirit_moss': [.37, .49, .12], 'spirit_leaf': [.28, .47, .21],
        'spirit_wood': [.34, .18, .26], 'spirit_gold': [.88, .62, .31],
        'spirit_pink': [.94, .38, .68], 'spirit_pearl': [.98, .84, .96],
        'spirit_cyan': [.28, .88, 1.0], 'spirit_lamp': [1.0, .79, .43],
    }.items(): mesh.materials[name] = (color, None, 0, .8)


def blossom(mesh, center, radius, mat='spirit_pink', horizontal=False):
    # Five readable petals, sculpted from small axis-aligned cubes.
    x, y, z = center
    for i in range(5):
        a = math.tau * i / 5
        dx, dy = math.cos(a) * radius * .61, math.sin(a) * radius * .61
        if horizontal:
            mesh.blob([x+dx, y+radius*.10, z+dy], [radius*.44, radius*.14, radius*.44], mat)
        else:
            mesh.blob([x+dx, y+dy, z], [radius*.44, radius*.44, radius*.15], mat)
    mesh.blob([x,y+radius*.14 if horizontal else y,z], [radius*.19,radius*.16,radius*.19], 'spirit_gold')


def pad(mesh):
    # A thin voxel leaf with a stepped rim and a clearly open lotus notch.
    mesh.fill([.0625,.03125,.0625],[.9375,.0625,.9375], 'spirit_moss',
              lambda p: (p[0]-.5)**2+(p[2]-.5)**2 < .43**2
              and not (p[0]>.64 and abs(p[2]-.5)<(p[0]-.58)*.24))
    for dx,dz in ((-.28,0),(-.22,-.22),(-.22,.22),(0,-.28),(0,.28)):
        # Flat, one-cube-wide veins share the leaf surface, with no tiny tubes.
        for i in range(8):
            x,z=.5+dx*i/8,.5+dz*i/8
            mesh.box([x-.008,.03125,z-.008],[x+.008,.0625,z+.008],'spirit_leaf')


def lantern(mesh, base=.32, roof=True):
    mesh.box([.28, base, .28], [.72, base + .34, .72], 'spirit_lamp', .014)
    for x in [.24, .70]:
        for z in [.24, .70]: mesh.box([x, base - .04, z], [x + .06, base + .38, z + .06], 'spirit_wood', .006)
    for y in [base - .055, base + .34]: mesh.box([.21, y, .21], [.79, y + .055, .79], 'spirit_edge', .008)
    for z in [.268, .732]: blossom(mesh, [.5, base + .17, z], .13, 'spirit_pearl')
    if roof:
        for i in range(3):
            low, high = .13 + i * .10, .87 - i * .10
            mesh.box([low, base + .395 + i * .04, low], [high, base + .435 + i * .04, high], 'spirit_rock', .012)


def prop(material):
    result = Mesh('spirit'); palette(result)
    mesh = VoxelSculpt()
    if material == 316:
        for x, z, h in [(.25, .38, .52), (.55, .55, .79), (.77, .32, .62)]:
            mesh.box([x - .018, 0, z - .018], [x + .018, h, z + .018], 'spirit_leaf', .003)
            blossom(mesh, [x, h, z], .12, 'spirit_cyan')
            mesh.box([x - .11, h * .4, z - .018], [x, h * .4 + .06, z + .018], 'spirit_leaf', .008)
    elif material == 317:
        # An open flowering bush, not four solid green pedestals. Thin branches
        # meet a shared root and carry separate leaves below upright flowers.
        root = [.5, .015, .5]
        junction = [.47, .22, .49]
        mesh.branch(root, junction, .036, 'spirit_wood', .025)
        for i, (x, z, h) in enumerate([(.28, .28, .47), (.69, .32, .58), (.34, .68, .66), (.69, .67, .43)]):
            elbow = [(x+.47)/2, h*.57, (z+.49)/2]
            mesh.branch(junction, elbow, .022, 'spirit_wood', .017)
            mesh.branch(elbow, [x,h-.025,z], .017, 'spirit_leaf', .012)
            for side in (-1, 1):
                center = [elbow[0]+side*.055, elbow[1]+.025, elbow[2]+side*.035]
                mesh.blob(center, [.075, .025, .038], 'spirit_leaf' if side < 0 else 'spirit_moss')
            blossom(mesh, [x, h, z], .09, 'spirit_pearl' if i % 2 else 'spirit_pink')
            if i % 2 == 0:
                blossom(mesh, [x+.055, h-.12, z+.015], .066, 'spirit_pink', True)
    elif material == 318:
        for lo, hi in [([.17, 0, .17], [.83, .08, .83]), ([.24, .08, .24], [.76, .16, .76]), ([.38, .16, .38], [.62, .43, .62])]:
            mesh.box(lo, hi, 'spirit_rock', .018, True)
        lantern(mesh, .43)
        mesh.box([.16, .08, .19], [.32, .10, .38], 'spirit_moss', .01)
    elif material in (319, 320):
        pad(mesh)
        if material == 319:
            blossom(mesh, [.5, .10, .5], .30, 'spirit_pink', True)
            blossom(mesh, [.5, .17, .5], .21, 'spirit_pearl', True)
            mesh.box([.46, .20, .46], [.54, .30, .54], 'spirit_gold', .012)
    elif material == 321:
        lantern(mesh, .38, False)
        mesh.box([.48, .76, .48], [.52, 1, .52], 'spirit_wood', .004)
        for x in [.43, .48, .53]: mesh.box([x, .10, .48], [x + .035, .34, .52], 'spirit_pink', .003)
    elif material == 322:
        for i in range(7):
            x, z = .12 + noise(i, 2) * .75, .12 + noise(i, 5) * .75
            blossom(mesh, [x, .015 + i * .004, z], .085, 'spirit_pearl' if i % 2 else 'spirit_pink', True)
    elif material == 323:
        mesh.box([.06, 0, .08], [.94, .13, .92], 'spirit_rock', .025, True)
        for x in [.15, .71]: mesh.box([x, .13, .18], [x + .14, .77, .82], 'spirit_edge', .018, True)
        mesh.box([.27, .15, .62], [.73, .76, .78], 'spirit_rock', .018, True)
        for i in range(3): mesh.box([.05 + i * .09, .75 + i * .065, .08], [.95 - i * .09, .815 + i * .065, .90], 'spirit_edge', .018, True)
        for i in range(28):
            a = i / 27 * math.tau * 1.5; r = .025 + i / 27 * .16
            x, y = .5 + math.cos(a) * r, .44 + math.sin(a) * r
            mesh.box([x - .013, y - .013, .602], [x + .013, y + .013, .625], 'spirit_cyan', .005)
        for x in [.12, .68]: mesh.box([x, .13, .12], [x + .19, .16, .29], 'spirit_moss', .009)
    return mesh.emit(result)


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--flower-models-only', action='store_true',
                        help='Rebuild only the six flower GLBs; preserve JSON/PNG/recipes')
    parser.add_argument('--prop-models-only', action='store_true',
                        help='Rebuild the eight voxel prop GLBs; preserve JSON/PNG/recipes')
    parser.add_argument('--textures-only', action='store_true',
                        help='Rebuild cube textures and inward surface styles; preserve all GLBs/props')
    args = parser.parse_args()
    if args.textures_only:
        cube_art_only()
        from biome_surfaces.foliage import refresh
        refresh({310, 311, 312})
        print('Rebuilt Spirit cube art and 512px side/top foliage; all GLBs and prop definitions unchanged.')
        return
    if args.flower_models_only or args.prop_models_only:
        for material, slug, _, _ in DEFINITIONS:
            if material in (316, 317, 318, 319, 321, 322) or (args.prop_models_only and material >= 316):
                export(prop(material), slug, ('spirit_cyan', 'spirit_lamp'))
        print('Rebuilt voxel prop GLBs; definitions, textures and recipes unchanged.')
        return
    # Individual authored textures remain independently replaceable by artists.

    for material, slug, name, description in DEFINITIONS:
        definition = dict(schema_version=1, id=material, name=name, description=description,
                          creative_category='nature' if material in range(310, 314) or material in (316, 317, 319, 320, 322) else 'decoration' if material >= 315 else 'terrain',
                          hardness=.3 if material in range(309, 313) or material in (316, 317, 319, 320, 322) else 1.4,
                          resistance=3, required_tool=None, mining_level=0)
        if material < 316:
            color, pattern = PATTERNS[material]
            texture(slug, color, pattern)
            names = [slug] * 6
            if material == 306:
                texture(slug + '_side', [151, 109, 140], 'grass_side')
                texture(slug + '_bottom', [151, 109, 140], 'dirt')
                names = [slug + '_side', slug + '_side', slug + '_bottom', slug, slug + '_side', slug + '_side']
            elif material == 313:
                texture(slug + '_end', [209, 142, 170], 'end')
                names = [slug, slug, slug + '_end', slug + '_end', slug, slug]
            definition.update(render_kind='cube', bevel_width=0, materials={n: dict(base_color=[1, 1, 1, 1]) for n in sorted(set(names))}, textures={n: f'textures/{n}.png' for n in sorted(set(names))}, face_materials=names)
            if material in SURFACES:
                definition['surface'] = SURFACES[material]
            if material == 315:
                for m in definition['materials'].values(): m['emission'] = [.9, .52, .16]
            mesh = Mesh('spirit')
            mesh.materials[slug] = ([1, 1, 1], slug + '.png', 0, .9)
            mesh.box([0, 0, 0], [1, 1, 1], slug, 0)
            export(mesh, slug)
        else:
            mesh = prop(material)
            textures = export(mesh, slug, ('spirit_cyan', 'spirit_lamp'))
            definition.update(render_kind='custom_mesh', model_layout='scene', model=f'models/{slug}.glb', struct_block=True, textures=textures, collision_boxes=mesh.collision)
        if material in (315, 316, 318, 321, 323):
            definition['light'] = dict(color=[.45, .80, 1] if material in (316, 323) else [1, .76, .43], intensity=70 if material == 316 else 900, range=5 if material == 316 else 13, falloff='soft_core', flicker=.035, cave_intensity=.6, shadows=False)
        (BLOCKS / f'{slug}.json').write_text(json.dumps(definition, indent=2) + '\n')
    from biome_surfaces.foliage import refresh
    refresh({310, 311, 312})
    recipes = ROOT / 'assets/crafting/biomes/spirit_blossom'; recipes.mkdir(parents=True, exist_ok=True)
    for material, inputs, count in [
        (314, [(313, 1)], 4), (309, [(322, 4)], 1), (315, [(308, 4), (316, 1)], 1),
        (318, [(308, 3), (315, 1)], 1), (321, [(314, 2), (315, 1)], 1),
        (323, [(308, 4), (316, 1), (314, 1)], 1),
    ]:
        _, slug, name, _ = DEFINITIONS[material - 306]
        recipe = dict(id='teramic::' + slug, name=name, station='workbench', output=dict(item=dict(block=material), count=count),
                      pattern=dict(kind='shapeless', ingredients=[dict(item=dict(block=block), count=amount) for block, amount in inputs]))
        (recipes / f'{slug}.json').write_text(json.dumps(recipe, indent=2) + '\n')
    print(f'Authored {len(DEFINITIONS)} block definitions and GLBs, individual textures and 6 recipes.')


if __name__ == '__main__':
    main()
