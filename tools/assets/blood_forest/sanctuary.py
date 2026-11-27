"""Voxel lanterns, memorial props and tileable shrine masonry for the Blood Forest."""
import json
import math

from PIL import Image, ImageDraw
from build_castle import Mesh, BLOCKS
from build_spirit_blossom import export
from biome_surfaces import SIZE, field, grain
from voxel_props.geometry import VoxelSculpt

CATALOG = [
    ('blighted_root_soil', 'Blighted Root Soil', 'Durchwurzelte Bluterde'),
    ('coagulated_mud', 'Coagulated Mud', 'Geronnener Blutschlamm'),
    ('bloodstained_bricks', 'Bloodstained Bricks', 'Blutbefleckte Ziegel'),
    ('carved_blood_stone', 'Carved Blood Stone', 'Gravierter Blutstein'),
    ('torn_blood_banner', 'Torn Blood Banner', 'Zerschlissenes Blutbanner'),
    ('standing_blood_lantern', 'Standing Blood Lantern', 'Stehende Blutlaterne'),
    ('blood_brazier', 'Blood Brazier', 'Blutfeuerschale'),
    ('weathered_ribcage', 'Weathered Ribcage', 'Verwitterter Brustkorb'),
    ('bone_stake', 'Bone Stake', 'Knochenpfahl'),
    ('blood_crystal_growth', 'Blood Crystal Growth', 'Blutkristalldruse'),
    ('hanging_blood_lantern', 'Hanging Blood Lantern', 'Hängende Blutlaterne'),
    ('fallen_blood_branches', 'Fallen Blood Branches', 'Gefallene Blutäste'),
]

DESCRIPTIONS = (
    'Crimson roots wind through damp forest earth.',
    'Burgundy mud with pools of dark sap.',
    'Weathered masonry stained by iron-red runoff.',
    'A spiral seal cut into violet-black shrine stone.',
    'An embroidered oath banner with a ragged hem.',
    'A crimson glass lamp on a squat iron plinth.',
    'Scarlet flames rise from a wrought iron bowl.',
    'Paired ribs curve around an exposed spine.',
    'An old grave marker bound in strips of crimson cloth.',
    'A cluster of ruby spires breaks through the stone.',
    'A ruby lantern hangs from a rusted iron loop.',
    'Broken bloodwood twigs still seep red sap.',
)

PALETTE = {
    'iron': [.12, .08, .13], 'rust': [.38, .19, .19], 'stone': [.29, .23, .29],
    'claret': [.36, .025, .09], 'seam': [.68, .10, .16], 'thread': [.89, .61, .40],
    'glass': [.88, .06, .11], 'heart': [1., .42, .20], 'bone': [.74, .61, .51],
    'ivory': [.93, .82, .66], 'bark': [.24, .115, .12], 'sap': [.52, .08, .11],
}


def mineral_texture(slug, index):
    base = [(119, 77, 74), (97, 39, 45), (171, 142, 146), (191, 166, 164)][index]
    image = Image.new('RGB', (SIZE, SIZE))
    out = []
    for y in range(SIZE):
        for x in range(SIZE):
            macro = field(x, y, 16, 4001 + index)
            scale = (.83 + .29 * macro) * grain(x, y, 4190)
            color = list(base)
            u, v = x / SIZE, y / SIZE
            if index == 0:
                channels = [abs((u * 4 + k + .23 * math.sin((v * 2 + k) * math.tau)
                                 + .07 * math.sin((v * 5 + k) * math.tau)) % 1 - .5)
                            for k in (.06, .66)]
                d = min(channels)
                if d < .07:
                    color = [154, 68, 62] if d < .045 else [62, 36, 41]
            elif index == 1:
                pool = field(x, y, 32, 3993) + .26 * macro
                if pool > .62:
                    color = [98, 22, 40] if pool < .83 else [120, 32, 47]
                if .83 < pool < .842:
                    color = [158, 74, 77]
            elif index == 2:
                row = int(v * 4)
                bx = (u * 2 + .5 * (row % 2)) % 1
                by = (v * 4) % 1
                if min(bx, 1 - bx) < .014 or min(by, 1 - by) < .031:
                    scale *= .52
                elif bx < .043 or by < .075:
                    scale *= 1.16
                if field(x, y, 32, 3982) + .15 * macro > .65:
                    color = [157, 87, 94]
            out.append(tuple(min(255, round(c * scale)) for c in color))
    image.putdata(out)
    if index == 3:
        pen = ImageDraw.Draw(image)
        points = [(32, 36), (218, 36), (218, 205), (65, 205), (65, 72),
                  (183, 72), (183, 171), (101, 171), (101, 106), (149, 106), (149, 138)]
        scale = SIZE / 256
        for off, ink, width in [(2, (220, 195, 174), 10), (0, (95, 74, 87), 7)]:
            pen.line([(round((x + off)*scale), round((y + off)*scale)) for x,y in points],
                     fill=ink, width=max(1, round(width*scale)))
    image.save(BLOCKS / 'textures' / f'{slug}.png')


def lantern(s, hanging):
    y = .23 if hanging else .32
    if hanging:
        s.box([.42,.91,.41],[.58,1.,.60],'iron')
        s.arch([.5,.80,.5],.055,.095,.055,'rust')
        s.branch([.5,.71,.5],[.5,.85,.5],.035,'iron')
    else:
        s.box([.15,0,.15],[.85,.11,.85],'stone',solid=True)
        s.box([.25,.11,.25],[.75,.18,.75],'rust')
        s.box([.42,.16,.42],[.58,.33,.58],'iron')
    for dy,r in [(0,.25),(.042,.19),(.43,.25),(.47,.17),(.51,.10)]:
        s.box([.5-r,y+dy,.5-r],[.5+r,y+dy+.044,.5+r],'iron')
    s.box([.34,y+.07,.34],[.66,y+.43,.66],'glass',solid=not hanging)
    s.box([.425,y+.11,.33],[.58,y+.35,.67],'heart')
    for x in (.30,.66):
        for z in (.30,.66):
            s.box([x,y+.04,z],[x+.045,y+.45,z+.045],'rust')


