#!/usr/bin/env python3
"""Author the complete BR52-inspired locomotive palette from deterministic geometry.

The exported mesh and collision volumes share meter units. This tool only writes
native assets and catalogs; it never opens a window or starts the game renderer.
"""
import json
import math
import re
import base64

from build_castle import BLOCKS, MODELS, RECIPES, ROOT, Mesh


PARTS = [
    (218, 'train_fuel_engine', 'Brennstoffmotor', 'Engine'),
    (219, 'train_boiler_straight', 'Lok-Kessel gerade', 'Body'),
    (220, 'train_boiler_curved', 'Lok-Kessel gebogen', 'Body'),
    (221, 'train_boiler_end', 'Lok-Kessel Abschluss', 'Body'),
    (222, 'train_smokebox', 'Rauchkammer', 'Body'),
    (223, 'train_smokebox_door', 'Rauchkammertür 52 4867', 'Body'),
    (224, 'train_chimney', 'Lok-Schornstein', 'Body'),
    (225, 'train_steam_dome', 'Dampfdom', 'Body'),
    (226, 'train_sand_dome', 'Sanddom', 'Body'),
    (227, 'train_brass_pipe_straight', 'Messingrohr gerade', 'Body'),
    (228, 'train_brass_pipe_bend', 'Messingrohr Bogen', 'Body'),
    (229, 'train_steam_valve', 'Lok-Dampfventil', 'Valve'),
    (230, 'train_pressure_gauge', 'Lok-Druckanzeige', 'Gauge'),
    (231, 'train_pressure_unit', 'Lok-Druckeinheit', 'Gauge'),
    (232, 'train_wheel_large', 'Treibrad groß', 'Wheel'),
    (233, 'train_wheel_medium', 'Lokrad mittel', 'Wheel'),
    (234, 'train_wheel_small', 'Laufrad klein', 'Wheel'),
    (235, 'train_axle', 'Lok-Achse', 'Axle'),
    (236, 'train_connecting_rod_long', 'Kuppelstange lang', 'Rod'),
    (237, 'train_connecting_rod_short', 'Kuppelstange kurz', 'Rod'),
    (238, 'train_rod_joint', 'Gestänge-Gelenk', 'Rod'),
    (239, 'train_piston_rod', 'Treibstange', 'Rod'),
    (240, 'train_crosshead', 'Kreuzkopf', 'Rod'),
    (241, 'train_main_frame', 'Lok-Hauptrahmen', 'Structure'),
    (242, 'train_red_frame', 'Lokrahmen rot', 'Structure'),
    (243, 'train_buffer_beam', 'Pufferbohle', 'Structure'),
    (244, 'train_buffer', 'Lok-Puffer', 'Body'),
    (245, 'train_cab_wall_side', 'Führerhaus-Seitenwand', 'Body'),
    (246, 'train_cab_wall_front', 'Führerhaus-Frontwand', 'Body'),
    (247, 'train_cab_window', 'Führerhaus-Fenster', 'Body'),
    (248, 'train_cab_door', 'Führerhaus-Tür', 'Body'),
    (249, 'train_roof_curved', 'Lokdach gebogen', 'Body'),
    (250, 'train_roof_flat', 'Lokdach flach', 'Body'),
    (251, 'train_roof_vent', 'Lok-Dachlüfter', 'Body'),
    (252, 'train_tender_wall', 'Tenderwand', 'Body'),
    (253, 'train_tender_front', 'Tenderfront', 'Body'),
    (254, 'train_water_tank', 'Tender-Wassertank', 'Tank'),
    (255, 'train_coal_load', 'Tender-Kohleladung', 'Body'),
    (256, 'train_ladder_side', 'Lokleiter Seite', 'Body'),
    (257, 'train_ladder_front', 'Lokleiter Front', 'Body'),
    (258, 'train_railing', 'Lok-Geländer', 'Body'),
    (259, 'train_handrail', 'Lok-Handlauf', 'Body'),
    (260, 'train_handrail_post', 'Handlauf-Stütze', 'Body'),
    (261, 'train_rivet_plate', 'Lok-Nietenplatte', 'Body'),
    (262, 'train_bolt', 'Lok-Schraubenblock', 'Body'),
    (263, 'train_hatch', 'Lok-Luke', 'Body'),
    (264, 'train_headlight_front', 'Lok-Scheinwerfer Front', 'Headlight'),
    (265, 'train_headlight_side', 'Lok-Scheinwerfer Seite', 'Headlight'),
    (266, 'train_number_plate', 'Lok-Nummernschild 52 4867', 'Body'),
    (267, 'train_toolbox', 'Lok-Werkzeugkasten', 'Body'),
    (268, 'train_wooden_plank', 'Lok-Holzplanken', 'Body'),
    (269, 'train_coal_piece', 'Lok-Kohlestücke', 'Body'),
]
FAMILIES = [('black_iron', 'Schwarzes Eisen'), ('dark_steel', 'Dunkler Stahl'),
            ('sheet_metal', 'Lok-Blech'), ('grate', 'Lok-Gitter')]
SHAPES = [('block', 'Block'), ('slab', 'Slab'), ('stairs', 'Treppe'),
          ('inner_corner', 'Innenecke'), ('outer_corner', 'Außenecke'), ('slope', 'Schräge')]
for family_index, (family, label) in enumerate(FAMILIES):
    for shape_index, (shape, shape_label) in enumerate(SHAPES):
        PARTS.append((270 + family_index * 6 + shape_index,
                      'train_' + family + '_' + shape, label + ' ' + shape_label, 'Structure'))

PARTS.append((294, 'train_ceiling_light', 'Wagen-Deckenleuchte', 'Headlight'))

GLYPHS = {
    ' ': ['000','000','000','000','000'],
    '0': ['111','101','101','101','111'], '1': ['010','110','010','010','111'],
    '2': ['111','001','111','100','111'], '3': ['111','001','111','001','111'],
    '4': ['101','101','111','001','001'], '5': ['111','100','111','001','111'],
    '6': ['111','100','111','101','111'], '7': ['111','001','010','010','010'],
    '8': ['111','101','111','101','111'], '9': ['111','101','111','001','111'],
}


