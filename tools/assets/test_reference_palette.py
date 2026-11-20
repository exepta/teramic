import json, math, sys, unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from build_block_palette import BLOCKS, FAMILIES, wood_pixel, mineral_pixel
from biome_surfaces.test_surfaces import pixels
from model_polish.gltf import Document

class ReferencePalette(unittest.TestCase):
    def test_palette_materials_are_shared_matte_pbr_with_complete_normal_maps(self):
        ids={i for pair in FAMILIES.values() for i in pair}|{3,4,42,13}
        definitions={json.loads(p.read_text())['id']:json.loads(p.read_text()) for p in BLOCKS.glob('*.json')}
        for i in ids:
            d=definitions[i]
            self.assertEqual(d['render_kind'],'cube')
            for name,m in d['materials'].items():
                self.assertGreaterEqual(m['roughness'],.9)
                self.assertNotIn('texture_pattern',m)
                for path in [d['textures'][name],m['normal_texture']]:
                    w,h,rows=pixels(BLOCKS/path)
                    self.assertEqual((w,h),(512,512))
                    self.assertTrue(all(row[0]==row[-1] for row in rows),path)
                    self.assertEqual(rows[0],rows[-1],path)
                _,_,normal=pixels(BLOCKS/m['normal_texture'])
                for row in normal[::17]:
                    for p in row[::17]:
                        n=[v/255*2-1 for v in p[:3]]
                        self.assertLess(abs(sum(v*v for v in n)-1),.025)
            if i in {pair[0] for pair in FAMILIES.values()}:
                self.assertNotEqual(d['face_materials'][0],d['face_materials'][3])
        # glTF +Y normal components oppose increasing height along positive UV.v.
        # This checks the shipped bytes, preventing an inverted relief convention.
        _,_,normals=pixels(BLOCKS/'textures/block_palette/oak_bark_normal.png')
        checks=0
        for x,y in [(x,y) for x in range(27,470,61) for y in range(31,475,59)]:
            h0=wood_pixel('oak','bark',x*256/511,(y-1)*256/511)[1]
            h1=wood_pixel('oak','bark',x*256/511,(y+1)*256/511)[1]
            if abs(h1-h0)*20>.015:
                ny=normals[y][x][1]/127.5-1
                self.assertLessEqual(ny*(h1-h0),0)
                checks+=1
        self.assertGreaterEqual(checks,2)
    def test_shrubs_are_open_stemmed_botany_and_keep_grounded_bounds(self):
        for slug in ['blossom_bush','flowering_shrub']:
            doc=Document(BLOCKS/'models'/f'{slug}.glb')
            definition=json.loads((BLOCKS/(slug+'.json')).read_text())
            model_materials={m['name'] for m in doc.doc['materials']}
            self.assertLessEqual(set(definition['textures']),model_materials)
            self.assertLessEqual(set(definition.get('materials',{})),model_materials)
            points=[]
            for m in doc.doc['meshes']:
                for p in m['primitives']:
                    points+=doc.values(p['attributes']['POSITION'])
            self.assertTrue(points)
            self.assertGreaterEqual(min(p[1] for p in points),0)
            self.assertLessEqual(max(p[1] for p in points),.8)
            self.assertLess(len(points),6000)
            # Large opaque crown boxes were nearly one filled half-block.
            green=[]
            for m in doc.doc['meshes']:
                for p in m['primitives']:
                    name=doc.doc['materials'][p['material']]['name']
                    if 'leaf' in name or 'moss' in name:
                        green+=doc.values(p['attributes']['POSITION'])
            self.assertTrue(green)
            self.assertLess(max(p[1] for p in green),.7)
            # Slender leaves/stems leave clear air beneath every flower head.
            self.assertLess(len(green),len(points)*.70)
    def test_species_have_distinct_colors_and_preserve_glass(self):
        colors={tuple(wood_pixel(s,'planks',103,27)[0]) for s in FAMILIES}
        self.assertEqual(len(colors),6)
        d=json.loads((BLOCKS/'glass.json').read_text())
        self.assertLess(d['materials']['Pane']['base_color'][3],.2)
        self.assertIn(d['frame_material'],d['materials'])


class FineSandAndBiomeTurf(unittest.TestCase):
    def test_shipped_sand_is_pale_beige_with_fine_low_contrast_relief(self):
        w,h,rows=pixels(BLOCKS/'textures/block_palette/sand.png')
        colors=[p for row in rows for p in row]
        mean=[sum(p[c] for p in colors)/len(colors) for c in range(3)]
        self.assertTrue(230<mean[0]<240,mean)
        self.assertTrue(210<mean[1]<222,mean)
        self.assertTrue(175<mean[2]<190,mean)
        self.assertLess(mean[0]-mean[2],60)
        adjacent=[abs(rows[y][x][0]-rows[y][x-1][0]) for y in range(h) for x in range(1,w)]
        self.assertLess(max(adjacent),12)
        self.assertGreater(sum(v>0 for v in adjacent)/len(adjacent),.5)
        _,_,normal=pixels(BLOCKS/'textures/block_palette/sand_normal.png')
        self.assertTrue(all(p[2]>=250 for row in normal for p in row))
    def test_biome_shader_normalizes_the_actual_shipped_turf_and_soil_means(self):
        import re
        shader=(BLOCKS.parents[1]/'shaders/terra_wet_surface.wesl').read_text()
        constants={tuple(map(float,v.split(','))) for v in re.findall(r'vec3<f32>\((0\.\d+, 0\.\d+, 0\.\d+)\)',shader)}
        for slug in ['spirit_blossom_grass','blood_grass','spirit_blossom_grass_bottom','blood_grass_bottom']:
            _,_,rows=pixels(BLOCKS/f'textures/{slug}.png')
            def linear(v):
                s=v/255
                return s/12.92 if s<=.04045 else ((s+.055)/1.055)**2.4
            samples=[p for row in rows for p in row]
            mean=[sum(linear(p[c]) for p in samples)/len(samples) for c in range(3)]
            self.assertTrue(any(max(abs(a-b) for a,b in zip(mean,v))<.00001 for v in constants),(slug,mean))

if __name__=='__main__':unittest.main()
