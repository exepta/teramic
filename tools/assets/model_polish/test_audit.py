"""Shipping asset validity and the concave crest regression."""
import math
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from model_polish.audit import audit
from build_castle import Mesh
from build_cemetery import LIGHTS, emission_offset, mesh_for
from cemetery.catalog import DEFINITIONS
import json


class AuditTests(unittest.TestCase):
    def test_cemetery_light_centers_follow_the_actual_emissive_geometry(self):
        root = Path(__file__).resolve().parents[3]
        for definition in DEFINITIONS:
            if definition['id'] not in LIGHTS:
                continue
            block = json.loads((root/'assets/terra/blocks'/(definition['slug']+'.json')).read_text())
            self.assertEqual(block['light']['offset'], emission_offset(mesh_for(definition)))

    def test_existing_block_assets_have_complete_geometry_and_texture_references(self):
        result = audit(399)
        self.assertEqual(result['blocks'], 399)
        self.assertEqual(result['failing_blocks'], 0, [row for row in result['definitions'] if row['errors']])
        self.assertFalse([row for row in result['geometry'] if row['warnings']])

    def test_concave_anchor_arms_cover_only_the_original_outline(self):
        outline = [(0,-.68),(.57,-.30),(.68,0),(.95,-.08),(.72,-.62),(0,-.87)]
        for sign in [-1,1]:
            points = [(x*sign,y,0) for x,y in outline]
            expected = abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(points,points[1:]+points[:1])))/2
            mesh = Mesh('fixture')
            mesh.polygon(points, 'anchor')
            part = mesh.parts['anchor']
            area = 0
            for i in range(0,len(part['indices']),3):
                tri = part['indices'][i:i+3]
                a,b,c = [part['position'][k] for k in tri]
                cross = (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
                self.assertGreater(cross*part['normal'][tri[0]][2], 0)
                area += abs(cross)/2
                # A simple-polygon ear clipping covers exactly its boundary area.
            self.assertTrue(math.isclose(area,expected,abs_tol=1e-10))
            self.assertEqual(len(part['indices'])//3,len(points)-2)


if __name__ == '__main__':
    unittest.main()
