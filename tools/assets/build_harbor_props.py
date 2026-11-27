"""Sculpt the salt-weathered fittings and salt-tolerant plants of the Misty Coast."""
import math
from pathlib import Path
import random

from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from build_spirit_blossom import export
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

ROOT = Path(__file__).resolve().parents[2]
COLORS = {
    'drift': (0.39, 0.31, 0.24),
    'endgrain': (0.57, 0.44, 0.31),
    'tar': (0.15, 0.14, 0.13),
    'rope': (0.68, 0.5, 0.3),
    'twist': (0.82, 0.66, 0.41),
    'bronze': (0.56, 0.39, 0.19),
    'verdigris': (0.28, 0.42, 0.4),
    'enamel': (0.1, 0.32, 0.48),
    'navy': (0.08, 0.18, 0.26),
    'chalk': (0.91, 0.9, 0.78),
    'lens': (1.0, 0.83, 0.49),
    'amber': (1.0, 0.61, 0.22),
    'brine': (0.97, 0.96, 0.88),
    'salt': (0.79, 0.82, 0.78),
    'shade': (0.56, 0.64, 0.64),
    'moss': (0.29, 0.37, 0.23),
    'olive': (0.4, 0.46, 0.25),
    'reed': (0.61, 0.59, 0.31),
    'straw': (0.78, 0.72, 0.44),
    'pink': (0.85, 0.42, 0.61),
    'petal': (0.99, 0.68, 0.76),
}


def mooring_post(m):
    cylinder(m, (0.5, 0.0, 0.5), (0.5, 0.95, 0.5), 0.193, 'drift', 9, end_radius=0.172)
    cylinder(m, (0.5, 0.94, 0.5), (0.5, 0.975, 0.5), 0.17, 'endgrain', 9)
    for i in range(3):
        cylinder(m, (0.5, 0.195 + i * 0.248, 0.5), (0.5, 0.226 + i * 0.248, 0.5), 0.196 - i * 0.005, 'tar', 10)
    points = []
    for i in range(57):
        angle = i * math.tau / 16.0
        points.append((0.5 + 0.202 * math.cos(angle), 0.46 + i * 0.0036, 0.5 + 0.202 * math.sin(angle)))
    points.extend([(0.29, 0.53, 0.52), (0.25, 0.26, 0.47), (0.19, 0.06, 0.4), (0.08, 0.037, 0.3)])
    for i, p in enumerate(points[:-1]):
        cylinder(m, p, points[i + 1], 0.026, 'twist' if i % 4 == 0 else 'rope', 6)
    m.collision.append(dict(minimum=[0.295, 0.0, 0.295], maximum=[0.705, 0.975, 0.705]))


def harbor_sign(m):
    m.box([0.11, 0.0, 0.44], [0.23, 0.96, 0.57], 'drift', 0.02, solid=True)
    m.box([0.11, 0.87, 0.43], [0.92, 0.94, 0.58], 'endgrain', 0.014, solid=True)
    for x in [0.41, 0.77]:
        cylinder(m, (x, 0.79, 0.495), (x, 0.88, 0.495), 0.013, 'bronze', 6)
    m.box([0.28, 0.38, 0.47], [0.9, 0.79, 0.54], 'navy', 0.033, solid=True)
    for z, face in [(0.461, -1), (0.549, 1)]:
        m.box([0.3, 0.407, z - 0.006], [0.88, 0.763, z + 0.006], 'enamel', 0.014)
        zz = z + face * 0.01
        n = [0.0, 0.0, face]
        m.polygon(
            [(0.41, 0.58, zz), (0.51, 0.65, zz), (0.63, 0.665, zz), (0.71, 0.601, zz), (0.72, 0.561, zz), (0.64, 0.52, zz), (0.53, 0.523, zz)],
            'chalk',
            n,
        )
        m.polygon([(0.43, 0.58, zz), (0.352, 0.653, zz), (0.365, 0.516, zz)], 'chalk', n)
        m.polygon([(0.54, 0.65, zz), (0.6, 0.705, zz), (0.64, 0.66, zz)], 'chalk', n)
        cylinder(m, (0.659, 0.612, zz), (0.659, 0.612, zz + face * 0.006), 0.013, 'navy', 7)
        for x in [0.335, 0.85]:
            for y in [0.44, 0.73]:
                cylinder(m, (x, y, zz), (x, y, zz + face * 0.004), 0.012, 'bronze', 6)


