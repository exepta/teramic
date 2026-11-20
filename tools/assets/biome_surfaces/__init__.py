"""Authored, seamless 256-pixel biome surfaces; no image resampling or atlas.

Large readable forms carry the colour at a distance. Veins, pores, fibres and
moss tips are sampled at output resolution for close views. All coordinate fields
are periodic; the side turf/earth transition intentionally has a top and bottom.
"""
import math
from functools import lru_cache

SIZE = 256


def noise(x, y, salt=0):
    n = ((int(x) * 73856093) ^ (int(y) * 19349663) ^ salt * 83492791) & 0xffffffff
    n = ((n ^ (n >> 13)) * 1274126177) & 0xffffffff
    return (n & 65535) / 65535


def field(x, y, cells, salt):
    """Periodic smooth value field, with the same sample at both tile boundaries."""
    x = (x % SIZE) * cells / SIZE
    y = (y % SIZE) * cells / SIZE
    ix, iy = math.floor(x), math.floor(y)
    u, v = x - ix, y - iy
    u, v = u * u * (3 - 2 * u), v * v * (3 - 2 * v)
    a, b = noise(ix % cells, iy % cells, salt), noise((ix + 1) % cells, iy % cells, salt)
    c, d = noise(ix % cells, (iy + 1) % cells, salt), noise((ix + 1) % cells, (iy + 1) % cells, salt)
    return (a + (b - a) * u) * (1 - v) + (c + (d - c) * u) * v


def rgb(color, shade=1):
    return [max(0, min(255, round(c * shade))) for c in color]


def mix(a, b, amount):
    return [x * (1 - amount) + y * amount for x, y in zip(a, b)]


def grain(x, y, salt=61):
    return .985 + .03 * noise(x % SIZE, y % SIZE, salt)


def soil(x, y, color):
    coarse = field(x, y, 8, 109)
    fine = field(x, y, 32, 113)
    # Continuous curved strata, small clay clods and pores, never brick courses.
    layer = math.sin(y * math.tau / 32 + field(x, y, 4, 114) * 4)
    shade = .90 + coarse * .15 + fine * .07 + layer * .018
    pore = noise(x % SIZE, y % SIZE, 111)
    shade *= .78 if pore > .994 else 1.08 if pore < .007 else 1
    root = (x + 8 * math.sin(y * math.tau / SIZE) + field(x, y, 8, 116) * 7) % 64
    if root < .8 and fine > .58:
        return rgb(mix(color, [201, 159, 155], .30), shade)
    return rgb(color, shade * grain(x, y))


def turf(x, y, color, earth, side=False, moss=False):
    if side:
        # Cube sides project UV.v along positive world Y. Their upper edge
        # therefore samples the last PNG rows, like reference_base/grass_side.
        # Keep this conversion local to side art; top/bottom maps stay intact.
        y = SIZE - 1 - y
        edge = 36 + 21 * field(x, 0, 16, 201) + 8 * math.sin(x * math.tau / 64)
        if y > edge:
            c = soil(x, y, earth)
            # Fine hanging roots join the irregular moss lip to the subsoil.
            root = (x + 3 * math.sin(y * math.tau / 64)) % 32
            if y < edge + 28 and root < 1.1:
                c = rgb(mix(c, color, .38))
            return c
    patch = field(x, y, 8, 203)
    shade = .87 + patch * .20 + field(x, y, 4, 204) * .05
    # Seeded tufts have different lean, width and length. A uniform diamond
    # lattice looks like woven cloth, so each small patch gets its own fronds.
    cx, cy = math.floor(x / 8) % 32, math.floor(y / 8) % 32
    n = noise(cx, cy, 211)
    local_x = x % 8 - 3 - 2 * noise(cx, cy, 213)
    local_y = y % 8
    height = 5 + 3 * n
    lean = (noise(cx, cy, 217) - .5) * .85
    blade = local_y < height and abs(local_x - local_y * lean) < (height - local_y) * (.16 + n * .13)
    shade += .055 if blade else -.012
    shade += .04 * field(x, y, 32, 205)
    if moss:
        shade += .04 * field(x, y, 64, 219)
    return rgb(color, shade * grain(x, y, 207))


def stone(x, y, color, vein=None, crystalline=False):
    # Warped Voronoi fracture edges: sites repeat, the cracks do not form a grid.
    wx = x % SIZE + (field(x, y, 4, 303) - .5) * 29 + (field(x, y, 16, 304) - .5) * 9
    wy = y % SIZE + (field(x, y, 4, 307) - .5) * 27 + (field(x, y, 16, 308) - .5) * 9
    cell = 64
    cx, cy = math.floor(wx / cell), math.floor(wy / cell)
    nearest = [(1e9, 0, 0), (1e9, 0, 0)]
    for j in range(cy - 1, cy + 2):
        for i in range(cx - 1, cx + 2):
            px = (i + .18 + .64 * noise(i % 4, j % 4, 311)) * cell
            py = (j + .18 + .64 * noise(i % 4, j % 4, 313)) * cell
            value = ((wx - px) ** 2 + (wy - py) ** 2, i, j)
            if value[0] < nearest[0][0]:
                nearest[1], nearest[0] = nearest[0], value
            elif value[0] < nearest[1][0]:
                nearest[1] = value
    a, b = nearest
    edge = math.sqrt(b[0]) - math.sqrt(a[0])
    face = noise(a[1] % 4, a[2] % 4, 317)
    shade = .89 + face * .14 + field(x, y, 32, 319) * .08
    if edge < 1.1:
        shade *= .64
    elif edge < 2.8:
        shade *= 1.08
    c = rgb(color, shade * grain(x, y, 331))
    if vein is not None and edge < 2 and face > .46:
        c = rgb(mix(c, vein, .76 if edge < .9 else .30))
    if crystalline and edge > 3:
        c = rgb(c, .94 + .13 * ((wx + wy) % 64) / 64)
    return c


