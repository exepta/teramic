"""Paint the growth bands and facet planes shared by raw and cut highland quartz."""
from pathlib import Path
import math
import random
import numpy as np
from PIL import Image, ImageDraw

from build_castle import BLOCKS, Mesh
from build_spirit_blossom import export
from build_salt_cave import save_json
from cemetery.geometry import cylinder
from voxel_props.surface import voxelize

ROOT = Path(__file__).resolve().parents[2]
DIR = 'crystal_highlands/'
LIGHT = [.76, .36, 1.0]
GEMS = ['body', 'cleavage', 'inclusion']
COLORS = {'gold': [.78,.43,.11], 'gilt': [.98,.78,.33], 'bronze': [.33,.18,.12],
          'slate': [.20,.19,.29], 'pearl': [.82,.77,.86], 'grip': [.26,.14,.083],
          'indigo': [.33,.13,.78], 'spark': [.83,.64,1.], 'ice_chip': [.23,.84,.99]}


def textures():
    # One continuous tile per several metres of prism. Intersecting growth
    # zones, coarse planar inclusions and rare pale needles remain legible at mip 3.
    n=256
    y,x=np.mgrid[0:n,0:n]/n
    for blue in (False,True):
        palette=np.array([[26,67,145],[41,142,205],[104,215,246],[216,246,255]]
                         if blue else [[68,42,134],[134,88,195],[198,156,238],[252,225,255]])
        # Straight cleavage planes and discrete trapezoidal growth zones.
        # Aperiodic in the vertical direction within the material tile.
        ridge=1.-abs(2*x-1.)
        wedge=1.-abs(2*((y+.15*ridge)%1.)-1.)
        light=.48+.13*ridge+.14*wedge
        band=(y+.09*ridge)%1.
        light+=.052*(band>.27)-.072*(band>.59)+.10*(band>.85)
        stop=np.clip(light,0,.999)*3
        base=(palette[np.int32(stop)]*(1-(stop%1)[...,None])+palette[np.int32(stop)+1]*(stop%1)[...,None])
        img=Image.fromarray(np.uint8(base))
        draw=ImageDraw.Draw(img,'RGBA')
        rng=random.Random(914 if blue else 928)
        for i in range(48):
            px,py=rng.randrange(n),rng.randrange(n)
            w,h=rng.randint(3,19),rng.randint(5,32)
            c=tuple(palette[(3 if i%4 else 0)].tolist())+(rng.randint(30,86),)
            pts=[(px,py),(px+w,py-h//2),(px+w//3,py+h)]
            for dx in (-n,0,n):
                for dy in (-n,0,n):
                    draw.polygon([(u+dx,v+dy) for u,v in pts],fill=c)
        # Angular inclusions and intermittent bright channels within the quartz.
        for _ in range(21):
            px,py=rng.randrange(12,230),rng.randrange(16,220)
            span=rng.randint(8,31)
            poly=[(px,py),(px+span,py-5),(px+span-7,py+5),(px+3,py+10)]
            draw.polygon(poly,fill=tuple(palette[3])+(rng.randint(16,43),))
            draw.line(poly[:2],fill=tuple(palette[3])+(rng.randint(50,125),),width=1)
        for _ in range(28):
            px,py=rng.randrange(6,250),rng.randrange(8,248)
            draw.line((px-2,py+1,px+2,py-2),fill=tuple(palette[3])+(151,),width=1)
        name='aquamarine_growth' if blue else 'amethyst_growth'
        img.save(BLOCKS/'textures'/DIR/(name+'.png'),optimize=True)


def palette(mesh, blue=False):
    tex=DIR+('aquamarine_growth.png' if blue else 'amethyst_growth.png')
    for key,factor,roughness in [('body',[.93,.97,1.],.21),('cleavage',[.77,.83,.98],.26),('inclusion',[1.,1.,1.],.15)]:
        mesh.materials[key]=(factor,tex,.13,roughness)
        mesh.materials['aqua_'+key]=(factor,DIR+'aquamarine_growth.png',.13,roughness)
    for key,c in COLORS.items():
        metal=key in ('gold','gilt','bronze')
        mesh.materials[key]=(c,None,.59 if metal else .07,.31 if metal else .51)


def skin(mesh,p,q,material, gleam=True):
    for i,a in enumerate(p):
        j=(i+1)%len(p)
        b,d=q[i],p[j]
        plane=[a,b,q[j],d]
        mesh.polygon(plane,material[i%len(material)])
        # Trapezoidal polished edges retain a bright silhouette between adjacent faces.
        if gleam and i%2==0:
            def mix(p,q,t):
                return [p[k]+(q[k]-p[k])*t for k in range(3)]
            rim=[mix(a,d,.01),mix(b,q[j],.011),mix(b,q[j],.033),mix(a,d,.041)]
            n=[sum(pt[k]-.5 for pt in plane)/len(plane) if k!=1 else 0. for k in range(3)]
            mesh.polygon([[pt[k]+n[k]*.003 for k in range(3)] for pt in rim], 'spark')


def ring(x,y,z,r,angle=0.,n=6):
    return [[x+r*math.cos(i*math.tau/n+angle),y,z+r*math.sin(i*math.tau/n+angle)] for i in range(n)]


def gem(mesh,x,y,z,r,h,dx=0.,dz=0.,angle=.2,blue=False):
    profile=[(.015,r*.24),(.20,r*.92),(.65,r),(.77,r*.88),(.99,.0015)]
    rings=[ring(x+dx*t,y+t*h,z+dz*t,s,angle) for t,s in profile]
    for lo,hi in zip(rings,rings[1:]):
        skin(mesh,lo,hi, ['aqua_body','aqua_inclusion','aqua_body','aqua_cleavage'] if blue else GEMS+['body','inclusion','cleavage'])
    mesh.polygon(rings[0], 'cleavage',[0,-1,0])


def band(mesh,y,r,m='gilt',thickness=.028,n=12):
    pts=ring(.5,y,.5,r,.13,n)
    for i,p in enumerate(pts):
        cylinder(mesh,p,pts[(i+1)%n],thickness,m,5)


def disc(mesh,y,r,h,mat,sides=8):
    cylinder(mesh,[.5,y,.5],[.5,y+h,.5],r,mat,sides)


def slab(mesh,y,r,h,mat):
    mesh.box([.5-r,y,.5-r],[.5+r,y+h,.5+r],mat,.015)


def artifact(mesh,part):
    # The reliquary consists of three independently mineable carvings. Its
    # golden hoops and cage run outside the jewel, staying within the cell.
    if part=='foot':
        slab(mesh,0.,.485,.13,'slate')
        slab(mesh,.13,.44,.07,'gilt')
        disc(mesh,.2,.40,.12,'pearl')
        disc(mesh,.32,.35,.07,'gold')
        disc(mesh,.39,.30,.035,'inclusion')
        for i in range(4):
            a=math.pi/4+i*math.pi/2
            x=.5+.40*math.cos(a)
            z=.5+.40*math.sin(a)
            mesh.box([x-.055,.19,z-.055],[x+.055,.47,z+.055],'gold')
            gem(mesh,x,.47,z,.046,.15)
        profile=[(.44,.016),(.61,.105),(.82,.25),(1.,.32)]
    elif part=='heart':
        profile=[(0.,.32),(.26,.35),(.70,.35),(1.,.32)]
        band(mesh,.62,.448,'gilt',.022,24)
        for i in range(4):
            a=math.pi/4+i*math.pi/2
            gem(mesh,.5+.445*math.cos(a),.50,.5+.445*math.sin(a),.037,.25)
    else:
        profile=[(0.,.32),(.22,.32),(.38,.28),(.83,.009)]
        band(mesh,.52,.434,'gold',.025,20)
        for i in range(6):
            a=i*math.tau/6+.13
            x=.5+.433*math.cos(a)
            z=.5+.433*math.sin(a)
            gem(mesh,x,.48,z,.032,.14)
    levels=[ring(.5,y,.5,r,.23) for y,r in profile]
    for lo,hi in zip(levels,levels[1:]):
        skin(mesh,lo,hi,GEMS+['inclusion','body','cleavage'])
        for i in range(6):
            # An optical core flashes through narrow planes on each cut face.
            # The irregular kite catches more light near the lower termination.
            j=(i+1)%6
            def facepoint(u,v):
                a=[lo[i][k]+u*(lo[j][k]-lo[i][k]) for k in range(3)]
                b=[hi[i][k]+u*(hi[j][k]-hi[i][k]) for k in range(3)]
                q=[a[k]+v*(b[k]-a[k]) for k in range(3)]
                return [.5+(q[0]-.5)*1.009, q[1], .5+(q[2]-.5)*1.009]
            mesh.polygon([facepoint(.16,.04),facepoint(.25,.96),
                          facepoint(.38,.80),facepoint(.31,.28)], 'spark')
    mesh.polygon(levels[0],'inclusion',[0,-1,0])
    mesh.polygon(levels[-1],'spark',[0,1,0])


def pillar(mesh,part):
    if part=='foot':
        slab(mesh,0.,.445,.12,'slate')
        slab(mesh,.12,.40,.08,'gilt')
        slab(mesh,.20,.37,.10,'pearl')
        disc(mesh,.30,.34,.06,'gold')
        levels=[(.35,.27),(1.,.325)]
    elif part=='tip':
        levels=[(0.,.325),(.61,.31),(.77,.25)]
        disc(mesh,.70,.355,.08,'gold')
        slab(mesh,.78,.357,.13,'pearl')
        slab(mesh,.91,.32,.06,'gilt')
    else:
        levels=[(0.,.325),(1.,.325)]
    rings=[ring(.5,y,.5,r,.22) for y,r in levels]
    for lo,hi in zip(rings,rings[1:]):
        skin(mesh,lo,hi,GEMS)
    mesh.polygon(rings[0],'inclusion',[0,-1,0])
    mesh.polygon(rings[-1],'inclusion',[0,1,0])
    # Engraved growing-branch glyphs appear on four sides of each pillar.
    for side in range(4):
        a=side*math.pi/2
        ux,uz=math.cos(a),math.sin(a)
        def pt(t,y):
            return [.5+ux*.336-uz*t,y,.5+uz*.336+ux*t]
        start=.45 if part=='foot' else .15
        end=.64 if part=='tip' else .83
        for t,level in [(-.095,.65),(.091,.80)]:
            cylinder(mesh,pt(0,start+(end-start)*.35),pt(t,start+(end-start)*level),.015,'spark',5)
        cylinder(mesh,pt(0,start),pt(0,end),.018,'spark',5)


def votive(mesh):
    slab(mesh,0,.43,.12,'slate')
    disc(mesh,.12,.41,.065,'gold')
    for i,(x,z,r,h,dx,dz) in enumerate([
            (.49,.53,.149,.79,-.04,.015),(.35,.54,.116,.51,-.18,.04),
            (.63,.54,.12,.62,.16,.03),(.49,.39,.084,.37,.02,-.15),(.61,.41,.076,.28,.14,-.10)]):
        gem(mesh,x,.17,z,r,h,dx,dz,i*.65)
    for i in range(7):
        a=i*math.tau/7+.23
        x=.5+.38*math.cos(a)
        z=.5+.38*math.sin(a)
        mesh.box([x-.042,.12,z-.042],[x+.042,.21+.069*(i%3),z+.042],'gilt')


def shards(mesh):
    for i,(x,z,r,h,dx,dz) in enumerate([(.28,.58,.075,.13,.20,-.13),(.58,.44,.075,.21,.20,.08),(.42,.29,.051,.12,-.1,.02),(.64,.67,.04,.11,-.08,0.)]):
        gem(mesh,x,.01,z,r,h,dx,dz,i*.82,blue=i==1)


def torch(mesh):
    cylinder(mesh,[.5,.012,.5],[.5,.54,.5],.053,'grip',8,end_radius=.061)
    for y in [.08,.36,.49]:
        disc(mesh,y,.066,.034,'bronze')
    disc(mesh,.51,.14,.075,'gold')
    for i in range(5):
        a=i*math.tau/5
        cylinder(mesh,[.5+.1*math.cos(a),.55,.5+.1*math.sin(a)], [.5+.14*math.cos(a),.70,.5+.14*math.sin(a)],.021,'gilt',5)
    gem(mesh,.5,.54,.5,.108,.38,.015,-.008)
    gem(mesh,.43,.57,.52,.05,.19,-.050,.037)
    gem(mesh,.57,.57,.51,.046,.17,.05,-.025)


def light(intensity,r, color=LIGHT):
    return dict(color=color,intensity=intensity,range=r,falloff='soft_core',flicker=0.,shadows=False)


def export_gem(mesh,slug,blue=False,raw_growth=False):
    voxelize(mesh, resolution=16 if raw_growth else 24)
    tex=export(mesh,slug)
    # Emission derives from the painted zones. A flat factor without a map
    # would erase all the growth bands as soon as daylight falls.
    import struct,json
    path=BLOCKS/'models'/(slug+'.glb')
    b=path.read_bytes()
    sz=struct.unpack_from('<I',b,12)[0]
    obj=json.loads(b[20:20+sz])
    binary=b[20+sz:]
    shade=[.15,.55,.75] if blue else [.39,.16,.60]
    sacred=slug.startswith(('altar_crystal','temple_crystal','amethyst_prism','violet_torch'))
    mask = None
    if raw_growth and any(m['name'].removeprefix('aqua_') in GEMS for m in obj['materials']):
        mask = {'index': len(obj['textures'])}
        obj['textures'].append({'source': len(obj['images'])})
        obj['images'].append({'uri': '../textures/crystal_highlands/raw_growth_glow.png'})
    for m in obj['materials']:
        k=m['name']
        kind=k.removeprefix('aqua_')
        if kind in GEMS:
            kf={'body':.63,'cleavage':.38,'inclusion':.94}[kind]
            hue=[.15,.55,.75] if k.startswith('aqua_') else shade
            m['emissiveTexture']=mask or m['pbrMetallicRoughness']['baseColorTexture']
            m['emissiveFactor']=[v*kf*(.62 if raw_growth else 1.) for v in hue]
        elif k=='spark':
            m['emissiveFactor']=[.64,.37,.88]
        elif k=='ice_chip':
            m['emissiveFactor']=[.075,.39,.61]
        elif sacred and k in ('gold','gilt'):
            m['emissiveFactor']=[.72,.41,.10] if k=='gilt' else [.37,.15,.034]
        if 'emissiveFactor' in m and sacred:
            strength=4.5 if k in GEMS else 2.6 if k=='spark' else 1.35
            m['extensions']={'KHR_materials_emissive_strength':{'emissiveStrength':strength}}
            obj['extensionsUsed']=['KHR_materials_emissive_strength']
    data=json.dumps(obj,separators=(',',':')).encode()
    data+=b' '*(-len(data)%4)
    path.write_bytes(struct.pack('<III',0x46546c67,2,20+len(data)+len(binary))+struct.pack('<I4s',len(data),b'JSON')+data+binary)
    # Bind the same authored maps to the mipmapped near and distant materials.
    return tex


def build():
    import json
    textures()
    blocks=[('violet_torch','Violet Torch','Violette Fackel',torch),
        ('highland_crystal_shards','Highland Crystal Shards','Hochland-Kristallsplitter',shards),
        ('altar_crystal_foot','Altar Crystal Pedestal','Altarkristallsockel',lambda m:artifact(m,'foot')),
        ('altar_crystal_heart','Altar Crystal Heart','Altarkristallherz',lambda m:artifact(m,'heart')),
        ('altar_crystal_crown','Altar Crystal Crown','Altarkristallkrone',lambda m:artifact(m,'crown')),
        ('temple_crystal_cluster','Temple Crystal Cluster','Tempelkristallgruppe',votive)]
    desc=[('A bound quartz flame casts violet light on the floor or wall.','Eine gebundene Quarzflamme wirft violettes Licht auf Boden oder Wand.'),
          ('Gathered mineral shards can be bound to a stick to make a violet torch.','Die gesammelten Mineralsplitter ergeben mit einem Stock eine violette Fackel.')]
    loc=[]
    for i,(slug,en,de,fun) in enumerate(blocks,791):
        m=Mesh('stone')
        palette(m)
        fun(m)
        tex=export_gem(m,slug)
        boxes=[dict(minimum=[.08,0,.08],maximum=[.92,.93,.92])]
        if i==791:
            boxes=json.loads((BLOCKS/'torch.json').read_text())['collision_boxes']
        if i==792:
            boxes=[]
        d,t=desc[i-791] if i<793 else (en,de)
        b=dict(schema_version=1,id=i,name=en,description=d,creative_category='decoration' if i!=792 else 'nature',render_kind='custom_mesh',model_layout='scene',model='models/'+slug+'.glb',struct_block=True,hardness=.15 if i==791 else .85,resistance=2.,mining_level=0,required_tool=None,textures=tex,collision_boxes=boxes)
        if i in (791,794,796):
            b['light']=light(*{791:(1900,24.),794:(3500,19.),796:(1850,14.)}[i])
        if i==791:
            b['light']['shadows']=True
        elif 'light' in b:
            b['light']['offset']=[.5,.72,.48]
        save_json(BLOCKS/(slug+'.json'),b)
        loc.append(dict(source=en,en=en,de=de))
        if en!=d:
            loc.append(dict(source=d,en=d,de=t))
    save_json(ROOT/'src-ui/locales/source/sacred-quartz.json',loc)
    save_json(ROOT/'assets/crafting/building/lighting/violet_torch.json',dict(id='teramic::violet_torch',name='Violet Torch',station='hand',output=dict(item={'block':791},count=1),pattern=dict(kind='shapeless',ingredients=[dict(item='stick',count=1),dict(item={'block':792},count=1)])))


if __name__=='__main__':
    build()
