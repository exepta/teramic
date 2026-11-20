#!/usr/bin/env python3
"""Build the reference cemetery block set from closed authored geometry."""
import json
from pathlib import Path
from build_castle import Mesh,BLOCKS
from build_spirit_blossom import export
from cemetery.catalog import DEFINITIONS
from cemetery.descriptions import describe
from cemetery import architecture,graves,decoration,textures
from cemetery.geometry import cylinder,ellipsoid

COLORS={'blue':(80,116,185),'red':(174,67,60),'green':(100,127,68),'yellow':(219,167,54),'white':(219,207,191),'black':(48,44,48),'gray':(135,127,134),'dark_gray':(75,69,77),'purple':(139,74,167),'pink':(219,123,173),'lime':(161,196,65),'turquoise':(72,169,174),'orange':(216,114,43)}
# Lanterns cover the 31 m crypt chambers; candles provide a smaller local pool.
# These sources still share the existing unshadowed light budget.
LIGHTS={358:([1,.73,.40],450,9),369:([1,.75,.44],2400,28),370:([1,.61,.27],700,11),371:([1,.77,.47],500,10),372:([1,.75,.43],900,13),373:([.70,.30,1],700,11),386:([.69,.30,1],100,6),395:([.60,.25,1],100,5),396:([.65,.30,1],240,8),397:([.67,.31,1],350,8)}


def emission_offset(mesh):
    points=[point for name,part in mesh.parts.items() if name in ('flame','soul','crystal') for point in part['position']]
    # Rune cubes have an emissive face palette instead of an inset flame.
    return [round((min(p[i] for p in points)+max(p[i] for p in points))/2,6) for i in range(3)] if points else [.5,.5,.5]


def palette(mesh,slug):
    for name,color in {
      'stone':[.58,.52,.59],'trim':[.73,.66,.69],'shadow':[.10,.085,.12],
      'gold':[.90,.63,.24],'iron':[.17,.18,.22],'wood':[.27,.16,.12],
      'wax':[.94,.85,.65],'flame':[1,.65,.24],'soul':[.78,.27,1],
      'bone':[.70,.65,.53],'ivory':[.88,.81,.66],'leaf':[.20,.34,.10],
      'moss':[.34,.42,.17],'flower':[.79,.35,.69],'soil':[.20,.14,.16],
      'cloth':[.28,.075,.36],'leather':[.30,.16,.09],'crystal':[.73,.23,1],
      'dry':[.58,.44,.18],
    }.items():mesh.materials[name]=(color,slug+'.png' if name=='stone' else None,.45 if name in ('gold','iron') else 0,.38 if name in ('gold','crystal') else .87)


def mesh_for(definition):
    slug=definition['slug'];family=definition['family'];mesh=Mesh('cemetery');palette(mesh,slug)
    if definition['id']<=343 or family in ('rune','inactive_rune','mist'):
        mesh.materials['block']=([1,1,1,.35] if family=='mist' else [1,1,1],slug+'.png',0,.25 if family=='mist' else .87)
        mesh.box([0,0,0],[1,1,1],'block',.012,True)
        return mesh
    for author in [architecture.build,graves.build,decoration.light,decoration.fabric,decoration.vegetation,decoration.mechanism]:
        if author(mesh,family):return mesh
    raise ValueError('Missing authored geometry: '+family)


def main(selected=None):
    report=[]
    for d in DEFINITIONS:
        if selected is not None and d['id'] not in selected:continue
        slug=d['slug'];family=d['family'];material=d['id']
        color=COLORS[family[5:]] if family.startswith('clay_') else (91,83,102) if family in ('dark_stone','cracked_stone','crypt_brick') else (149,132,148)
        textures.write(BLOCKS/'textures'/f'{slug}.png',color,family)
        mesh=mesh_for(d)
        texture_refs=export(mesh,slug,('flame','soul','crystal'))
        definition=dict(schema_version=1,id=material,name=d['name'],description=describe(d),creative_category='terrain' if material<=343 else 'nature' if 381<=material<=387 else 'decoration',hardness=.4 if 359<=material<=361 or 381<=material<=387 else 1.8,resistance=4,required_tool=None,mining_level=0)
        if material<=343 or material in (397,398,399):
            palette_def=dict(base_color=[1,1,1,.35] if material==399 else [1,1,1,1])
            if material==397:palette_def['emission']=[.75,.13,1.2]
            if material==399:palette_def.update(roughness=.3)
            definition.update(render_kind='cube',bevel_width=0,materials={slug:palette_def},textures={slug:f'textures/{slug}.png'},face_materials=[slug]*6)
        else:
            definition.update(render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.glb',struct_block=True,textures=texture_refs,collision_boxes=mesh.collision)
        if material in LIGHTS:
            color,intensity,radius=LIGHTS[material]
            definition['light']=dict(color=color,intensity=intensity,range=radius,falloff='soft_core',flicker=.025,cave_intensity=.65,shadows=False,offset=emission_offset(mesh))
        (BLOCKS/f'{slug}.json').write_text(json.dumps(definition,indent=2)+'\n')
        report.append(dict(id=material,slug=slug,vertices=sum(len(p['position']) for p in mesh.parts.values()),triangles=sum(len(p['indices'])//3 for p in mesh.parts.values()),collision_boxes=len(mesh.collision)))
    out=Path(__file__).resolve().parent/'cemetery/geometry-report.json'
    existing={r['id']:r for r in json.loads(out.read_text())} if selected is not None and out.exists() else {}
    existing.update({r['id']:r for r in report})
    out.write_text(json.dumps(sorted(existing.values(),key=lambda r:r['id']),indent=2)+'\n')
    print(f'Generated {len(report)} cemetery blocks; {sum(r["triangles"] for r in report)} authored triangles total; max {max(r["triangles"] for r in report)}.')

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser();parser.add_argument('--only',type=int,nargs='+');args=parser.parse_args()
    main(set(args.only) if args.only else None)
