#!/usr/bin/env python3
"""Author nine distinct one-cell profession depots using the project's mesh exporter."""
import json
import math
import struct
from build_castle import Mesh, BLOCKS, MODELS, RECIPES, unit, cross
from build_locomotive_parts import cylinder


DEFINITIONS = [
    (297, 'lumberjack_station', 'Holzfaeller-Station'),
    (298, 'smith_station', 'Schmiede-Station'),
    (299, 'market_station', 'Verkaufsstand'),
    (300, 'army_station', 'Armee-Station'),
    (301, 'railway_station', 'Lok-Station'),
    (302, 'mine_station', 'Minen-Station'),
    (303, 'butcher_station', 'Metzger-Station'),
    (304, 'bishop_station', 'Bischofs-Station'),
    (305, 'farm_station', 'Farmer-Station'),
]


def palette(mesh):
    mesh.materials.update({
        'station_wood': ([.78, .58, .32], 'oak-crafted-hd.png', 0, .86),
        'station_dark_wood': ([.36, .23, .12], 'machinery-oak-hd.png', 0, .92),
        'station_stone': ([.72, .75, .73], 'stone-strata-hd.png', 0, .95),
        'station_iron': ([.34, .38, .40], 'forged-iron-warm.png', .65, .45),
        'station_steel': ([.67, .73, .76], 'machinery-iron-hd.png', .7, .32),
        'station_gold': ([.88, .63, .21], None, .5, .4),
        'station_red': ([.70, .11, .09], None, 0, .9),
        'station_white': ([.93, .89, .75], None, 0, .94),
        'station_blue': ([.10, .29, .47], None, 0, .95),
        'station_green': ([.23, .43, .12], None, 0, .94),
        'station_hay': ([.83, .65, .20], None, 0, .98),
        'station_dark': ([.065, .07, .065], None, 0, .95),
        'station_orange': ([.9, .39, .07], None, 0, .9),
        'station_meat': ([.56, .15, .14], None, 0, .85),
    })


def beam(mesh, a, b, width, material):
    axis = unit([b[i] - a[i] for i in range(3)])
    side = unit(cross(axis, [0, 1, 0] if abs(axis[1]) < .9 else [1, 0, 0]))
    other = cross(axis, side)
    ends = []
    for point in [a, b]:
        ends.append([[point[i] + width * .5 * (s * side[i] + t * other[i])
                      for i in range(3)] for s, t in [(-1, -1), (1, -1), (1, 1), (-1, 1)]])
    mesh.polygon(list(reversed(ends[0])), material)
    mesh.polygon(ends[1], material)
    for i in range(4):
        mesh.polygon([ends[0][i], ends[0][(i + 1) % 4], ends[1][(i + 1) % 4], ends[1][i]], material)


def table(mesh, height=.61):
    for x in [.08, .81]:
        for z in [.10, .78]:
            mesh.box([x, .025, z], [x + .11, height, z + .12], 'station_dark_wood', solid=True)
    for i in range(5):
        mesh.box([.045, height, .045 + i * .18], [.955, height + .085, .215 + i * .18], 'station_wood', solid=True)
    mesh.box([.13, .18, .14], [.87, .24, .86], 'station_dark_wood', solid=True)
    # The depot drawer is visibly shared across the working tables.
    mesh.box([.24, .29, .19], [.76, height - .025, .80], 'station_wood', solid=True)
    mesh.box([.25, .34, .81], [.75, height - .045, .85], 'station_dark_wood')
    mesh.box([.44, .40, .85], [.56, .43, .88], 'station_iron')


def axe(mesh, a, b, material='station_steel'):
    beam(mesh, a, b, .045, 'station_dark_wood')
    mesh.box([b[0] - .14, b[1] - .025, b[2] - .025],
             [b[0] + .095, b[1] + .105, b[2] + .025], material)
    mesh.box([b[0] - .15, b[1] - .045, b[2] - .031],
             [b[0] - .115, b[1] + .115, b[2] + .031], 'station_white', .002)


