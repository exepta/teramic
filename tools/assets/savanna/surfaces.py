"""Paint tileable rock, grass and wood, plus the two botanical canopy atlases."""
import math
import random

import numpy as np
from PIL import Image, ImageDraw

from biome_surfaces.base_materials import normal_pixels
from build_ashlands import stone_fields
from build_castle import BLOCKS
from build_highlands import far_crown

STONE = {
    "dry_savanna_soil": ((131, 91, 54), 7, 6),
    "savanna_loam": ((157, 100, 70), 4, 4),
    "carved_sandstone": ((208, 177, 122), 3, 3),
    "red_sandstone": ((176, 96, 58), 4, 3),
    "savanna_rock": ((141, 123, 104), 4, 3),
    "layered_savanna_rock": ((155, 128, 99), 3, 6),
    "weathered_savanna_stone": ((153, 145, 119), 4, 4),
    "pale_basalt": ((110, 108, 120), 4, 3),
    "ochre_stone": ((192, 141, 60), 5, 3),
}


def write(slug, face, pixels, folder="savanna"):
    path = f"textures/{folder}/{slug}_{face}.png"
    dest = BLOCKS / path
    dest.parent.mkdir(parents=True, exist_ok=True)
    pixels.save(dest, optimize=True)
    if folder == "savanna":
        normals = normal_pixels(pixels.width, list(pixels.convert("RGB").getdata()))
        normal = Image.fromarray(np.asarray(
            normals, dtype=np.uint8).reshape(pixels.height, pixels.width, 4))
        normal.save(dest.with_name(dest.stem + "_normal.png"), optimize=True)
    return path


def mineral(slug, i):
    base, cols, rows = STONE[slug]
    rng = np.random.default_rng(12713 + i)
    y, x = np.mgrid[:128, :128]
    angle = math.tau / 128
    curl = np.sin(angle * (x + 2 * y)) + 0.5 * np.sin(angle * (3 * x - y))
    cells, seam = stone_fields(x + curl * 1.8, y + 2 * np.sin(x * angle), rng, cols, rows)
    coarse = 3 * np.sin((x - y) * angle) + 3.2 * np.cos((2 * x + y) * angle)
    relief = coarse + rng.normal(0, 1.8, x.shape)
    relief += (cells % 5 - 2) * 1.2 - np.exp(-seam * seam * 140) * 13
    rgb = np.broadcast_to(base, (128, 128, 3)).astype(float).copy()
    if slug in ("red_sandstone", "layered_savanna_rock", "savanna_loam"):
        strata = np.cos((y + 2 * np.sin(x * angle)) * rows * angle)
        relief += strata * 3.5 - np.maximum(strata - .82, 0) * 54
    if slug == "weathered_savanna_stone":
        lichen = (coarse + curl > 3.9) & (seam < .16)
        rgb[lichen] = [112, 126, 66]
    if slug == "carved_sandstone":
        a, b = abs(x - 63.5), abs(y - 63.5)
        border = np.maximum(a, b)
        glyph = (abs(border - 50) < 1.5) | ((abs(a + b - 38) < 2.1) & (border < 39))
        disc = np.hypot(a, b)
        glyph |= (abs(disc - 15) < 1.5) | ((np.minimum(a, b) < 1.5) & (disc > 20) & (disc < 28))
        relief[glyph] -= 28
        relief[np.roll(glyph, 2, axis=0) & ~glyph] += 13
    pixels = np.uint8(np.clip(rgb + relief[:, :, None], 0, 255))
    return Image.fromarray(pixels).resize((256, 256), Image.Resampling.NEAREST)


def turf(slug, face):
    dry = slug == "dry_savanna_turf"
    rng = random.Random(6072 + dry)
    shade = (167, 146, 65) if dry else (141, 149, 62)
    top = Image.new("RGB", (128, 128), shade)
    brush = ImageDraw.Draw(top)
    # Each blade also paints into its neighbour's edge; the texture wraps at any LOD.
    for _ in range(1250):
        x, y = rng.randrange(128), rng.randrange(128)
        light = rng.randrange(-21, 24)
        color = tuple(max(0, min(255, c + light)) for c in shade)
        lean = rng.randrange(-2, 4)
        length = rng.randrange(2, 6)
        for dx in (0, -128, 128):
            for dy in (0, -128, 128):
                brush.line((x + dx, y + dy, x + dx + lean, y + dy + length), fill=color, width=2)
    if face == "side":
        side = mineral("dry_savanna_soil", 2).resize((128, 128), Image.Resampling.NEAREST)
        depth = np.arange(128)[:, None]
        x = np.arange(128)
        rim = 107 + 4 * np.sin(x * math.tau / 64) + 2 * np.cos(x * math.tau / 16) + \
            2 * np.sin(x * math.tau / 8)
        pixels = np.asarray(side).copy()
        pixels[(depth >= rim - 2) & (depth < rim)] //= 2
        grass = depth >= rim
        pixels[grass] = np.asarray(top)[grass]
        top = Image.fromarray(pixels)
    return top.resize((256, 256), Image.Resampling.NEAREST)