def palette(mesh):
    mesh.materials.update({
        'train_black_iron': ([.25, .265, .28], 'machinery-iron-hd.png', .56, .66),
        'train_dark_steel': ([.30, .315, .335], 'machinery-iron-hd.png', .62, .60),
        'train_sheet_metal': ([.30, .31, .315], 'machinery-iron-hd.png', .78, .59),
        'train_grate': ([.10, .115, .13], 'machinery-iron-hd.png', .88, .57),
        'train_red': ([.52, .055, .035], 'machinery-iron-hd.png', .66, .6),
        'train_brass': ([.90, .56, .14], None, .82, .35),
        'train_coal': ([.035, .04, .045], 'stone.png', .08, .95),
        'train_wheel_tire': ([.115, .12, .13], 'machinery-iron-hd.png', .88, .42),
        'train_wheel_spokes': ([.64, .055, .035], 'machinery-iron-hd.png', .08, .62),
        'train_axle': ([.29, .30, .31], 'machinery-iron-hd.png', .88, .42),
        'train_rod_main': ([.54, .55, .56], 'machinery-iron-hd.png', .9, .37),
        'train_rod_joint': ([.65, .64, .60], 'machinery-iron-hd.png', .9, .33),
        'train_rod_piston': ([.59, .60, .61], 'machinery-iron-hd.png', .92, .3),
        'train_rod_crosshead': ([.37, .38, .39], 'machinery-iron-hd.png', .88, .43),
        'train_lamp_glow': ([1.0, .79, .38], None, 0, .23),
        'train_gauge_face': ([.88, .83, .68], None, .05, .86),
        'train_gauge_needle': ([.61, .04, .025], None, .15, .55),
        'train_valve_handle': ([.57, .06, .035], 'machinery-iron-hd.png', .7, .53),
    })


def cylinder(mesh, center, radius, low, high, axis, material, sides=16, solid=True, inner=0):
    """Capped polygonal cylinder or annulus along one cardinal axis."""
    u, v = (axis + 1) % 3, (axis + 2) % 3
    ring = [(math.cos(i * math.tau / sides), math.sin(i * math.tau / sides)) for i in range(sides)]
    def point(distance, r, angle):
        p = list(center); p[axis] = distance
        p[u] += r * angle[0]; p[v] += r * angle[1]
        return p
    for index, a in enumerate(ring):
        b = ring[(index + 1) % sides]
        mesh.polygon([point(low, radius, a), point(low, radius, b),
                      point(high, radius, b), point(high, radius, a)], material)
        if inner:
            mesh.polygon([point(low, inner, a), point(high, inner, a),
                          point(high, inner, b), point(low, inner, b)], material)
            for d in [low, high]:
                face = [point(d, inner, a), point(d, radius, a),
                        point(d, radius, b), point(d, inner, b)]
                mesh.polygon(list(reversed(face)) if d == low else face, material)
        else:
            mesh.polygon([point(low, 0, a), point(low, radius, b), point(low, radius, a)], material)
            mesh.polygon([point(high, 0, a), point(high, radius, a), point(high, radius, b)], material)
    if solid:
        # Thin chord strips retain the silhouette without a full-cell hit box.
        for index in range(8):
            first = -radius + 2 * radius * index / 8
            last = -radius + 2 * radius * (index + 1) / 8
            near = min(abs(first), abs(last)) if first * last > 0 else 0
            extent = math.sqrt(max(0, radius * radius - near * near))
            p = list(center); q = list(center)
            p[axis] = low; q[axis] = high
            p[u] += first; q[u] += last
            p[v] -= extent; q[v] += extent
            mesh.collision.append({'minimum': p, 'maximum': q})


def rod(mesh, start, end, width, material, solid=False):
    direction = [end[i] - start[i] for i in range(3)]
    length = math.sqrt(sum(v * v for v in direction))
    direction = [v / length for v in direction]
    normal = [0, 1, 0] if abs(direction[1]) < .9 else [1, 0, 0]
    u = [direction[1] * normal[2] - direction[2] * normal[1],
         direction[2] * normal[0] - direction[0] * normal[2],
         direction[0] * normal[1] - direction[1] * normal[0]]
    size = math.sqrt(sum(v * v for v in u)); u = [v / size * width / 2 for v in u]
    v = [direction[1] * u[2] - direction[2] * u[1],
         direction[2] * u[0] - direction[0] * u[2],
         direction[0] * u[1] - direction[1] * u[0]]
    corners = [[p[i] + a * u[i] + b * v[i] for i in range(3)]
               for p in [start, end] for a, b in [(-1, -1), (1, -1), (1, 1), (-1, 1)]]
    for ids in [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]]:
        mesh.polygon([corners[i] for i in ids], material)
    if solid:
        mesh.collision.append({'minimum': [min(p[i] for p in corners) for i in range(3)],
                               'maximum': [max(p[i] for p in corners) for i in range(3)]})


def rivets(mesh, axis=2, plane=.015, low=.08, high=.92, y_low=.08, y_high=.92):
    for t in [low, (low + high) / 2, high]:
        for y in [y_low, y_high]:
            p = [t - .022, y - .022, plane]; q = [t + .022, y + .022, plane + .025]
            if axis == 0:
                p = [plane, y - .022, t - .022]; q = [plane + .025, y + .022, t + .022]
            mesh.box(p, q, 'rivet', bevel=.008)


def text(mesh, value, x=.16, y=.39, z=.01, width=.68, height=.20):
    pixel_x = width / (len(value) * 4 - 1)
    pixel_y = height / 5
    for letter, char in enumerate(value):
        for row, line in enumerate(GLYPHS[char]):
            for column, bit in enumerate(line):
                if bit == '1':
                    p = [x + (letter * 4 + column) * pixel_x,
                         y + (4 - row) * pixel_y, z]
                    mesh.box(p, [p[0] + pixel_x * .9, p[1] + pixel_y * .9, z + .008], 'train_brass', bevel=0)


