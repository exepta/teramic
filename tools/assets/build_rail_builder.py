#!/usr/bin/env python3
"""Author the locomotive's three-meter-wide clearing and track-laying attachment."""
import json
from build_castle import Mesh, BLOCKS, MODELS, RECIPES
from build_locomotive_parts import palette, cylinder

slug = 'train_rail_builder'
mesh = Mesh(slug)
palette(mesh)
mesh.materials['builder_yellow'] = ([.92,.57,.07], 'machinery-iron-hd.png', .35,.7)
mesh.box([-.65,.32,.12],[1.65,.73,.94],'builder_yellow',solid=True)
mesh.box([.25,.25,.78],[.75,.75,1.0],'train_dark_steel',solid=True)
# A stepped scoop, its teeth and hazard stripes share a welded support beam.
for i in range(12):
    x=-1+i*.25
    z=.06 + abs(x+.125-.5)*.18
    mesh.box([x,.03,z],[x+.25,.36,z+.18],'builder_yellow',solid=True)
    mesh.box([x+.07,0,z-.04],[x+.18,.09,z+.20],'train_dark_steel',solid=False)
    mesh.box([x+.02,.27,z-.006],[x+.13,.34,z+.002],'train_black_iron',solid=False)
for x in [-.72,1.72]:
    cylinder(mesh,[.5,.25,.58],.23,x-.08,x+.08,0,'train_dark_steel',solid=False,sides=16)
    cylinder(mesh,[.5,.25,.58],.12,x-.085,x+.085,0,'train_brass',solid=False,sides=12)
    mesh.box([x-.08,.74,.26],[x+.08,.92,.44],'train_dark_steel',solid=False)
    mesh.box([x-.06,.76,.245],[x+.06,.90,.26],'train_lamp_glow',solid=False)
for x in [-.40,.5,1.4]:
    mesh.box([x-.06,.72,.34],[x+.06,.91,.84],'train_dark_steel',solid=False)
mesh.export(slug)
(BLOCKS/(slug+'.json')).write_text(json.dumps(dict(schema_version=1,id=296,name='Lok-Freiräumer und Gleisleger',description='Front attachment. Clears a five-block-wide, nine-block-high path. Enable from train controls. Uses cargo rails and ballast in Survival; maximum 3 m/s with one freight wagon up to 15 m.',creative_category='trains',render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.gltf',struct_block=True,hardness=2.0,resistance=12,required_tool='pickaxe',mining_level=0,collision_boxes=mesh.collision),indent=2)+'\n')
cells=[{'item':{'mineral':'iron_ingot'},'count':1}]*9
cells[4]={'item':{'block':196},'count':1}
cells[7]={'item':{'block':29},'count':1}
(RECIPES/(slug+'.json')).write_text(json.dumps(dict(id='teramic::'+slug,name='Lok-Freiräumer und Gleisleger',station='workbench',output={'item':{'block':296},'count':1},pattern={'kind':'shaped','width':3,'height':3,'cells':cells}),indent=2)+'\n')
