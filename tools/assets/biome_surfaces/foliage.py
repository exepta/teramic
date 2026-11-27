#!/usr/bin/env python3
"""Irregular botanical leaf sprays with authored transparent gaps and whole-face UVs.

Side UVs are world U/Y: V=0 is the bottom of the block, so tips point towards
smaller image V. Top faces use branching rosettes. Gaps between individual blades are transparent;
leaf interiors remain opaque. No rectangular cluster lattice or material IDs added.
"""
import argparse
import json
import math
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces import noise, rgb, mix
from build_spirit_blossom import png

ROOT = Path(__file__).resolve().parents[3]
BLOCKS = ROOT / 'assets/terra/blocks'
SIZE = 512
# Whole-sheet foliage cards must never slice a blade at their UV boundary.
# Include antialias support in every element's bounds, leaving >2 clear pixels.
MARGIN = .005
# ID, file slug, botanical profile, sRGB leaf colour, optional flower colour.
PROFILES = [
    (7, 'leaves', 'oak', [135, 169, 55], None),
    (8, 'autumn-leaves', 'autumn', [225, 132, 43], None),
    (9, 'silver-leaves', 'birch', [179, 188, 72], None),
    (132, 'cherry-leaves', 'cherry_pink', [200, 96, 137], [249, 163, 194]),
    (133, 'white-cherry-leaves', 'cherry_white', [194, 176, 178], [245, 232, 224]),
    (195, 'apple-leaves', 'apple', [124, 164, 51], None),
    (310, 'pink_blossom_leaves', 'spirit_pink', [188, 97, 177], [235, 157, 221]),
    (311, 'white_blossom_leaves', 'spirit_pearl', [111, 188, 202], [226, 244, 250]),
    (312, 'lavender_blossom_leaves', 'spirit_lavender', [158, 103, 212], [204, 163, 239]),
    (406, 'blood_leaves', 'blood', [224, 82, 104], None),
]


class Canvas(list):
    """Straight-alpha pixels and exact mask coverage while painting leaf silhouettes."""
    def __init__(self, color):
        super().__init__([rgb(color) + [0] for _ in range(SIZE * SIZE)])
        self.covered = 0


def blend_at(canvas, x, y, color, coverage=1):
    if coverage <= 0:
        return
    if not 0 <= x < SIZE or not 0 <= y < SIZE:
        raise ValueError('Botanical silhouette escaped its authored image bounds')
    index = y * SIZE + x
    previous = canvas[index]
    a = min(1, max(0, coverage))
    alpha = a + previous[3] / 255 * (1 - a)
    if alpha <= 0:
        return
    output = [round((color[c] * a + previous[c] * previous[3] / 255 * (1 - a)) / alpha)
              for c in range(3)] + [round(alpha * 255)]
    canvas.covered += int(output[3] >= 128) - int(previous[3] >= 128)
    canvas[index] = output


def contained(center, extent):
    """Translate a complete element into the sheet; never crop or fade its tips."""
    return tuple(max(MARGIN + extent[c], min(1 - MARGIN - extent[c], center[c]))
                 for c in range(2))


def leaf_extent(length, width, angle, salt):
    # Maximum half-width includes asymmetric shape modulation and curved spine.
    # Rotating that complete local rectangle also contains the antialias fringe.
    across = width * .5 * 1.15 * 1.035 + abs((noise(salt, 4, 2029) - .5) * width * .32)
    along = length * .5
    ca, sa = abs(math.cos(angle)), abs(math.sin(angle))
    return (ca * across + sa * along + .5 / SIZE,
            sa * across + ca * along + .5 / SIZE)