def sculpt(s, index):
    if index == 4:
        s.box([.05,.87,.45],[.95,.99,.56],'bark')
        for k in range(12):
            x=.14+k*.06
            tip=.13+(.05,.12,.0,.17)[k%4]+.08*abs(k-5)/6
            s.box([x,tip,.47],[x+.065,.91,.52], 'seam' if k in (0,11) else 'claret')
        s.branch([.49,.73,.46],[.49,.31,.46],.024,'thread')
        for side in (-1,1):
            s.branch([.49,.5,.46],[.49+side*.18,.66,.46],.025,'thread')
            s.branch([.49,.51,.46],[.49+side*.11,.39,.46],.018,'thread')
    elif index in (5,10):
        lantern(s, index == 10)
    elif index == 6:
        s.box([.19,0,.19],[.81,.12,.81],'stone',solid=True)
        s.box([.34,.12,.34],[.66,.41,.66],'iron',solid=True)
        for dy,r in [(.37,.24),(.44,.32),(.51,.39)]:
            s.box([.5-r,dy,.5-r],[.5+r,dy+.075,.5+r],'rust' if dy==.44 else 'iron')
        for i in range(7):
            a=i*math.tau/7; x=.5+.19*math.cos(a); z=.5+.19*math.sin(a)
            s.branch([x,.54,z],[x+.05,.73+.13*(i%3),z-.04],.084,'glass',.01)
        s.branch([.5,.55,.5],[.47,.91,.52],.12,'heart',.015)
    elif index == 7:
        s.branch([.5,.08,.06],[.53,.22,.91],.046,'bone')
        for i in range(5):
            z=.13+i*.155; w=.26+.10*math.sin(i*.8)
            for sign in (-1,1):
                tip=[.50+sign*w,.07,z+.075]
                joint=[.50+sign*w*.69,.38+.067*math.sin(i*.66),z+.012]
                s.branch([.51,.17+i*.012,z],joint,.038,'bone')
                s.branch(joint,tip,.041,'ivory',.019)
    elif index == 8:
        s.branch([.46,0,.52],[.54,.95,.46],.05,'bark',.031)
        s.branch([.18,.74,.47],[.82,.82,.46],.052,'bone')
        for x in (.18,.82): s.blob([x,.78,.47],[.067,.07,.075],'ivory')
        for h in (.63,.68,.73): s.box([.43,h,.40],[.59,h+.035,.56],'seam')
        s.box([.60,.35,.45],[.67,.77,.51],'claret')
    elif index == 9:
        s.blob([.5,.08,.5],[.43,.11,.38],'stone')
        for i,(x,z,h,r) in enumerate([(.46,.5,.93,.11),(.29,.6,.60,.09),(.71,.47,.72,.10),
                                       (.61,.72,.48,.09),(.30,.32,.38,.07)]):
            for j in range(8):
                t=j/8; a=(1 if i%2 else -1)*.08*t
                width=r*(1-min(1,max(0,(t-.55)*2)))
                s.box([x+a-width,.06+h*t,z-width],[x+a+width,.06+h*(j+1)/8,z+width],
                      'glass' if j<6 else 'heart')
    elif index == 11:
        for j,(lo,hi) in enumerate([([.06,.05,.17],[.88,.10,.71]),([.43,.08,.44],[.67,.25,.16]),
                                     ([.19,.10,.65],[.78,.19,.35]),([.67,.24,.16],[.75,.37,.2])]):
            s.branch(lo,hi,.045 if j==0 else .032,'bark',.017)
            s.blob(hi,[.024]*3,'sap')


def build():
    for index,(slug,name,_) in enumerate(CATALOG):
        d = dict(schema_version=1,id=492+index,name=name,description=DESCRIPTIONS[index],
                 creative_category='nature' if index>=4 else 'terrain',hardness=1.8 if index<4 else .8,
                 resistance=4,required_tool=None,mining_level=0)
        if index<4:
            mineral_texture(slug,index)
            d.update(render_kind='cube',bevel_width=0,surface=('earth','earth','rock','rock')[index],
                     materials={slug:dict(base_color=[1,1,1,1], roughness=(.93,.32,.84,.76)[index],
                                           reflectance=(.19,.44,.28,.31)[index])},
                     textures={slug:'textures/'+slug+'.png'},
                     face_materials=[slug]*6)
        else:
            mesh=Mesh(slug)
            for key,color in PALETTE.items(): mesh.materials[key]=(color,None,.18 if key=='iron' else 0,.66)
            s=VoxelSculpt()
            sculpt(s,index)
            s.emit(mesh)
            textures=export(mesh,slug,('glass','heart'))
            d.update(render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.glb',
                     textures=textures,struct_block=True,collision_boxes=mesh.collision)
            if index in (5,6,9,10):
                d['light']=dict(color=[1.,.075,.1],intensity=360 if index==6 else 155,
                                range=9 if index==6 else 6,falloff='soft_core',flicker=.025 if index==6 else 0,
                                cave_intensity=.8,shadows=False)
        (BLOCKS/f'{slug}.json').write_text(json.dumps(d,indent=2)+'\n')


if __name__=='__main__':
    build()
