"""Orthogonal microvoxel silhouettes for the wetland understory and settlements."""
import math
import random

from build_castle import Mesh
from build_spirit_blossom import export
from voxel_props.geometry import VoxelSculpt

COLORS = {
    'stem': (.23, .33, .11), 'sedge': (.51, .57, .22), 'sage': (.34, .46, .21),
    'moss': (.40, .49, .13), 'tips': (.65, .67, .22), 'reed': (.64, .57, .32),
    'head': (.43, .26, .14), 'bark': (.25, .18, .14), 'ridge': (.36, .27, .19),
    'heart': (.59, .43, .28), 'stone': (.38, .40, .38), 'edge': (.48, .49, .44),
    'silt': (.28, .25, .21), 'violet': (.67, .36, .68), 'iris': (.80, .62, .82),
    'amber': (.99, .73, .26), 'cream': (.85, .76, .54), 'ochre': (.73, .55, .33),
    'scarlet': (.76, .22, .14), 'teal': (.24, .94, .75), 'canvas': (.65, .61, .47),
    'iron': (.21, .23, .23)
}


def plants(v, name, rng):
    dense = name in ('SwampSedge', 'SwampReeds')
    for i in range(13 if dense else 7):
        x, z = (rng.uniform(.18, .82) for _ in range(2))
        h = rng.uniform(.29, .76) if dense else rng.uniform(.44, .92)
        a = i * 2.40
        dx, dz = math.cos(a), math.sin(a)
        end = (x + .09 * dx, h, z + .09 * dz)
        v.branch((x, 0, z), end, .034, 'sage' if i % 2 else 'sedge', .018)
        if name == 'SwampCattails' and i % 2 == 0:
            v.branch((end[0], h - .13, end[2]), end, .052, 'head')
            v.branch(end, (end[0], h + .05, end[2]), .017, 'reed')
        elif name == 'SwampReeds' and i % 3 == 0:
            for j in range(4):
                v.branch((end[0], h - .12 + j * .033, end[2]), (end[0] + .05 *
                                                                dx, h - .04 + j * .035, end[2] + .06 * dz), .025, 'reed')
        elif name == 'BogLoosestrife':
            for j in range(4):
                v.blob((end[0] + (-1)**j * .028, h - .18 + .06 * j, end[2]),
                       (.054 - .006 * j, .045, .044), 'violet')
        elif name == 'BogIris' and i % 2 == 0:
            for j in range(3):
                b = j * math.tau / 3
                v.branch(end, (end[0] + .09 * math.cos(b), h + .03,
                               end[2] + .09 * math.sin(b)), .042, 'iris')
                v.blob((end[0] + .095 * math.cos(b), h - .003, end[2] + .095 *
                        math.sin(b)), (.042, .054, .042), 'violet')
            v.blob(end, (.024, .030, .026), 'amber')
        elif name == 'MarshFireflies' and i % 2 == 0:
            v.blob((end[0], h + .04, end[2]), (.019, .021, .021), 'amber')


def fern(v):
    for i in range(9):
        a = i * 2.40
        dx, dz = math.cos(a), math.sin(a)
        for j in range(1, 8):
            t = j / 8
            r = .48 * t
            y = (.4 + .07 * (i % 3)) * math.sin(t * 2.15)
            center = (.50 + dx * r, y, .50 + dz * r)
            prev = (.50 + dx * (r - .06), (.4 + .07 * (i % 3)) *
                    math.sin((t - .125) * 2.15), .50 + dz * (r - .06))
            v.branch(prev, center, .023, 'stem')
            leaf = .11 * math.sin(t * math.pi) + .023
            for sign in [-1, 1]:
                v.branch(center, (center[0] + dx * .045 + dz * sign * leaf, y + .025, center[2] +
                                  dz * .045 - dx * sign * leaf), .038, 'sage' if j % 2 else 'sedge', .02)


def mushrooms(v, name):
    for i, (x, z, h, r) in enumerate([(.28, .57, .45, .19), (.64, .39, .66, .245), (.72, .72, .29, .14)]):
        luminous = name == 'SwampGlowcaps'
        cap = 'teal' if luminous else 'scarlet' if name == 'SwampToadstools' else 'ochre'
        v.branch((x + .03, 0, z), (x, h, z), .043, 'sage' if luminous else 'cream', .033)
        v.blob((x, h - .03, z), (r, .065, r), 'cream' if not luminous else 'sage')
        v.blob((x, h + .02, z), (r, .106 if i != 2 else .068, r), cap)
        if name == 'SwampToadstools':
            for j in range(4):
                b = j * 1.57
                v.blob((x + .58 * r * math.cos(b), h + .10, z + .58 *
                        r * math.sin(b)), (.027, .026, .030), 'cream')


def moss(v, x, y, z, span, rng):
    for i in range(6):
        xx = x + rng.uniform(-span, span)
        zz = z + rng.uniform(-span, span)
        v.branch((xx, y, zz), (xx + .025, y - rng.uniform(.08, .33),
                               zz + .016), .027, 'moss' if i % 2 else 'tips', .015)


