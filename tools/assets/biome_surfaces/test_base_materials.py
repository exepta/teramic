"""Material-only grass derivative: real data, consistent near/LOD UVs, stable PBR."""
import hashlib
import json
import math
import os
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from biome_surfaces.base_materials import (
    BLOCKS, FILES, MODEL, SOURCE_MODEL, SAMPLERS, near_pixel,
)
from biome_surfaces.test_surfaces import pixels
from model_polish.gltf import Document

PREVIEW = os.environ.get('TERRA_BASE_PREVIEW')
TEXTURES = Path(PREVIEW) if PREVIEW else BLOCKS / 'textures/reference_base'
MODEL_PATH = TEXTURES / MODEL if PREVIEW else BLOCKS / 'models' / MODEL


class BaseMaterialArt(unittest.TestCase):
    def test_native_resolution_and_low_contrast_continuous_earth(self):
        for name in FILES:
            size, height, rows = pixels(TEXTURES / (name + '.png'))
            self.assertEqual((size, height), (1024, 1024) if name == 'grass_near' else (512, 512))
            self.assertTrue(all(p[3] == 255 for row in rows for p in row))
        _, _, rows = pixels(TEXTURES / 'earth.png')
        # Soil layers should not reproduce isolated bright gray/confetti pixels.
        jumps = [max(abs(rows[y][x][c] - rows[y][x + 1][c]) for c in range(3))
                 for y in range(512) for x in range(511)]
        self.assertLess(sum(value > 12 for value in jumps) / len(jumps), .005)
        self.assertTrue(all(p[0] > p[1] > p[2] for row in rows for p in row))
        self.assertGreater(max(p[0] for row in rows for p in row) - min(p[0] for row in rows for p in row), 20)
        # The paired normals use the new height forms, not the old pebble normals.
        for name in FILES:
            if name == 'snow':
                continue
            size, _, rows = pixels(TEXTURES / (name + '_normal.png'))
            for row in rows[::13]:
                for pixel in row[::13]:
                    normal = [value / 255 * 2 - 1 for value in pixel[:3]]
                    self.assertLess(abs(sum(v * v for v in normal) - 1), .016)
                    self.assertGreater(normal[2], .7)

    def test_model_keeps_geometry_shared_material_slots_samplers_and_matte_orm(self):
        parent, model = Document(SOURCE_MODEL), Document(MODEL_PATH)
        for key in ('meshes', 'accessors', 'nodes', 'scenes', 'scene', 'samplers', 'textures'):
            self.assertEqual(model.doc.get(key), parent.doc.get(key), key)
        self.assertEqual(len(model.doc['images']), len(parent.doc['images']))
        self.assertEqual(len(model.doc['materials']), 7)
        self.assertEqual(model.buffers[0][:len(parent.buffers[0])], parent.buffers[0])
        for a, b in zip(parent.doc['materials'], model.doc['materials']):
            self.assertEqual(a, b, 'Only image payloads change; every material property stays intact')
        proof = json.loads(MODEL_PATH.with_suffix('.art.json').read_text())
        self.assertEqual(proof['parent_sha256'], hashlib.sha256(SOURCE_MODEL.read_bytes()).hexdigest())

    def test_near_to_lod_affine_uv_mapping_including_mirrored_faces(self):
        provenance = json.loads(SOURCE_MODEL.with_suffix('.provenance.json').read_text())
        samplers = {'Up': SAMPLERS['grass_top'], 'Down': SAMPLERS['earth'],
                    'West': SAMPLERS['grass_side'], 'South': SAMPLERS['grass_side'],
                    'East': SAMPLERS['grass_side_mirrored'], 'North': SAMPLERS['grass_side_mirrored']}
        for face in provenance['faces']:
            for x, y in [(0.153, 0.837), (.391, .443), (.827, .237)]:
                uv = [a * x + b * y + c for a, b, c in face['lod_uv_from_positive_world_axes']]
                a, b = near_pixel(*uv), samplers[face['name']](x, y)
                self.assertTrue(all(abs(v - w) <= 1 for v, w in zip(a, b)), (face['name'], a, b))

    @unittest.skipIf(PREVIEW, 'Active JSON bindings are not modified during preview generation')
    def test_active_binding_retains_dirt_source_and_existing_oak_bark(self):
        grass = json.loads((BLOCKS / 'grass.json').read_text())
        dirt = json.loads((BLOCKS / 'dirt.json').read_text())
        oak = json.loads((BLOCKS / 'wood.json').read_text())
        snow = json.loads((BLOCKS / 'snow.json').read_text())
        self.assertEqual(grass['model'], 'models/' + MODEL)
        self.assertTrue(all(m['coarse_face'] == {'block': 1, 'side': 2} for m in dirt['materials'].values()))
        self.assertEqual(oak['textures']['Twisted tree bark'], 'textures/block_palette/oak_bark.png')
        self.assertEqual(snow['textures']['Soft snow'], 'textures/reference_base/snow.png')
        layer = json.loads((BLOCKS / 'snow_layer.json').read_text())
        self.assertEqual(layer['textures'], snow['textures'])


if __name__ == '__main__':
    unittest.main()