def crate(mesh, low, high, fill=None):
    x, y, z = low
    u, v, w = high
    mesh.box(low, [u, y + .045, w], 'station_dark_wood', solid=True)
    for axis in [0, 2]:
        for side in [0, 1]:
            for row in range(3):
                a = [x, y + .03 + row * (v - y) / 3, z]
                b = [u, min(v, a[1] + (v - y) / 3 - .008), w]
                a[axis] = low[axis] if side == 0 else high[axis] - .035
                b[axis] = a[axis] + .035
                mesh.box(a, b, 'station_wood', .004, True)
    if fill:
        for i in range(3):
            for j in range(2):
                xx = x + .08 + i * (u - x - .16) / 2
                zz = z + .075 + j * (w - z - .15)
                mesh.box([xx - .035, v - .055, zz - .035], [xx + .035, v + .025, zz + .035], fill, .012)


def make(slug):
    mesh = Mesh(slug)
    palette(mesh)
    if slug == 'lumberjack_station':
        table(mesh)
        cylinder(mesh, [.29, 0, .32], .15, .695, .85, 1, 'station_dark_wood', sides=12, solid=True)
        cylinder(mesh, [.29, 0, .32], .138, .85, .862, 1, 'station_wood', sides=12)
        axe(mesh, [.44, .70, .61], [.32, .86, .35])
        for z in [.3, .47, .64]:
            cylinder(mesh, [0, .33, z], .07, .18, .77, 0, 'station_dark_wood', sides=10)
            cylinder(mesh, [0, .33, z], .06, .77, .785, 0, 'station_wood', sides=10)
        beam(mesh, [.67, .72, .15], [.85, .90, .54], .035, 'station_steel')
        for i in range(7):
            mesh.box([.655 + i * .025, .713 + i * .025, .16 + i * .05], [.67 + i * .025, .733 + i * .025, .18 + i * .05], 'station_steel', .002)
    elif slug == 'smith_station':
        mesh.masonry([.04, .02, .05], [.96, .47, .95])
        mesh.box([.20, .47, .18], [.80, .57, .80], 'station_dark', solid=True)
        for x in [.12, .72]:
            mesh.box([x, .48, .10], [x + .16, .77, .72], 'station_stone', solid=True)
        mesh.box([.12, .72, .10], [.88, .82, .29], 'station_stone', solid=True)
        mesh.box([.27, .53, .42], [.63, .61, .72], 'station_iron', solid=True)
        mesh.box([.34, .60, .47], [.56, .74, .67], 'station_iron', solid=True)
        mesh.box([.23, .73, .40], [.69, .82, .74], 'station_steel', solid=True)
        mesh.polygon([[.23, .73, .40], [.23, .82, .40], [.06, .78, .57]], 'station_steel')
        beam(mesh, [.66, .57, .80], [.82, .83, .56], .038, 'station_dark_wood')
        mesh.box([.72, .80, .51], [.91, .90, .62], 'station_iron')
        for i in range(4):
            mesh.box([.28 + i * .10, .54, .29], [.35 + i * .10, .59, .37], 'station_orange', .015)
    elif slug == 'market_station':
        table(mesh, .45)
        for x in [.07, .88]:
            for z in [.08, .85]:
                mesh.box([x, .05, z], [x + .05, .93, z + .05], 'station_dark_wood', .005, True)
        for i in range(8):
            x = .025 + i * .119
            material = 'station_red' if i % 2 == 0 else 'station_white'
            mesh.polygon([[x, .87, .02], [x + .119, .87, .02], [x + .119, .98, .48], [x, .98, .48]], material, unit([0, 1, -.2]))
            mesh.polygon([[x, .98, .48], [x + .119, .98, .48], [x + .119, .87, .98], [x, .87, .98]], material, unit([0, 1, .2]))
            mesh.box([x, .80, .02], [x + .118, .87, .045], material, .003)
        mesh.collision.append({'minimum': [.025, .82, .02], 'maximum': [.977, .98, .98]})
        crate(mesh, [.15, .54, .17], [.48, .69, .49], 'station_red')
        crate(mesh, [.53, .54, .17], [.85, .69, .49], 'station_green')
        mesh.box([.64, .54, .65], [.85, .59, .86], 'station_white')
        for x in [.67, .74, .81]:
            cylinder(mesh, [x, 0, .75], .028, .59, .62, 1, 'station_gold', sides=10)
    elif slug == 'army_station':
        mesh.masonry([.08, .01, .07], [.92, .32, .93])
        mesh.box([.18, .32, .18], [.82, .40, .82], 'station_iron', solid=True)
        mesh.box([.12, .36, .72], [.18, .98, .78], 'station_dark_wood', solid=True)
        mesh.box([.18, .66, .735], [.70, .96, .76], 'station_blue', .005)
        mesh.box([.38, .68, .765], [.43, .94, .78], 'station_gold', .002)
        mesh.box([.28, .80, .765], [.54, .85, .78], 'station_gold', .002)
        beam(mesh, [.30, .42, .27], [.70, .89, .27], .045, 'station_dark_wood')
        beam(mesh, [.70, .42, .33], [.30, .89, .33], .045, 'station_dark_wood')
        mesh.box([.235, .83, .30], [.42, .93, .36], 'station_steel', .006)
        mesh.box([.61, .83, .24], [.78, .93, .30], 'station_steel', .006)
        mesh.box([.34, .42, .14], [.67, .73, .20], 'station_iron', .04, True)
        mesh.box([.47, .48, .125], [.53, .66, .14], 'station_gold', .006)
    elif slug == 'railway_station':
        mesh.box([.06, .03, .10], [.94, .26, .90], 'station_dark_wood', solid=True)
        for z in [.2, .45, .70]:
            mesh.box([.10, .25, z], [.90, .30, z + .08], 'station_wood', .006)
        for x in [.25, .71]:
            mesh.box([x, .30, .10], [x + .04, .35, .88], 'station_steel', .003)
        cylinder(mesh, [0, .57, .55], .205, .15, .85, 0, 'station_iron', sides=16, solid=True)
        for x in [.18, .72]:
            cylinder(mesh, [0, .57, .55], .215, x, x + .035, 0, 'station_gold', sides=16)
        cylinder(mesh, [.68, 0, .55], .065, .74, .96, 1, 'station_dark', sides=12, solid=True)
        cylinder(mesh, [.68, 0, .55], .085, .93, .98, 1, 'station_iron', sides=12)
        cylinder(mesh, [.35, 0, .55], .08, .76, .86, 1, 'station_gold', sides=12)
        cylinder(mesh, [.5, .58, 0], .09, .31, .34, 2, 'station_white', sides=12)
        beam(mesh, [.50, .58, .304], [.55, .64, .304], .01, 'station_dark')
        crate(mesh, [.08, .30, .10], [.31, .48, .36], 'station_dark')
    elif slug == 'mine_station':
        mesh.masonry([.04, .01, .06], [.96, .27, .94])
        for x in [.22, .72]:
            mesh.box([x, .27, .10], [x + .04, .32, .90], 'station_steel', .003)
        crate(mesh, [.15, .40, .17], [.85, .67, .83], 'station_stone')
        for x in [.22, .72]:
            for z in [.25, .69]:
                cylinder(mesh, [0, .37, z], .07, x - .04, x + .04, 0, 'station_iron', sides=12)
        for i, material in enumerate(['station_gold', 'station_steel', 'station_dark']):
            mesh.box([.23 + i * .19, .65, .41], [.37 + i * .19, .79, .56], material, .022)
        beam(mesh, [.25, .69, .68], [.72, .92, .36], .035, 'station_dark_wood')
        beam(mesh, [.55, .96, .31], [.86, .87, .45], .047, 'station_steel')
    elif slug == 'butcher_station':
        table(mesh)
        mesh.box([.12, .70, .18], [.81, .73, .79], 'station_white', solid=True)
        mesh.box([.25, .73, .30], [.67, .82, .66], 'station_meat', .035)
        for i in range(4):
            mesh.box([.29 + i * .08, .81, .32], [.31 + i * .08, .825, .63], 'station_white', .003)
        beam(mesh, [.69, .74, .70], [.80, .89, .58], .035, 'station_dark_wood')
        mesh.box([.70, .84, .47], [.89, .97, .51], 'station_steel', .004)
        mesh.box([.13, .70, .79], [.32, .77, .87], 'station_dark_wood', .008)
    elif slug == 'bishop_station':
        mesh.masonry([.11, .01, .13], [.89, .30, .87])
        mesh.box([.32, .30, .30], [.68, .63, .70], 'station_dark_wood', solid=True)
        for i in range(5):
            z = .20 + i * .12
            mesh.box([.18, .62 + i * .025, z], [.82, .68 + i * .025, z + .12], 'station_wood', .006, True)
        mesh.box([.29, .78, .39], [.70, .82, .71], 'station_red', .005)
        mesh.box([.31, .82, .40], [.68, .845, .69], 'station_white', .004)
        mesh.box([.48, .845, .40], [.50, .85, .69], 'station_gold', .002)
        for x in [.18, .78]:
            cylinder(mesh, [x, 0, .17], .055, .30, .35, 1, 'station_gold', sides=12)
            cylinder(mesh, [x, 0, .17], .018, .35, .79, 1, 'station_gold', sides=10)
            cylinder(mesh, [x, 0, .17], .027, .79, .94, 1, 'station_white', sides=10)
            mesh.box([x - .015, .94, .155], [x + .015, .98, .185], 'station_orange', .007)
    elif slug == 'farm_station':
        table(mesh, .43)
        crate(mesh, [.11, .52, .50], [.50, .69, .87], 'station_green')
        mesh.box([.55, .52, .23], [.87, .80, .76], 'station_hay', .025, True)
        for z in [.32, .61]:
            mesh.box([.54, .515, z], [.88, .81, z + .025], 'station_dark_wood', .003)
        for i in range(9):
            x = .56 + i * .033
            mesh.box([x, .54, .215], [x + .01, .78, .23], 'station_white', .002)
        beam(mesh, [.23, .51, .27], [.35, .94, .20], .03, 'station_dark_wood')
        for x in [.29, .34, .39]:
            beam(mesh, [x, .84, .20], [x, .98, .20], .015, 'station_steel')
        mesh.box([.28, .83, .19], [.40, .86, .22], 'station_steel', .002)
    return mesh


