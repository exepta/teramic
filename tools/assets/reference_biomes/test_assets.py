"""Validate shipped authored plants, the actual GLBs and cactus cube seams."""
import json,math,sys,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from build_castle import BLOCKS
from model_polish.gltf import Document
from reference_biomes.catalog import DEFINITIONS

class ReferencePlants(unittest.TestCase):
    def test_catalog_ids_individual_texture_paths_and_nonblocking_flora(self):
        for material,slug,_,_ in DEFINITIONS:
            d=json.loads((BLOCKS/f'{slug}.json').read_text())
            self.assertEqual(d['id'],material)
            if material>422:
                self.assertEqual(d['render_kind'],'custom_mesh')
                self.assertEqual(d['collision_boxes'],[])
                self.assertTrue(d['struct_block'])
                self.assertNotIn('light',d)
            for filename in d['textures'].values():
                data=(BLOCKS/filename).read_bytes()
                self.assertEqual(data[:8],b'\x89PNG\r\n\x1a\n')
                self.assertEqual([int.from_bytes(data[i:i+4],'big') for i in (16,20)],[256,256])
    def test_cactus_has_six_complete_seamless_faces_and_three_surface_textures(self):
        d=json.loads((BLOCKS/'cactus.json').read_text())
        self.assertEqual(d['render_kind'],'cube')
        self.assertEqual(d['bevel_width'],0)
        self.assertEqual(len(set(d['textures'].values())),3)
        self.assertEqual(d['face_materials'][0],d['face_materials'][1])
        self.assertNotEqual(d['face_materials'][2],d['face_materials'][3])
        doc=Document(BLOCKS/'models/cactus.glb')
        p=doc.doc['meshes'][0]['primitives'][0]
        positions=doc.values(p['attributes']['POSITION'])
        for axis in range(3):
            self.assertEqual(min(v[axis] for v in positions),0)
            self.assertEqual(max(v[axis] for v in positions),1)
    def test_voxel_silhouettes_winding_tangents_and_closed_outer_surface(self):
        for material,slug,_,_ in DEFINITIONS[1:]:
            doc=Document(BLOCKS/f'models/{slug}.glb');triangles=0
            for mesh in doc.doc['meshes']:
                for p in mesh['primitives']:
                    points=doc.values(p['attributes']['POSITION']);normals=doc.values(p['attributes']['NORMAL'])
                    tangents=doc.values(p['attributes']['TANGENT'])
                    indices=[x[0] for x in doc.values(p['indices'])]
                    self.assertTrue(all(0<=v<=1 for point in points for v in point),slug)
                    self.assertTrue(all(abs(v*32-round(v*32))<1e-4 for point in points for v in point),slug)
                    for n,t in zip(normals,tangents):
                        self.assertEqual(sum(abs(v)>.5 for v in n),1,slug)
                        self.assertAlmostEqual(sum(v*v for v in n),1,places=4)
                        self.assertLess(abs(sum(n[i]*t[i] for i in range(3))),1e-4)
                    for i in range(0,len(indices),3):
                        a,b,c=[points[k] for k in indices[i:i+3]]
                        u=[b[j]-a[j] for j in range(3)];v=[c[j]-a[j] for j in range(3)]
                        cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                        self.assertGreater(sum(cross[j]*normals[indices[i]][j] for j in range(3)),0,slug)
                    triangles+=len(indices)//3
            self.assertGreater(triangles,40)
            self.assertLess(triangles,6000,(slug,triangles))

if __name__=='__main__':unittest.main()