def wheel(mesh, radius, center=(.5, .5, .5), low=.40, high=.60, prefix='train_wheel'):
    """Flanged steel tire, red spokes, counterweight and eccentric crank pin."""
    tire, spokes, rim = prefix + '_tire', prefix + '_spokes', prefix + '_rim'
    mesh.materials[tire] = mesh.materials['train_wheel_tire']
    mesh.materials[spokes] = mesh.materials['train_wheel_spokes']
    mesh.materials[rim] = ([.56, .58, .59], None, .94, .30)
    cylinder(mesh, center, radius, low, high, 0, tire, inner=radius * .82, solid=False, sides=32)
    for rim_low, rim_high in [(low - .012, low + .002), (high - .002, high + .012)]:
        cylinder(mesh, center, radius * .985, rim_low, rim_high, 0, rim, inner=radius * .91, solid=False, sides=32)
    cylinder(mesh, center, radius * .17, low - .015, high + .015, 0, spokes, solid=False, sides=20)
    for index in range(12):
        angle = index * math.tau / 12
        p = [(low + high) / 2, center[1], center[2]]
        q = [p[0], center[1] + math.cos(angle) * radius * .85, center[2] + math.sin(angle) * radius * .85]
        rod(mesh, p, q, radius * .075, spokes)
    # The balance weight sits opposite the eccentric pin and turns with the rim.
    for side, face in enumerate([low - .003, high + .003]):
        points = [[face, center[1], center[2]]]
        for index in range(9):
            angle = math.pi * .72 + index * math.pi * .56 / 8
            points.append([face, center[1] + math.sin(angle) * radius * .73, center[2] + math.cos(angle) * radius * .73])
        mesh.polygon(list(reversed(points)) if side else points, spokes)
    crank = [center[0], center[1], center[2] + min(.12, radius * .30)]
    cylinder(mesh, crank, radius * .085, low - .018, high + .018, 0, rim, solid=False, sides=16)
    mesh.collision.append({'minimum': [low - .018, center[1] - radius, center[2] - radius],
                           'maximum': [high + .018, center[1] + radius, center[2] + radius]})


def boiler(mesh, material='train_black_iron', door=False):
    cylinder(mesh, [.5, .5, .5], .48, 0, 1, 2, material)
    for x in [.20, .72]:
        mesh.box([x, 0, .16], [x + .08, .20, .84], 'train_black_iron', solid=True)
    for x in [.025, .975]:
        cylinder(mesh, [x, .62, .5], .014, 0, 1, 2, 'train_brass', solid=False, sides=8)
    for z in [.03, .91]:
        cylinder(mesh, [.5, .5, .5], .49, z, z + .055, 2, 'train_dark_steel', inner=.465, solid=False)
    for z in [.10, .50, .90]:
        for angle in range(0, 16, 2):
            a = angle * math.tau / 16
            p = [.5 + math.cos(a) * .465 - .017, .5 + math.sin(a) * .465 - .017, z - .017]
            mesh.box(p, [p[0] + .034, p[1] + .034, z + .017], 'rivet', bevel=.007)
    if door:
        mesh.box([.15, .35, .001], [.85, .65, .028], 'train_dark_steel', bevel=.012)
        # The runtime renders the saved vehicle name on a separate front plane.
        for x in [.13, .81]:
            mesh.box([x, .31, .03], [x + .06, .7, .12], 'rivet')


def curved_boiler(mesh):
    rows, sides, radius, bend = 12, 16, .48, .5
    rings = []
    for step in range(rows + 1):
        t = math.pi / 2 * step / rows
        rings.append([[math.sin(t) * (bend + math.cos(i * math.tau / sides) * radius),
                       .5 + math.sin(i * math.tau / sides) * radius,
                       math.cos(t) * (bend + math.cos(i * math.tau / sides) * radius)]
                      for i in range(sides)])
    for step in range(rows):
        for i in range(sides):
            j = (i + 1) % sides
            mesh.polygon([rings[step][i], rings[step + 1][i], rings[step + 1][j], rings[step][j]], 'train_black_iron')
        points = rings[step] + rings[step + 1]
        mesh.collision.append({'minimum': [min(p[i] for p in points) for i in range(3)],
                               'maximum': [max(p[i] for p in points) for i in range(3)]})
    mesh.polygon(rings[0], 'train_dark_steel')
    mesh.polygon(list(reversed(rings[-1])), 'train_dark_steel')
    for x, z in [(.15, .34), (.34, .15)]:
        mesh.box([x, 0, z], [x + .18, .20, z + .18], 'train_black_iron', solid=True)
    for ring in [rings[1], rings[-2]]:
        for index in range(0, sides, 2):
            p = [max(.001, min(.965, c - .015)) for c in ring[index]]
            mesh.box(p, [c + .03 for c in p], 'rivet', bevel=.006)


def wall(mesh, window=False, door=False, sill=.32):
    if not window and not door:
        mesh.box([0, 0, 0], [1, 1, .16], 'train_black_iron', solid=True)
    else:
        for p, q in [([0, 0, 0], [.14, 1, .16]), ([.86, 0, 0], [1, 1, .16]),
                     ([.14, .88, 0], [.86, 1, .16])]:
            mesh.box(p, q, 'train_black_iron', solid=True)
        if window:
            mesh.box([.14, 0, 0], [.86, sill, .16], 'train_black_iron', solid=True)
            mesh.box([.14, sill, .067], [.86, .88, .09], 'glass', bevel=0)
            for x in [.14, .48, .82]:
                mesh.box([x, sill, .025], [x + .035, .88, .125], 'train_brass')
        if door:
            mesh.box([.18, .02, .035], [.82, .9, .095], 'train_dark_steel', solid=True)
            mesh.box([.25, .46, .02], [.75, .80, .035], 'glass', bevel=0)
            mesh.box([.71, .39, .005], [.77, .44, .10], 'train_brass')
    rivets(mesh)


