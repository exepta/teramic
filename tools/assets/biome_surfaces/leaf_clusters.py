#!/usr/bin/env python3
"""Four complete botanical motifs per family, for individually sculpted leaves.

A 2x2 sheet stores a light blade, a shadow blade, and two blossom/fruit motifs.
Every cell has a clear perimeter; UVs select an entire cell without cutting a
blade or blossom. Distant cube artwork remains unchanged.
"""
import argparse
import json
import math
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces import rgb
from biome_surfaces.foliage import BLOCKS, PROFILES, SIZE, Canvas, blend_at, disc, flower, leaf, stem
from build_spirit_blossom import png


def blade(color, style, ident, shade=1):
    canvas = Canvas(color)
    leaf(canvas, (.5, .5), .91, .64, 0, rgb(color, shade), style, ident * 107)
    return canvas


def blossom(color, style, ident, scale=2.7):
    canvas = Canvas(color)
    flower(canvas, (.5, .5), color, style, ident * 109, scale)
    return canvas


def pine_cone(color):
    canvas = Canvas(color)
    for y in range(SIZE):
        for x in range(SIZE):
            u, v = (x + .5) / SIZE, (y + .5) / SIZE
            t = (v - .11) / .78
            if not 0 < t < 1:
                continue
            half = .29 * math.sin(math.pi * t) ** .60 * (.72 + .35 * t)
            edge = (half - abs(u - .5)) * SIZE
            if edge < -.5:
                continue
            ridge = .5 + .5 * math.cos(t * math.pi * 15 + abs(u - .5) * 33)
            pigment = rgb([126, 80, 39], .73 + .33 * ridge + .12 * (1 - abs(u - .5) / half))
            blend_at(canvas, x, y, pigment, min(1, edge + .5))
    return canvas


def rosette(color, style, ident):
    canvas = Canvas(color)
    for index in range(4):
        phase = index * math.tau / 4 + .2
        leaf(canvas, (.5 + math.sin(phase) * .16, .5 + math.cos(phase) * .16),
             .57, .37, -phase, rgb(color, .91 + index * .045), style, ident * 31 + index)
    return canvas


def sheet(tiles, color):
    canvas = Canvas(color)
    # Box sample the authored motif into one cell, retaining straight alpha and
    # its full contour. The transparent margins prevent adjacent-cell bleed.
    for tile, source in enumerate(tiles):
        tx, ty = tile % 2 * (SIZE // 2), tile // 2 * (SIZE // 2)
        for y in range(SIZE // 2):
            for x in range(SIZE // 2):
                samples = [source[(y * 2 + dy) * SIZE + x * 2 + dx] for dy in range(2) for dx in range(2)]
                a = sum(p[3] for p in samples)
                c = [round(sum(p[k] * p[3] for p in samples) / a) for k in range(3)] if a else list(color)
                canvas[(ty + y) * SIZE + tx + x] = c + [round(a / 4)]
    canvas.covered = sum(p[3] >= 128 for p in canvas)
    return canvas


def image(profile):
    ident, _, style, color, petals = profile
    # Slightly cooler greens keep lit leaves green instead of yellow plastic.
    color = {'oak': [88, 138, 48], 'birch': [146, 169, 76], 'apple': [77, 134, 42]}.get(style, color)
    if style in ('cherry_pink', 'cherry_white'):
        # Cherry crowns are predominantly blossoms in the reference. Each
        # primary mesh now carries one whole flower rather than a pink or
        # grey leaf with an occasional separate blossom attached to it.
        colours = ([249, 154, 189], [244, 145, 184], [255, 181, 205], [248, 165, 195]) if style == 'cherry_pink' else (
            [253, 244, 226], [248, 239, 230], [255, 248, 237], [247, 236, 224])
        sprays=[]
        for k,c in enumerate(colours):
            canvas=Canvas(c)
            # Keep the central blossom readable; offset paired buds and a
            # bronze leaf along its spur make each outer silhouette different.
            sign = 1 if k % 2 else -1
            leaf(canvas,(.5+sign*.19,.69),.32,.16,sign*.94,[161,141,89],style,ident+k)
            flower(canvas,(.5+sign*.26,.62),c,style,ident+49+k,.83)
            flower(canvas,(.5+sign*.20,.74),rgb(c,.88),style,ident+87+k,.58)
            flower(canvas,(.5,.5),c,style,(ident+k)*109,2.55)
            sprays.append(canvas)
        return sheet(sprays, color)
    first, second = blade(color, style, ident, 1.07), blade(color, style, ident + 1, .88)
    if petals or style == 'birch':
        petals = petals or [249, 239, 197]
        accent = blossom(petals, style, ident)
        extra = blossom(rgb(petals, .95), style, ident + 1, 2.3)
    else:
        accent, extra = rosette(color, style, ident), blade(color, style, ident + 2, 1.12)
    return sheet([first, second, accent, extra], color)


def needles():
    color = [42, 94, 66]
    tiles = []
    for variant in range(2):
        canvas = Canvas(color)
        stem(canvas, (.5, .10), (.5, .91), [107, 97, 51], .008)
        for tier in range(9):
            y = .18 + tier * .077
            length = .16 + math.sin(tier / 8 * math.pi) * .15
            for side in [-1, 1]:
                leaf(canvas, (.5 + side * (.11 + math.sin(tier / 8 * math.pi) * .06), y), length, .08,
                     side * (1.03 + variant * .12), rgb(color, 1.05 + tier * .015 - variant * .13),
                     'spruce', variant * 79 + tier + 377)
        leaf(canvas, (.5, .145), .17, .065, 0, rgb(color, 1.22), 'spruce', 379)
        tiles.append(canvas)
    return sheet([*tiles, pine_cone(color), rosette(color, 'spruce', 389)], color)


def bind(profiles=PROFILES):
    for ident, slug, style, _, _ in profiles:
        path = BLOCKS / (slug + '.json')
        data = json.loads(path.read_text())
        if data['id'] != ident:
            raise ValueError(f'Unexpected foliage ID at {path}')
        name = 'leaf_cluster'
        data['materials'][name] = {'base_color': [1, 1, 1, 1], 'roughness': .94, 'reflectance': .08}
        data['textures'][name] = f'textures/foliage/{style}_cluster.png'
        data['foliage_cluster'] = {'material': name}
        if ident == 7:
            data['materials']['needle_cluster'] = dict(data['materials'][name])
            data['textures']['needle_cluster'] = 'textures/foliage/spruce_cluster.png'
            data['foliage_cluster']['needles'] = 'needle_cluster'
        path.write_text(json.dumps(data, indent=2) + '\n')


def export(directory, profiles=PROFILES):
    directory.mkdir(parents=True, exist_ok=True)
    for profile in profiles:
        png(directory / f'{profile[2]}_cluster.png', image(profile), SIZE)
    if any(profile[0] == 7 for profile in profiles):
        png(directory / 'spruce_cluster.png', needles(), SIZE)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('/tmp/terra-leaf-clusters'))
    parser.add_argument('--activate', action='store_true')
    args = parser.parse_args()
    target = BLOCKS / 'textures/foliage' if args.activate else args.output
    export(target)
    if args.activate:
        bind()
    print(f'Generated 11 four-cell botanical sheets in {target}; active={args.activate}')


if __name__ == '__main__':
    main()
