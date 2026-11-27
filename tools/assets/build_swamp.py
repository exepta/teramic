"""Author the swamp's pixel-painted cubes, willow canopy atlas and voxel props."""
import math
import re
import random
import numpy as np
from PIL import Image, ImageDraw

from biome_surfaces.base_materials import normal_pixels
from build_ashlands import stone_fields
from build_castle import BLOCKS, ROOT
from build_highlands import far_crown
from build_salt_cave import save_json
from swamp.catalog import CUBES, DESCRIPTIONS, PROPS
from swamp.sculpt import COLLIDERS, model

BASE = {
    'SwampTurf': (86, 85, 57), 'SwampMud': (83, 67, 52), 'Peat': (71, 58, 47),
    'StonySwampMud': (90, 74, 59), 'MossyPeat': (80, 73, 43), 'SwampClay': (96, 79, 85),
    'SwampStone': (112, 115, 110), 'MossySwampStone': (107, 111, 99),
    'SwampRuinedBricks': (105, 111, 102), 'WillowLog': (92, 77, 58),
    'MossyWillowLog': (95, 79, 54), 'WillowPlanks': (121, 99, 74),
}


def surface(name, face, seed):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[:128, :128]
    a = math.tau / 128
    ripple = 1.8 * np.sin((x + 2 * y) * a) + 1.1 * np.cos((3 * x - y) * a)
    silt = np.sin((x - y) * a) * 3.5 + np.cos((x + 2 * y) * a) * 2.8
    grain = rng.normal(0, 2, x.shape)
    field, seam = stone_fields(x + ripple, y + 1.5 * np.sin(x * a), rng, 5, 4)
    rgb = np.broadcast_to(BASE[name], (128, 128, 3)).astype(float).copy()
    relief = silt + grain
    moss = (np.sin(2 * a * x + .72 * np.sin(y * a)) + np.cos((x + y) * a) + .35 * np.sin(y * 5 * a)) * .5
    if name in ('WillowLog', 'MossyWillowLog', 'WillowPlanks'):
        if face == 'end':
            rgb[:] = [152, 125, 88]
            radius = np.hypot(x - 64 + ripple, y - 64 + ripple * .5)
            relief += 7 * np.sin(radius * .72) + 3 * np.cos(radius * .36) - (radius > 55) * 27
        elif name == 'WillowPlanks':
            relief += 3 * np.sin((y + ripple) * a * 13) - ((y % 32 < 2)
                                                           | ((x + 41 * (y // 32)) % 128 < 2)) * 25
        else:
            ridge = np.cos((x + ripple) * a * 11)
            relief += 6 * ridge - 50 * np.maximum(0, ridge - .67)
    elif name in ('SwampStone', 'MossySwampStone'):
        relief += (field % 5 - 2) * 2.9 - 24 * np.exp(-seam**2 * 110)
    elif name == 'SwampRuinedBricks':
        row = y // 32
        joints = (y % 32 < 3) | ((x + (row % 2) * 25) % 64 < 3)
        relief -= 20 * joints
        relief += 4 * ((y % 32 == 3) | ((x + (row % 2) * 25) % 64 == 3))
    elif name == 'SwampTurf':
        turf = (y > 103 + 5 * np.sin(x * 3 * a) + 3 * np.cos(x * 7 * a) +
                np.mod(x // 4, 3)) if face == 'side' else np.ones(x.shape, bool)
        rgb[~turf] = BASE['SwampMud']
        # Damp peat shows through the sod in low patches between sedge roots.
        exposed = turf & (moss + .18 * np.sin((x - 2 * y) * 3 * a) < -.15)
        rgb[exposed] = [75, 65, 49]
        relief += np.where(turf, 4 * np.cos((x + 2 * y) * 24 * a) + 4 *
                           np.sin((x - y) * 10 * a), -3 * np.exp(-seam**2 * 100))
    elif name == 'Peat':
        relief += 3.5 * np.cos((y + ripple) * a * 11) + 2.5 * np.sin((x + y) * a * 18)
    elif name == 'StonySwampMud':
        gravel = (seam > .43) & (field % 3 != 0)
        rgb[gravel] = np.array([127, 118, 99]) + (field[gravel, None] % 4) * 3
    else:
        relief += np.cos((x + y + ripple) * a * 4) * 2 - 7 * np.exp(-seam**2 * 90)
    if name in ('MossyPeat', 'MossySwampStone', 'MossyWillowLog', 'SwampRuinedBricks') and face != 'end':
        cutoff = {'MossyPeat': -.23, 'MossyWillowLog': .19,
                  'MossySwampStone': .18, 'SwampRuinedBricks': .52}[name]
        patch = moss + .10 * np.sin((x * 4 + y) * a * 3) > cutoff
        rgb[patch] = np.array([78, 83, 51] if name == 'MossyPeat' else [102, 116, 48]) + moss[patch, None] * 12
    return Image.fromarray(np.uint8(np.clip(rgb + relief[:, :, None], 0, 255))).resize((256, 256), Image.Resampling.NEAREST)


def willow_canopy():
    atlas = Image.new('RGBA', (512, 512))
    for i in range(4):
        rng = random.Random(7341 + i)
        sprite = Image.new('RGBA', (96, 96))
        b = ImageDraw.Draw(sprite)
        for j in range(5):
            start = 23 + j * 11 + rng.randrange(-4, 5)
            length = rng.randrange(47, 87)
            dx = rng.randrange(-20, 19)
            b.line([(48, 5), (start, 19), (start + dx, length)], fill=(91, 87, 46, 255), width=2)
            for k in range(7):
                t = k / 7
                px = start + t * dx
                py = 19 + (length - 16) * t
                for sign in (-1, 1):
                    bend = sign * (4 + rng.randrange(4))
                    col = rng.choice([(108, 129, 54), (137, 149, 62), (150, 159, 69), (87, 112, 49)])
                    b.polygon([(px, py), (px + bend, py + 2), (px + 1.3 * bend,
                                                               py + 10), (px + bend * .45, py + 7)], fill=col)
        atlas.paste(sprite.resize((256, 256), Image.Resampling.NEAREST), ((i % 2) * 256, (i // 2) * 256))
    return {'spray': atlas, 'top': far_crown(atlas, False, 'top'), 'side': far_crown(atlas, False, 'side')}


def cube(name, slug, ident):
    foliage = name == 'WillowLeaves'
    faces = ('side',)
    if name.endswith('Log'):
        faces = ('side', 'end')
    elif name == 'SwampTurf':
        faces = ('side', 'top')
    images = willow_canopy() if foliage else {f: surface(name, f, ident) for f in faces}
    textures = {}
    mats = {}
    for face, img in images.items():
        folder = 'foliage' if foliage else 'swamp'
        suffix = 'cluster' if face == 'spray' else face
        path = f'textures/{folder}/{slug}_{suffix}.png'
        img.save(BLOCKS / path, optimize=True)
        textures[face] = path
        mats[face] = {'roughness': .66 if name in ('SwampMud', 'SwampClay') else .94, 'reflectance': .09}
        if not foliage:
            pixels = np.asarray(img.convert('RGB')).reshape(-1, 3).tolist()
            normals = normal_pixels(img.width, pixels)
            normal = path[:-4] + '_normal.png'
            normal_image = Image.fromarray(np.uint8(normals).reshape(img.height, img.width, 4))
            normal_image.save(BLOCKS / normal, optimize=True)
            mats[face]['normal_texture'] = normal
    top = 'top' if 'top' in textures else faces[-1]
    bottom = top
    if name == 'SwampTurf':
        textures['soil'] = 'textures/swamp/swamp_mud_side.png'
        bottom = 'soil'
        mats['soil'] = {'roughness': .82, 'normal_texture': 'textures/swamp/swamp_mud_side_normal.png'}
    if foliage:
        bottom = 'side'
    block = dict(render_kind='cube', bevel_width=0, textures=textures, materials=mats,
                 face_materials=['side', 'side', bottom, top, 'side', 'side'])
    if foliage:
        block['foliage_cluster'] = {'material': 'spray'}
    return block


def build():
    locale = {'Willow': 'Weide', 'Accepts planks from any woodworking species, including mixed wood types.':
              'Akzeptiert Bretter aller Bauholzarten, auch gemischt.'}
    (BLOCKS / 'textures/swamp').mkdir(parents=True, exist_ok=True)
    for ident, (name, en, de) in enumerate(CUBES + PROPS, 670):
        slug = re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()
        wood = name in ('WillowLog', 'MossyWillowLog', 'WillowPlanks')
        hard = wood or 676 <= ident <= 678
        category = 'terrain'
        if wood:
            category = 'wood'
        elif ident >= 682:
            category = 'nature'
        block = dict(
            schema_version=1, id=ident, name=en, description=DESCRIPTIONS[name][0],
            creative_category=category, mining_level=0, required_tool=None,
            hardness=1.6 if hard else .46, resistance=2.2 if hard else .62,
        )
        if ident < 683:
            block.update(cube(name, slug, ident))
        else:
            boxes = [{'minimum': box[:3], 'maximum': box[3:]} for box in COLLIDERS.get(name, ())]
            block.update(render_kind='custom_mesh', model_layout='scene', model=f'models/{slug}.glb', struct_block=True,
                         collision_boxes=boxes, textures=model(name, slug, ident))
        save_json(BLOCKS / (slug + '.json'), block)
        locale[en] = de
        source, translation = DESCRIPTIONS[name]
        locale[source] = translation
    save_json(ROOT / 'src-ui/locales/source/swamp-blocks.json',
              [dict(source=en, en=en, de=de) for en, de in locale.items()])


if __name__ == '__main__':
    build()
