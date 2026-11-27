"""Author the Cherry Grove's petal stone, meadow, flower sprays and garden furniture."""
import math
import random

import numpy as np
from PIL import Image, ImageDraw

from build_castle import BLOCKS, ROOT, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid


DATA = [
    ("petal_turf", "Petal Meadow Turf", "Blütenwiesen-Grasnarbe", "Cherry petals are caught in the sunlit grass.", "Kirschblütenblätter liegen im sonnenwarmen Gras."),
    ("blossom_stone", "Blossom Stone", "Blütenfels", "Warm grey stone split by seams of rose quartz.", "Warmer grauer Fels mit Adern aus Rosenquarz."),
    ("mossy_blossom_stone", "Mossy Blossom Stone", "Bemooster Blütenfels", "Soft green moss grows between the blocks of blossom stone.", "Weiches grünes Moos wächst in den Klüften des Blütenfelsens."),
    ("sakura_bellflowers", "Sakura Bellflowers", "Sakura-Glockenblumen", "Nodding bells on slender green stalks.", "Nickende Blütenglocken auf schlanken grünen Stängeln."),
    ("sakura_flower_tuft", "Orchard Anemones", "Obstgarten-Anemonen", "A spray of ivory flowers with golden hearts.", "Ein Büschel elfenbeinfarbener Blüten mit goldenen Herzen."),
    ("fallen_cherry_petals", "Fallen Cherry Petals", "Gefallene Kirschblüten", "A dusting of individual rose and pearl petals.", "Ein Hauch von rosa und perlmuttweißen Blütenblättern."),
    ("sakura_fox_statue", "Garden Fox Statue", "Gartenfuchs-Statue", "A little stone guardian wears a rose silk scarf.", "Ein steinerner Fuchswächter trägt ein rosa Seidentuch."),
    ("sakura_garden_lantern", "Sakura Garden Lantern", "Sakura-Gartenlaterne", "A swept cherry roof shelters warm amber light.", "Ein geschwungenes Kirschdach schützt den warmen Bernsteinschein."),
    ("sakura_silk_banner", "Cherry Blossom Banner", "Kirschblütenbanner", "A notched silk banner embroidered with a cherry flower.", "Ein geschlitztes Seidenbanner mit gestickter Kirschblüte."),
    ("sakura_petal_shrub", "Flowering Sakura Shrub", "Blühender Sakura-Strauch", "Carmine and pearl blossoms peek out of the glossy leaves.", "Karminrote und perlmuttweiße Blüten ragen aus glänzendem Laub."),
]
PIGMENT = {
    'basalt': [.34,.29,.32], 'stone': [.55,.51,.52], 'edge': [.69,.65,.63],
    'moss': [.42,.56,.19], 'leaf': [.27,.48,.12], 'fern': [.54,.70,.21],
    'gold': [.95,.62,.21], 'wood': [.39,.18,.18], 'rose': [.84,.30,.47],
    'silk': [.98,.50,.63], 'pearl': [1,.88,.79], 'lamp': [1,.73,.33],
}