def beacon_lens(m):
    cylinder(m, (0.5, 0.0, 0.5), (0.5, 0.115, 0.5), 0.44, 'bronze', 16, end_radius=0.39)
    cylinder(m, (0.5, 0.115, 0.5), (0.5, 0.19, 0.5), 0.33, 'verdigris', 16)
    cylinder(m, (0.5, 0.185, 0.5), (0.5, 0.84, 0.5), 0.21, 'amber', 20)
    # Fresnel rings widen towards the equator, each with its own refracting shoulder.
    for i in range(9):
        y = 0.19 + i * 0.069
        radius = 0.35 - 0.012 * abs(i - 4)
        cylinder(m, (0.5, y, 0.5), (0.5, y + 0.021, 0.5), radius - 0.056, 'amber', 20, end_radius=radius)
        cylinder(m, (0.5, y + 0.021, 0.5), (0.5, y + 0.065, 0.5), radius, 'lens', 20, end_radius=radius - 0.054)
    for i in range(4):
        a = (i + 0.5) * math.pi / 2.0
        x = 0.5 + 0.4 * math.cos(a)
        z = 0.5 + 0.4 * math.sin(a)
        cylinder(m, (x, 0.1, z), (x, 0.88, z), 0.021, 'verdigris', 6)
    cylinder(m, (0.5, 0.83, 0.5), (0.5, 0.875, 0.5), 0.39, 'bronze', 16)
    cylinder(m, (0.5, 0.874, 0.5), (0.5, 0.97, 0.5), 0.4, 'verdigris', 16, end_radius=0.095)
    m.collision.append(dict(minimum=[0.13, 0.02, 0.13], maximum=[0.87, 0.94, 0.87]))


def blade(m, x, z, angle, height, lean, width, material, wind):
    dx, dz = (math.cos(angle), math.sin(angle))
    side = [-dz * width, dx * width]
    base = (x, 0.013, z)
    mid = (x + lean * dx * 0.35 + wind * 0.23, height * 0.54, z + lean * dz * 0.35)
    tip = (x + lean * dx + wind, height, z + lean * dz)
    a = (x + side[0], 0.013, z + side[1])
    b = (x - side[0], 0.013, z - side[1])
    c = (mid[0] + side[0] * 0.61, mid[1], mid[2] + side[1] * 0.61)
    d = (mid[0] - side[0] * 0.61, mid[1], mid[2] - side[1] * 0.61)
    ridge = (mid[0] - 0.015 * dx, mid[1], mid[2] - 0.015 * dz)
    # Both sides of a bent blade must survive backface culling.
    for pts in [[a, c, ridge, base], [base, ridge, d, b], [c, tip, ridge], [ridge, tip, d]]:
        m.polygon(pts, material)
        m.polygon(pts[::-1], material)


def tuft(m, flowering):
    rng = random.Random(2017 if flowering else 2018)
    for i in range(29 if flowering else 23):
        theta = rng.random() * math.tau
        radius = 0.05 + 0.17 * rng.random()
        x = 0.46 + math.cos(theta) * radius
        z = 0.49 + math.sin(theta) * radius
        height = 0.12 + 0.15 * rng.random() if flowering else 0.3 + 0.42 * rng.random()
        lean = 0.08 + 0.09 * rng.random()
        colors = ('moss', 'olive', 'reed') if flowering else ('moss', 'olive', 'reed', 'straw')
        blade(m, x, z, theta, height, lean, 0.014 if flowering else 0.021,
              colors[i % len(colors)], 0.018 if flowering else 0.13)
    if flowering:
        for i in range(14):
            a = i * 2.399
            radius = 0.1 + 0.21 * rng.random()
            x = 0.49 + radius * math.cos(a)
            z = 0.5 + radius * math.sin(a)
            height = 0.23 + 0.12 * rng.random()
            cylinder(m, (x * 0.72 + 0.49 * 0.28, 0.035, z * 0.72 + 0.5 * 0.28), (x, height, z), 0.008, 'olive', 5)
            ellipsoid(m, (x, height, z), (0.054, 0.038, 0.054), 'pink', 8, 3)
            for j in range(5):
                a = j * math.tau / 5 + i
                ellipsoid(
                    m,
                    (x + 0.034 * math.cos(a), height + 0.021, z + 0.034 * math.sin(a)),
                    (0.025, 0.019, 0.025),
                    'petal' if (i + j) % 3 else 'pink',
                    6,
                    3,
                )


