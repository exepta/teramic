"""Shipped 256px biome art and deterministic sampling, without external libraries."""
import json
from pathlib import Path
import struct
import sys
import unittest
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces import SIZE, field, leaf, stone
from build_blood_forest import NAMES, SURFACES as BLOOD_SURFACES
from build_block_palette import FAMILIES, SIZE as WOOD_SIZE
from biome_surfaces.foliage import SIZE as FOLIAGE_SIZE
from build_spirit_blossom import DEFINITIONS, SURFACES as SPIRIT_SURFACES

BLOCKS = Path(__file__).resolve().parents[3] / 'assets/terra/blocks'


def pixels(path):
    data = path.read_bytes()
    width, height = struct.unpack('>II', data[16:24])
    parts = []
    cursor = 8
    while cursor < len(data):
        length = struct.unpack_from('>I', data, cursor)[0]
        if data[cursor + 4:cursor + 8] == b'IDAT':
            parts.append(data[cursor + 8:cursor + 8 + length])
        cursor += length + 12
    raw = zlib.decompress(b''.join(parts))
    rows = []
    for row in range(height):
        start = row * (width * 4 + 1)
        # Our procedural encoder uses unfiltered RGBA rows, not an image resizer.
        if raw[start] != 0:
            raise AssertionError('Unexpected filtered procedural row')
        rows.append([tuple(raw[i:i + 4]) for i in range(start + 1, start + 1 + width * 4, 4)])
    return width, height, rows


class BiomeSurfaceArt(unittest.TestCase):
    def cube_specs(self):
        slugs = [slug for material, slug, _, _ in DEFINITIONS if material <= 315]
        slugs += [slug for slug, _ in NAMES[:12]]
        return [json.loads((BLOCKS / (slug + '.json')).read_text()) for slug in slugs]

    def test_authored_biome_textures_have_native_detail_and_material_specific_alpha(self):
        files = {p for spec in self.cube_specs() for p in spec['textures'].values()}
        self.assertEqual(len(files), 40)
        for name in sorted(files):
            with self.subTest(texture=name):
                width, height, rows = pixels(BLOCKS / name)
                size = {'foliage': FOLIAGE_SIZE, 'block_palette': WOOD_SIZE}.get(Path(name).parent.name, SIZE)
                self.assertEqual((width, height), (size, size))
                foliage = name.startswith('textures/foliage/')
                if foliage:
                    coverage = sum(p[3] >= 128 for row in rows for p in row) / (width * height)
                    if name.endswith('_cluster.png'):
                        self.assertTrue(.22 <= coverage <= .72, (name, coverage))
                    else:
                        self.assertTrue(.65 <= coverage <= .75, (name, coverage))
                else:
                    self.assertTrue(all(p[3] == 255 for row in rows for p in row))
                # A 64/128px nearest-neighbour upscale would leave homogeneous
                # 2x2 cells. Real leaf veins/pores/fibres must vary within them.
                visible_cells = [tuple(rows[y + dy][x + dx] for dy in (0, 1) for dx in (0, 1))
                                 for y in range(0, height, 2) for x in range(0, width, 2)
                                 if not foliage or all(rows[y + dy][x + dx][3] == 255
                                                       for dy in (0, 1) for dx in (0, 1))]
                distinct = sum(len(set(cell)) > 1 for cell in visible_cells)
                self.assertGreater(distinct / len(visible_cells), .90)

    def test_appropriate_inward_surface_styles_preserve_closed_cube_contract(self):
        styles = SPIRIT_SURFACES | BLOOD_SURFACES
        for spec in self.cube_specs():
            self.assertEqual(spec['render_kind'], 'cube')
            palette_wood = spec['id'] in FAMILIES['blood'] + FAMILIES['ghost']
            self.assertEqual(spec['bevel_width'], .025 if palette_wood else 0)
            self.assertEqual(len(spec['face_materials']), 6)
            self.assertEqual(spec.get('model', ''), '')
            self.assertEqual(spec.get('surface'), None if palette_wood else styles.get(spec['id']))

    def test_biome_turf_side_caps_join_the_upper_face_with_positive_world_y_uvs(self):
        def chroma(values):
            mean = [sum(p[c] for p in values) / len(values) for c in range(3)]
            return [value / sum(mean) for value in mean]

        def distance(a, b):
            return sum((x - y) ** 2 for x, y in zip(a, b))

        for slug in ('spirit_blossom_grass', 'blood_grass', 'crimson_mycelium', 'blood_moss'):
            with self.subTest(block=slug):
                spec = json.loads((BLOCKS / (slug + '.json')).read_text())
                side = spec['face_materials'][0]
                self.assertTrue(all(spec['face_materials'][face] == side for face in (1, 4, 5)))
                _, _, rows = pixels(BLOCKS / spec['textures'][side])
                _, _, top = pixels(BLOCKS / spec['textures'][spec['face_materials'][3]])
                _, _, bottom = pixels(BLOCKS / spec['textures'][spec['face_materials'][2]])
                cap = chroma([p for row in top[::8] for p in row[::8]])
                soil = chroma([p for row in bottom[::8] for p in row[::8]])
                # Actual cube projection is v=worldY on +/-X and +/-Z.
                # Sample inside each cell edge, away from repeat-filter seams.
                upper = chroma([p for row in rows[-16:-4] for p in row[::8]])
                lower = chroma([p for row in rows[4:16] for p in row[::8]])
                self.assertLess(distance(upper, cap), distance(upper, soil), 'grass cap is upside down')
                self.assertLess(distance(lower, soil), distance(lower, cap), 'soil must meet the block below')

    def test_periodic_leaf_fracture_and_field_sampling_has_no_boundary_reset(self):
        for x, y in [(0, 0), (255, 173), (-7, 27), (73, 128)]:
            for dx, dy in [(SIZE, 0), (0, SIZE), (-SIZE, SIZE)]:
                self.assertEqual(leaf(x, y, True), leaf(x + dx, y + dy, True))
                self.assertEqual(stone(x, y, [160, 143, 181]), stone(x + dx, y + dy, [160, 143, 181]))
                self.assertAlmostEqual(field(x, y, 8, 21), field(x + dx, y + dy, 8, 21))

    def test_blood_leaf_bark_palette_remains_readable_without_emission(self):
        for slug, minimum in [('blood_leaves', (175, 78, 100)), ('bloodwood_log', (125, 78, 72))]:
            spec = json.loads((BLOCKS / (slug + '.json')).read_text())
            path = spec['textures'][spec['face_materials'][0]]
            width, height, rows = pixels(BLOCKS / path)
            visible = [p for row in rows for p in row if p[3] == 255]
            mean = [sum(p[c] for p in visible) / len(visible) for c in range(3)]
            self.assertTrue(all(value > limit for value, limit in zip(mean, minimum)), (slug, mean))
            spec = json.loads((BLOCKS / (slug + '.json')).read_text())
            self.assertNotIn('light', spec)
            self.assertTrue(all('emission' not in material for material in spec['materials'].values()))


if __name__ == '__main__':
    unittest.main()