def deadwood(v, name, rng):
    if name == 'MossyStump':
        v.branch((.48, .05, .53), (.47, .55, .5), .20, 'bark', .18)
        v.blob((.47, .55, .5), (.166, .026, .163), 'heart')
        v.blob((.47, .576, .5), (.09, .022, .089), 'ridge')
    elif name == 'FallenWillow':
        v.branch((.12, .17, .24), (.86, .25, .73), .155, 'bark')
        v.blob((.50, .34, .50), (.30, .064, .13), 'moss')
        for x in [.39, .58, .72]:
            v.blob((x, .28, x - .12), (.09, .035, .07), 'ochre')
        return
    for i in range(6 if name != 'RootArch' else 3):
        b = i * 2.4
        x, z = .5 + .44 * math.cos(b), .50 + .43 * math.sin(b)
        start = (.49, .42, .52)
        for a, e, rr in [(start, ((x + .5) / 2, .36, (z + .50) / 2), .087),
                         (((x + .5) / 2, .36, (z + .50) / 2), (x, .03, z), .062)]:
            v.branch(a, e, rr, 'ridge' if i % 2 else 'bark', rr * .65)
    if name == 'RootArch':
        v.arch((.5, .03, .5), .33, .28, .59, 'bark')
        moss(v, .49, .60, .52, .12, rng)
    elif name == 'MossyStump':
        moss(v, .49, .56, .55, .15, rng)


def scenery(v, name, rng):
    if name == 'SwampBoulder':
        for x, z, w, h in ((.35, .51, .30, .50), (.65, .58, .25, .32), (.75, .24, .13, .16)):
            v.blob((x, h * .46, z), (w, h * .62, w * .79), 'stone')
            v.blob((x - .01, h * .83, z), (w * .82, h * .28, w * .70), 'edge')
            v.blob((x - .03, h * .98, z + .024), (w * .70, .055, w * .56), 'moss')
    elif name == 'WillowBoat':
        for i in range(9):
            x = .07 + .105 * i
            half = .09 + .17 * math.sin(math.pi * i / 8)
            y = .09 + .10 * abs(i - 4) / 4
            v.box((x - .060, y - .06, .5 - half), (x + .060, y, .5 + half), 'bark')
            for sign in [-1, 1]:
                z = .5 + sign * half
                v.box((x - .06, y, z - .032), (x + .06, y + .15, z + .032), 'ridge')
            if i in (2, 5):
                v.box((x - .04, y + .063, .5 - half), (x + .04, y + .095, .5 + half), 'heart')
    else:
        v.box((.27, 0, .44), (.37, .95, .56), 'ridge')
        v.box((.31, .84, .45), (.83, .91, .54), 'bark')
        if name == 'SwampBanner':
            for i in range(6):
                x = .39 + i * .070
                v.box((x, .28 + (i % 3) * .057, .47), (x + .067, .845, .52), 'canvas' if i != 2 else 'silt')
        else:
            v.branch((.70, .84, .50), (.70, .68, .50), .022, 'iron')
            v.box((.61, .48, .41), (.79, .65, .59), 'amber')
            for y in [.45, .65]:
                v.box((.58, y, .38), (.82, y + .047, .62), 'iron')
            for x in [.6, .785]:
                for z in [.405, .58]:
                    v.box((x, .48, z), (x + .024, .65, z + .024), 'iron')


def model(name, slug, ident):
    v = VoxelSculpt(24 if name not in ('SwampReeds', 'SwampSedge') else 20)
    rng = random.Random(ident)
    if name == 'HangingMoss':
        for i in range(5):
            x = .15 + i * .16
            z = .46 + rng.uniform(-.15, .16)
            v.branch((x, .99, z), (x + .035, .16 + .13 * (i % 3), z + .05), .04, 'moss', .02)
            for j in range(3):
                v.blob((x - .018, .43 + j * .17, z + .038), (.044, .064, .034), 'tips')
    elif name == 'SwampFern':
        fern(v)
    elif name in ('SwampMushrooms', 'SwampGlowcaps', 'SwampToadstools'):
        mushrooms(v, name)
    elif name in ('MossyStump', 'FallenWillow', 'WillowRoots', 'RootArch'):
        deadwood(v, name, rng)
    elif name in ('WillowBoat', 'SwampBanner', 'SwampLanternPost', 'SwampBoulder'):
        scenery(v, name, rng)
    else:
        plants(v, name, rng)
    mesh = Mesh('swamp')
    mesh.materials = {k: (list(rgb), None, 0, .91) for k, rgb in COLORS.items()}
    v.emit(mesh)
    return export(mesh, slug, {'teal', 'amber'} if name in ('MarshFireflies', 'SwampGlowcaps', 'SwampLanternPost') else ())


COLLIDERS = {
    'MossyStump': [(.29, 0, .34, .67, .57, .69)],
    'FallenWillow': [(.09, .035, .18, .53, .35, .54), (.48, .08, .45, .91, .4, .82)],
    'SwampBoulder': [(.09, 0, .28, .61, .53, .75), (.53, 0, .38, .83, .34, .77)],
    'WillowRoots': [(.39, 0, .39, .62, .41, .62)],
    'RootArch': [(.10, 0, .37, .25, .42, .63), (.75, 0, .37, .9, .42, .63), (.21, .43, .36, .79, .62, .64)],
    'WillowBoat': [(.1, 0, .33, .9, .13, .67), (.18, .13, .25, .85, .28, .32), (.18, .13, .68, .85, .28, .75)],
    'SwampLanternPost': [(.27, 0, .44, .37, .95, .56)],
    'SwampBanner': [(.27, 0, .44, .37, .95, .56)],
}