def leaf(canvas, center, length, width, angle, color, style, salt, petal=False):
    """Asymmetric curled blade; edges, branching veins and pigment share leaf coordinates."""
    ca, sa = math.cos(angle), math.sin(angle)
    extent = leaf_extent(length, width, angle, salt)
    cx, cy = contained(center, extent)
    bend = (noise(salt, 4, 2029) - .5) * width * .32
    phase = noise(salt, 7, 2039) * math.tau
    veins = 6 + int(noise(salt, 3, 2053) * 3)
    for py in range(math.floor((cy - extent[1]) * SIZE), math.ceil((cy + extent[1]) * SIZE)):
        for px in range(math.floor((cx - extent[0]) * SIZE), math.ceil((cx + extent[0]) * SIZE)):
            dx, dy = (px + .5) / SIZE - cx, (py + .5) / SIZE - cy
            across, along = dx * ca + dy * sa, -dx * sa + dy * ca
            t = along / length + .5
            if not 0 < t < 1:
                continue
            across -= bend * math.sin(t * math.pi) ** 1.4
            if petal:
                half = width * .5 * math.sin(math.pi * t) ** .38 * (1.10 - .30 * t)
                # A real shallow cleft in the rounded outer petal, not a pointed star.
                if t < .055 * math.exp(-(across / (width * .12)) ** 2):
                    continue
            else:
                half = width * .5 * math.sin(math.pi * t) ** .68 * (.78 + .37 * t)
                side = -1 if across < 0 else 1
                half *= 1 + .035 * math.sin(t * 13 + phase + side)
                if style in ('oak', 'autumn'):
                    half *= .86 + .14 * math.cos(t * math.pi * 8 + side * .35)
                elif style == 'birch':
                    teeth = abs(((t * 19 + side * .15) % 1) * 2 - 1)
                    half *= .95 + .05 * teeth
                elif style == 'blood':
                    half *= .91 + .09 * math.sin(t * math.pi * 5 + side * .3)
            edge = (half - abs(across)) * SIZE
            if edge < -.5:
                continue
            coverage = min(1, edge + .5)
            # Antialiasing samples just outside a sharp tip must use its edge
            # pigment, not extrapolate the convex shading into negative light.
            signed = max(-1, min(1, across / max(half, 1e-8)))
            relative = abs(signed)
            if petal:
                shade = .87 + .16 * (1 - relative) + .10 * (1 - t)
                shade += .035 * math.sin(t * 21 + signed * 3 + phase)
                shade += .045 * math.cos(relative * 11 - t * 7) * (1 - relative)
                pigment = rgb(color, shade)
                pigment = rgb(mix(pigment, [255, 245, 236], .08 * (1 - t)))
            else:
                # Convex blade + a narrow concave central fold. Light is broad
                # and soft, with colored shadows rather than black outlines.
                fold = math.exp(-abs(signed) * 8)
                shade = .80 + .24 * (1 - relative ** 1.7)
                shade += .10 * -signed + .045 * math.sin(t * math.pi)
                shade -= .10 * fold
                branch_phase = t * veins + relative * .88 + .11 * math.sin(relative * 4 + phase)
                ridge = math.cos(branch_phase * math.tau)
                shade += .035 * ridge * (1 - relative)
                shade += .018 * math.sin(t * 95 + relative * 17 + phase) * (1 - relative)
                shade *= .99 + .02 * noise(px % SIZE, py % SIZE, salt)
                pigment = rgb(color, shade)
                # Sparse yellow/rose pigment striations follow vein fields.
                wash = (.5 + .5 * math.sin(t * 12 + signed * 4 + phase)) * .09
                warmer = [min(255, color[0] * 1.16 + 6), min(255, color[1] * 1.05 + 4), color[2]]
                pigment = rgb(mix(pigment, warmer, wash))
                if .075 < t < .96:
                    main = math.exp(-((across * SIZE) / .83) ** 2) * .62
                    phase_distance = abs((branch_phase + .5) % 1 - .5)
                    branch = math.exp(-(phase_distance / .035) ** 2) * .32 * (1 - relative)
                    fine_phase = t * veins * 3 + relative * 3.2 + math.sin(t * 11 + phase) * .18
                    fine_distance = abs((fine_phase + .5) % 1 - .5)
                    fine = math.exp(-(fine_distance / .045) ** 2) * .10 * (1 - relative)
                    vein_color = [min(255, c * 1.30 + 18) for c in color]
                    pigment = rgb(mix(pigment, vein_color, max(main, branch, fine)))
                # Thin highlighted curled margin follows lobes/serrations exactly.
                rim = math.exp(-((edge - 1.0) / 1.05) ** 2) * .09
                pigment = rgb(mix(pigment, [min(255, c * 1.22 + 10) for c in color], rim))
            blend_at(canvas, px, py, pigment, coverage)


