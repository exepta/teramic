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

    def test_savanna_leaf_tips_remain_attached_to_the_rosette(self):
        from savanna.scenery import succulent
        for slug in ("savanna_aloe", "savanna_agave", "savanna_succulent"):
            with self.subTest(slug=slug):
                plant = VoxelSculpt(24)
                succulent(plant, slug)
                remaining = set(plant.cells)
                edge = [remaining.pop()]
                while edge:
                    x, y, z = edge.pop()
                    neighbors = {(x-1, y, z), (x+1, y, z), (x, y-1, z),
                                 (x, y+1, z), (x, y, z-1), (x, y, z+1)}
                    adjacent = remaining.intersection(neighbors)
                    edge.extend(adjacent)
                    remaining.difference_update(adjacent)
                self.assertFalse(remaining)

    def test_arch_opening_is_preserved_and_mushroom_light_is_exposed(self):
        arch = VoxelSculpt()
        sculpt_prop(arch, 20)
        for y in range(1, 18):
            self.assertNotIn((12, y, 12), arch.cells)
        for index in (12, 19):
            mushrooms = VoxelSculpt()
            sculpt_prop(mushrooms, index)
            self.assertIn('glow', mushrooms.emit(Mesh('test')).parts)

    def test_surface_biome_models_ship_stepped_silhouettes_and_keep_their_palette(self):
        for slug in ['ember_crystals', 'fossil_bones', 'blackened_stump', 'ash_mushrooms',
                     'desert_agave', 'pine_cones', 'wisteria_tassel', 'reef_arch',
                     'alpine_conifer', 'sakura_fox_statue', 'spirit_geode',
                     'root_arch', 'swamp_glowcaps', 'swamp_lantern_post', 'willow_boat',
                     'swamp_cattails', 'swamp_fern', 'fallen_willow']:
            with self.subTest(slug=slug):
                document = Document(MODELS / (slug+'.glb'))
                primitives = [p for mesh in document.doc['meshes'] for p in mesh['primitives']]
                self.assertGreaterEqual(len({p['material'] for p in primitives}), 2)
                for p in primitives:
                    for normal in document.values(p['attributes']['NORMAL']):
                        self.assertEqual(sum(abs(v) for v in normal), 1.)
                    for point in document.values(p['attributes']['POSITION']):
                        self.assertTrue(all(abs(v*24 - round(v*24)) < 0.0001 for v in point))

    def test_fossil_wood_minerals_and_fungal_colonies_ship_the_authored_voxel_surfaces(self):
        from collections import Counter
        for slug, grid in [
            ('petrified_dead_root', 24), ('shattered_stone_root', 24), ('cinder_root', 16),
            ('dark_mushrooms', 24), ('glowing_purple_mushrooms', 24), ('mineral_strata', 16),
            ('petrified_rubble', 16), ('petrified_mineral_deposit', 16),
        ]:
            with self.subTest(slug=slug):
                model = Document(MODELS / (slug + '.glb'))
                materials = model.doc['materials']
                counts = Counter()
                for mesh in model.doc['meshes']:
                    for p in mesh['primitives']:
                        name = materials[p['material']]['name']
                        counts[name] += model.doc['accessors'][p['indices']]['count'] // 3
                        for normal in model.values(p['attributes']['NORMAL']):
                            self.assertEqual(sorted(map(abs, normal)), [0., 0., 1.])
                        for point in model.values(p['attributes']['POSITION']):
                            self.assertTrue(all(abs(v*grid - round(v*grid)) < .0001 for v in point))
                self.assertLess(sum(counts.values()), 1200)
                if slug == 'mineral_strata':
                    self.assertEqual(set(counts), {'stone','gold','turquoise','glass','ochre','sand'})
                if slug == 'glowing_purple_mushrooms':
                    glow = next(m for m in materials if m['name'] == 'soul')
                    self.assertGreater(max(glow['emissiveFactor']), 0)
                    self.assertGreater(counts['soul'], 0)
                    self.assertGreater(counts['cloth'] + counts['flower'], counts['soul'])

    def test_raw_highland_quartz_keeps_voxel_faces_and_its_growth_mask(self):
        for slug in ('massive_amethyst', 'amethyst_lance', 'massive_aquamarine',
                     'aquamarine_lance', 'crystal_crown', 'crystal_shard_cluster'):
            with self.subTest(slug=slug):
                model = Document(MODELS / (slug + '.glb'))
                gem_materials = set()
                for mesh in model.doc['meshes']:
                    for primitive in mesh['primitives']:
                        attrs = primitive['attributes']
                        for normal in model.values(attrs['NORMAL']):
                            self.assertEqual(sorted(abs(v) for v in normal), [0., 0., 1.])
                        for point in model.values(attrs['POSITION']):
                            self.assertTrue(all(abs(v*16 - round(v*16)) < 1.e-5 for v in point))
                        material = model.doc['materials'][primitive['material']]
                        name = material['name'].removeprefix('aqua_')
                        if name in ('body', 'cleavage', 'inclusion'):
                            tex = model.doc['textures'][material['emissiveTexture']['index']]
                            self.assertTrue(model.doc['images'][tex['source']]['uri'].endswith('raw_growth_glow.png'))
                            gem_materials.add(name)
                self.assertEqual(gem_materials, {'body', 'cleavage', 'inclusion'})

    def test_salt_and_soul_crystals_keep_their_light_and_placement_orientation(self):
        for slug in ('salt_crystal_cluster', 'salt_stalagmite', 'salt_stalactite',
                     'salt_rubble', 'salt_pillar_fragment', 'soul_crystal_small', 'soul_crystal_large'):
            with self.subTest(model=slug):
                doc = Document(MODELS / (slug + '.glb'))
                ys = []
                lit = set()
                for mesh in doc.doc['meshes']:
                    for primitive in mesh['primitives']:
                        material = doc.doc['materials'][primitive['material']]
                        if any(material.get('emissiveFactor', [])):
                            lit.add(material['name'])
                        for n in doc.values(primitive['attributes']['NORMAL']):
                            self.assertEqual(sorted(map(abs, n)), [0., 0., 1.])
                        ys.extend(p[1] for p in doc.values(primitive['attributes']['POSITION']))
                if slug.startswith('soul'):
                    self.assertEqual(lit, {'crystal'})
                if slug == 'salt_crystal_cluster':
                    self.assertIn('Warm core', lit)
                if slug == 'salt_stalactite':
                    self.assertEqual(max(ys), 1.)
                else:
                    self.assertEqual(min(ys), 0.)

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
