#!/usr/bin/env python3
"""Deterministic voxel lanterns and interlocking chain, using the castle bevels.

One lantern mesh carries three explicitly named material groups. Runtime and
item previews select exactly one mount; no hidden overlapping pose geometry.
"""
from pathlib import Path
import json
from build_castle import Mesh
from recipe_paths import recipe_path

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'


def lantern():
    mesh = Mesh('iron')
    for pose in range(3):
        prefix = f'lantern_{pose}_'
        colors = {
            'iron': ([.27, .28, .30], .72, .56),
            'edge': ([.46, .44, .39], .65, .49),
            'wood': ([.43, .22, .08], 0, .9),
            'gold': ([.89, .55, .16], .48, .6),
            'glass': ([1.0, .46, .045], 0, .28),
            'heart': ([1.0, .83, .30], 0, .4),
        }
        for name, (color, metal, rough) in colors.items():
            mesh.materials[prefix + name] = (color, None, metal, rough)

        def box(x, y, z, w, h, d, material='iron', bevel=.009):
            mesh.box([x, y, z], [x+w, y+h, z+d], prefix+material, bevel)

        center = .66 if pose == 1 else .5
        # Stepped iron sill/cap, small joints and warm amber glass panels.
        for y, width, height in [(.12, .52, .065), (.18, .59, .045),
                                  (.66, .59, .055), (.71, .47, .055),
                                  (.765, .34, .045), (.81, .22, .035)]:
            box(.5-width/2, y, center-width/2, width, height, width,
                'edge' if y in [.18, .66] else 'iron')
        for x in [.245, .705]:
            for z in [center-.255, center+.205]:
                box(x, .215, z, .05, .45, .05)
                for y in [.20, .635]:
                    box(x-.018, y, z-.018, .086, .055, .086, 'edge')
        for y in [.28, .49]:
            for z in [center-.256, center+.238]:
                box(.285, y, z, .42, .027, .018, 'gold', .003)
            for x in [.244, .738]:
                box(x, y, center-.205, .018, .027, .41, 'gold', .003)
        # Smaller separated luminous panes retain the voxel reference's pattern.
        for row in range(3):
            for col in range(3):
                mat = 'heart' if (row+col) % 3 == 1 else 'glass'
                for z in [center-.228, center+.208]:
                    box(.305+col*.13, .245+row*.13, z, .115, .115, .02, mat, .004)
                for x in [.272, .708]:
                    box(x, .245+row*.13, center-.19+col*.13, .02, .115, .115, mat, .004)
        # Hollow handle: light still shines through the top ring.
        for x in [.40, .565]:
            box(x, .825, center-.026, .035, .10, .052, 'edge', .007)
        box(.40, .91, center-.026, .20, .035, .052, 'edge', .007)
        if pose == 0:
            # A compact wooden table foot, rather than a full block pedestal.
            box(.15, .025, .15, .70, .075, .70, 'wood')
            box(.22, .10, .22, .56, .025, .56, 'edge')
            for x in [.19, .70]:
                for z in [.19, .70]:
                    box(x, 0, z, .11, .04, .11, 'iron')
        elif pose == 1:
            box(.35, .40, 0, .30, .60, .07, 'wood')
            box(.40, .43, .065, .20, .54, .025, 'iron')
            box(.45, .92, .04, .10, .08, .66, 'wood')
            # Two step braces visibly connect plate and arm without a triangle.
            box(.46, .70, .08, .08, .22, .10, 'wood')
            box(.46, .81, .18, .08, .11, .16, 'wood')
            box(.46, .88, .61, .08, .05, .09, 'edge')
            for y in [.46, .91]:
                box(.42, y, .088, .055, .055, .025, 'edge')
                box(.53, y, .088, .055, .055, .025, 'edge')
        else:
            box(.465, .91, .465, .07, .09, .07, 'edge')
    mesh.export('lantern')
    path = BLOCKS / 'models/lantern.gltf'
    gltf = json.loads(path.read_text())
    gltf['asset']['generator'] = 'Teramic lantern/chain authored voxel builder'
    gltf['extensionsUsed'] = ['KHR_materials_emissive_strength']
    for material in gltf['materials']:
        name = material['name']
        if name.endswith('_glass') or name.endswith('_heart'):
            material['emissiveFactor'] = [1, .40, .035] if name.endswith('_glass') else [1, .72, .20]
            material['extensions'] = {'KHR_materials_emissive_strength': {'emissiveStrength': 2.2}}
    path.write_text(json.dumps(gltf, separators=(',', ':'))+'\n')


def chain():
    mesh = Mesh('iron')
    for link in range(4):
        y = link*.25
        boxes = [([.40,y,.465],[.46,min(1.,y+.30),.535]),
                 ([.54,y,.465],[.60,min(1.,y+.30),.535]),
                 ([.40,y,.465],[.60,y+.06,.535]),
                 ([.40,min(.94,y+.24),.465],[.60,min(1.,y+.30),.535])]
        for low, high in boxes:
            if link % 2:
                low, high = [low[2],low[1],1-high[0]], [high[2],high[1],1-low[0]]
            mesh.box(low, high, 'iron', .008)
    mesh.export('chain')


def main():
    lantern()
    chain()
    for material, slug, name, description, hardness, collision in [
        (193,'lantern','Lantern','A warm iron lantern for tables, walls and ceiling chains.',.30,
         [{'minimum':[.15,0,.15],'maximum':[.85,.12,.85]},
          {'minimum':[.20,.12,.20],'maximum':[.80,.845,.80]},
          {'minimum':[.38,.825,.45],'maximum':[.62,.945,.55]}]),
        (194,'chain','Chain','Interlocking iron links. Attach beneath a solid beam and hang a lantern below.',.45,
         [{'minimum':[.40,0,.40],'maximum':[.60,1,.60]}]),
    ]:
        definition = {'schema_version':1,'id':material,'name':name,'description':description,
                      'creative_category':'decoration','render_kind':'custom_mesh','model_layout':'scene',
                      'model':f'models/{slug}.gltf','struct_block':True,'hardness':hardness,
                      'resistance':1.5,'required_tool':None,'mining_level':0,'collision_boxes':collision}
        if material == 193:
            definition['light'] = {'color':[1,.76,.40],'intensity':1500,'range':24,'falloff':'soft_core',
                                   'flicker':.025,'cave_intensity':.6,'shadows':False}
        (BLOCKS/(slug+'.json')).write_text(json.dumps(definition,indent=2)+'\n')
        ingredients = [{'item':{'mineral':'iron_ingot'},'count':2}]
        if material == 193:
            ingredients += [{'item':{'block':15},'count':1},{'item':{'block':40},'count':1}]
        recipe = {'id':'teramic::'+slug,'name':name,'station':'workbench',
                  'output':{'item':{'block':material},'count':1 if material==193 else 4},
                  'pattern':{'kind':'shapeless','ingredients':ingredients}}
        recipe_path(slug).write_text(json.dumps(recipe,indent=2)+'\n')


if __name__ == '__main__':
    main()
