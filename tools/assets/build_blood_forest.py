#!/usr/bin/env python3
"""Original Blood Forest block art: individual textures and closed meshes, no atlas."""
import json, math
from build_castle import Mesh, BLOCKS
from build_spirit_blossom import export, png, noise
from voxel_props.geometry import VoxelSculpt
from biome_surfaces import SIZE, rgb, mix, grain, field, leaf, soil, turf, stone, wood

NAMES=[('blood_grass','Blood Grass'),('blood_dirt','Blood Dirt'),('blood_stone','Blood Stone'),('veined_stone','Veined Stone'),('bloodwood_planks','Bloodwood Planks'),('bloodwood_log','Bloodwood Log'),('blood_leaves','Blood Leaves'),('crimson_mycelium','Crimson Mycelium'),('blood_moss','Blood Moss'),('blood_crystal_block','Blood Crystal Block'),('blood_fungal_block','Blood Fungal Block'),('blood_glow_mushroom_block','Blood Glow Mushroom Block'),('blood_mushrooms','Blood Mushrooms'),('blood_thorn_bush','Blood Thorn Bush'),('blood_flower','Blood Flower'),('hanging_blood_vines','Hanging Blood Vines'),('blood_roots','Blood Roots'),('blood_bones','Blood Bones'),('blood_fern','Blood Fern'),('giant_blood_mushroom','Giant Blood Mushroom'),('blood_ruined_arch','Blood Ruined Arch'),('blood_crystal_pillar','Blood Crystal Pillar')]
# Rose, warm bark and pale mauve carry enough green/blue luminance to remain
# readable below a canopy. These are albedos, not extra emissive terrain lights.
COLORS=[
    [173,90,110], [123,83,87], [135,108,125], [151,121,144],
    [174,96,103], [142,86,82], [199,89,124], [191,113,128],
    [162,85,109], [212,63,98], [151,104,118], [238,152,101],
]
PALETTE={
    'ivory':[.94,.87,.75], 'shadow':[.085,.035,.055],
    'wood':[.32,.17,.17], 'ridge':[.48,.26,.27],
    'leaf':[.65,.14,.23], 'petal':[.90,.25,.36],
    'pale':[1.,.65,.58], 'stem':[.40,.31,.22],
    'stone':[.43,.35,.45], 'trim':[.61,.48,.57],
    'bone':[.84,.77,.65], 'crystal':[.92,.10,.24], 'glow':[1.,.43,.17],
}

SURFACES = {400: 'turf', 401: 'earth', 402: 'rock', 403: 'rock',
            404: 'wood', 405: 'wood', 407: 'turf', 408: 'turf', 409: 'rock'}


def blood_leaf_pixel(x, y):
    # Preserve the brighter burgundy palette; add fine tapered/secondary veins.
    return leaf(x, y, blood=True)


def blood_bark_pixel(x, y, end=False):
    return wood(x, y, [174, 114, 99] if end else [145, 91, 85], end=end)


def texture_pixel(x, y, index, side=False, end=False):
    color = COLORS[min(index, 11)]
    if index in (0, 7, 8):
        return turf(x, y, color, COLORS[1], side=side, moss=index != 0)
    if index == 1:
        return soil(x, y, color)
    if index in (2, 3, 9):
        return stone(x, y, color, vein=[224, 101, 133] if index != 2 else None,
                     crystalline=index == 9)
    if index == 4:
        return wood(x, y, color, planks=True)
    if index == 5:
        return blood_bark_pixel(x, y, end)
    if index == 6:
        return blood_leaf_pixel(x, y)
    # Fungal pores and soft curved gills: irregular forms instead of dot grids.
    phase = y * math.tau / 32 + 2 * math.sin(x * math.tau / SIZE)
    shade = .93 + .055 * math.sin(phase) + .07 * field(x, y, 16, 721)
    c = rgb(color, shade * grain(x, y, 727))
    if field(x, y, 32, 733) > .82:
        c = rgb(mix(c, [248, 175, 133], .48))
    return c


def texture(slug, index, side=False, end=False):
    pixels = [texture_pixel(x, y, index, side, end) + [255]
              for y in range(SIZE) for x in range(SIZE)]
    png(BLOCKS / 'textures' / f'{slug}.png', pixels, SIZE)


