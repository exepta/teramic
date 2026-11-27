"""Cell-built savanna plants, weathered rock and termite architecture."""
import math
import random

from build_castle import Mesh
from build_spirit_blossom import export
from voxel_props.geometry import VoxelSculpt

PALETTE = {
    "straw": (.74, .59, .23), "seed": (.93, .74, .30), "olive": (.43, .46, .17),
    "leaf": (.37, .54, .23), "sage": (.45, .65, .38), "blue": (.29, .55, .49), "blade": (.54, .73, .58),
    "bark": (.36, .24, .15), "heart": (.60, .39, .23), "twig": (.56, .43, .27),
    "yellow": (.98, .73, .16), "red": (.87, .26, .13), "pink": (.93, .35, .54),
    "grey": (.51, .46, .39), "stone": (.60, .51, .40), "shade": (.42, .38, .35),
    "strata": (.65, .43, .25), "sandstone": (.76, .56, .32), "ochre": (.73, .39, .19),
    "clay": (.58, .29, .16), "dark": (.24, .17, .13), "bone": (.84, .77, .60),
    "ivory": (.93, .86, .69), "horn": (.66, .58, .43), "thorn": (.86, .76, .42),
}


def tussock(v, rng, tall):
    for i in range(14 if tall else 12):
        angle = rng.uniform(0, math.tau)
        dx, dz = math.cos(angle), math.sin(angle)
        spread = rng.uniform(.06, .28)
        h = rng.uniform(.47, .86) if tall else rng.uniform(.19, .42)
        root_x, root_z = .50 + dx * spread, .50 + dz * spread
        lean = rng.uniform(.06, .22)
        for j in range(1 + math.ceil(h * v.resolution)):
            y = min(j / v.resolution, h)
            t = y / h
            x, z = root_x + lean * dx * t * t, root_z + lean * dz * t * t
            reach = .039 * (1 - t) + .014
            v.box((x - reach, y - .02, z - .021), (x + reach, y + .031, z + .021),
                  "olive" if t < .22 else "straw" if i % 3 else "seed")
        if tall and i % 3 == 0:
            v.blob((x, y, z), (.042, .075, .039), "seed")


def branching_tree(v, slug):
    sapling = slug == "acacia_sapling"
    thicket = slug == "savanna_thorn_bush"
    radius = .037 if thicket else .068 if sapling else .11
    stem = (.49, .42, .51)
    v.branch((.51, .012, .5), stem, radius, "bark", radius * .72)
    for i in range(6):
        a = i * 2.39996
        dx, dz = math.cos(a), math.sin(a)
        base = (.49 + dx * .03, .17 + .035 * i, .51 + dz * .03)
        fork = (.50 + dx * .18, .47 + .035 * (i % 3), .50 + dz * .18)
        tip = (.50 + dx * .36, .63 + .10 * (i % 3), .50 + dz * .36)
        v.branch(base, fork, radius * .70, "bark", radius * .5)
        v.branch(fork, tip, radius * .55, "twig", .022)
        spur = (fork[0] - dz * .13, fork[1] + .17, fork[2] + dx * .13)
        v.branch(fork, spur, .026, "twig", .014)
        if sapling:
            v.blob(tip, (.13, .07, .12), "olive")
            v.blob((tip[0] - .02, tip[1] + .035, tip[2]), (.10, .05, .09), "leaf")
        elif thicket:
            v.blob(tip, (.045, .045, .05), "straw")


def succulent(v, slug):
    aloe = slug == "savanna_aloe"
    agave = slug == "savanna_agave"
    n = 10 if agave else 8
    for i in range(n):
        a = i * math.tau / n
        dx, dz = math.cos(a), math.sin(a)
        reach = .39 if agave else .33 if aloe else .23
        h = (.61 if agave else .48 if aloe else .30) + .08 * (i % 2)
        previous = (.5, .06, .5)
        for k in range(25):
            t = k / 24
            center = (.5 + dx * (.06 + reach * t), .06 + h * t**1.45, .5 + dz * (.06 + reach * t))
            width = (.08 if agave else .11 if aloe else .14) * (1. - t * .80)
            material = "blue" if agave else "leaf"
            if t >= .74:
                material = "blade" if agave else "sage"
            v.branch(previous, center, .012, material)
            v.blob(center, (width, .09 * (1 - .52 * t), width), material)
            previous = center
    v.blob((.5, .16, .5), (.13, .16, .13), "sage")


