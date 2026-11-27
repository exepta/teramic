"""Seamless lush cave faces drawn at the texel scale of the native stone and turf."""
import math
import random

import numpy as np
from PIL import Image

GRID = 32
TILE = 128
Y, X = np.mgrid[:GRID, :GRID]
RGB_WEIGHTS = [0.2126, 0.7152, 0.0722]


def field(rng, cells):
    """Periodic square patches, with low-frequency transitions between beds."""
    knots = rng.uniform(-1, 1, (cells, cells))
    x, y = X * cells / GRID, Y * cells / GRID
    col, row = x.astype(int), y.astype(int)
    u, v = x - col, y - row
    u, v = u * u * (3 - 2 * u), v * v * (3 - 2 * v)
    upper = knots[row, col] * (1 - u) + knots[row, (col + 1) % cells] * u
    lower = knots[(row + 1) % cells, col] * (1 - u) + knots[(row + 1) % cells, (col + 1) % cells] * u
    return upper * (1 - v) + lower * v


def native_rock_or_soil(path, tint):
    with Image.open(path) as image:
        reduced = image.convert('RGB').resize((GRID, GRID), Image.Resampling.BOX)
    light = np.asarray(reduced) @ RGB_WEIGHTS
    return np.array(tint) + (light - light.mean())[:, :, None] * .82


def roots(rgb, top, seed):
    # Bent and forked roots, disappearing into the soil with no edge or baked shadow.
    rng = random.Random(seed)
    for _ in range(4 if top else 3):
        x, y = rng.randrange(GRID), rng.randrange(GRID)
        angle = rng.uniform(0, math.tau) if top else rng.uniform(.9, 2.0)
        length = rng.randrange(9, 19)
        for step in range(length):
            angle += rng.uniform(-.42, .42)
            x, y = x + math.cos(angle), y + math.sin(angle)
            row, col = int(y) % GRID, int(x) % GRID
            rgb[row, col] = [134, 107, 75] if step < length * .66 else [117, 92, 65]
            if step == length // 2:
                for offset in range(1, 4):
                    rgb[int(y + offset * math.sin(angle - .8)) % GRID,
                        int(x + offset * math.cos(angle - .8)) % GRID] = [123, 98, 69]


def build_lush_tiles(directory):
    rng = np.random.default_rng(471)
    rock = native_rock_or_soil(directory.parent / 'block_palette/stone.png', [108, 114, 112])
    soil = native_rock_or_soil(directory.parent / 'reference_base/earth.png', [110, 85, 62])
    for top in (True, False):
        side = 'top' if top else 'side'
        bed = field(rng, 3)
        sprigs = field(rng, 9)
        grain = rng.uniform(-1, 1, (GRID, GRID))
        cover = bed * .71 + sprigs * .29
        moss = np.array([80, 104, 66]) + (bed * 10 + sprigs * 9 + grain * 3)[:, :, None]
        grass = np.array([87, 113, 67]) + (bed * 12 + sprigs * 9 + grain * 2)[:, :, None]
        grass[(sprigs > .38) & (grain > .25)] += [10, 12, 1]
        stone = np.roll(rock, (3, 5) if top else (0, 0), axis=(0, 1))
        rooted = soil.copy()
        roots(rooted, top, 1703 if top else 1709)
        # Grass hangs over an earthen side. Texture coordinates place the top at row zero.
        turf = grass if top else np.where((Y < 4 + cover * 2)[:, :, None], grass, soil)
        overgrown = np.where((cover > (.14 if top else .24))[:, :, None], moss, stone)
        mat = np.where((cover < (-.55 if top else -.28))[:, :, None], stone, moss)
        glow = mat.copy()
        glow[(cover > .16) & (sprigs > .35) & (grain > .64)] = [168, 164, 92]
        for name, rgb in [('rock', stone), ('roots', rooted), ('overgrown', overgrown),
                          ('turf', turf), ('moss', mat), ('glow', glow)]:
            rgb = np.clip(rgb, 0, 255)
            image = Image.fromarray(rgb.astype('uint8'))
            image.resize((TILE, TILE), Image.Resampling.NEAREST).save(directory / f'{name}_{side}.png')
            height = np.asarray(image.convert('L'), dtype=float) / 255
            nx = (np.roll(height, 1, 1) - np.roll(height, -1, 1)) * .85
            ny = (np.roll(height, -1, 0) - np.roll(height, 1, 0)) * .85
            normals = np.stack([nx, ny, np.ones_like(nx)], axis=-1)
            normals /= np.linalg.norm(normals, axis=2, keepdims=True)
            image = Image.fromarray(((normals + 1) * 127.5).round().astype('uint8'))
            image.resize((TILE, TILE), Image.Resampling.NEAREST).save(directory / f'{name}_{side}_normal.png')