def disc(canvas, center, radius, color):
    cx, cy = center
    for y in range(math.floor((cy - radius) * SIZE - 1), math.ceil((cy + radius) * SIZE + 1)):
        for x in range(math.floor((cx - radius) * SIZE - 1), math.ceil((cx + radius) * SIZE + 1)):
            distance = math.hypot((x + .5) / SIZE - cx, (y + .5) / SIZE - cy)
            coverage = (radius - distance) * SIZE + .5
            if coverage > 0:
                blend_at(canvas, x, y, color, coverage)


def stem(canvas, start, end, color, width=.0025):
    extent = (width + .5 / SIZE,) * 2
    start, end = contained(start, extent), contained(end, extent)
    distance = math.dist(start, end)
    steps = max(1, math.ceil(distance * SIZE * 1.5))
    for i in range(steps + 1):
        t = i / steps
        point = tuple(start[c] + (end[c] - start[c]) * t for c in range(2))
        disc(canvas, point, width * (1 - t * .45), color)


def flower(canvas, center, color, style, salt, scale=1):
    phase = noise(salt, 0, 1021) * math.tau
    petals = []
    for k in range(5):
        n = noise(k, salt, 1031)
        angle = phase + k * math.tau / 5 + (n - .5) * .15
        r = (.052 + n * .012) * scale
        offset = (math.sin(angle) * r, -math.cos(angle) * r)
        length, width = (.15 + n * .022) * scale, (.110 + n * .018) * scale
        petals.append((offset, length, width, angle, n, salt + 1033 + k))
    # Move the whole flower, preserving its petal/stamen arrangement. Clamping
    # individual petals independently would squash blossoms against card edges.
    extent = tuple(max(abs(p[0][c]) + leaf_extent(p[1], p[2], p[3], p[5])[c]
                       for p in petals) for c in range(2))
    center = contained(center, extent)
    for offset, length, width, angle, n, petal_salt in petals:
        c = (center[0] + offset[0], center[1] + offset[1])
        tint = rgb(color, .94 + .10 * n)
        leaf(canvas, c, length, width, angle, tint, style, petal_salt, petal=True)
    core = [166, 216, 223] if style.startswith('spirit') else [228, 161, 65]
    disc(canvas, center, .017 * scale, core)
    for k in range(7):
        angle = phase + k * math.tau / 7
        c = (center[0] + math.sin(angle) * .018 * scale, center[1] - math.cos(angle) * .018 * scale)
        stem(canvas, center, c, [225, 178, 83], .0008 * scale)
        disc(canvas, c, .003 * scale, [255, 239, 171])


def cluster_centres(ident, top):
    """Bounded best-candidate spacing, with no rectangular/stamped lattice."""
    chosen = []
    for index in range(64):
        candidates = [(noise(index, k, ident + 3011 + int(top) * 97),
                       noise(index, k, ident + 3023 + int(top) * 97)) for k in range(18)]
        def spacing(p):
            return min((sum((p[c] - q[c]) ** 2
                            for c in range(2)) for q in chosen), default=1.)
        point = max(candidates, key=spacing)
        chosen.append(point)
        yield index, point