def wildflowers(v, red):
    for i, (x, z, h) in enumerate([(.33, .38, .49), (.64, .45, .63), (.49, .68, .40)]):
        v.branch((x - .03, .0, z), (x, h, z), .025, "leaf")
        v.branch((x, h * .33, z), (x + .10, h * .49, z + .05), .045, "olive", .02)
        for j in range(5):
            a = j * math.tau / 5 + i
            v.blob((x + .084 * math.cos(a), h, z + .084 * math.sin(a)),
                   (.064, .042, .062), "red" if red else "yellow")
        v.blob((x, h + .024, z), (.044, .035, .044), "seed")


def cactus(v, tall):
    h, r = (.88, .21) if tall else (.46, .24)
    for k in range(9):
        a = math.tau * k / 9
        x, z = .50 + math.cos(a) * r * .67, .50 + math.sin(a) * r * .67
        v.branch((x, .035, z), (x, h - .05, z), r * .46, "leaf" if k % 3 else "olive", r * .43)
        v.blob((x, h - .025, z), (r * .43, .065, r * .43), "sage")
        for j in range(2 if tall else 1):
            v.blob((x + .069 * math.cos(a), .17 + j * .35 + .045 * (k % 2),
                    z + .069 * math.sin(a)), (.023, .023, .023), "thorn")
    v.fill((.5 - r, 0, .5 - r), (.5 + r, h, .5 + r), "leaf",
           lambda p: (p[0] - .5)**2 + (p[2] - .5)**2 < (r * .77)**2)
    if tall:
        for j in range(6):
            a = j * math.tau / 6
            v.blob((.5 + .082 * math.cos(a), h + .04, .5 + .082 * math.sin(a)), (.061, .041, .061), "pink")
        v.blob((.5, h + .063, .5), (.042, .033, .042), "yellow")


def rocks(v, slug):
    if slug == "savanna_stone_arch":
        v.arch((.50, .035, .50), .26, .48, .43, "sandstone")
        # The voussoirs share faces, with darker joints traced through the arch.
        for key in v.cells:
            x, y, _ = [(k + .5) / v.resolution for k in key]
            angle = math.atan2(y - .035, x - .50)
            stone = angle / math.pi * 9
            if abs(stone - round(stone)) < .13:
                v.cells[key] = "strata"
            elif int(stone) % 3 == 1:
                v.cells[key] = "stone"
        for x in (.08, .92):
            v.blob((x, .058, .50), (.07, .075, .24), "strata")
    elif slug == "savanna_plateau_stone":
        v.blob((.5, .26, .5), (.47, .28, .43), "strata")
        v.blob((.51, .48, .5), (.46, .105, .42), "sandstone")
        v.blob((.51, .54, .5), (.41, .071, .37), "olive")
    elif slug == "savanna_rock_pile":
        for x, z, y, w in [(.29, .56, .14, .26), (.63, .47, .15, .30), (.43, .49, .36, .25), (.55, .48, .54, .19), (.75, .75, .09, .17)]:
            v.box((x - w, y - .12, z - w * .65), (x + w, y + .12,
                                                  z + w * .65), "strata" if y < .3 else "sandstone")
    else:
        big = slug == "large_savanna_boulder"
        for x, z, w, h, mat in [(.48, .51, .35, .61, "grey"), (.72, .48, .22, .34, "stone"), (.21, .63, .15, .16, "shade")]:
            h *= 1. if big else .48
            v.blob((x, h * .35, z), (w, h * .65, w * .86), mat)
            v.blob((x - .018, h * .71, z - .016), (w * .78, h * .26, w * .68), "stone")


def mound(v, tall):
    for k in range(7):
        a = k * 2.39996
        d = .045 + .26 * (k / 7)
        x, z = .5 + d * math.cos(a), .50 + d * math.sin(a)
        h = (.91 - .077 * k) * (1 if tall else .60)
        r = .16 - .006 * k
        v.branch((x, .02, z), (x, h, z), r, "clay", r * .30)
        v.branch((x - .04, .04, z - .04), (x - .022, h - .03, z - .021), r * .65, "ochre", r * .21)
    v.blob((.49, .06, .5), (.36, .13, .34), "clay")
    for x, z in ((.33, .37), (.68, .46), (.45, .71)):
        v.blob((x, .09, z), (.05, .06, .05), "dark")