def export_glb(mesh, slug):
    textures = mesh.export(slug)
    source = MODELS / (slug + '.gltf')
    binary = MODELS / (slug + '.bin')
    document = json.loads(source.read_text())
    document['asset']['generator'] = 'TERAMIC profession station builder'
    document['buffers'][0].pop('uri')
    encoded = json.dumps(document, separators=(',', ':')).encode()
    encoded += b' ' * ((-len(encoded)) % 4)
    data = binary.read_bytes()
    data += b'\0' * ((-len(data)) % 4)
    result = struct.pack('<III', 0x46546c67, 2, 12 + 8 + len(encoded) + 8 + len(data))
    result += struct.pack('<I4s', len(encoded), b'JSON') + encoded
    result += struct.pack('<I4s', len(data), b'BIN\0') + data
    (MODELS / (slug + '.glb')).write_bytes(result)
    source.unlink()
    binary.unlink()
    return textures


for material, slug, name in DEFINITIONS:
    mesh = make(slug)
    textures = export_glb(mesh, slug)
    definition = dict(schema_version=1, id=material, name=name,
                      description='Berufsstation mit eigenem Lager fuer Werkzeuge und Waren.',
                      creative_category='decoration', render_kind='custom_mesh', model_layout='scene',
                      model=f'models/{slug}.glb', struct_block=True, hardness=1.4, resistance=6,
                      required_tool=None, mining_level=0, textures=textures, collision_boxes=mesh.collision)
    (BLOCKS / (slug + '.json')).write_text(json.dumps(definition, indent=2) + '\n')
    ingredients = [dict(item=dict(block=16), wood=True, count=2),
                   dict(item=dict(block=29), count=1)]
    extra = {297: dict(tool='iron_axe'), 298: dict(mineral='iron_ingot'), 299: dict(block=105),
             300: dict(mineral='iron_ingot'), 301: dict(block=196), 302: dict(tool='iron_pickaxe'),
             303: dict(mineral='iron_ingot'), 304: dict(block=40), 305: dict(crop='wheat') }[material]
    ingredients.append(dict(item=extra, count={298: 3, 300: 2, 304: 2, 305: 4}.get(material, 1)))
    # Counts alone do not distinguish shapeless recipes: one iron also matches
    # a grid containing three. Each metal station needs a distinct real component.
    if material in (298, 300, 303):
        component = {298: dict(block=3), 300: dict(block=103), 303: dict(animal='beef')}[material]
        ingredients.append(dict(item=component, count=1))
    recipe = dict(id='teramic::' + slug, name=name, station='workbench',
                  output=dict(item=dict(block=material), count=1),
                  pattern=dict(kind='shapeless', ingredients=ingredients))
    (RECIPES / (slug + '.json')).write_text(json.dumps(recipe, indent=2) + '\n')
