"""Geometry/data regressions; run python3 -m unittest discover -s tools/assets/model_polish."""
import json
import math
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from model_polish.gltf import Document
from model_polish.bevel import components,box_bounds,replacement
from cemetery.catalog import DEFINITIONS
from build_castle import Mesh

ROOT=Path(__file__).resolve().parents[3]
BLOCKS=ROOT/'assets/terra/blocks'

class GeometryTests(unittest.TestCase):
    def test_closed_box_bevel_preserves_bounds_and_face_uv(self):
        mesh=Mesh('fixture');mesh.box([.1,.2,.3],[.8,.9,.7],'box',0);p=mesh.parts['box']
        attrs={'POSITION':[tuple(x) for x in p['position']],'NORMAL':p['normal'],'TEXCOORD_0':p['uv'],'TANGENT':p['tangent']}
        groups=list(components(attrs['POSITION'],p['indices']));self.assertEqual(len(groups),1)
        bounds=box_bounds(attrs['POSITION'],p['indices'],groups[0]);self.assertIsNotNone(bounds)
        result,indices=replacement(attrs,p['indices'],groups[0],bounds)
        self.assertEqual(len(indices)//3,44)
        for k in range(3):
            self.assertAlmostEqual(min(x[k] for x in result['POSITION']),bounds[0][k])
            self.assertAlmostEqual(max(x[k] for x in result['POSITION']),bounds[1][k])
        for n,t in zip(result['NORMAL'],result['TANGENT']):
            self.assertAlmostEqual(sum(x*x for x in n),1)
            self.assertAlmostEqual(sum(x*x for x in t[:3]),1)
            self.assertAlmostEqual(sum(x*y for x,y in zip(n,t)),0)
        self.assertTrue(all(math.isfinite(x) for uv in result['TEXCOORD_0'] for x in uv))

    def test_already_beveled_and_thin_surfaces_are_preserved(self):
        for bevel,high in [(0.02,[1,1,1]),(0,[1,.005,1])]:
            mesh=Mesh('fixture');mesh.box([0,0,0],high,'box',bevel);p=mesh.parts['box'];positions=[tuple(x) for x in p['position']]
            self.assertTrue(all(box_bounds(positions,p['indices'],group) is None for group in components(positions,p['indices'])))

    def test_polished_props_reference_all_vertices_and_keep_resolvable_bevel_faces(self):
        report=json.loads((ROOT/'tools/assets/model_polish/report.json').read_text())
        for record in report:
            if not record['beveled_boxes']:continue
            doc=Document(ROOT/record['path'])
            for mesh in doc.doc['meshes']:
                for primitive in mesh['primitives']:
                    positions=doc.values(primitive['attributes']['POSITION'])
                    indices=[v[0] for v in doc.values(primitive['indices'])]
                    self.assertEqual(set(indices),set(range(len(positions))),record['path'])
                    if 'terra_polish_source' not in primitive.get('extras',{}):continue
                    original=primitive['extras']['terra_polish_source']
                    source=doc.values(original['attributes']['POSITION'])
                    source_indices=[v[0] for v in doc.values(original['indices'])]
                    source_triangles={frozenset(source[k] for k in source_indices[i:i+3]) for i in range(0,len(source_indices),3)}
                    for axis in range(3):
                        self.assertAlmostEqual(min(p[axis] for p in positions),min(p[axis] for p in source),places=6)
                        self.assertAlmostEqual(max(p[axis] for p in positions),max(p[axis] for p in source),places=6)
                    for i in range(0,len(indices),3):
                        a,b,c=[positions[k] for k in indices[i:i+3]]
                        u=[b[k]-a[k] for k in range(3)];v=[c[k]-a[k] for k in range(3)]
                        cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                        if sum(x*x for x in cross)<=1e-10:
                            self.assertIn(frozenset((a,b,c)),source_triangles,record['path'])
                        self.assertGreater(sum(x*x for x in cross),0,record['path'])

    def test_exact_mechanical_profiles_and_authored_tangents_are_not_reshaped(self):
        from polish_models import policies
        excluded=policies()
        for name in ['crusher','hand_crank','conveyor','mixer','gearbox_horizontal',
                     'item_hopper','belt_distributor','wood_pipe','pump','rail_straight','rail_switch_left']:
            path=BLOCKS/'models'/(name+'.gltf')
            self.assertIn(path.resolve(),excluded)
            doc=Document(path)
            self.assertNotIn('terra_model_polish',doc.doc['asset'].get('extras',{}))

    def test_ornate_chest_lid_collision_matches_the_visible_stepped_top(self):
        spec=json.loads((BLOCKS/'ornate_crypt_chest.json').read_text())
        boxes=spec['collision_boxes']
        self.assertEqual(len(boxes),9)
        doc=Document(BLOCKS/'models/ornate_crypt_chest.glb')
        tested=0
        for mesh in doc.doc['meshes']:
            for primitive in mesh['primitives']:
                if doc.doc['materials'][primitive['material']]['name']!='cloth':continue
                for point in doc.values(primitive['attributes']['POSITION']):
                    if point[1]<=.540001:continue
                    tested+=1
                    self.assertTrue(any(all(lo-1e-6<=v<=hi+1e-6 for lo,v,hi in zip(box['minimum'],point,box['maximum'])) for box in boxes))
        self.assertGreater(tested,100)
        self.assertGreater(max(box['maximum'][1] for box in boxes),.70)

    def test_collapsed_face_repair_preserves_vertices_uvs_and_materials(self):
        import tempfile,base64,struct
        from model_polish.repair import remove_collapsed_faces
        positions=[(0.,0.,0.),(1.,0.,0.),(0.,1.,0.),(0.,1.,0.)]
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'fixture.gltf'
            data=b''.join(struct.pack('<3f',*p) for p in positions)+struct.pack('<6I',0,1,2,0,2,3)
            source={'asset':{'version':'2.0'},'buffers':[{'byteLength':len(data),'uri':'data:application/octet-stream;base64,'+base64.b64encode(data).decode()}],
                'bufferViews':[{'buffer':0,'byteLength':48},{'buffer':0,'byteOffset':48,'byteLength':24}],
                'accessors':[{'bufferView':0,'componentType':5126,'type':'VEC3','count':4},{'bufferView':1,'componentType':5125,'type':'SCALAR','count':6}],
                'materials':[{'name':'original material'}],'meshes':[{'primitives':[{'attributes':{'POSITION':0},'indices':1,'material':0}]}]}
            path.write_text(json.dumps(source))
            self.assertEqual(remove_collapsed_faces(path),1)
            doc=Document(path)
            self.assertEqual(doc.values(0),positions)
            self.assertEqual(doc.doc['materials'],source['materials'])
            self.assertEqual(doc.doc['meshes'][0]['primitives'][0]['attributes'],{'POSITION':0})
            self.assertEqual(doc.values(doc.doc['meshes'][0]['primitives'][0]['indices']),[(0,),(1,),(2,)])
            before=path.read_bytes();self.assertEqual(remove_collapsed_faces(path),0);self.assertEqual(path.read_bytes(),before)

    def test_cemetery_ids_models_textures_and_collision_fit_voxel(self):
        self.assertEqual([d['id'] for d in DEFINITIONS],list(range(324,400)))
        for d in DEFINITIONS:
            with self.subTest(block=d['slug']):
                definition=json.loads((BLOCKS/(d['slug']+'.json')).read_text());self.assertEqual(definition['id'],d['id'])
                for texture in definition['textures'].values():self.assertTrue((BLOCKS/texture).is_file())
                doc=Document(BLOCKS/'models'/(d['slug']+'.glb'))
                for mesh in doc.doc['meshes']:
                    for p in mesh['primitives']:
                        positions=doc.values(p['attributes']['POSITION']);normals=doc.values(p['attributes']['NORMAL']);indices=[v[0] for v in doc.values(p['indices'])]
                        self.assertTrue(all(-1e-5<=v<=1.00001 for point in positions for v in point))
                        self.assertTrue(all(abs(sum(x*x for x in normal)-1)<1e-5 for normal in normals))
                        for i in range(0,len(indices),3):
                            a,b,c=[positions[index] for index in indices[i:i+3]];u=[b[k]-a[k] for k in range(3)];v=[c[k]-a[k] for k in range(3)];cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                            self.assertGreater(sum(x*x for x in cross),1e-18)
                for box in definition.get('collision_boxes',[]):
                    self.assertTrue(all(0<=a<b<=1 for a,b in zip(box['minimum'],box['maximum'])))
                if 'light' in definition:self.assertFalse(definition['light']['shadows'])

if __name__=='__main__':unittest.main()
