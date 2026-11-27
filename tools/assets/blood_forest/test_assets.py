"""Check the real shipped Blood Forest geometry, separate textures and unit seams."""
import unittest,json,math,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from model_polish.gltf import Document
from build_blood_forest import NAMES, SIZE as BIOME_SIZE
from blood_forest.sanctuary import CATALOG
from biome_surfaces.foliage import SIZE as FOLIAGE_SIZE
from build_block_palette import SIZE as PALETTE_SIZE, FAMILIES
BLOCKS=Path(__file__).resolve().parents[3]/'assets/terra/blocks'
class BloodAssets(unittest.TestCase):
    def test_complete_stable_catalog_and_individual_textures(self):
        for i,(slug,_) in enumerate(NAMES,400):
            spec=json.loads((BLOCKS/(slug+'.json')).read_text());self.assertEqual(spec['id'],i)
            for tex in spec['textures'].values():self.assertTrue((BLOCKS/tex).is_file(),tex)
            if i<412:
                for tex in spec['textures'].values():
                    data=(BLOCKS/tex).read_bytes()
                    size = {'foliage': FOLIAGE_SIZE, 'block_palette': PALETTE_SIZE}.get(
                        Path(tex).parent.name, BIOME_SIZE)
                    self.assertEqual((int.from_bytes(data[16:20],'big'),int.from_bytes(data[20:24],'big')),(size,size))
                self.assertEqual(spec['bevel_width'], .025 if i in FAMILIES['blood'] else 0)
                self.assertEqual(len(spec['face_materials']),6)
    def test_real_glbs_have_unit_bounds_correct_normals_and_no_degenerate_faces(self):
        model_slugs = [slug for slug, _ in NAMES] + [slug for slug, _, _ in CATALOG[4:]]
        for slug in model_slugs:
            doc=Document(BLOCKS/'models'/(slug+'.glb'))
            for mesh in doc.doc['meshes']:
                for p in mesh['primitives']:
                    pos=doc.values(p['attributes']['POSITION']);norm=doc.values(p['attributes']['NORMAL']);idx=[r[0] for r in doc.values(p['indices'])]
                    self.assertTrue(all(-1e-5<=v<=1.00001 for point in pos for v in point),slug)
                    self.assertTrue(all(abs(sum(v*v for v in n)-1)<1e-4 for n in norm),slug)
                    self.assertTrue(all(0<=n<len(pos) for n in idx),slug)
                    for at in range(0,len(idx),3):
                        a,b,c=[pos[n] for n in idx[at:at+3]];u=[b[k]-a[k] for k in range(3)];v=[c[k]-a[k] for k in range(3)]
                        cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                        self.assertGreater(sum(n*n for n in cross),1e-18,(slug,at))
                        self.assertGreater(sum(cross[k]*sum(norm[n][k] for n in idx[at:at+3]) for k in range(3)),0,(slug,at))
    def test_grass_and_log_faces_have_distinct_authored_top_and_sides(self):
        for slug in ('blood_grass','blood_moss','crimson_mycelium','bloodwood_log'):
            spec=json.loads((BLOCKS/(slug+'.json')).read_text())
            self.assertNotEqual(spec['face_materials'][0],spec['face_materials'][3])

    def test_shrine_materials_have_authored_shapes_textures_and_registered_light(self):
        for index, (slug, name, _) in enumerate(CATALOG):
            spec = json.loads((BLOCKS / (slug + '.json')).read_text())
            self.assertEqual((spec['id'], spec['name']), (492 + index, name))
            if index < 4:
                self.assertTrue((BLOCKS / spec['textures'][slug]).is_file())
                self.assertEqual(len(spec['face_materials']), 6)
            else:
                self.assertTrue((BLOCKS / spec['model']).is_file())
            if index in (5, 6, 9, 10):
                self.assertGreater(spec['light']['range'], 5)
                data = Document(BLOCKS / spec['model']).doc
                self.assertTrue(any(max(m.get('emissiveFactor', [0])) > 0 for m in data['materials']))
if __name__=='__main__':unittest.main()