def wood(x, y, color, end=False, planks=False):
    x, y = x % SIZE, y % SIZE
    if end:
        dx, dy = x - 127.5, y - 127.5
        angle = math.atan2(dy, dx)
        radius = math.hypot(dx, dy) + 3 * math.sin(angle * 5) + 1.5 * math.sin(angle * 9)
        ring = (radius % 17) / 17
        shade = .81 if ring < .10 else 1.07 if ring < .23 else .99
        if radius > 119:
            shade *= .84
        return rgb(color, shade * grain(x, y, 401))
    u, v = (y, x) if planks else (x, y)
    fibre = (u + 5 * math.sin(v * math.tau / SIZE) + 2 * math.sin(v * math.tau / 64)) / 32
    ridge = fibre % 1
    shade = .83 if ridge < .07 else .94 if ridge < .18 else 1.06 if ridge < .35 else 1
    # Branching fine fissures and longitudinal grain are evaluated at 256px.
    shade += .025 * math.sin(u * 1.13 + 2 * math.sin(v * math.tau / 128))
    if .61 < ridge < .635 and math.sin(v * math.tau / 128 + math.floor(fibre)) > .2:
        shade *= .87
    if planks:
        board = y // 64
        if y % 64 < 2 or (x + board * 73) % SIZE < 2:
            shade *= .76
    return rgb(color, shade * grain(x, y, 409))


@lru_cache(maxsize=2)
def leaves(blood=False):
    result = []
    for layer in range(2):
        for row in range(4):
            for col in range(4):
                n = noise(col, row, 503 + layer)
                cx = (col * 64 + row % 2 * 32 + layer * 26 + 13 * (n - .5)) % SIZE
                cy = (row * 64 + layer * 34 + 16 * (noise(col, row, 509) - .5)) % SIZE
                angle = (-.6 if (row + col) % 2 else .55) + (n - .5) * .7
                base = [178 + layer * 20 + 10 * n, 80 + layer * 16 + 9 * n, 108 + layer * 16 + 9 * n] if blood else [104 + n * 16, 143 + layer * 9, 155 + layer * 14]
                result.append((cx, cy, math.cos(angle), math.sin(angle), 29 + 6 * n, 48 + 9 * n, base))
    return result


def leaf(x, y, blood=False):
    x, y = x % SIZE, y % SIZE
    c = [166, 75, 105] if blood else [94, 132, 151]
    for cx, cy, ca, sa, width, length, base in leaves(blood):
        dx, dy = (x - cx + 128) % SIZE - 128, (y - cy + 128) % SIZE - 128
        u, v = dx * ca - dy * sa, dx * sa + dy * ca
        t = abs(v) / length
        if t >= 1:
            continue
        u -= 3 * (v / length) ** 2
        edge = abs(u) / (width * (1 - t) ** .78)
        if edge >= 1:
            continue
        shade = (1.03 if u < 0 else .955) - .07 * max(0, (edge - .8) / .2)
        # Tapered midrib and alternating oblique secondary veins, not white lines.
        if abs(u) < .7 + (1 - t) * .5:
            shade += .085
        secondary = (v - abs(u) * .65) % 12
        if secondary < .85 and edge < .91:
            shade += .045 * (1 - edge)
        c = rgb(base, shade)
    return rgb(c, grain(x, y, 521))


@lru_cache(maxsize=1)
def flower_centers():
    return [(col * 64 + 16 * (noise(col, row, 601) - .5),
             row * 64 + 18 * (noise(col, row, 607) - .5),
             noise(col, row, 613) * math.tau)
            for row in range(4) for col in range(4)]


def blossom(x, y, color, white=False, petals_only=False):
    c = rgb(color, .84) if petals_only else leaf(x, y)
    for cx, cy, angle in flower_centers():
        dx, dy = (x - cx + 128) % SIZE - 128, (y - cy + 128) % SIZE - 128
        radius = math.hypot(dx, dy)
        a = math.atan2(dy, dx) + angle
        lobe = .5 + .5 * math.cos(5 * a)
        boundary = 22 + 16 * lobe
        if radius > boundary:
            continue
        fold = (radius / boundary) ** .7
        shade = .86 + .17 * fold - .07 * (1 - lobe) + .04 * (-dx + dy) / boundary
        vein = abs(math.sin(5 * a)) * radius
        if vein < .8 and radius > 8:
            shade += .045
        c = rgb(color, shade)
        if radius < 5.5:
            c = [157, 212, 238] if white else [246, 207, 141]
        elif radius < 8 and noise(x % SIZE, y % SIZE, 617) > .69:
            c = [249, 222, 174]
    return rgb(c, grain(x, y, 619))