def salt_crate(m):
    m.box([0.07, 0.02, 0.12], [0.93, 0.13, 0.88], 'tar', 0.018)
    for row in range(3):
        y = 0.13 + row * 0.15
        for x in [0.07, 0.861]:
            m.box([x, y, 0.12], [x + 0.073, y + 0.14, 0.88], ('drift', 'endgrain', 'drift')[row], 0.012)
        for z in [0.12, 0.811]:
            m.box([0.14, y, z], [0.86, y + 0.14, z + 0.069], ('endgrain', 'drift', 'endgrain')[row], 0.012)
    for x in [0.08, 0.84]:
        for z in [0.13, 0.79]:
            m.box([x, 0.065, z], [x + 0.075, 0.605, z + 0.074], 'tar', 0.01)
    m.box([0.143, 0.45, 0.19], [0.857, 0.51, 0.811], 'salt', 0.018)
    rng = random.Random(517)
    for i in range(38):
        x = 0.2 + 0.6 * rng.random()
        z = 0.24 + 0.52 * rng.random()
        h = 0.54 + 0.124 * (1.0 - min(1.0, abs(x - 0.51) * 2.0 + abs(z - 0.49) * 1.4))
        r = 0.035 + 0.045 * rng.random()
        m.box(
            [x - r, h - r * 0.8, z - r * 0.8],
            [x + r, h + r * 0.5, z + r * 0.8],
            ('brine', 'salt', 'shade', 'brine')[i % 4],
            0.012,
        )
    m.collision.append(dict(minimum=[0.07, 0.02, 0.12], maximum=[0.93, 0.63, 0.88]))


PROPS = [
    ('mooring_post', 'Mooring Post', 'Poller mit Tau', mooring_post),
    ('harbor_sign', 'Harbor Sign', 'Hafenschild', harbor_sign),
    ('beacon_lens', 'Fresnel Beacon Lens', 'Fresnellinse des Leuchtfeuers', beacon_lens),
    ('sea_thrift', 'Sea Thrift', 'Strandnelkenpolster', lambda m: tuft(m, True)),
    ('dune_grass_tuft', 'Windbent Dune Grass', 'Windgebeugter Strandhafer', lambda m: tuft(m, False)),
    ('salt_crate', 'Salt Crate', 'Salzkiste', salt_crate),
]


def build():
    locale = []
    for material, (slug, en, de, sculpt) in enumerate(PROPS, 785):
        m = Mesh('drift')
        for name, rgb in COLORS.items():
            texture = 'misty_coast/harbor_timber.png' if name == 'drift' else None
            m.materials[name] = (
                list(rgb),
                texture,
                0.44 if name in ('bronze', 'verdigris') else 0.01,
                0.24 if name in ('lens', 'amber') else 0.83,
            )
        sculpt(m)
        plant = material in (788, 789)
        if plant:
            textures = export_voxels(m, slug, resolution=16)
        else:
            textures = export(m, slug, ('lens', 'amber') if material == 787 else ())
        spec = dict(
            schema_version=1,
            id=material,
            name=en,
            description=en,
            creative_category='nature' if plant else 'decoration',
            mining_level=0,
            required_tool=None,
            hardness=0.22 if plant else 1.2,
            resistance=1.1 if plant else 3.8,
            render_kind='custom_mesh',
            model_layout='scene',
            model=f'models/{slug}.glb',
            struct_block=True,
            textures=textures,
            collision_boxes=m.collision,
        )
        if material == 787:
            spec['light'] = dict(
                color=[1.0, 0.76, 0.39],
                intensity=2400,
                range=23.0,
                falloff='soft_core',
                flicker=0.0,
                shadows=False,
                offset=[0.5, 0.53, 0.5],
            )
        save_json(BLOCKS / f'{slug}.json', spec)
        locale.append(dict(source=en, en=en, de=de))
    save_json(ROOT / 'src-ui/locales/source/harbor-props.json', locale)


if __name__ == '__main__':
    build()