def cube_art_only():
    """Do not rewrite voxel props, materials, collision or sculpted GLB meshes."""
    for index, (slug, _) in enumerate(NAMES[:12]):
        texture(slug, index)
        if index in (0, 7, 8):
            texture(slug + '_side', index, side=True)
            texture(slug + '_bottom', 1)
        elif index == 5:
            texture(slug + '_end', index, end=True)
        material = index + 400
        if material in SURFACES:
            path = BLOCKS / f'{slug}.json'
            definition = json.loads(path.read_text())
            definition['surface'] = SURFACES[material]
            path.write_text(json.dumps(definition, indent=2) + '\n')


def crystal(mesh,x,y,z,h=.65):
    # Square, visibly stepped crystal tiers, with no slanted smooth faces.
    for tier in range(6):
        r=h*.17*(1-tier/7)
        mesh.box([x-r,y+h*tier/6,z-r],[x+r,y+h*(tier+1)/6,z+r],'crystal')

def mushroom(mesh,x,z,h=.65):
    mesh.branch([x,.025,z],[x,h*.65,z],h*.1,'pale',end_radius=h*.07)
    mesh.blob([x,h*.65,z],[h*.4,h*.21,h*.4],'leaf')
    # The luminous underside is outside the cap rim; buried emissive surfaces
    # would correctly disappear during voxel boundary extraction.
    mesh.blob([x,h*.49,z],[h*.34,h*.035,h*.34],'glow')
    for i in range(7):
        a=i*math.tau/7; r=h*.24
        mesh.blob([x+math.cos(a)*r,h*.78,z+math.sin(a)*r],[h*.045,h*.012,h*.045],'pale')

def prop(mesh,index):
    sculpt=VoxelSculpt()
    sculpt_prop(sculpt,index)
    sculpt.emit(mesh)


def sculpt_prop(mesh,index):
    if index in (12,19):
        for x,z,h in ([(.5,.5,.96)] if index==19 else [(.3,.35,.49),(.65,.62,.65),(.72,.25,.34)]):mushroom(mesh,x,z,h)
    elif index==13:
        for i in range(9):
            a=i*math.tau/9
            low=[.5,.08,.5];high=[.5+math.cos(a)*.38,.35+(i%3)*.13,.5+math.sin(a)*.38]
            mesh.branch(low,high,.033,'wood',end_radius=.017)
            for j in (1,2):
                c=[low[k]+(high[k]-low[k])*j/3 for k in range(3)]
                mesh.branch(c,[c[0]+math.cos(a+.8)*.10,c[1]+.15,c[2]+math.sin(a+.8)*.10],.035,'petal',end_radius=.001)
            mesh.blob([high[0],high[1]-.03,high[2]],[.038]*3,'crystal')
    elif index==14:
        mesh.branch([.5,.02,.5],[.5,.60,.5],.025,'stem')
        for i in range(7):
            a=i*math.tau/7
            mesh.blob([.5+math.cos(a)*.13,.65,.5+math.sin(a)*.13],[.12,.07,.10],'petal')
        mesh.blob([.5,.67,.5],[.07,.04,.07],'glow')
    elif index==15:
        for i in range(5):
            x=.15+i*.16;end=.08+(i%3)*.09
            mesh.branch([x,.97,.55],[x+.035,end,.48],.012,'wood')
            for j in range(5):
                y=.86-j*.145
                if y>end:mesh.blob([x+(.035 if j%2 else -.035),y,.5],[.05,.095,.035],'leaf')
    elif index==16:
        for i in range(7):
            a=i*math.tau/7
            mesh.branch([.5,.55,.5],[.5+math.cos(a)*.43,.025,.5+math.sin(a)*.43],.085,'wood',end_radius=.025)
    elif index==17:
        mesh.box([.375,.06,.39],[.585,.22,.55],'bone')
        mesh.box([.405,.025,.365],[.555,.09,.49],'ivory')
        for x in (.40,.50):mesh.box([x,.125,.365],[x+.065,.19,.40],'shadow')
        mesh.box([.46,.07,.35],[.49,.115,.39],'shadow')
        for i in range(4):mesh.branch([.18+i*.17,.06,.2],[.21+i*.17,.15,.8],.025,'bone')
    elif index==18:
        for i in range(7):
            a=i*math.tau/7
            tip=[.5+math.cos(a)*.35,.35+(i%2)*.2,.5+math.sin(a)*.35]
            mesh.branch([.5,.025,.5],tip,.012,'stem')
            for j in range(1,5):
                t=j/5;c=[.5+(tip[0]-.5)*t,tip[1]*t,.5+(tip[2]-.5)*t]
                mesh.blob(c,[.07,.02,.045],'leaf')
    elif index==20:
        for x in (.12,.88):mesh.box([x-.09,.02,.34],[x+.09,.61,.66],'stone',.012)
        mesh.arch([.5,.53,.5],.28,.40,.30,'stone')
        for x,z in ((.1,.7),(.8,.22)):mesh.box([x,.0,z],[x+.14,.12,z+.16],'trim',.008)
    elif index==21:
        mesh.box([.2,.0,.2],[.8,.12,.8],'stone',.014)
        mesh.box([.3,.12,.3],[.7,.72,.7],'trim',.014)
        for x,z,h in ((.25,.28,.42),(.7,.65,.29),(.5,.47,.85)):crystal(mesh,x,.10,z,h)

