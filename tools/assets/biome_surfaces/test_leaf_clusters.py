"""Complete cutout contours and explicit sprite bindings, independent of far faces."""
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces.foliage import BLOCKS, PROFILES, SIZE
from biome_surfaces.leaf_clusters import image, needles
from biome_surfaces.test_surfaces import pixels


class LeafClusters(unittest.TestCase):
    def test_all_families_have_separate_bound_sprites_and_preserve_far_materials(self):
        paths = set()
        for ident, slug, style, _, _ in PROFILES:
            data = json.loads((BLOCKS / (slug + '.json')).read_text())
            self.assertEqual(data['id'], ident)
            cluster = data['foliage_cluster']
            self.assertEqual(data['textures'][cluster['material']], f'textures/foliage/{style}_cluster.png')
            self.assertNotIn(cluster['material'], data['face_materials'])
            self.assertEqual(data['textures'][data['face_materials'][0]], f'textures/foliage/{style}_side.png')
            self.assertEqual(data['textures'][data['face_materials'][3]], f'textures/foliage/{style}_top.png')
            if ident == 7:
                self.assertEqual(data['textures'][cluster['needles']], 'textures/foliage/spruce_cluster.png')
            else:
                self.assertNotIn('needles', cluster)
            for material in cluster.values():
                paths.add(data['textures'][material])
                self.assertEqual(data['materials'][material]['base_color'], [1, 1, 1, 1])
                self.assertNotIn('emission', data['materials'][material])
        self.assertEqual(len(paths), 11)

    def test_actual_images_have_clear_edges_real_gaps_and_fully_opaque_leaf_interiors(self):
        for path in (BLOCKS / 'textures/foliage').glob('*_cluster.png'):
            width, height, rows = pixels(path)
            self.assertEqual((width, height), (SIZE, SIZE))
            covered = sum(p[3] >= 128 for row in rows for p in row)
            opaque = sum(p[3] == 255 for row in rows for p in row)
            self.assertGreater(covered / (width * height), .22, path.name)
            self.assertLess(covered / (width * height), .72, path.name)
            self.assertGreater(opaque / covered, .95, path.name)
            self.assertTrue(all(p[3] == 0 for edge in [rows[0], rows[-1], [r[0] for r in rows], [r[-1] for r in rows]] for p in edge), path.name)
            self.assertTrue(all(max(p[:3]) > 30 for row in rows for p in row), path.name)
            self.assertGreater(len({p[:3] for row in rows for p in row if p[3] == 255}), 256, path.name)

    def test_each_motif_is_whole_and_distinct_with_transparent_cell_margins(self):
        for path in (BLOCKS / 'textures/foliage').glob('*_cluster.png'):
            _, _, rows = pixels(path)
            motifs = []
            for tile in range(4):
                ox, oy = tile % 2 * 256, tile // 2 * 256
                cell = [row[ox:ox + 256] for row in rows[oy:oy + 256]]
                motifs.append(tuple(p for row in cell for p in row))
                count = sum(p[3] >= 128 for row in cell for p in row)
                self.assertGreater(count / 65536, .20, (path.name, tile))
                self.assertLess(count / 65536, .46, (path.name, tile))
                self.assertTrue(all(p[3] == 0 for edge in [cell[0], cell[-1], [r[0] for r in cell], [r[-1] for r in cell]] for p in edge), (path.name, tile))
                for y, row in enumerate(cell):
                    for x, p in enumerate(row):
                        if p[3] < 128:
                            continue
                        u, v = (x + .5) / 256, (y + .5) / 256
                        lo = .5 - 2 * v if v < .25 else 0 if v <= .75 else 2 * v - 1.5
                        self.assertTrue(lo <= u <= 1 - lo, (path.name, tile, x, y, 'motif must fit its six-sided geometry'))
            self.assertGreaterEqual(len(set(motifs)), 3, path.name)

    def test_cherry_primary_cells_contain_whole_blossoms_with_gold_stamens(self):
        for style in ('cherry_pink', 'cherry_white'):
            _, _, rows = pixels(BLOCKS / f'textures/foliage/{style}_cluster.png')
            for tile in range(4):
                ox, oy = tile % 2 * 256, tile // 2 * 256
                core = [rows[oy + y][ox + x] for y in range(118, 138) for x in range(118, 138)]
                gold = sum(p[3] >= 128 and p[0] > p[1] + 20 and p[1] > p[2] + 35 for p in core)
                self.assertGreater(gold, 20, (style, tile, 'a blossom must have its own golden centre'))

    def test_shipped_sprites_equal_reproducible_complete_authored_groups(self):
        for profile in PROFILES:
            _, _, rows = pixels(BLOCKS / f'textures/foliage/{profile[2]}_cluster.png')
            self.assertEqual([tuple(p) for p in image(profile)], [p for row in rows for p in row])
        _, _, rows = pixels(BLOCKS / 'textures/foliage/spruce_cluster.png')
        self.assertEqual([tuple(p) for p in needles()], [p for row in rows for p in row])


if __name__ == '__main__':
    unittest.main()