def surface(slug, face):
    # Wrapped nearest-cell stone gives continuous courses on adjacent blocks.
    n = 128
    rng = np.random.default_rng(7819 if face == 'side' else 7821)
    y, x = np.mgrid[:n, :n]
    shade = rng.normal(0, 3, (n, n))
    wave = 4*np.sin(2*math.pi*(x+y)/n) + 3*np.cos(4*math.pi*y/n)
    if slug == 'petal_turf':
        turf = np.full((n,n,3), [134,149,89], dtype=float)
        if face == 'bottom':
            turf[:] = [123,83,58]
        elif face == 'side':
            # Cube UV.v follows local world Y, so the turf cap is on the last rows.
            edge = 100 + 5*np.sin(4*math.pi*x/n) + 3*np.cos(10*math.pi*x/n)
            turf[y < edge] = [136,94,61]
            turf[(y >= edge-4) & (y < edge)] = [90,88,43]
        rgb = np.clip(turf+(shade+wave)[:,:,None],0,255).astype('uint8')
        im = Image.fromarray(rgb)
        draw = ImageDraw.Draw(im)
        for _ in range(232 if face=='top' else 37):
            px,py = rng.integers(2,125,2)
            if face=='bottom' or (face=='side' and py < 114): continue
            if rng.random()<.27:
                draw.polygon([(px-2,py),(px,py-1),(px+2,py),(px+1,py+2)],fill=(250,176,193))
                draw.point((px+1,py),fill=(255,216,209))
            else:
                draw.line([(px,py),(px+1,py-2)],fill=(165,174,111))
    else:
        points = rng.uniform(.18,.82,(4,4,2))
        distances = []
        for row,col in np.ndindex(4,4):
            cx,cy = (np.array([col,row])+points[row,col])*32
            dx = (x-cx+64)%128-64
            dy = (y-cy+64)%128-64
            distances.append((dx+2*np.sin(y*np.pi/16))**2+(dy*1.13)**2)
        distances=np.array(distances)
        nearest=distances.argmin(axis=0)
        pair=np.sqrt(np.partition(distances,1,axis=0)[:2])
        seam=pair[1]-pair[0]
        tint=np.array([[160,146,155],[167,153,162],[148,143,153],[176,159,163]])[nearest%4].astype(float)
        shade+=np.where(seam<1.4,-52,np.where(seam<3.6,-28,np.where(seam<5,10,0)))
        moss=np.sin(2*np.pi*(x+y)/n)+np.sin(4*np.pi*x/n+np.cos(2*np.pi*y/n))
        if slug=='mossy_blossom_stone':
            tint[(moss+0.4>seam/8)&(moss>-.3)]=[109,136,62]
        tint[(seam<1.4)&(moss<-.64)] = [229,169,184]
        im=Image.fromarray(np.clip(tint+(shade+wave)[:,:,None],0,255).astype('uint8'))
    path=f'textures/sakura/{slug}_{face}.png'
    im.resize((256,256),Image.Resampling.NEAREST).save(BLOCKS/path,optimize=True)
    return path


def flower(mesh, p, r, color, facing=False):
    x,y,z = p
    for i in range(5):
        a = math.tau*i/5
        dx,dy = r*math.sin(a)*.56, r*math.cos(a)*.56
        if facing:
            ellipsoid(mesh,[x+dx,y+dy,z],[r*.43,r*.45,r*.15],color,6,3)
        else:
            ellipsoid(mesh,[x+dx,y+r*.07,z+dy],[r*.48,r*.23,r*.48],color,6,3)
    ellipsoid(mesh,[x,y+(0 if facing else r*.18),z+(r*.17 if facing else 0)],[r*.23]*3,'gold',6,3)