def remains(v, skull):
    if not skull:
        v.branch((.17, .07, .5), (.84, .07, .5), .045, "bone")
        for i in range(5):
            x = .21 + i * .12
            for side in [-1, 1]:
                points = [(x, .07, .5), (x - .025, .14, .5 + .16 * side),
                          (x, .28, .5 + .20 * side), (x + .025, .37, .5 + .11 * side)]
                for a, b in zip(points, points[1:]):
                    v.branch(a, b, .034, "ivory", .029)
        return
    v.blob((.5, .24, .48), (.22, .21, .22), "bone")
    v.blob((.50, .12, .70), (.14, .105, .15), "ivory")
    for sign in (-1, 1):
        v.blob((.5 + .133 * sign, .245, .64), (.068, .062, .06), "dark")
        pts = [(.5 + .14 * sign, .31, .45), (.5 + .29 * sign, .32, .43),
               (.5 + .40 * sign, .43, .41), (.5 + .42 * sign, .57, .38)]
        for i, (a, b) in enumerate(zip(pts, pts[1:])):
            v.branch(a, b, .066 - .015 * i, "horn" if i else "bone", .05 - .015 * i)


def stump(v):
    for i in range(6):
        a = i * math.tau / 6
        v.branch((.50, .19, .49), (.50 + .39 * math.cos(a), .028, .49 + .36 * math.sin(a)), .10, "bark", .034)
    v.fill((.25, 0, .25), (.75, .45, .75), "bark",
           lambda p: math.hypot(p[0] - .5, p[2] - .5) < .236 + .015 * math.sin(6 * math.atan2(p[2] - .5, p[0] - .5)))
    for radius, color in [(.196, "heart"), (.144, "twig"), (.095, "heart"), (.045, "twig")]:
        v.fill((.29, .408, .29), (.71, .45, .71), color,
               lambda p: (p[0] - .5)**2 + (p[2] - .5)**2 < radius**2)


def model(slug, ident):
    # Frequent tufts use a coarser grid to keep broad grasslands affordable.
    resolution = 20 if slug in ("savanna_short_grass", "savanna_tall_grass",
                                "acacia_sapling", "savanna_plateau_stone") else 24
    v = VoxelSculpt(resolution)
    if slug.endswith("_grass"):
        tussock(v, random.Random(ident), slug == "savanna_tall_grass")
    elif slug in ("savanna_thorn_bush", "acacia_sapling", "dry_acacia_snag"):
        branching_tree(v, slug)
    elif slug in ("savanna_aloe", "savanna_agave", "savanna_succulent"):
        succulent(v, slug)
    elif slug.endswith("_flower"):
        wildflowers(v, slug.startswith("red_"))
    elif slug.endswith("_cactus"):
        cactus(v, slug.startswith("flowering"))
    elif slug.endswith("_mound"):
        mound(v, slug.startswith("tall"))
    elif slug in ("savanna_ribs", "horned_skull"):
        remains(v, slug == "horned_skull")
    elif slug == "weathered_acacia_stump":
        stump(v)
    else:
        rocks(v, slug)
    mesh = Mesh("savanna")
    mesh.materials = {name: (list(rgb), None, 0, .92) for name, rgb in PALETTE.items()}
    v.emit(mesh)
    return export(mesh, slug)


# Keep arch openings, spaces around roots, and places between rocks walkable.
COLLIDERS = {
    "small_savanna_boulder": [(.14, 0, .25, .80, .25, .75)],
    "large_savanna_boulder": [(.16, 0, .25, .78, .54, .76), (.73, 0, .34, .90, .25, .63)],
    "savanna_rock_pile": [(.11, 0, .31, .89, .28, .69), (.25, .28, .33, .72, .54, .65)],
    "small_termite_mound": [(.26, 0, .26, .74, .31, .74), (.37, .31, .39, .61, .49, .61)],
    "tall_termite_mound": [(.24, 0, .24, .76, .45, .76), (.32, .45, .35, .66, .68, .65), (.43, .68, .42, .62, .91, .62)],
    "dry_acacia_snag": [(.40, 0, .40, .60, .56, .60)],
    "weathered_acacia_stump": [(.27, 0, .27, .73, .45, .73)],
    "savanna_stone_arch": [(.04, 0, .29, .24, .31, .71), (.76, 0, .29, .96, .31, .71), (.20, .32, .29, .80, .49, .71)],
    "savanna_plateau_stone": [(.12, 0, .13, .89, .53, .87)],
    "horned_skull": [(.32, 0, .31, .68, .41, .71)],
}
