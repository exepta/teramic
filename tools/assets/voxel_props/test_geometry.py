"""Regressions for the shipped voxel props, not just their authoring helpers."""
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_castle import Mesh, cross
from build_blood_forest import NAMES, sculpt_prop
from build_spirit_blossom import DEFINITIONS
from model_polish.gltf import Document
from voxel_props.geometry import VoxelSculpt

ROOT = Path(__file__).resolve().parents[3]
MODELS = ROOT / 'assets/terra/blocks/models'
SLUGS = [slug for slug, _ in NAMES[12:]] + [slug for i, slug, *_ in DEFINITIONS if i >= 316]


def mesh_area(mesh):
    area = 0
    for part in mesh.parts.values():
        for at in range(0, len(part['indices']), 3):
            a, b, c = [part['position'][i] for i in part['indices'][at:at+3]]
            n = cross([b[k]-a[k] for k in range(3)], [c[k]-a[k] for k in range(3)])
            area += math.sqrt(sum(v*v for v in n)) / 2
    return area


class VoxelPropTests(unittest.TestCase):
    def test_touching_cells_remove_their_shared_face_even_between_materials(self):
        for second in ('a', 'b'):
            sculpture = VoxelSculpt(4)
            sculpture.cells = {(1, 1, 1): 'a', (2, 1, 1): second}
            mesh = sculpture.emit(Mesh('test'))
            self.assertAlmostEqual(mesh_area(mesh), 10 / 4**2)
            triangles = sum(len(p['indices'])//3 for p in mesh.parts.values())
            self.assertEqual(triangles, 12 if second == 'a' else 20)
            for part in mesh.parts.values():
                for p, n in zip(part['position'], part['normal']):
                    self.assertFalse(p[0] == .5 and abs(n[0]) == 1)

    def test_overlapping_solids_have_only_the_union_boundary(self):
        sculpt = VoxelSculpt(8)
        sculpt.box([0, 0, 0], [.75, .75, .75], 'body')
        sculpt.box([.25, .25, .25], [1, 1, 1], 'trim')
        exposed = 0
        for cell in sculpt.cells:
            for axis in range(3):
                for sign in (-1, 1):
                    other = list(cell)
                    other[axis] += sign
                    exposed += tuple(other) not in sculpt.cells
        self.assertAlmostEqual(mesh_area(sculpt.emit(Mesh('test'))), exposed / 8**2)

    def test_thin_diagonal_vines_are_face_connected_and_small_centres_survive(self):
        sculpt = VoxelSculpt()
        sculpt.branch([.08, .08, .08], [.90, .88, .83], .005, 'vine')
        remaining = set(sculpt.cells)
        pending = [remaining.pop()]
        while pending:
            current = pending.pop()
            for axis in range(3):
                for sign in (-1, 1):
                    neighbor = list(current)
                    neighbor[axis] += sign
                    neighbor = tuple(neighbor)
                    if neighbor in remaining:
                        remaining.remove(neighbor)
                        pending.append(neighbor)
        self.assertFalse(remaining)
        small = VoxelSculpt()
        small.blob([.5, .55, .268], [.01, .01, .01], 'gold')
        self.assertGreater(len(small.cells), 0)

    def test_arch_opening_is_preserved_and_mushroom_light_is_exposed(self):
        arch = VoxelSculpt()
        sculpt_prop(arch, 20)
        for y in range(1, 18):
            self.assertNotIn((12, y, 12), arch.cells)
        for index in (12, 19):
            mushrooms = VoxelSculpt()
            sculpt_prop(mushrooms, index)
            self.assertIn('glow', mushrooms.emit(Mesh('test')).parts)

    def test_all_eighteen_shipped_models_are_bounded_axis_aligned_voxel_surfaces(self):
        total_vertices = total_triangles = 0
        for slug in SLUGS:
            with self.subTest(model=slug):
                doc = Document(MODELS / f'{slug}.glb')
                vertices = triangles = 0
                seen = set()
                for mesh in doc.doc['meshes']:
                    for primitive in mesh['primitives']:
                        attrs = {key: doc.values(index) for key, index in primitive['attributes'].items()}
                        pos, normals, tangents = attrs['POSITION'], attrs['NORMAL'], attrs['TANGENT']
                        indices = [v[0] for v in doc.values(primitive['indices'])]
                        self.assertEqual(set(indices), set(range(len(pos))))
                        vertices += len(pos)
                        triangles += len(indices)//3
                        for p, n, t in zip(pos, normals, tangents):
                            self.assertTrue(all(-1e-6 <= v <= 1.000001 for v in p))
                            self.assertTrue(all(abs(v*24-round(v*24)) < 1e-5 for v in p))
                            self.assertEqual(sorted(abs(v) for v in n), [0, 0, 1])
                            self.assertAlmostEqual(sum(v*v for v in t[:3]), 1, places=5)
                            self.assertAlmostEqual(sum(a*b for a, b in zip(n, t)), 0, places=5)
                        self.assertTrue(all(math.isfinite(v) for values in attrs.values() for row in values for v in row))
                        for at in range(0, len(indices), 3):
                            ids = indices[at:at+3]
                            a, b, c = [pos[i] for i in ids]
                            key = tuple(sorted((a, b, c)))
                            self.assertNotIn(key, seen, 'Duplicate / coincident surface')
                            seen.add(key)
                            n = cross([b[k]-a[k] for k in range(3)], [c[k]-a[k] for k in range(3)])
                            self.assertGreater(sum(v*v for v in n), 1e-12)
                            for i in ids:
                                self.assertGreater(sum(a*b for a, b in zip(n, normals[i])), 0)
                # Caps and diagonal roots need more faces than a smooth cylinder;
                # the dense foliage set still has a strict small prop budget.
                self.assertLessEqual(vertices, 3_000)
                self.assertLessEqual(triangles, 1_500)
                total_vertices += vertices
                total_triangles += triangles
        self.assertLessEqual(total_vertices, 20_000)
        self.assertLessEqual(total_triangles, 10_000)


if __name__ == '__main__':
    unittest.main()
