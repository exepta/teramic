#!/usr/bin/env python3
"""Reproducible reference-biome plants: independent JSON/GLB/256px textures."""
import json,math
from pathlib import Path
from build_castle import Mesh,BLOCKS
from build_spirit_blossom import png,export,noise
from reference_biomes.catalog import DEFINITIONS,PALETTE
from reference_biomes.geometry import sculpture
from reference_biomes.bark import generate as bark_texture

def texture(slug,side=None):
    pixels=[]
    for y in range(256):
        for x in range(256):
            grain=.96+.06*noise(x//2,y//2,177)
            if side=='side':
                u=x/256;v=y/256
                ridge=(u*10+.07*math.sin(v*math.tau))%1
                shade=.85+.20*math.sin(ridge*math.pi)
                c=[77*shade,124*shade,59*shade]
                # Staggered ivory areoles with short paired spines.
                xx=(x+(y//40%2)*13)%26-13;yy=y%40-20
                if xx*xx+yy*yy<7 or (abs(yy)<1 and abs(xx)<5):c=[215,207,161]
            elif side in ('top','bottom'):
                dx=x-127.5;dy=y-127.5;r=math.hypot(dx,dy)
                shade=.90+.10*math.cos(r*.12)
                c=([108,154,67] if side=='top' else [155,155,92])
                c=[v*shade for v in c]
                if side=='top' and r<26:
                    a=math.atan2(dy,dx)
                    if r<15+8*math.cos(a*6):c=[235,121,147]
            else:
                # Soft surface facets retain detail without sparkling noise.
                shade=.91+.08*noise(x//20,y//20,31)
                vein=abs((x+int(3*math.sin(y*.05)))%47-23)<1
                c=[255*(shade-.04*vein)]*3
            pixels.append([max(0,min(255,int(v*grain))) for v in c]+[255])
    name=f'cactus_{side}' if side else slug
    png(BLOCKS/'textures'/f'{name}.png',pixels,256)

def main():
    bark_texture()
    locales=[]
    for material,slug,name,german in DEFINITIONS:
        description=f'{name}, shaped for the regional biome flora.'
        if material==422:
            description='A branching desert cactus with solid ivory thorns. Touching its sides or crown injures creatures and survival players.'
        d=dict(schema_version=1,id=material,name=name,description=description,
               creative_category='nature',hardness=.5 if material==422 else .15,
               resistance=.4,required_tool=None,mining_level=0)
        mesh=Mesh('reference_biomes')
        if material==422:
            for side in ('side','top','bottom'):texture(slug,side)
            d.update(render_kind='cube',bevel_width=0,
                     textures={f'cactus_{s}':f'textures/cactus_{s}.png' for s in ('side','top','bottom')},
                     materials={f'cactus_{s}':dict(base_color=[1,1,1,1],roughness=.91) for s in ('side','top','bottom')},
                     face_materials=['cactus_side','cactus_side','cactus_bottom','cactus_top','cactus_side','cactus_side'])
            mesh.materials['cactus_side']=([1,1,1],'cactus_side.png',0,.91)
            mesh.box([0,0,0],[1,1,1],'cactus_side');export(mesh,slug)
        else:
            texture(slug)
            for key,color in PALETTE.items():mesh.materials[key]=(color,slug+'.png',0,.9)
            sculpture(material).emit(mesh)
            textures=export(mesh,slug)
            d.update(render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.glb',
                     textures=textures,struct_block=True,collision_boxes=[])
        (BLOCKS/f'{slug}.json').write_text(json.dumps(d,indent=2)+'\n')
        locales.extend([dict(source=name,en=name,de=german),
                        dict(source=description,en=description,de=f'{german} für die regionale Biomvegetation.')])
    destination=Path(__file__).resolve().parents[2]/'src-ui/locales/source/reference-biomes.json'
    destination.write_text(json.dumps(locales,indent=2,ensure_ascii=False)+'\n')
    print('Generated eight independent biome blocks with GLBs and 256px detail textures')

if __name__=='__main__':main()
