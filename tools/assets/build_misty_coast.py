"""Build the tileable maritime palette from the nine authored shore swatches."""
from pathlib import Path
import json
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'
TEXTURES = BLOCKS / 'textures/misty_coast'
MATERIALS = [
    ('coastal_granite', 'Coastal Granite', 'Küstengranit'),
    ('tideworn_rock', 'Tideworn Rock', 'Nasser Gezeitenfels'),
    ('salt_stained_stone', 'Salt-Stained Stone', 'Salzkrustenstein'),
    ('beach_shingle', 'Beach Shingle', 'Strandkiesel'),
    ('coarse_shore_sand', 'Coarse Shore Sand', 'Grober Küstensand'),
    ('mossy_coastal_stone', 'Mossy Coastal Stone', 'Moosiger Küstenstein'),
    ('lichen_coastal_stone', 'Lichen-Covered Stone', 'Flechtenstein'),
    ('whitewashed_plaster', 'Whitewashed Plaster', 'Gekalkter Putz'),
    ('harbor_timber', 'Harbor Timber', 'Hafenbalken'),
    ('lighthouse_stripe', 'Lighthouse Stripe', 'Leuchtturmstreifen'),
    ('harbor_roof_tiles', 'Harbor Roof Tiles', 'Hafendachziegel'),
]


def tile(swatch):
    # Cross-fade opposite edges while retaining the grain and joints in the centre.
    rgb = np.asarray(swatch.resize((256, 256), Image.Resampling.LANCZOS)).astype(float)
    edge_distance = np.minimum(np.arange(256), np.arange(255, -1, -1))
    fade = np.clip(1 - edge_distance / 24, 0, 1) * 0.5
    for axis, weight in enumerate((fade[:, None, None], fade[None, :, None])):
        rgb = rgb * (1 - weight) + np.flip(rgb, axis=axis) * weight
    return rgb


def write_material(atlas, index, slug, name):
    # Painted lighthouse bands and roof tiles share the same whitewashed base.
    swatch = index if index < 9 else 7
    col, row = swatch % 3, swatch // 3
    width, height = atlas.size
    bounds = (
        col * width // 3 + 3, row * height // 3 + 3,
        (col + 1) * width // 3 - 3, (row + 1) * height // 3 - 3,
    )
    rgb = tile(atlas.crop(bounds))
    if slug == 'lighthouse_stripe':
        rgb[63:193] *= [0.73, 0.35, 0.29]
    elif slug == 'harbor_roof_tiles':
        rgb *= [0.76, 0.40, 0.31]
        for y in range(0, 256, 32):
            rgb[y:y + 3] *= 0.63
            for x in range((y // 32 % 2) * 32, 256, 64):
                rgb[y:y + 32, x:x + 2] *= 0.67
    Image.fromarray(rgb.clip(0, 255).astype('uint8')).save(
        TEXTURES / f'{slug}.png', optimize=True
    )
    light = rgb.mean(2) / 255
    dx = np.roll(light, -1, 1) - np.roll(light, 1, 1)
    dy = np.roll(light, -1, 0) - np.roll(light, 1, 0)
    normals = np.stack((-dx, dy, np.full_like(dx, 0.8)), axis=-1)
    normals /= np.linalg.norm(normals, axis=2, keepdims=True)
    Image.fromarray(((normals * 0.5 + 0.5) * 255).astype('uint8')).save(
        TEXTURES / f'{slug}_normal.png', optimize=True
    )
    texture = f'textures/misty_coast/{slug}'
    wet = slug == 'tideworn_rock'
    spec = {
        'schema_version': 1,
        'id': 774 + index,
        'name': name,
        'description': name,
        'creative_category': 'nature' if index < 7 else 'building',
        'mining_level': 0,
        'required_tool': None,
        'hardness': 0.7 if index in (3, 4) else 1.8,
        'resistance': 3.0,
        'render_kind': 'cube',
        'textures': {'side': f'{texture}.png'},
        'materials': {'side': {
            'normal_texture': f'{texture}_normal.png',
            'roughness': 0.34 if wet else 0.84,
            'reflectance': 0.28 if wet else 0.12,
        }},
        'face_materials': ['side'] * 6,
    }
    (BLOCKS / f'{slug}.json').write_text(json.dumps(spec, indent=2) + '\n')


def write():
    atlas = Image.open(Path(__file__).with_name('misty_coast') / 'shore-materials.jpg').convert('RGB')
    TEXTURES.mkdir(exist_ok=True)
    names = [
        ('Misty Coast', 'Nebelküste'),
        ('Misty Coast · Temperate', 'Nebelküste · Gemäßigt'),
        ('Coastal Shipwreck', 'Küstenwrack'),
        ('Fishing Village', 'Fischerdorf'),
        ('Coastal Lighthouse', 'Küstenleuchtturm'),
        ('Tidal Cave', 'Gezeitenhöhle'),
    ]
    for index, (slug, english, german) in enumerate(MATERIALS):
        write_material(atlas, index, slug, english)
        names.append((english, german))
    catalog = [dict(source=en, en=en, de=de) for en, de in names]
    (ROOT / 'src-ui/locales/source/misty-coast.json').write_text(
        json.dumps(catalog, indent=2, ensure_ascii=False) + '\n'
    )


if __name__ == '__main__':
    write()
