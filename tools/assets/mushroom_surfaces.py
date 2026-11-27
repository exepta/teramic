"""Seamless, metre-scale woodland surfaces. Cap spots live on the entire parasol."""
import numpy as np
from PIL import Image


def build_textures(directory):
    size = 256
    y, x = np.mgrid[0:size, 0:size] / size
    rng = np.random.default_rng(161012)

    def noise(nx, ny):
        lattice = rng.uniform(-1, 1, (ny, nx))
        px, py = x * nx, y * ny
        ix, iy = px.astype(int), py.astype(int)
        u, v = px - ix, py - iy
        u *= u * (3 - 2 * u)
        v *= v * (3 - 2 * v)
        return ((1 - v) * ((1 - u) * lattice[iy, ix] + u * lattice[iy, (ix + 1) % nx]) +
                v * ((1 - u) * lattice[(iy + 1) % ny, ix] + u * lattice[(iy + 1) % ny, (ix + 1) % nx]))

    broad = noise(4, 4)
    mineral = noise(19, 19)
    fuzz = noise(64, 64)
    warp = noise(5, 4)
    grain = np.sin(2 * np.pi * (18 * x + 0.50 * noise(7, 2) + 0.13 * noise(10, 5)))
    fine = np.sin(2 * np.pi * (43 * x + 0.35 * noise(10, 2)))
    fine_spore = np.clip((noise(67, 67) - 0.77) * 4, 0, 1)
    directory.mkdir(parents=True, exist_ok=True)

    def paint(style, face, color, values, normal_gain):
        image = np.clip(np.array(color) + values, 0, 255).astype("uint8")
        Image.fromarray(image).save(directory / f"{style}_{face}.png", optimize=True)
        light = image @ np.array([0.213, 0.715, 0.072]) / 255.
        nx = (np.roll(light, 1, 1) - np.roll(light, -1, 1)) * normal_gain
        ny = (np.roll(light, -1, 0) - np.roll(light, 1, 0)) * normal_gain
        normal = np.stack([nx, ny, np.ones_like(nx)], axis=2)
        normal /= np.linalg.norm(normal, axis=2)[:, :, None]
        Image.fromarray(np.round((normal * 0.5 + 0.5) * 255).astype("uint8")).save(
            directory / f"{style}_{face}_normal.png", optimize=True)

    def ink(mask, rgb):
        return mask[:, :, None] * np.array(rgb)

    skin = ink(broad, [6, 4, 4]) + ink(mineral, [4, 3, 3]) + ink(fuzz, [3, 2, 2])
    for name, color in [("scarlet", [173, 49, 43]), ("cream", [222, 200, 161]),
                        ("chestnut", [144, 90, 53]), ("amethyst", [175, 127, 172])]:
        paint(name, "top", color, skin + ink(fine_spore, [10, 9, 5]), 0.84)
        if name in ("chestnut", "amethyst"):
            paint(name, "side", color, skin + ink(noise(13, 6), [5, 3, 2]), 0.85)
    fibres = ink(grain, [5, 6, 5]) + ink(fine, [2, 2, 1]) + \
        ink(noise(9, 2), [9, 9, 8]) + ink(fuzz, [2, 2, 3])
    paint("stem", "side", [216, 197, 161], fibres, 0.94)
    # Fine oyster-like pleats read as radial ribs in the voxel silhouette.
    gill = ink(grain, [11, 10, 8]) + ink(fine, [3, 3, 2]) + ink(broad, [6, 6, 4])
    paint("gill", "side", [218, 191, 148], gill, 1.25)
    # Thin wandering mycelium threads grow beneath a green moss carpet. Keep
    # soil and mycelium differences subtle under strong biome sunlight.
    mycelium = np.exp(-np.abs(np.sin(2 * np.pi * (x + y + 0.16 * warp))) * 19)
    moss = ink(broad, [12, 12, 7]) + ink(mineral, [7, 8, 5]) + ink(fuzz, [7, 8, 5])
    paint("mycelium", "top", [111, 127, 67], moss + ink(mycelium, [18, 10, 15]), 1.15)
    soil = ink(broad, [8, 8, 6]) + ink(mineral, [10, 9, 7]) + ink(fuzz, [6, 5, 4])
    for face in ("top", "side"):
        paint("spore", face, [88, 72, 63], soil + ink(fine_spore, [19, 11, 14]), 1.2)
    edge = np.clip((0.18 + 0.055 * noise(10, 2) - y) * 28, 0, 1)
    paint("mycelium", "side", [89, 71, 56], soil +
          ink(edge, [22, 52, 12]) + ink(mycelium, [12, 8, 9]), 1.12)
    # Narrow grain highlights are continuous. Offset every second row of
    # cedar shakes, with gently rounded tips and a few weathered edges.
    for face in ("top", "side"):
        row = np.floor(y * 4)
        joint = np.mod(x * 5 + 0.5 * (row % 2), 1)
        progress = np.mod(y * 4, 1)
        grooves = (np.exp(-np.minimum(joint, 1 - joint) * 40) * 0.75 +
                   np.exp(-np.minimum(progress, 1 - progress) * 20))
        cedar = ink(grain, [7, 6, 4]) + ink(broad, [12, 11, 11]) + ink(mineral, [5, 5, 3])
        paint("shingles", face, [121, 81, 67], cedar - ink(grooves, [29, 23, 18]), 1.5)
        timber = ink(grain, [9, 7, 4]) + ink(fine, [4, 4, 2]) + ink(noise(6, 2), [12, 9, 6])
        if face == "top":
            rings = np.sin(2 * np.pi * (np.hypot(x - 0.51, y - 0.52) * 15 + 0.23 * warp))
            timber = ink(rings, [7, 6, 4]) + ink(broad, [6, 6, 4])
        paint("timber", face, [139, 104, 73], timber, 1.1)