def metal_shape(mesh, family, shape):
    material = 'train_' + family
    boxes = []
    if shape == 'block':
        boxes = [([0, 0, 0], [1, 1, 1])]
    elif shape == 'slab':
        boxes = [([0, 0, 0], [1, .5, 1])]
    elif shape == 'stairs':
        boxes = [([0, i / 3, i / 3], [1, (i + 1) / 3, 1]) for i in range(3)]
    elif shape == 'inner_corner':
        boxes = [([0, 0, 0], [1, .5, 1]), ([0, .5, 0], [.5, 1, 1]), ([.5, .5, .5], [1, 1, 1])]
    elif shape == 'outer_corner':
        boxes = [([0, 0, 0], [1, .5, 1]), ([0, .5, .5], [.5, 1, 1])]
    else:
        # A true planar slope in the mesh, with tightly bounded collision columns.
        if family == 'grate':
            for x in [.03, .25, .5, .75, .94]:
                rod(mesh, [x, .035, .035], [x, .965, .965], .045, material)
                for i in range(12):
                    z0, z1 = i / 12, (i + 1) / 12
                    mesh.collision.append({'minimum': [x - .025, max(0, z0 - .025), z0],
                                           'maximum': [x + .025, min(1, z1 + .025), z1]})
            for z in [.03, .25, .5, .75, .97]:
                rod(mesh, [0, z, z], [1, z, z], .045, material)
                mesh.collision.append({'minimum': [0, max(0, z - .025), max(0, z - .025)],
                                       'maximum': [1, min(1, z + .025), min(1, z + .025)]})
            return
        points = [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1], [1, 1, 1], [0, 1, 1]]
        for ids in [[0, 3, 2, 1], [0, 1, 4, 5], [3, 5, 4, 2], [0, 5, 3], [1, 2, 4]]:
            mesh.polygon([points[i] for i in ids], material)
        for i in range(12):
            mesh.collision.append({'minimum': [0, 0, i / 12], 'maximum': [1, (i + 1) / 12, (i + 1) / 12]})
        return
    for p, q in boxes:
        if family == 'grate':
            # Open lattice. Its bars, rather than an invisible solid cube, collide.
            for axis in [0, 2]:
                other = 2 - axis
                for step in [0, .25, .5, .75, .94]:
                    low, high = list(p), list(q)
                    low[other] = p[other] + (q[other] - p[other]) * step
                    high[other] = min(q[other], low[other] + .06)
                    if high[other] > low[other]:
                        mesh.box(low, high, material, bevel=.004, solid=True)
        else:
            mesh.box(p, q, material, solid=True)
    if family != 'grate':
        for axis in [0, 2]:
            rivets(mesh, axis, .005, y_high=.42 if shape == 'slab' else .90)


