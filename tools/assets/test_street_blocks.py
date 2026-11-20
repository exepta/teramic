#!/usr/bin/env python3
"""Validate the street palette's catalog, seamless PBR and road contact shapes."""
import json
import math
from pathlib import Path
import unittest
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'


class StreetBlocks(unittest.TestCase):
    def test_materials_have_unique_append_only_ids_and_complete_linear_normal_pairs(self):
        definitions = [json.loads(p.read_text()) for p in BLOCKS.glob('*.json')]
        ids = [d['id'] for d in definitions]
        self.assertEqual(len(ids), len(set(ids)))
        added = {d['id']: d for d in definitions if 440 <= d['id'] <= 448}
        self.assertEqual(set(added), set(range(440, 449)))
        for d in added.values():
            surface = d['materials']['Surface']
            self.assertTrue((BLOCKS / surface['normal_texture']).is_file())
            self.assertEqual(len(d['face_materials']), 6)
        self.assertLess(added[440]['materials']['Surface']['roughness'], 0.4)
        self.assertGreater(added[448]['materials']['Surface']['roughness'], 0.85)

    def test_repeat_edges_and_normal_map_vectors_are_seamless_and_finite(self):
        for texture in (BLOCKS / 'textures/street').rglob('*.png'):
            image = Image.open(texture).convert('RGBA')
            self.assertEqual(image.size, (256, 256))
            self.assertEqual(list(image.crop((0, 0, 256, 1)).getdata()),
                             list(image.crop((0, 255, 256, 256)).getdata()))
            self.assertEqual(list(image.crop((0, 0, 1, 256)).getdata()),
                             list(image.crop((255, 0, 256, 256)).getdata()))
            colors = set(image.getdata())
            self.assertGreater(len(colors), 128)
            if texture.stem.endswith('_normal'):
                for r, g, b, a in colors:
                    length = sum((channel / 127.5 - 1) ** 2 for channel in (r, g, b)) ** 0.5
                    self.assertAlmostEqual(length, 1, delta=0.015)
                    self.assertGreaterEqual(b, 128)
                    self.assertEqual(a, 255)

    def test_six_profiles_have_solid_decks_and_a_lowered_connection_to_asphalt_cover(self):
        profiles = {json.loads(p.read_text())['id']: json.loads(p.read_text())['collision_boxes']
                    for p in BLOCKS.glob('sidewalk*.json') if 'collision_boxes' in json.loads(p.read_text())}
        self.assertEqual(set(profiles), set(range(441, 448)))
        for id, boxes in profiles.items():
            for box in boxes:
                self.assertTrue(all(0 <= lo < hi <= 1 for lo, hi in zip(box['minimum'], box['maximum'])))
            if id in (441, 442, 443, 444):
                self.assertEqual(len(boxes), 1, 'curbs must not rise above the walking deck')
                height = .5 if id == 441 else .25
                self.assertEqual(boxes[0], {'minimum': [0, 0, 0], 'maximum': [1, height, 1]})
            if id in (445, 446):
                self.assertTrue(all(box['maximum'][1] == .25 for box in boxes))
                area = sum((b['maximum'][0] - b['minimum'][0]) *
                           (b['maximum'][2] - b['minimum'][2]) for b in boxes)
                self.assertAlmostEqual(area, math.pi / 4, delta=.001)
        ramp = profiles[447]
        self.assertEqual(ramp[0]['minimum'], [0, 0, 0])
        self.assertLess(ramp[0]['maximum'][1] - 0.125, 0.025)
        self.assertEqual(ramp[-1]['maximum'], [1, 0.25, 1])
        self.assertTrue(all(a['maximum'][2] == b['minimum'][2] for a, b in zip(ramp, ramp[1:])))

    def test_curbs_are_darker_deck_materials_in_the_actual_left_right_and_curve_regions(self):
        base = Image.open(BLOCKS / 'textures/street/paving.png').convert('RGB')
        samples = {
            442: ((128, 20), (128, 180)),
            443: ((20, 128), (128, 128)),
            444: ((235, 128), (128, 128)),
            445: ((91, 91), (128, 128)),
            446: ((164, 91), (128, 128)),
            447: ((128, 20), (128, 180)),
        }
        for definition_path in BLOCKS.glob('sidewalk*.json'):
            definition = json.loads(definition_path.read_text())
            if definition['id'] not in samples:
                continue
            self.assertEqual(definition['face_materials'][3], 'Deck')
            self.assertEqual(definition['materials']['Deck']['normal_texture'],
                             definition['materials']['Surface']['normal_texture'])
            top = Image.open(BLOCKS / definition['textures']['Deck']).convert('RGB')
            dark, light = samples[definition['id']]
            self.assertLess(sum(top.getpixel(dark)), sum(base.getpixel(dark)) * .85)
            self.assertEqual(top.getpixel(light), base.getpixel(light))


if __name__ == '__main__':
    unittest.main()