def prop(slug):
    m=Mesh('sakura')
    for key,color in PIGMENT.items(): m.materials[key]=(color,None,0,.84)
    rng=random.Random(7822)
    def b(x0,y0,z0,x1,y1,z1,color,solid=False):
        m.box([x0,y0,z0],[x1,y1,z1],color,.008,solid)
    def e(center,radius,color):
        ellipsoid(m,center,radius,color,7,4)
    if slug in ('sakura_bellflowers','sakura_flower_tuft','sakura_petal_shrub'):
        shrub=slug.endswith('shrub')
        for i in range(11 if shrub else 7):
            a=i*2.39996
            x,z=.5+math.cos(a)*(.12+.2*rng.random()),.5+math.sin(a)*(.16+.15*rng.random())
            h=rng.uniform(.25,.47) if shrub else rng.uniform(.27,.65)
            if shrub:
                e([x,h*.61,z],[.15,.16,.15], 'fern' if i%3==0 else 'leaf')
            mid=[x,h*.77,z]
            cylinder(m,[.5+rng.uniform(-.06,.06),.001,.5],mid,.014,'leaf',5,end_radius=.009)
            e([x+.03,h*.62,z-.04],[.063,.028,.043],'fern')
            if slug=='sakura_bellflowers':
                end=[x+.054,h-.06,z+.03]
                cylinder(m,mid,[x,h+.06,z],.01,'leaf',5,end_radius=.007)
                cylinder(m,[x,h+.06,z],end,.025,'rose',7,end_radius=.058)
                for j in range(5):
                    theta=math.tau*j/5
                    e([end[0]+.039*math.sin(theta),end[1],end[2]+.039*math.cos(theta)],[.022,.035,.022],'silk')
            else:
                cylinder(m,mid,[x,h,z],.01,'leaf',5,end_radius=.008)
                flower(m,[x,h,z],.065 if shrub else .088,('rose' if i%3==0 else 'silk') if shrub else 'pearl')
    elif slug=='fallen_cherry_petals':
        for i in range(20):
            x,z=rng.uniform(.06,.94),rng.uniform(.06,.94)
            e([x,.011,z],[.023+rng.random()*.015,.009,.018],'pearl' if i%4==0 else 'silk')
    elif slug=='sakura_fox_statue':
        b(.16,0,.16,.84,.08,.83,'basalt',True)
        b(.19,.08,.19,.81,.16,.80,'moss',True)
        e([.48,.41,.45],[.21,.28,.19],'stone')
        e([.65,.37,.33],[.13,.24,.13],'edge')
        e([.69,.55,.30],[.087,.13,.085],'pearl')
        for x in (.38,.57): b(x-.043,.15,.51,x+.043,.43,.63,'edge',True)
        e([.47,.61,.47],[.16,.06,.15],'rose')
        b(.48,.42,.594,.56,.64,.65,'silk')
        e([.47,.73,.49],[.18,.145,.15],'edge')
        e([.47,.68,.63],[.105,.073,.085],'stone')
        e([.47,.70,.70],[.038,.024,.019],'basalt')
        for x in (.35,.59):
            cylinder(m,[x,.80,.47],[x + (-.026 if x<.5 else .026),.98,.43],.078,'stone',4,end_radius=.005)
            b(x+.016 if x<.5 else x-.055,.743,.607,x+.050 if x<.5 else x-.021,.756,.626,'basalt')
        m.collision.append(dict(minimum=[.28,.16,.29],maximum=[.67,.84,.63]))
    elif slug=='sakura_garden_lantern':
        b(.20,0,.20,.80,.08,.80,'basalt',True)
        b(.25,.08,.25,.75,.16,.75,'moss',True)
        b(.38,.16,.38,.62,.42,.62,'stone',True)
        b(.22,.42,.22,.78,.49,.78,'edge',True)
        b(.30,.49,.30,.70,.71,.70,'lamp',True)
        for x in (.27,.69):
            for z in (.27,.69): b(x,.49,z,x+.04,.73,z+.04,'wood')
        for z in (.283,.701):
            for x in (.40,.56): b(x,.50,z,x+.025,.70,z+.016,'wood')
        b(.12,.71,.12,.88,.76,.88,'rose',True)
        b(.20,.76,.20,.80,.81,.80,'silk',True)
        b(.29,.81,.29,.71,.87,.71,'rose',True)
        b(.39,.87,.39,.61,.91,.61,'silk')
        e([.5,.945,.5],[.047,.05,.047],'gold')
        for x in (.12,.78):
            for z in (.12,.78): b(x,.76,z,x+.10,.79,z+.10,'rose')
    elif slug=='sakura_silk_banner':
        b(.11,.88,.46,.89,.95,.54,'wood')
        for x in (.22,.65): b(x,.81,.46,x+.11,.91,.55,'gold')
        b(.18,.19,.483,.82,.84,.53,'rose')
        for x in (.18,.62):
            b(x,.11,.483,x+.20,.2,.53,'rose')
            b(x,.09,.484,x+.15,.13,.531,'silk')
        for z in (.476,.54):
            flower(m,[.50,.54,z],.18,'pearl',True)
            for x in (.20,.78): b(x,.20,z,x+.015,.83,z+.01,'silk')
    if slug in ('sakura_bellflowers','sakura_flower_tuft','sakura_petal_shrub','fallen_cherry_petals'):
        foot=min(y for part in m.parts.values() for _,y,_ in part['position'])
        for part in m.parts.values():
            part['position']=[[x,y-foot,z] for x,y,z in part['position']]
    textures=export_voxels(m,slug,['lamp'])
    return textures,m.collision