def make(material, slug):
    mesh = Mesh(slug); palette(mesh)
    if 270 <= material <= 293:
        family = FAMILIES[(material - 270) // 6][0]
        shape = SHAPES[(material - 270) % 6][0]
        metal_shape(mesh, family, shape)
    elif material == 218:
        mesh.box([.04, 0, .02], [.96, .91, .98], 'train_black_iron', solid=True)
        mesh.box([.12, .10, .001], [.88, .67, .09], 'train_red')
        mesh.box([.19, .17, 0], [.81, .57, .035], 'train_coal')
        for x in [.25, .40, .55, .70]:
            mesh.box([x, .20, .001], [x + .035, .54, .07], 'train_dark_steel')
        cylinder(mesh, [.5, .71, .5], .13, .92, 1, 1, 'train_brass')
        rivets(mesh)
    elif material in [219, 221, 222, 223]:
        boiler(mesh, 'train_dark_steel' if material == 222 else 'train_black_iron', material == 223)
        if material == 221:
            cylinder(mesh, [.5, .5, .5], .44, .01, .13, 2, 'train_dark_steel')
    elif material == 220:
        curved_boiler(mesh)
    elif material in [224, 225, 226]:
        # A thin visible saddle overlaps the boiler skin; the occupied fitting
        # volume stays inside its own cell, as the shared collision contract.
        mesh.box([.12, -.14, .12], [.88, .10, .88], 'train_black_iron', solid=False)
        mesh.box([.12, 0, .12], [.88, .12, .88], 'train_black_iron', solid=True)
        if material == 224:
            cylinder(mesh, [.5, .5, .5], .26, .10, .90, 1, 'train_black_iron', inner=.20)
            cylinder(mesh, [.5, .5, .5], .32, .84, .98, 1, 'train_brass', inner=.20, solid=False)
        else:
            for radius, low, high in [(.35, .10, .22), (.32, .22, .36), (.25, .36, .48), (.14, .48, .55)]:
                cylinder(mesh, [.5, .5, .5], radius, low, high, 1, 'train_black_iron')
            cylinder(mesh, [.5,.5,.5], .08, .54, .62, 1, 'train_brass')
    elif material in [227, 228, 229]:
        if material == 227:
            cylinder(mesh, [.5, .5, .5], .065, 0, 1, 2, 'train_brass')
            for z in [.04, .9]: cylinder(mesh, [.5, .5, .5], .10, z, z + .06, 2, 'train_brass', sides=8)
        else:
            cylinder(mesh, [.5, .5, .5], .065, 0, .52, 2, 'train_brass')
            cylinder(mesh, [.5, .5, .5], .065, .48, 1, 1, 'train_brass')
            cylinder(mesh, [.5, .5, .5], .10, .46, .56, 2, 'train_brass')
            if material == 229:
                cylinder(mesh, [.5, .5, .5], .15, .20, .8, 2, 'train_dark_steel')
                cylinder(mesh, [.5, .7, .5], .22, .68, .75, 1, 'train_valve_handle', inner=.15, solid=False)
                for axis in [0, 2]:
                    p = [.5, .715, .5]; q = list(p); p[axis] -= .21; q[axis] += .21
                    rod(mesh, p, q, .04, 'train_valve_handle')
    elif material in [230, 231]:
        mesh.box([.16, 0, .24], [.84, .34, .77], 'train_dark_steel', solid=True)
        cylinder(mesh, [.5, .62, .5], .27, .11, .30, 2, 'train_brass')
        cylinder(mesh, [.5, .62, .5], .23, .095, .12, 2, 'train_gauge_face', solid=False)
        rod(mesh, [.5, .62, .089], [.64, .75, .089], .021, 'train_gauge_needle')
        for step in range(9):
            a = -.8 * math.pi + step * 1.6 * math.pi / 8
            p = [.5 + math.sin(a) * .18, .62 + math.cos(a) * .18, .091]
            q = [.5 + math.sin(a) * .21, .62 + math.cos(a) * .21, .091]
            rod(mesh, p, q, .012, 'train_black_iron')
        if material == 231:
            for x in [.2, .8]: cylinder(mesh, [x, .18, .5], .045, 0, .5, 2, 'train_brass')
    elif material in [232, 233, 234]:
        wheel(mesh, .85, [.02, .90, .5], -.07, .11) if material == 232 else wheel(mesh, {233: .38, 234: .28}[material])
        cylinder(mesh, [.5, .5, .5], .075, 0, .50, 0, 'train_axle')
    elif material == 235:
        cylinder(mesh, [.5, .5, .5], .12, 0, 1, 0, 'train_axle')
        for x in [.03, .82]: mesh.box([x, .28, .28], [x + .15, .72, .72], 'train_red', solid=True)
    elif material in [236, 237, 238, 239, 240]:
        length = .95 if material in [236, 239] else .6
        name = {236: 'train_rod_main', 237: 'train_rod_main', 238: 'train_rod_joint',
                239: 'train_rod_piston', 240: 'train_rod_crosshead'}[material]
        p = [.12, .46, .025]; q = [.31, .56, length]
        mesh.box(p, q, name, solid=True)
        for z in [.12, length - .08]:
            collar = 'train_rod_joint' if material in [236, 237, 238] else name
            cylinder(mesh, [.5, .5, z], .12, 0, .36, 0, collar)
        if material in [238, 240]:
            mesh.box([.05, .26, .27], [.40, .75, .73], name, solid=True)
    elif material in [241, 242, 243]:
        mesh.box([0, 0, 0], [1, 1, 1], 'train_red' if material != 241 else 'train_black_iron', solid=True)
        for axis in [0, 2]: rivets(mesh, axis, .005)
        if material == 243:
            for x in [.13, .73]:
                cylinder(mesh, [x, .5, .5], .11, .0, .20, 2, 'train_dark_steel')
    elif material == 244:
        cylinder(mesh, [.5, .5, .5], .12, 0, .8, 2, 'train_dark_steel')
        cylinder(mesh, [.5, .5, .5], .32, .76, .96, 2, 'train_black_iron')
        mesh.box([.20, .2, 0], [.80, .8, .1], 'train_red', solid=True)
    elif material in [245, 246, 247, 248, 252, 253]:
        wall(mesh, window=material in [246, 247], door=material == 248,
             sill=.08 if material == 246 else .32)
        if material in [252, 253]:
            mesh.box([0, .88, .08], [1, 1, .35], 'train_dark_steel', solid=True)
    elif material in [249, 250, 251]:
        if material == 249:
            for i in range(8):
                x0, x1 = i / 8, (i + 1) / 8
                y = .10 + .22 * math.sin(i / 7 * math.pi)
                mesh.box([x0, max(0, y - .10), 0], [x1, y, 1], 'train_black_iron', solid=True)
        else:
            mesh.box([0, 0, 0], [1, .18, 1], 'train_black_iron', solid=True)
        if material == 251:
            mesh.box([.24, .18, .24], [.76, .57, .76], 'train_dark_steel', solid=True)
            for z in [.25, .39, .53, .67]: mesh.box([.2, .57, z], [.8, .62, z + .055], 'rivet')
    elif material == 254:
        mesh.box([0, 0, 0], [1, .93, 1], 'train_dark_steel', solid=True)
        for x in [.12, .82]: mesh.box([x, 0, 0], [x + .06, .96, 1], 'train_black_iron')
        cylinder(mesh, [.5, .5, .5], .17, .92, 1, 1, 'train_brass')
    elif material in [255, 269]:
        if material == 255: mesh.box([0, 0, 0], [1, .42, 1], 'train_black_iron', solid=True)
        for x in range(4):
            for z in range(4):
                height = .28 + ((x * 7 + z * 3) % 5) * .09
                bottom = .36 if material == 255 else 0
                mesh.box([x / 4 + .01, bottom, z / 4 + .01],
                         [(x + 1) / 4 - .01, bottom + height, (z + 1) / 4 - .01],
                         'train_coal', bevel=.055, solid=True)
    elif material in [256, 257]:
        for x in [.14, .80]: mesh.box([x, 0, .02], [x + .055, 1, .12], 'train_dark_steel', solid=True)
        for y in [.12, .35, .58, .81]: mesh.box([.14, y, .02], [.855, y + .045, .12], 'rivet', solid=True)
        if material == 256:
            for x in [.14, .80]: mesh.box([x, .08, .10], [x + .055, .15, .33], 'train_black_iron', solid=True)
    elif material in [258, 259, 260]:
        for x in ([.10, .85] if material == 258 else [.48]):
            cylinder(mesh, [x, .5, .5], .025, 0, .95, 1, 'train_brass')
            mesh.box([x - .055, 0, .45], [x + .055, .09, .55], 'train_dark_steel', solid=True)
        if material != 260:
            cylinder(mesh, [.5, .90, .5], .028, 0, 1, 0, 'train_brass')
            if material == 258: cylinder(mesh, [.5, .4, .5], .021, .1, .9, 0, 'train_brass')
    elif material in [261, 262, 263, 266]:
        depth = .15 if material == 262 else .065
        mesh.box([0, 0, 0], [1, 1, depth], 'train_black_iron', solid=True)
        rivets(mesh)
        if material == 262:
            cylinder(mesh, [.5, .5, .5], .27, .14, .40, 2, 'train_dark_steel', sides=6)
            cylinder(mesh, [.5, .5, .5], .10, .39, .6, 2, 'rivet')
        elif material == 263:
            mesh.box([.11, .11, .04], [.89, .89, .15], 'train_dark_steel', solid=True)
            mesh.box([.36, .35, .15], [.64, .42, .22], 'train_brass')
        elif material == 266:
            pass  # Runtime lettering occupies its own front plane.
    elif material == 294:
        mesh.box([.12, .84, .18], [.88, 1, .82], 'train_black_iron', solid=True)
        mesh.box([.2, .78, .25], [.8, .85, .75], 'train_lamp_glow', solid=True)
    elif material in [264, 265]:
        mesh.box([.25, 0, .32], [.75, .22, .78], 'train_black_iron', solid=True)
        cylinder(mesh, [.5, .55, .5], .29, .14, .75, 2, 'train_dark_steel')
        cylinder(mesh, [.5, .55, .5], .24, .12, .16, 2, 'train_brass', inner=.2, solid=False)
        cylinder(mesh, [.5, .55, .5], .20, .10, .13, 2, 'train_lamp_glow', solid=False)
        if material == 265:
            mesh.box([.1, .1, .55], [.4, .22, .9], 'train_black_iron', solid=True)
    elif material == 267:
        mesh.box([.05, 0, .1], [.95, .64, .9], 'oak', solid=True)
        mesh.box([.03, .64, .08], [.97, .77, .92], 'oak', solid=True)
        for x in [.12, .80]: mesh.box([x, 0, .07], [x + .08, .77, .93], 'train_dark_steel')
        mesh.box([.4, .48, .03], [.6, .65, .11], 'train_brass')
    elif material == 268:
        for z in range(4):
            mesh.box([0, 0, z / 4 + .004], [1, .26, (z + 1) / 4 - .004], 'oak', solid=True)
            for x in [.08, .9]: mesh.box([x, .26, z / 4 + .08], [x + .025, .28, z / 4 + .105], 'rivet')
    assert mesh.parts, slug
    return mesh


def recipe(material, slug, role):
    if 270 <= material <= 293:
        centre = 210 if (material - 270) % 6 == 0 else material - (material - 270) % 6
    else:
        centre = {218: 59, 219: 210, 220: 219, 221: 219, 222: 219, 223: 222,
                  224: 219, 225: 219, 226: 219, 227: 165, 228: 227, 229: 166,
                  230: 143, 231: 230, 232: 143, 233: 143, 234: 142, 235: 146,
                  236: 210, 237: 210, 238: 143, 239: 146, 240: 143,
                  241: 210, 242: 241, 243: 242, 244: 210, 245: 241, 246: 245,
                  247: 15, 248: 245, 249: 241, 250: 241, 251: 250, 252: 241,
                  253: 252, 254: 169, 255: 32, 256: 210, 257: 210, 258: 146,
                  259: 146, 260: 146, 261: 210, 262: 143, 263: 210,
                  264: 193, 265: 193, 266: 210, 267: 29, 268: 18, 269: 32, 294: 193}[material]
    ingredient = lambda item: {'item': item, 'count': 1}
    cells = [None] * 9
    cells[0] = ingredient({'mineral': 'iron_ingot'})
    cells[8] = ingredient({'mineral': 'iron_ingot'})
    cells[4] = ingredient({'block': centre})
    if material in [255, 269]:
        cells[4] = ingredient({'mineral': 'coal'})
    for bit, slot in enumerate([1, 2, 3, 5, 6, 7]):
        if (material - 218) & (1 << bit): cells[slot] = ingredient({'mineral': 'iron_ingot'})
    if material in [267, 268]:
        cells[0] = {'item': {'block': 18}, 'count': 1, 'wood': True}
        cells[8] = {'item': {'block': 18}, 'count': 1, 'wood': True}
        if material == 268: cells[4]['wood'] = True
    definition = {
        'schema_version': 2, 'id': 'teramic::' + slug, 'station': 'workbench',
        'pattern': {'type': 'shaped', 'width': 3, 'height': 3, 'cells': cells},
        'output': {'item': {'block': material}, 'count': 4 if material >= 270 and (material - 270) % 6 else 1},
        'unlock': {'type': 'always'},
    }
    # Keep exactly the established recipe shape/schema used by the existing train kit.
    template = json.loads((RECIPES / 'train_chassis.json').read_text())
    template['id'] = 'teramic::' + slug
    template['name'] = next(name for number, _, name, _ in PARTS if number == material)
    template['pattern']['cells'] = cells
    template['output']['item'] = {'block': material}
    template['output']['count'] = definition['output']['count']
    (RECIPES / (slug + '.json')).write_text(json.dumps(template, indent=2) + '\n')


def rust_name(slug):
    return ''.join(word.title() for word in slug.split('_'))


def catalogs():
    rows = '\n'.join(f'    Definition {{ id: {i}, slug: "{slug}", name: "{name}", kind: Kind::{kind} }},'
                     for i, slug, name, kind in PARTS)
    sources = '\n'.join(f'    include_str!("../../../assets/terra/blocks/{slug}.json"),' for _, slug, _, _ in PARTS)
    catalog = '''//! Complete BR52-inspired native block palette and its functional identities.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::sync::OnceLock;

pub const FIRST: u16 = 218;
pub const LAST: u16 = 294;
pub const ENGINE: u16 = 218;
pub const WATER_TANK: u16 = 254;
pub const THROTTLE_PIVOT: [f32; 3] = [0.72, 0.57, 0.25];
pub const COUNT: usize = (LAST - FIRST + 1) as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Engine, Body, Wheel, Axle, Rod, Structure, Valve, Gauge, Tank, Headlight }
#[derive(Clone, Copy, Debug)]
pub struct Definition {
    pub id: u16,
    pub slug: &'static str,
    pub name: &'static str,
    pub kind: Kind,
}
pub const DEFINITIONS: [Definition; COUNT] = [
''' + rows + '''
];
const SOURCES: [&str; COUNT] = [
''' + sources + '''
];
pub const fn is_part(material: u16) -> bool { material >= FIRST && material <= LAST }
pub const fn definition(material: u16) -> Option<&'static Definition> {
    if is_part(material) { Some(&DEFINITIONS[(material - FIRST) as usize]) } else { None }
}
pub const fn is_metal_shape(material: u16) -> bool { material >= 270 && material <= 293 }
pub const fn is_slab(material: u16) -> bool { is_metal_shape(material) && (material - 270) % 6 == 1 }
pub const fn is_stair(material: u16) -> bool { is_metal_shape(material) && (material - 270) % 6 == 2 }
pub fn is_animated(material: u16) -> bool { matches!(material, 211 | 232..=234 | 236..=240) }
pub const fn wheel_radius(material: u16) -> Option<f64> {
    match material { 211 => Some(0.45), 232 => Some(0.85), 233 => Some(0.38), 234 => Some(0.28), _ => None }
}
pub const fn supports_wood(material: u16) -> bool { matches!(material, 267 | 268) }
pub const fn is_full_support(material: u16) -> bool { matches!(material, 210 | 212 | 241 | 242 | 254 | 270 | 276 | 282) }
pub fn valid_facing(material: u16, facing: u8) -> bool {
    if crate::construction::is_shape(material) { crate::construction::valid_facing(material, facing) }
    else { facing < if matches!(material, 219..=223 | 247 | 248) { 8 } else { 4 } }
}
/// Preserve authored lattice bars while applying the shared slab/stair pose.
pub fn grate_boxes(material: u16, facing: u8) -> Option<Vec<ShapeBox>> {
    if let Some(halves) = crate::construction::slab_halves(material, facing)
        && halves.iter().any(|(id, _)| *id == 289)
    {
        return Some(halves.into_iter().flat_map(|(id, pose)| {
            grate_boxes(id, pose).unwrap_or_else(|| vec![crate::construction::slab_bounds(pose)])
        }).collect());
    }
    if !(288..=293).contains(&material) { return None; }
    let basis = if material == 290 { crate::construction::stair_basis(facing) }
        else if material == 289 { match facing {
            4 => [[1,0,0], [0,-1,0], [0,0,-1]],
            5 => [[0,1,0], [1,0,0], [0,0,-1]],
            6 => [[0,-1,0], [-1,0,0], [0,0,-1]],
            7 => [[1,0,0], [0,0,1], [0,-1,0]],
            8 => [[1,0,0], [0,0,-1], [0,1,0]],
            _ => [[1,0,0], [0,1,0], [0,0,1]],
        }} else { crate::construction::stair_basis(facing % 4) };
    Some(base_boxes(material).iter().map(|bounds| {
        let mut transformed = ShapeBox { minimum: [f64::INFINITY;3], maximum:[f64::NEG_INFINITY;3], hinged:false };
        for corner in 0..8 {
            let p: [f64;3] = std::array::from_fn(|axis| if corner & (1<<axis) == 0 { bounds.minimum[axis]-0.5 } else { bounds.maximum[axis]-0.5 });
            for (axis, (minimum, maximum)) in transformed.minimum.iter_mut().zip(&mut transformed.maximum).enumerate() {
                let value = 0.5 + p.iter().zip(basis).map(|(coordinate, basis)| coordinate * f64::from(basis[axis])).sum::<f64>();
                *minimum = minimum.min(value);
                *maximum = maximum.max(value);
            }
        }
        transformed
    }).collect())
}
pub fn base_boxes(material: u16) -> &'static [ShapeBox] {
    static PROFILES: [OnceLock<Vec<ShapeBox>>; COUNT] = [const { OnceLock::new() }; COUNT];
    #[derive(Deserialize)] struct Shape { collision_boxes: Vec<ShapeBox> }
    if !is_part(material) { return &[]; }
    let index = usize::from(material - FIRST);
    PROFILES[index].get_or_init(|| {
        serde_json::from_str::<Shape>(SOURCES[index]).expect("valid authored locomotive profile").collision_boxes
    })
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_complete_palette_has_distinct_authored_assets_and_finite_collision() {
        let mut names = std::collections::BTreeSet::new();
        for (index, definition) in DEFINITIONS.iter().enumerate() {
            assert_eq!(definition.id, FIRST + index as u16);
            assert!(names.insert(definition.slug));
            let boxes = base_boxes(definition.id);
            assert!(!boxes.is_empty() && boxes.len() <= 128, "{}: {} boxes", definition.slug, boxes.len());
            assert!(boxes.iter().all(|bounds| bounds.valid_for(definition.id)), "{}", definition.slug);
            let model = crate::asset_directory().join(format!("terra/blocks/models/{}.gltf", definition.slug));
            assert!(model.is_file(), "{}", model.display());
            assert!(crate::asset_directory().join(format!("crafting/{}.json", definition.slug)).is_file());
        }
        for kind in [Kind::Engine, Kind::Wheel, Kind::Rod, Kind::Structure, Kind::Valve, Kind::Gauge, Kind::Tank, Kind::Headlight] {
            assert!(DEFINITIONS.iter().any(|definition| definition.kind == kind));
        }
        assert!(DEFINITIONS.iter().filter(|definition| (270..=293).contains(&definition.id)).count() == 24);
    }
    #[test] fn every_component_is_craftable_from_the_embedded_catalog() {
        let recipes = crate::crafting::RecipeBook::builtins();
        for definition in DEFINITIONS {
            let key = crate::crafting::recipe_key(&format!("teramic::{}", definition.slug));
            assert!(recipes.get(key).is_some(), "{}", definition.slug);
        }
    }
    #[test] fn boilers_and_roofs_have_supports_at_their_owner_cell_floor() {
        for material in [219, 220, 221, 222, 223, 249, 250, 251] {
            assert!(base_boxes(material).iter().any(|bounds| bounds.minimum[1] == 0.0));
        }
    }
    #[test] fn piston_and_crosshead_collars_share_their_rigid_body_motion() {
        for (slug, expected) in [("train_piston_rod", "train_rod_piston"), ("train_crosshead", "train_rod_crosshead")] {
            let source: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
                crate::asset_directory().join(format!("terra/blocks/models/{slug}.gltf"))).unwrap()).unwrap();
            let names: std::collections::BTreeSet<_> = source["materials"].as_array().unwrap().iter()
                .filter_map(|material| material["name"].as_str()).filter(|name| name.starts_with("train_rod_")).collect();
            assert_eq!(names, [expected].into());
        }
    }
    #[test] fn mixed_grate_slabs_keep_both_authored_halves_and_the_open_lattice() {
        for first in [82, 289] {
            for second in [82, 289] {
                if first != 289 && second != 289 { continue; }
                for facing in [0, 4, 5, 6, 7, 8] {
                    let merged = crate::construction::merged_slab(first, facing, second).unwrap();
                    let boxes = grate_boxes(first, merged).unwrap();
                    let ordinary_halves = usize::from(first != 289) + usize::from(second != 289);
                    assert_eq!(boxes.len(), (2 - ordinary_halves) * base_boxes(289).len() + ordinary_halves);
                    assert!(boxes.iter().all(|bounds| bounds.valid_for(289)));
                    assert_eq!(boxes, crate::construction::placed_boxes(first, merged, [None; 6]));
                }
            }
        }
        assert_eq!(grate_boxes(289, 9).unwrap().len(), base_boxes(289).len() * 2);
    }
}
'''
    (ROOT / 'crates/terra-core/src/locomotive_parts.rs').write_text(catalog)
    path = ROOT / 'crates/terra-voxel/src/materials.rs'
    source = path.read_text()
    for name in ['enum', 'identifiers', 'raw']:
        source = re.sub(r'\n\s*// F54_' + name + r'_START.*?// F54_' + name + r'_END\n', '\n', source, flags=re.S)
    enum = '\n'.join(f'    {rust_name(slug)} = {i},' for i, slug, _, _ in PARTS)
    identifiers = '\n'.join(f'            Self::{rust_name(slug)} => "teramic::{slug}",' for _, slug, _, _ in PARTS)
    raw = '\n'.join(f'            {i} => Self::{rust_name(slug)},' for i, slug, _, _ in PARTS)
    for anchor, name, body in [
        ('    TrainAssemblyStation = 217,', 'enum', enum),
        ('            Self::TrainAssemblyStation => "teramic::train_assembly_station",', 'identifiers', identifiers),
        ('            217 => Self::TrainAssemblyStation,', 'raw', raw),
    ]:
        assert anchor in source
        source = source.replace(anchor, anchor + '\n    // F54_' + name + '_START\n' + body + '\n    // F54_' + name + '_END')
    source = source.replace('Self::TrainAssemblyStation as u16;', 'Self::TrainCeilingLight as u16;').replace('Self::TrainGrateSlope as u16;', 'Self::TrainCeilingLight as u16;')
    source = source.replace('assert_eq!(MaterialId::LAST, 217);', 'assert_eq!(MaterialId::LAST, 294);').replace('assert_eq!(MaterialId::LAST, 293);', 'assert_eq!(MaterialId::LAST, 294);')
    path.write_text(source)
    update_builtin_recipes()


def update_builtin_recipes():
    path = ROOT / 'crates/terra-core/src/crafting/builtins.rs'
    source = re.sub(r'\n\s*// F54_RECIPES_START.*?// F54_RECIPES_END\n', '\n', path.read_text(), flags=re.S)
    recipes = '\n'.join(f'    include_str!("../../../../assets/crafting/{slug}.json"),' for _, slug, _, _ in PARTS)
    source = source.replace('[\n', '[\n    // F54_RECIPES_START\n' + recipes + '\n    // F54_RECIPES_END\n', 1)
    path.write_text(source)


def wood_metadata():
    """Keep every non-default train wood variant in the strict item registry."""
    items = ROOT / 'assets/terra/items'
    for material, slug in [(214, 'train_cab'), (215, 'train_seat'),
                           (216, 'train_cargo'), (217, 'train_assembly_station'),
                           (267, 'train_toolbox'), (268, 'train_wooden_plank')]:
        block = json.loads((BLOCKS / (slug + '.json')).read_text())
        for wood in ['oak', 'birch', 'cherry']:
            metadata = {
                'item': {'wooden_block': {'material': material, 'wood': wood}},
                'name': f"{block['name']} ({wood})",
                'description': f"{block['description']} {wood.capitalize()} wood variant.",
            }
            (items / f'{wood}_{slug}.json').write_text(json.dumps(metadata, indent=2) + '\n')


def main(materials=None):
    generated = 0
    for material, slug, name, role in PARTS:
        if materials is not None and material not in materials:
            continue
        generated += 1
        mesh = make(material, slug)
        mesh.export(slug)
        if material in [247, 248]:
            source_slug = 'oak_window' if material == 247 else 'oak_door'
            source = json.loads((MODELS / (source_slug + '.gltf')).read_text())
            source_uri = source['buffers'][0]['uri']
            source_buffer = (base64.b64decode(source_uri.split(',', 1)[1])
                             if source_uri.startswith('data:') else (MODELS / source_uri).read_bytes())
            source['buffers'][0]['uri'] = slug + '.bin'
            for entry in source['materials']:
                material_name = entry['name']
                entry['name'] = material_name.replace('wood', 'black')
                if 'glass' not in material_name:
                    pbr = entry['pbrMetallicRoughness']
                    pbr['baseColorFactor'] = [.12, .13, .15, 1] if 'wood' in material_name else [.42, .43, .45, 1]
                    pbr['metallicFactor'] = .15 if 'wood' in material_name else .82
            for image in source.get('images', []):
                image['uri'] = '../textures/machinery-iron-hd.png'
            (MODELS / (slug + '.gltf')).write_text(json.dumps(source, separators=(',', ':')) + '\n')
            (MODELS / (slug + '.bin')).write_bytes(source_buffer)
            mesh.collision = json.loads((BLOCKS / (source_slug + '.json')).read_text())['collision_boxes']
        gltf_path = MODELS / (slug + '.gltf')
        gltf = json.loads(gltf_path.read_text())
        gltf['asset']['generator'] = 'Teramic BR52 modular locomotive authoring'
        for entry in gltf['materials']:
            if entry['name'] == 'train_lamp_glow':
                entry['emissiveFactor'] = [1, .74, .28]
        gltf_path.write_text(json.dumps(gltf, separators=(',', ':')) + '\n')
        functional = {
            'Engine': 'Functional fuel-powered train engine. Open its fuel inventory and drive from a connected cab.',
            'Wheel': 'Functional running gear. Wheel rotation follows the actual travelled distance of an assembled train.',
            'Rod': 'Functional visual running gear. Motion follows the wheel phase of an assembled train.',
            'Valve': 'Steam valve required for the highest locomotive gear. Combine with a six-block boiler, water tank, gauge and fuel engine.',
            'Gauge': 'Pressure instrument required by the steam system. Water and pressure are displayed in the locomotive controls.',
            'Tank': 'Steam locomotive water tank. Fill it from the train controls using water buckets. Fuel gears one through five need no water.',
            'Headlight': 'Functional warm train light. Front and side lamps cast a forward cone; the ceiling lamp lights the carriage below.',
        }
        definition = {
            'schema_version': 1, 'id': material, 'name': name,
            'description': ('Structural BR52 frame block. Connect it to the train chassis as part of the vehicle body.' if material in [241, 242]
                            else 'Structural buffer mounting beam. Attach buffers and couplings to the connected train body.' if material == 243
                            else 'Structural locomotive construction shape. Build the connected vehicle body with matching metal parts.' if 270 <= material <= 293
                            else 'Functional framed locomotive opening. Use to open and close its leaf.' if material in [247, 248]
                            else functional.get(role, 'Decorative BR52 locomotive component. Attach to the connected vehicle body before assembly.')),
            'creative_category': 'trains', 'render_kind': 'custom_mesh',
            'model_layout': 'scene', 'model': f'models/{slug}.gltf',
            'struct_block': True, 'hardness': 1.8, 'resistance': 12,
            'required_tool': 'pickaxe', 'mining_level': 0,
            'collision_boxes': mesh.collision,
        }
        (BLOCKS / (slug + '.json')).write_text(json.dumps(definition, indent=2) + '\n')
        recipe(material, slug, role)
    if materials is None:
        catalogs()
        wood_metadata()
    print(f'Authored {generated} BR52 components')


if __name__ == '__main__':
    main()
