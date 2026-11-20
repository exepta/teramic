"""Analytic helper contracts and valid, bounded shipped flower geometry."""
import math
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_castle import Mesh, cross
from cemetery.geometry import ellipsoid
from model_polish.flowers import untextured_ellipsoid
from model_polish.gltf import Document

ROOT = Path(__file__).resolve().parents[3]
FLOWERS = ['glowing_spirit_flowers', 'blossom_bush', 'spirit_lantern_post',
           'lotus_flower', 'hanging_blossom_lantern', 'petal_ground_cover']


def triangles(part):
    return sorted(tuple(sorted(tuple(round(v, 12) for v in part['position'][index])
                               for index in part['indices'][i:i+3]))
                  for i in range(0, len(part['indices']), 3))


class FlowerTests(unittest.TestCase):
    def test_indexed_petals_keep_all_faces_bounds_and_analytic_outward_normals(self):
        center, radii = [.37, .21, .58], [.24, .065, .19]
        old, new = Mesh('flower'), Mesh('flower')
        for mesh in (old, new):
            mesh.materials['petal'] = ([.9, .4, .7], None, 0, .8)
        ellipsoid(old, center, radii, 'petal', 10, 5)
        untextured_ellipsoid(new, center, radii, 'petal', 10, 5)
        part = new.parts['petal']
        self.assertEqual(len(part['position']), 42)
        self.assertEqual(len(part['indices']) // 3, 80)
        self.assertEqual(triangles(part), triangles(old.parts['petal']))
        self.assertEqual(set(part['indices']), set(range(42)))
        for axis in range(3):
            self.assertAlmostEqual(min(p[axis] for p in part['position']), min(p[axis] for p in old.parts['petal']['position']))
            self.assertAlmostEqual(max(p[axis] for p in part['position']), max(p[axis] for p in old.parts['petal']['position']))
        for p, n, t, uv in zip(part['position'], part['normal'], part['tangent'], part['uv']):
            self.assertAlmostEqual(sum(v*v for v in n), 1.)
            self.assertAlmostEqual(sum(v*v for v in t[:3]), 1.)
            self.assertAlmostEqual(sum(a*b for a,b in zip(n,t)), 0.)
            self.assertEqual(t[3], 1.)
            gradient = [(p[k]-center[k]) / radii[k]**2 for k in range(3)]
            length = math.sqrt(sum(v*v for v in gradient))
            self.assertGreater(sum(n[k]*gradient[k] / length for k in range(3)), .999999)
            self.assertTrue(all(math.isfinite(v) and 0 <= v <= 1 for v in uv))
        for offset in range(0, len(part['indices']), 3):
            ids = part['indices'][offset:offset+3]
            a,b,c = [part['position'][i] for i in ids]
            normal = cross([b[k]-a[k] for k in range(3)], [c[k]-a[k] for k in range(3)])
            self.assertGreater(sum(v*v for v in normal), 1e-16)
            for index in ids:
                self.assertGreater(sum(v*n for v,n in zip(normal,part['normal'][index])), 0.)

    def test_textured_material_cannot_silently_use_untextured_uv_parameterization(self):
        mesh = Mesh('flower')
        mesh.materials['textured'] = ([1, 1, 1], 'must-preserve-uv.png', 0, .8)
        with self.assertRaisesRegex(ValueError, 'untextured'):
            untextured_ellipsoid(mesh, [.5]*3, [.2]*3, 'textured')
        self.assertFalse(mesh.parts)

    def test_six_shipped_flower_glbs_have_complete_valid_tangent_frames_and_indices(self):
        total_vertices = total_triangles = 0
        for slug in FLOWERS:
            doc = Document(ROOT/'assets/terra/blocks/models'/f'{slug}.glb')
            for mesh in doc.doc['meshes']:
                for primitive in mesh['primitives']:
                    attrs = {key: doc.values(index) for key,index in primitive['attributes'].items()}
                    positions, normals, tangents = attrs['POSITION'], attrs['NORMAL'], attrs['TANGENT']
                    indices = [i[0] for i in doc.values(primitive['indices'])]
                    self.assertEqual(set(indices),set(range(len(positions))),slug)
                    total_vertices += len(positions); total_triangles += len(indices)//3
                    for n,t in zip(normals,tangents):
                        self.assertAlmostEqual(sum(v*v for v in n),1.,places=5)
                        self.assertAlmostEqual(sum(v*v for v in t[:3]),1.,places=5)
                        self.assertAlmostEqual(sum(a*b for a,b in zip(n,t)),0.,places=5)
                        self.assertEqual(abs(t[3]),1.)
                    self.assertTrue(all(math.isfinite(v) for a in attrs.values() for row in a for v in row))
                    for i in range(0,len(indices),3):
                        ids=indices[i:i+3]; a,b,c=[positions[k] for k in ids]
                        normal=cross([b[k]-a[k] for k in range(3)],[c[k]-a[k] for k in range(3)])
                        self.assertGreater(sum(v*v for v in normal),1e-18,slug)
                        for index in ids:
                            self.assertGreater(sum(v*n for v,n in zip(normal,normals[index])),0.,slug)
        self.assertLessEqual(total_vertices,10_000)
        # The user-requested voxel art replaces the historical curved-petal
        # meshes. Their current surface/budget contract is checked separately in
        # voxel_props/test_geometry.py; do not freeze an obsolete smooth layout.
        self.assertLessEqual(total_triangles,4_000)


if __name__ == '__main__': unittest.main()