def build():
    (BLOCKS/'textures/sakura').mkdir(exist_ok=True)
    # Box meshes sample one continuous UV range along the height of the post.
    # Keep the rails rose, with a brass fillet and pearl lacquer on the cap.
    boards=Image.open(BLOCKS/'textures/block_palette/sakura_planks.png').convert('RGB')
    pixels=np.array(boards,dtype=float)
    size=pixels.shape[0]
    tone=pixels.mean(axis=2,keepdims=True)/(pixels.mean()+1e-6)
    for low,high,color in [(.82,.84,[138,83,44]),(.84,.86,[229,185,105]),(.86,.94,[241,199,173]),(.94,1.0,[249,223,195])]:
        rows=slice(round(size*low),round(size*high))
        pixels[rows]=tone[rows]*color
    Image.fromarray(np.clip(pixels,0,255).astype('uint8')).save(BLOCKS/'textures/sakura/cherry_fence_lacquer.png',optimize=True)
    locale=[]
    for ident,(slug,en,de,desc,de_desc) in enumerate(DATA,596):
        block=dict(schema_version=1,id=ident,name=en,description=desc,creative_category='nature',required_tool=None,mining_level=0)
        if ident<=598:
            faces=('side','top','bottom') if ident==596 else ('side','top')
            textures={face:surface(slug,face) for face in faces}
            block.update(render_kind='cube',bevel_width=0.0,hardness=0.6 if ident==596 else 1.7,resistance=2.0,
                         face_materials=['side','side','bottom' if ident==596 else 'top','top','side','side'],textures=textures,
                         materials={f:dict(base_color=[1,1,1,1],roughness=.92,reflectance=.09) for f in faces})
        else:
            textures, collision=prop(slug)
            block.update(render_kind='custom_mesh',struct_block=True,model_layout='scene',model=f'models/{slug}.glb',
                         hardness=1.2 if collision else .25,resistance=2 if collision else .4,textures=textures,collision_boxes=collision)
            if ident in (602,603,604): block['creative_category']='decoration'
            if ident==603: block['light']=dict(color=[1,.69,.35],intensity=730,range=11,falloff='soft_core',flicker=.025,cave_intensity=.6,shadows=False)
        save_json(BLOCKS/f'{slug}.json',block)
        locale += [dict(source=en,en=en,de=de),dict(source=desc,en=desc,de=de_desc)]
    recipes = ROOT/'assets/crafting/biomes/cherry_forest'
    recipes.mkdir(exist_ok=True)
    for ident, ingredients in [(602, [(597,3),(601,1)]), (603, [(597,2),(135,1),(40,1)]), (604, [(127,1),(135,1),(601,1)])]:
        slug,en,*_ = DATA[ident-596]
        save_json(recipes/f'{slug}.json',dict(id=f'teramic::{slug}',name=en,station='workbench',
                  output=dict(item=dict(block=ident),count=1),
                  pattern=dict(kind='shapeless',ingredients=[dict(item=dict(block=i),count=n) for i,n in ingredients])))
    save_json(ROOT/'src-ui/locales/source/sakura-garden.json',locale)


if __name__=='__main__':
    build()
