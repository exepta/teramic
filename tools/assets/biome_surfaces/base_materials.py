#!/usr/bin/env python3
"""Base biome albedos sampled directly at 512px, with existing model UVs intact.

Generate into a preview directory first. --activate writes only the explicitly
listed PNGs, a material-only GLB derivative and three JSON bindings; imported
geometry and source imagery remain unchanged.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces import field, noise, rgb, mix
from build_spirit_blossom import png
from model_polish.gltf import Document

ROOT = Path(__file__).resolve().parents[3]
BLOCKS = ROOT / 'assets/terra/blocks'
RESOLUTION = 512
PREFIX = 'textures/reference_base/'
FILES = ('grass_top', 'grass_side', 'grass_side_mirrored', 'earth', 'snow', 'grass_near')
MODEL = 'fiverr_grass_reference_base.glb'
SOURCE_MODEL = BLOCKS / 'models/fiverr_grass_faces.glb'


def clay_grain(x, y):
    """Small connected clay fragments, all in the same warm earth material."""
    wx = (x + 2 * math.sin(y * math.tau / 64)) % 256
    wy = (y + 2 * math.sin(x * math.tau / 128)) % 256
    cx, cy = math.floor(wx / 16), math.floor(wy / 16)
    first, second, selected = 1e9, 1e9, 0
    for row in range(cy - 1, cy + 2):
        for col in range(cx - 1, cx + 2):
            seed = noise(col % 16, row % 16, 941)
            px = (col + .2 + seed * .6) * 16
            py = (row + .2 + noise(col % 16, row % 16, 947) * .6) * 16
            distance = (wx - px) ** 2 + (wy - py) ** 2
            if distance < first:
                second, first, selected = first, distance, seed
            elif distance < second:
                second = distance
    edge = math.sqrt(second) - math.sqrt(first)
    crease = .045 * max(0, 1 - edge / .8)
    lip = .025 * max(0, 1 - abs(edge - 1.1) / .7)
    return .975 + selected * .05 - crease + lip


def roots(x, y):
    coverage = 0
    for k in range(4):
        seed = noise(k, 0, 953)
        start = 18 + noise(k, 0, 967) * 130
        length = 48 + seed * 74
        t = (y - start) / length
        if not 0 < t < 1:
            continue
        center = k * 64 + 12 + seed * 36 + math.sin(t * 4 + seed * 5) * 7
        distance = abs((x - center + 128) % 256 - 128)
        coverage = max(coverage, max(0, 1 - distance / (.55 * (1 - t) + .15)))
        if t > .40:
            fork = center + (t - .40) * (25 if k % 2 else -25)
            distance = abs((x - fork + 128) % 256 - 128)
            coverage = max(coverage, max(0, 1 - distance / .4) * (1 - t))
    return coverage


def earth(u, v):
    x, y = u * 256, v * 256
    broad, clod = field(x, y, 4, 801), field(x, y, 16, 803)
    # Beds have different widths, irregular eroded edges and interrupted seams.
    # Fine clay fragments and root fibres carry the close-up detail between them.
    warped = (y + 13 * math.sin(x * math.tau / 256)
              + (field(x, y, 8, 809) - .5) * 17
              + (field(x, y, 32, 810) - .5) * 3) % 256
    edges = (0, 39, 112, 176, 256)
    row = next(i for i in range(4) if warped < edges[i + 1])
    into = warped - edges[row]
    width = edges[row + 1] - edges[row]
    discontinuity = .3 + .7 * field(x, row * 48, 16, 811)
    seam = 1 - .12 * max(0, 1 - into / 4) * discontinuity
    lip = .028 * max(0, 1 - abs(into - 6) / 4)
    shade = (.91 + broad * .09 + clod * .045 + lip) * seam * clay_grain(x, y)
    colors = ([139, 103, 68], [145, 108, 71], [131, 97, 64], [140, 105, 70])
    c = mix(colors[row], colors[(row + 1) % 4], (into / width) * .18)
    c = mix(c, [181, 144, 94], roots(x, y) * .40)
    detail = .984 + .032 * noise(int(u * 512) % 512, int(v * 512) % 512, 821)
    return rgb(c, shade * detail)


def grass(u, v):
    x, y = u * 256, v * 256
    # Large pointed blades overlap in staggered tufts. Their folded warm edges
    # match the broad leaves in the reference without a woven micro-grid.
    c = mix([91, 127, 41], [122, 153, 52], field(x,y,4,827))
    shade = .94 + field(x,y,16,829)*.055
    for layer, size in [(0,32),(1,16)]:
        count=256//size
        wx=(x+layer*11)%256; wy=(y+layer*19)%256
        cx,cy=int(wx//size)%count,int(wy//size)%count
        seed=noise(cx,cy,853+layer*4)
        dx=wx%size-size*.5; dy=wy%size-size*.5
        angle=(seed-.5)*2.2
        across=dx*math.cos(angle)-dy*math.sin(angle)
        along=dx*math.sin(angle)+dy*math.cos(angle)
        t=along/(size*.8)+.5
        width=size*.24*max(0,math.sin(max(0,min(1,t))*math.pi))
        if 0<t<1 and abs(across)<width:
            shade+=(.11 if across<0 else -.035)*(1 if layer==0 else .6)
            if abs(across)<.45:shade+=.065
    return rgb(c,shade*(.992+.016*noise(int(x*2),int(y*2),859)))


def grass_side(u, v):
    depth = 1 - v
    edge = .16 + .09 * field(u * 256, 0, 8, 863) + .035 * math.sin(u * math.tau * 4)
    if depth <= edge:
        return grass(u, depth * 2)
    c = earth(u, v)
    root = (u * 256 + 2 * math.sin(v * math.tau * 2)) % 32
    if depth < edge + .12 and root < .6:
        c = rgb(mix(c, [152, 155, 67], .26))
    return c


def snow(u, v):
    x, y = u * 256, v * 256
    drift = field(x, y, 4, 877)
    soft = field(x, y, 16, 881)
    # Broad cool hollows and warm white raised snow, with no blue pixel flecks.
    c = mix([212, 223, 234], [247, 246, 239], .35 + .65 * drift)
    shade = .985 + .018 * soft
    return rgb(c, shade * (.998 + .004 * noise(int(u * 512), int(v * 512), 883)))


SAMPLERS = {'grass_top': grass, 'grass_side': grass_side,
            'grass_side_mirrored': lambda u, v: grass_side(1 - u, v),
            'earth': earth, 'snow': snow}


def near_pixel(u, v):
    # The immutable Fiverr face UVs occupy four quadrants. The measured affine
    # maps include a 0.0015 guard band. Use their inverse, not a guessed stretch.
    if u < .5 and v < .5:
        return grass((u - .0015) / .497, (v - .0015) / .497)
    if u < .5:
        return earth((u - .0015) / .497, (.9985 - v) / .497)
    top = .4985 if v < .5 else .9985
    return grass_side((u - .5015) / .497, (top - v) / .497)


def generate(directory):
    directory.mkdir(parents=True, exist_ok=True)
    generated = {}
    for name, sample in SAMPLERS.items():
        pixels = [sample((x + .5) / RESOLUTION, (y + .5) / RESOLUTION) + [255]
                  for y in range(RESOLUTION) for x in range(RESOLUTION)]
        png(directory / f'{name}.png', pixels, RESOLUTION)
        generated[name] = (RESOLUTION, pixels)
    # Compatibility with one existing authored model, not a new global atlas:
    # each distant face and every other block still use standalone textures.
    size = RESOLUTION * 2
    pixels = [near_pixel((x + .5) / size, (y + .5) / size) + [255]
              for y in range(size) for x in range(size)]
    png(directory / 'grass_near.png', pixels, size)
    generated['grass_near'] = (size, pixels)
    for name, (size, colors) in generated.items():
        if name != 'snow':
            png(directory / (name + '_normal.png'), normal_pixels(size, colors), size)
    return generated


def normal_pixels(size, colors):
    # A deliberately shallow height proxy follows the authored forms, replacing
    # the unrelated pebbly source normal. Tangent-space convention is glTF +Y.
    heights = [(c[0] * .2126 + c[1] * .7152 + c[2] * .0722) / 255 for c in colors]
    result = []
    for y in range(size):
        for x in range(size):
            nx = (heights[y * size + (x - 1) % size] - heights[y * size + (x + 1) % size]) * 3
            ny = (heights[((y - 1) % size) * size + x] - heights[((y + 1) % size) * size + x]) * 3
            length = math.sqrt(nx * nx + ny * ny + 1)
            result.append([round((nx / length * .5 + .5) * 255),
                           round((ny / length * .5 + .5) * 255),
                           round((1 / length * .5 + .5) * 255), 255])
    return result


def derive_model(directory, output):
    source = Document(SOURCE_MODEL)
    original_binary = bytes(source.buffers[0])
    original_views = list(source.doc['bufferViews'])
    images = {}
    material_images = {'FiverrGrassReferenceHD': 'grass_near', 'UpLod': 'grass_top',
                       'DownLod': 'earth', 'WestLod': 'grass_side', 'SouthLod': 'grass_side',
                       'EastLod': 'grass_side_mirrored', 'NorthLod': 'grass_side_mirrored'}

    def replace_image(texture, name):
        # Reuse all existing texture/image slots and sampler modes. The imported
        # near image clamps, while coarse faces repeat across greedy rectangles.
        if name not in images:
            data = source.buffers[0]
            data.extend(b'\0' * (-len(data) % 4))
            offset = len(data)
            image_bytes = (directory / (name + '.png')).read_bytes()
            data.extend(image_bytes)
            view = len(source.doc['bufferViews'])
            source.doc['bufferViews'].append({'buffer': 0, 'byteOffset': offset, 'byteLength': len(image_bytes)})
            images[name] = view
        image = source.doc['textures'][texture]['source']
        source.doc['images'][image] = {'name': 'ReferenceBase_' + name,
                                     'bufferView': images[name], 'mimeType': 'image/png'}

    for material in source.doc['materials']:
        name = material_images[material['name']]
        replace_image(material['pbrMetallicRoughness']['baseColorTexture']['index'], name)
        replace_image(material['normalTexture']['index'], name + '_normal')
    assert source.buffers[0][:len(original_binary)] == original_binary
    assert source.doc['bufferViews'][:len(original_views)] == original_views
    source.doc['asset'].setdefault('extras', {})['reference_base_art'] = {
        'parent': SOURCE_MODEL.name,
        'parent_sha256': hashlib.sha256(SOURCE_MODEL.read_bytes()).hexdigest(),
        'geometry': 'unchanged accessors, nodes, meshes, UVs, indices and original binary prefix',
        'albedo_resolution': 512,
        'normal': 'paired shallow height gradients; glTF tangent space',
        'orm': 'unchanged matte source ORM and material roughness',
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    source.path = output
    source.save()
    (output.with_suffix('.art.json')).write_text(json.dumps(source.doc['asset']['extras']['reference_base_art'], indent=2) + '\n')


def bindings():
    path = BLOCKS / 'grass.json'
    data = json.loads(path.read_text())
    data['model'] = 'models/' + MODEL
    path.write_text(json.dumps(data, indent=2) + '\n')
    for name in ('snow', 'snow_layer'):
        path = BLOCKS / (name + '.json')
        data = json.loads(path.read_text())
        data.setdefault('textures', {})['Soft snow'] = PREFIX + 'snow.png'
        path.write_text(json.dumps(data, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('/tmp/terra-reference-base-preview'))
    parser.add_argument('--activate', action='store_true')
    args = parser.parse_args()
    output = BLOCKS / 'textures/reference_base' if args.activate else args.output
    generate(output)
    model = BLOCKS / 'models' / MODEL if args.activate else output / MODEL
    derive_model(output, model)
    if args.activate:
        bindings()
    print(f'Generated {len(FILES)} base albedos in {output}; active bindings changed: {args.activate}')


if __name__ == '__main__':
    main()