def main():
    import argparse
    parser=argparse.ArgumentParser()
    parser.add_argument('--prop-models-only', action='store_true',
                        help='Rebuild only the ten voxel prop GLBs; preserve JSON and PNG files')
    parser.add_argument('--textures-only', action='store_true',
                        help='Rebuild cube textures and inward surface styles; preserve all GLBs/props')
    args=parser.parse_args()
    if args.textures_only:
        cube_art_only()
        from biome_surfaces.foliage import refresh
        refresh({406})
        print('Rebuilt Blood cube art and 512px side/top foliage; all GLBs and prop definitions unchanged.')
        return
    for index,(slug,name) in enumerate(NAMES):
        if args.prop_models_only and index<12:continue
        material=index+400
        if not args.prop_models_only:texture(slug,index)
        mesh=Mesh('blood_forest')
        for key,color in PALETTE.items():mesh.materials[key]=(color,None,0,.82)
        if index<12:
            mesh.materials[slug]=([1,1,1],slug+'.png',0,.83)
            mesh.box([0,0,0],[1,1,1],slug)
        else:prop(mesh,index)
        textures=export(mesh,slug,('glow','crystal'))
        if args.prop_models_only:continue
        d=dict(schema_version=1,id=material,name=name,description=f'{name} from the crimson woodland.',creative_category='terrain' if index<12 else 'nature',hardness=.35 if index==6 or index>=12 else 1.8,resistance=4,required_tool=None,mining_level=0)
        if index<12:d.update(render_kind='cube',bevel_width=0,materials={slug:dict(base_color=[1,1,1,1])},textures={slug:f'textures/{slug}.png'},face_materials=[slug]*6)
        else:d.update(render_kind='custom_mesh' ,model_layout='scene',model=f'models/{slug}.glb',textures=textures,struct_block=True,collision_boxes=mesh.collision)
        if material in SURFACES:d['surface']=SURFACES[material]
        if material in (400,407,408):
            side=slug+'_side';bottom=slug+'_bottom'
            texture(side,index,side=True);texture(bottom,1)
            for key in (side,bottom):
                d['textures'][key]=f'textures/{key}.png';d['materials'][key]=dict(base_color=[1,1,1,1])
            d['face_materials']=[side,side,bottom,slug,side,side]
        if material==405:
            end=slug+'_end';texture(end,index,end=True)
            d['textures'][end]=f'textures/{end}.png';d['materials'][end]=dict(base_color=[1,1,1,1])
            d['face_materials']=[slug,slug,end,end,slug,slug]
        if material in (409,411,414,421):d['light']=dict(color=[1,.10,.06],intensity=140 if material!=421 else 260,range=7,falloff='soft_core',flicker=.02,cave_intensity=.7,shadows=False)
        (BLOCKS/f'{slug}.json').write_text(json.dumps(d,indent=2)+'\n')
    if not args.prop_models_only:
        from biome_surfaces.foliage import refresh
        refresh({406})
    print('Rebuilt ten voxel prop GLBs; definitions and textures unchanged.' if args.prop_models_only
          else 'Generated 22 independent Blood Forest blocks with individual JSON, GLB and PNG assets.')
if __name__=='__main__':main()