def wood(slug, face):
    y, x = np.mgrid[:128, :128]
    p = math.tau / 128
    rng = np.random.default_rng(7361)
    knot = 1.6 * np.sin(p * (x + 2 * y)) + 1.2 * np.sin(3 * p * y)
    if face == "end":
        radius = np.hypot(x - 64 + knot, y - 63 + np.sin(p * x) * 2)
        angle = np.arctan2(y - 63, x - 64)
        base = (176, 108, 64) if slug == "acacia_log" else (186, 147, 100)
        relief = 7 * np.cos(radius * .81 + np.sin(angle * 3)) + 3 * np.sin(radius * 1.58)
        relief -= (radius > 55) * 27
        relief -= ((np.cos(angle * 5 + 0.17 * radius / 30) > .999) & (radius > 30)) * 17
    elif slug == "acacia_planks":
        base = (160, 86, 53)
        relief = 4 * np.sin((y + knot) * p * 11) + 2 * np.cos((y + knot) * p * 21)
        relief -= ((y % 32 < 2) | ((x + (y // 32) * 41) % 128 < 2)) * 28
    else:
        base = (105, 78, 55) if slug == "acacia_log" else (143, 127, 107)
        ridge = np.cos(p * (x + knot) * 9)
        relief = ridge * 6 + np.sin(p * (x + knot) * 19) * 3 - np.maximum(ridge - .76, 0) * 67
    rgb = np.array(base) + (relief + rng.normal(0, 1.75, x.shape))[:, :, None]
    return Image.fromarray(np.uint8(np.clip(rgb, 0, 255))).resize((256, 256), Image.Resampling.NEAREST)


def canopy(slug):
    """One four-motif sheet per species, used for close leaves and far cube crowns."""
    acacia = slug == "acacia"
    atlas = Image.new("RGBA", (512, 512), (108, 118, 51, 0))
    for i in range(4):
        motif = Image.new("RGBA", (128, 128), (108, 118, 51, 0))
        draw = ImageDraw.Draw(motif)
        tint = (122 + 11 * (i % 2), 137 + 6 * (i % 3), 50 + 3 *
                i) if acacia else (99 + 9 * i, 130 + 4 * i, 49 + 3 * i)

        def leaf(center, direction, length, width):
            x, y = center
            vx, vy = math.cos(direction), math.sin(direction)
            nx, ny = -vy, vx
            mid = (x + vx * length * .52, y + vy * length * .52)
            tip = (x + vx * length, y + vy * length)
            draw.polygon([(x, y), (mid[0] + nx * width, mid[1] + ny * width), tip,
                          (mid[0] - nx * width, mid[1] - ny * width)], fill=(*tint, 255))
            draw.line([(x, y), tip], fill=tuple(c + 18 for c in tint) + (255,), width=1)
        if acacia:
            draw.line([(22, 104), (99, 20)], fill=(101, 84, 42, 255), width=3)
            for j in range(8):
                x, y = 27 + 8.8 * j, 98 - 9.7 * j
                length = 19 + 11 * math.sin(j / 8 * math.pi)
                leaf((x, y), -1.77, length, 5.1)
                leaf((x, y), .22, length * .91, 5.6)
        else:
            draw.line([(64, 115), (64, 77)], fill=(102, 84, 51, 255), width=4)
            for j in range(5):
                leaf((64, 78), -math.pi / 2 + (j - 2) * .57, 58 - 4 * abs(j - 2), 8.4)
        atlas.paste(motif.resize((256, 256), Image.Resampling.NEAREST), ((i % 2) * 256, (i // 2) * 256))
    return {face: write(slug, face, img, "foliage") for face, img in
            (("side", far_crown(atlas, False, "side")), ("top", far_crown(atlas, False, "top")), ("cluster", atlas))}
