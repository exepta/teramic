"""The shipped botanical silhouettes, real alpha gaps and stable material bindings."""
import json
import math
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces.foliage import BLOCKS, PROFILES, SIZE, Canvas, flower, leaf
from biome_surfaces import foliage
from biome_surfaces.test_surfaces import pixels


class FoliageArt(unittest.TestCase):
    def test_existing_ten_ids_bind_distinct_top_and_side_with_authored_gaps(self):
        self.assertEqual([p[0] for p in PROFILES], [7, 8, 9, 132, 133, 195, 310, 311, 312, 406])
        paths = set()
        for ident, slug, style, _, _ in PROFILES:
            spec = json.loads((BLOCKS / (slug + '.json')).read_text())
            self.assertEqual(spec['id'], ident)
            self.assertEqual(spec['render_kind'], 'cube')
            self.assertEqual(spec['bevel_width'], 0)
            self.assertEqual(spec.get('model', ''), '')
            self.assertNotIn('light', spec)
            faces = spec['face_materials']
            self.assertEqual(faces, [faces[0], faces[0], faces[0], faces[3], faces[0], faces[0]])
            self.assertNotEqual(faces[0], faces[3])
            self.assertEqual(len(spec['materials']), 4 if ident == 7 else 3)
            self.assertEqual(spec['textures'][faces[0]], f'textures/foliage/{style}_side.png')
            self.assertEqual(spec['textures'][faces[3]], f'textures/foliage/{style}_top.png')
            for name in dict.fromkeys(faces):
                material = spec['materials'][name]
                self.assertEqual(material['base_color'], [1, 1, 1, 1])
                self.assertNotIn('texture_pattern', material)
                self.assertNotIn('emission', material)
                path = spec['textures'][name]
                paths.add(path)
                width, height, rows = pixels(BLOCKS / path)
                self.assertEqual((width, height), (512, 512))
                covered = sum(p[3] >= 128 for row in rows for p in row)
                opaque = sum(p[3] == 255 for row in rows for p in row)
                self.assertGreaterEqual(covered / (width * height), .65, path)
                self.assertLessEqual(covered / (width * height), .75, path)
                self.assertGreater(opaque / covered, .94, path)
                for border in range(2):
                    edges = (rows[border], rows[-1-border],
                             [r[border] for r in rows], [r[-1-border] for r in rows])
                    self.assertTrue(all(p[3] == 0 for edge in edges for p in edge), path)
                self.assertGreater(len({p[:3] for row in rows for p in row}), 256, path)
                self.assertTrue(all(max(p[:3]) > 30 for row in rows for p in row), path)
        self.assertEqual(len(paths), 20)

    def test_edge_elements_translate_whole_without_clipping_or_wrapping(self):
        color = [160, 190, 90]
        # Compare actual rasterized areas, not the bounding-box helper's own
        # arithmetic: moving any tip towards a sheet corner must keep it whole.
        for index, corner in enumerate([(0, 0), (0, 1), (1, 0), (1, 1)]):
            angle = .31 + index * math.pi / 3
            centered, moved = Canvas(color), Canvas(color)
            for canvas, position in [(centered, (.5, .5)), (moved, corner)]:
                leaf(canvas, position, .24, .16, angle, color, 'oak', 91 + index)
            self.assertAlmostEqual(moved.covered / centered.covered, 1, delta=.01)
            visible = [(i % SIZE, i // SIZE) for i, p in enumerate(moved) if p[3] > 0]
            self.assertTrue(all(2 <= x < SIZE - 2 and 2 <= y < SIZE - 2 for x, y in visible))
        centered, moved = Canvas(color), Canvas(color)
        for canvas, position in [(centered, (.5, .5)), (moved, (1, 0))]:
            flower(canvas, position, [249, 163, 194], 'cherry_pink', 172, .8)
        self.assertAlmostEqual(moved.covered / centered.covered, 1, delta=.01)
        self.assertTrue(all(2 <= i % SIZE < SIZE - 2 and 2 <= i // SIZE < SIZE - 2
                            for i, p in enumerate(moved) if p[3] > 0))

    def test_sprays_use_only_short_petiole_lines_without_naked_branch_wires(self):
        # Inspect the real full-sheet author's line calls: shared spray centres
        # used to leave long exposed triangles after boundary containment.
        with patch.object(foliage, 'stem', wraps=foliage.stem) as lines:
            foliage.image(PROFILES[0])
        self.assertGreater(len(lines.call_args_list), 20)
        for call in lines.call_args_list:
            _, start, end, *_ = call.args
            self.assertLessEqual(math.dist(start, end), .02)

    def test_cutout_layout_has_real_spaces_without_the_old_third_tile_repeat(self):
        for _, _, style, _, _ in PROFILES:
            for face in ['side', 'top']:
                name = f'{style}_{face}'
                width, height, rows = pixels(BLOCKS / f'textures/foliage/{name}.png')
                # A repeated 3x3 stamp would match its own mask after a third-tile
                # translation. Independent irregular sprays must not do that.
                for dx, dy in [(width // 3, 0), (0, height // 3)]:
                    equal = sum((rows[y][x][3] >= 128) ==
                                (rows[(y + dy) % height][(x + dx) % width][3] >= 128)
                                for y in range(0, height, 2) for x in range(0, width, 2))
                    self.assertLess(equal / (width * height / 4), .85, name)
                # Empty regions remain broad enough to see between leaves;
                # isolated one-pixel alpha noise cannot satisfy this check.
                open_squares = sum(all(rows[y + yy][x + xx][3] == 0
                                       for yy in range(8) for xx in range(8))
                                   for y in range(0, height - 7, 8)
                                   for x in range(0, width - 7, 8))
                self.assertGreater(open_squares, 120, name)

    def test_palette_families_and_top_rosettes_remain_distinct(self):
        means = {}
        for _, _, style, _, _ in PROFILES:
            _, _, side = pixels(BLOCKS / f'textures/foliage/{style}_side.png')
            _, _, top = pixels(BLOCKS / f'textures/foliage/{style}_top.png')
            self.assertNotEqual(side, top, style)
            visible = [p for row in side for p in row if p[3] == 255]
            means[style] = [sum(p[c] for p in visible) / len(visible) for c in range(3)]
        for style in ['oak', 'birch', 'apple']:
            self.assertGreater(means[style][1], means[style][0], style)
            self.assertGreater(means[style][1], means[style][2] * 1.5, style)
        self.assertGreater(means['autumn'][0], means['autumn'][1] * 1.4)
        self.assertGreater(means['blood'][0], means['blood'][1] * 1.8)
        self.assertGreater(means['spirit_pearl'][2], means['spirit_pearl'][0])
        self.assertGreater(means['spirit_lavender'][2], means['spirit_lavender'][1])


if __name__ == '__main__':
    unittest.main()