def image(profile, top=False):
    ident, _, style, color, petals = profile
    canvas = Canvas(color)
    for index, center in cluster_centres(ident, top):
        # Different lengths, rotations, pigment and branch counts form a canopy
        # of irregular sprays. There are deliberately no nine fixed hero leaves.
        salt = ident * 97 + index * 23 + int(top) * 7919
        phase = noise(index, 0, salt) * math.tau if top else (noise(index, 0, salt) - .5) * 1.4
        count = 3 if index % 3 else 4
        stem_color = rgb(color, .62)
        for k in range(count):
            n = noise(index, k, salt + 11)
            angle = phase + (k - (count - 1) * .5) * (.5 + n * .25)
            length = .225 + n * .105
            width = length * (.59 if style == 'blood' else .68 if style == 'birch' else .76)
            reach = .045 + k * .011
            anchor = (center[0] + math.sin(angle) * reach, center[1] - math.cos(angle) * reach)
            anchor = contained(anchor, leaf_extent(length, width, angle, salt + k))
            # A tiny petiole joins this blade's actual contained base. Connecting
            # repositioned blades back to the old spray centre produces naked
            # triangular branch wires around alpha-card silhouettes.
            base = (anchor[0] - math.sin(angle) * length * .48,
                    anchor[1] + math.cos(angle) * length * .48)
            tip = (anchor[0] - math.sin(angle) * (length * .5 + .008),
                   anchor[1] + math.cos(angle) * (length * .5 + .008))
            stem(canvas, base, tip, stem_color, .0015)
            tint = rgb(color, .88 + n * .20)
            leaf(canvas, anchor, length, width, angle, tint, style, salt + k)
        if style.startswith('cherry_'):
            # Five-flowered spurs collect at the ends of branches. One deeper
            # bud shadows the centre; pale outer flowers catch the overhead light.
            offsets = [(0.,0.,1.08),(-.078,-.044,.72),(.080,.034,.69),(.017,.084,.54)]
            for j,(dx,dy,scale) in enumerate(offsets):
                pigment = mix(petals,[255,232,213],.19 * (j % 3)) if j else rgb(petals,.92)
                flower(canvas,(center[0]+dx,center[1]+dy),pigment,style,salt+19*j,scale)
        elif petals and index % 3 != 1:
            flower(canvas, center, petals, style, salt, .72 + noise(index, 7, salt) * .30)
            if index % 4 == 0:
                flower(canvas, (center[0] + .10, center[1] + .08), petals, style, salt + 3, .48)
        # Stop after a whole botanical spray, never punch random holes in leaves
        # or force opacity to fit a statistical target. Every gap is between art.
        if canvas.covered / (SIZE * SIZE) >= .665:
            break
    return canvas


def export(directory, profiles=PROFILES):
    directory.mkdir(parents=True, exist_ok=True)
    for profile in profiles:
        for face, top in [('side', False), ('top', True)]:
            png(directory / f'{profile[2]}_{face}.png', image(profile, top), SIZE)


def bind(profiles=PROFILES):
    for ident, slug, style, _, _ in profiles:
        path = BLOCKS / (slug + '.json')
        data = json.loads(path.read_text())
        if data['id'] != ident:
            raise ValueError(f'Unexpected foliage ID at {path}')
        side = data['face_materials'][0]
        names = [side, side + '_top']
        data['materials'] = {name: {'base_color': [1, 1, 1, 1], 'roughness': .94, 'reflectance': .08}
                             for name in names}
        data['textures'] = {name: f'textures/foliage/{style}_{face}.png'
                            for name, face in zip(names, ('side', 'top'))}
        data['face_materials'] = [names[0], names[0], names[0], names[1], names[0], names[0]]
        path.write_text(json.dumps(data, indent=2) + '\n')
    # Preserve the separate near/middle spray contract when a biome generator
    # refreshes the complete far/preview face textures.
    from biome_surfaces.leaf_clusters import bind as bind_clusters
    bind_clusters(profiles)


def refresh(material_ids):
    """Keep full biome regeneration bound to the current shared foliage art."""
    profiles = [profile for profile in PROFILES if profile[0] in material_ids]
    export(BLOCKS / 'textures/foliage', profiles)
    bind(profiles)
    from biome_surfaces.leaf_clusters import export as export_clusters
    export_clusters(BLOCKS / 'textures/foliage', profiles)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('/tmp/terra-foliage-preview'))
    parser.add_argument('--profile', choices=[p[2] for p in PROFILES])
    parser.add_argument('--activate', action='store_true')
    args = parser.parse_args()
    profiles = [p for p in PROFILES if args.profile is None or p[2] == args.profile]
    directory = BLOCKS / 'textures/foliage' if args.activate else args.output
    export(directory, profiles)
    if args.activate:
        bind(profiles)
        from biome_surfaces.leaf_clusters import export as export_clusters
        export_clusters(directory, profiles)
    print(f'Generated {len(profiles) * 2} foliage textures in {directory}; active={args.activate}')


if __name__ == '__main__':
    main()
