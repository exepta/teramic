"""Build the tufa, calcite and sediment palette used in dripstone chambers."""
import argparse
import math
import random

from build_castle import BLOCKS, Mesh
from build_salt_cave import column, sample_swatches, save_json, stone
from build_spirit_blossom import export

SWATCHES = {
    'dripstone': [(83,234),(161,199),(273,235),(186,276),(186,390),(273,345)],
    'wet': [(307,234),(404,199),(504,235),(405,278),(405,389),(504,349)],
    'moss': [(540,235),(640,200),(736,234),(636,277),(636,393),(736,352)],
    'calcite': [(764,234),(858,198),(960,235),(867,280),(867,392),(960,345)],
    'mud': [(979,235),(1079,198),(1171,235),(1080,277),(1080,392),(1171,347)],
}
STONES = [
    (37, 'dripstone', 'Dripstone', 'Tropfstein', 'dripstone', .61, .25),
    (39, 'dripstone_formation', 'Dripstone Formation', 'Tropfsteinformation', 'dripstone', .61, .25),
    (465, 'wet_cave_stone', 'Wet Cave Stone', 'Nasses Höhlengestein', 'wet', .24, .39),
    (466, 'cave_calcite', 'Pale Calcite', 'Heller Calcit', 'calcite', .37, .32),
    (469, 'mossy_cave_stone', 'Mossy Cave Stone', 'Bemoostes Höhlengestein', 'moss', .86, .14),
    (470, 'cave_mud', 'Cave Mud', 'Höhlenschlamm', 'mud', .46, .26),
]
DESCRIPTIONS = {
    'wet': ('Water-darkened rock at the base of a dripstone shelf.', 'Feuchtes Gestein am Fuß einer Tropfsteinterrasse.'),
    'calcite': ('Pale bands of crystalline limestone.', 'Helle Bänder aus kristallinem Kalkstein.'),
    'moss': ('Moss grows in joints above the waterline.', 'Moos wächst in Spalten oberhalb der Wasserlinie.'),
    'mud': ('Fine sediment deposited in shallow cave pools.', 'Feines Sediment aus flachen Höhlenbecken.'),
}


def outcrop(number, slug, name, description, luminous):
    mesh = Mesh('stone')
    for layer, (color, tile, roughness) in {
        'Wet stone': ([.72, .68, .63], 'wet_side', .47),
        'Pale edge': ([.96, .90, .78], 'calcite_side', .39),
        'Calcite core': ([.92, .66, .39], 'calcite_side', .32),
    }.items():
        mesh.materials[layer] = (color, 'dripstone_cave/' + tile + '.png', .04, roughness)
    rng = random.Random(number)
    for i in range(16):
        angle = rng.random() * math.tau
        r = .29 * math.sqrt(i / 16)
        x, z = .5 + math.cos(angle) * r, .5 + math.sin(angle) * r
        width = rng.uniform(.06, .14)
        column(mesh, x, z, width, width*.82, rng.uniform(.11, .29)*(1-r), angle,
               (.025*math.cos(angle), .025*math.sin(angle)), ('Wet stone', 'Pale edge'))
    if luminous:
        for i in range(6):
            angle = i * 2.4
            r = .20 * math.sqrt(i / 6)
            column(mesh, .5+math.cos(angle)*r, .5+math.sin(angle)*r, .11, .084,
                   .91-r*2.1, angle, (.35*r*math.cos(angle), .35*r*math.sin(angle)),
                   ('Calcite core', 'Pale edge'))
    textures = export(mesh, slug, ('Calcite core',) if luminous else ())
    definition = dict(schema_version=1, id=number, name=name, description=description,
                      creative_category='decoration', hardness=.65, resistance=2, mining_level=0,
                      struct_block=True, render_kind='custom_mesh', model_layout='scene',
                      model='models/'+slug+'.glb', textures=textures,
                      collision_boxes=[dict(minimum=[.1,0,.1],maximum=[.9,.22,.9])])
    if luminous:
        definition['collision_boxes'].append(dict(minimum=[.33,.22,.33],maximum=[.67,.91,.67]))
        definition['light'] = dict(color=[1,.77,.47], intensity=1700, range=13, falloff='soft_core',
                                   flicker=0, cave_intensity=.88, shadows=False, offset=[.5,.41,.5])
    save_json(BLOCKS/(slug+'.json'), definition)


def build():
    translations = []
    for number, slug, title, de, tile, roughness, reflectance in STONES:
        desc = DESCRIPTIONS.get(tile)
        stone(number, slug, title, desc[0] if desc else None, tile, roughness, reflectance,
              directory='dripstone_cave')
        if desc:
            translations.extend([(title,de), desc])
    for number, slug, name, de, desc, desc_de in [
        (467, 'cave_pebbles', 'Cave Pebbles', 'Höhlengeröll', 'Rounded chips of cave limestone.', 'Abgerundete Kalksteinsplitter.'),
        (468, 'cave_crystal_cluster', 'Calcite Crystal Cluster', 'Calcitkristallgruppe', 'A warm fan of calcite prisms.', 'Ein warmer Fächer aus Calcitprismen.'),
    ]:
        outcrop(number, slug, name, desc, number == 468)
        translations.extend([(name, de), (desc, desc_de)])
    save_json(BLOCKS.parents[2]/'src-ui/locales/source/dripstone-cave.json',
              [dict(source=en,en=en,de=de) for en,de in translations])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--palette', help='1448 × 1086 dripstone reference for resampling the checked-in tiles')
    palette = parser.parse_args().palette
    if palette:
        sample_swatches(palette, BLOCKS/'textures/dripstone_cave', SWATCHES)
    build()
