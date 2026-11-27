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
    [174,96,103], [142,86,82], [199,89,124], [132,65,73],
    [132,61,75], [212,63,98], [151,104,118], [238,152,101],
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
    if index in (1,7,8) and not side:
        from PIL import Image, ImageDraw
        from random import Random
        img = Image.new('RGBA', (SIZE,SIZE))
        img.putdata([tuple(pixel) for pixel in pixels])
        pen = ImageDraw.Draw(img)
        rng = Random(5400+index)
        for _ in range(59 if index in (7,8) else 27):
            x, y = rng.randrange(SIZE), rng.randrange(SIZE)
            angle = rng.random()*math.tau
            ux, uy = math.cos(angle), math.sin(angle)
            length, width = rng.uniform(4,14), rng.uniform(2,5)
            ink = rng.choice([(170,43,58), (120,39,55), (104,45,46), (94,42,47), (158,58,61)])
            for tx in (-SIZE,0,SIZE):
                for ty in (-SIZE,0,SIZE):
                    def p(along, across):
                        return (x+tx+ux*along-uy*across, y+ty+uy*along+ux*across)
                    pen.polygon([p(-length,0),p(-length*.28,-width),p(length*.30,-width*.8),
                                 p(length,.6),p(1,width*.6),p(-length*.21,width)],fill=ink+(255,))
                    pen.line([p(-length*.80,0),p(length*.70,0)],fill=(ink[0]+22,ink[1]+9,ink[2]+6,255))
        pixels = list(img.getdata())
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
        # Interleaved arches rise from both feet. Hooked thorns point along the stem.
        for stem, (yaw, height, radius) in enumerate([(0.32,.78,.42), (2.15,.52,.35)]):
            dx, dz = math.cos(yaw), math.sin(yaw)
            previous = None
            for step in range(11):
                angle = step * math.pi / 10
                across = math.cos(angle) * radius
                bend = math.sin(2 * angle + stem) * .067
                point = [.5 + dx*across - dz*bend, .035 + height*math.sin(angle),
                         .5 + dz*across + dx*bend]
                if previous is not None:
                    mesh.branch(previous, point, .051 if stem == 0 else .035, 'wood')
                if 0 < step < 10 and step % 2:
                    tip = [point[0] + dx*.12*math.cos(angle) - dz*.035,
                           point[1] + .11*math.sin(angle), point[2] + dz*.12*math.cos(angle) + dx*.035]
                    mesh.branch(point, tip, .037, 'petal', end_radius=.008)
                    mesh.blob(tip,[.015]*3,'crystal')
                previous = point
    elif index==14:
        for x,z,h,spread in [(.39,.45,.90,.17),(.70,.62,.63,.13)]:
            mesh.branch([.49,.016,.52],[x,h-.04,z],.025,'stem')
            for dx, dz in [(1,0),(-1,0),(0,1),(0,-1)]:
                cx,cz = x + dx*spread*.75, z + dz*spread*.75
                mesh.box([cx-.09,h-.095,cz-.08],[cx+.09,h-.034,cz+.08],'petal')
                mesh.box([cx-.061+dx*.047,h-.046,cz-.054+dz*.047],
                         [cx+.061+dx*.047,h+.008,cz+.054+dz*.047],'petal')
            mesh.box([x-.046,h-.06,z-.046],[x+.05,h+.014,z+.05],'glow')
    elif index==15:
        for i,(x, end) in enumerate([(.18,.16),(.50,.03),(.79,.29)]):
            previous = [x,.99,.56]
            for j in range(1,9):
                t = j / 8
                p = [x + .06*math.sin(i+t*6.2), .99 + t*(end-.99), .52 + .07*math.cos(i+t*4)]
                mesh.branch(previous,p,.021,'wood')
                side = .056 if j%2 else -.056
                mesh.blob([p[0]+side,p[1]-.032,p[2]],[.065,.081,.038],'leaf')
                previous = p
    elif index==16:
        for i in range(6):
            a=i*math.tau/6
            elbow = [.5 + .20*math.cos(a+.48),.37+(i%3)*.053,.5+.20*math.sin(a+.48)]
            ankle = [.5 + .31*math.cos(a-.13),.072,.5+.31*math.sin(a-.13)]
            tip = [.5+.45*math.cos(a),.025,.5+.45*math.sin(a)]
            mesh.branch([.5,.44,.5],elbow,.084,'wood')
            mesh.branch(elbow,ankle,.084,'wood',end_radius=.052)
            mesh.branch(ankle,tip,.052,'wood',end_radius=.022)
            mesh.branch([elbow[0],elbow[1]+.04,elbow[2]],ankle,.019,'petal')
    elif index==17:
        # Long bones crisscross in three narrowing layers below the skull.
        for layer, count in [(0,3),(1,2),(2,1)]:
            for j in range(count):
                offset=(j-(count-1)*.5)*.20
                a=(.04,1.49,.68)[layer]+j*.03
                x=.51-math.sin(a)*offset; z=.51+math.cos(a)*offset
                r=.35-.046*layer; dx=math.cos(a)*r; dz=math.sin(a)*r
                lo=[x-dx,.046+layer*.088,z-dz]; hi=[x+dx,.08+layer*.088,z+dz]
                mesh.branch(lo,hi,.038,'bone')
                for tip in (lo,hi): mesh.blob(tip,[.056,.048,.055],'ivory')
        mesh.box([.35,.26,.4],[.63,.51,.62],'bone')
        mesh.box([.39,.24,.36],[.60,.35,.51],'ivory')
        for x in (.37,.53):mesh.box([x,.38,.374],[x+.08,.46,.415],'shadow')
        mesh.box([.461,.312,.357],[.509,.375,.396],'shadow')
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
        from blood_forest.sanctuary import build as build_sanctuary
        refresh({406})
        build_sanctuary()
    print('Rebuilt ten voxel prop GLBs; definitions and textures unchanged.' if args.prop_models_only
          else 'Generated the 34 Blood Forest blocks and props.')
if __name__=='__main__':main()
