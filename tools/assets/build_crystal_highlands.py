"""Bake the quartz massif's painted face atlas and faceted alpine models."""
import math
from pathlib import Path
import random

import numpy as np
from PIL import Image, ImageDraw
from build_castle import BLOCKS, Mesh
import highland_gems as gems
from build_salt_cave import save_json
from build_spirit_blossom import export
from voxel_props.surface import voxelize
from cemetery.geometry import cylinder, ellipsoid

ROOT = Path(__file__).resolve().parents[2]
MASTER = Path(__file__).with_name("crystal_highlands") / "quartz-geology.jpg"
FOLDER = BLOCKS / 'textures/crystal_highlands'
CATALOG = [
    ('crystal_stone','Crystal Stone','Kristallstein'),
    ('crystal_ore','Crystal Ore','Kristallerz'),
    ('polished_highland_rock','Polished Highland Rock','Polierter Hochlandfels'),
    ('auricite_ore','Auricite Ore','Auriziterz'),
    ('crystal_gravel','Crystal Gravel','Kristallkies'),
    ('highland_cliff_stone','Highland Cliff Stone','Hochland-Klippenstein'),
    ('amethyst_spire','Amethyst Spire','Amethystspitze'),
    ('cyan_spire','Cyan Quartz Spire','Aquamarinspitze'),
    ('crystal_ashlar','Crystal Ashlar','Kristallquader'),
    ('gilded_runestone','Gilded Runestone','Vergoldeter Runenstein'),
    ('violet_roof_slate','Violet Roof Slate','Violetter Dachschiefer'),
    ('crystal_shard_cluster','Crystal Shards','Kristallsplitter'),
    ('crystal_crown','Crystal Crown','Kristallkrone'),
    ('auricite_deposit','Auricite Deposit','Aurizitvorkommen'),
    ('mixed_crystal_vein','Mixed Crystal Vein','Gemischte Kristallader'),
    ('alpine_crystal_bloom','Alpine Crystal Bloom','Alpenkristallblüte'),
    ('highland_tussock','Hardy Highland Grass','Hochlandbüschel'),
    ('frosted_moss','Frosted Moss','Bereiftes Polstermoos'),
    ('highland_scree','Loose Highland Stones','Loser Hochlandschutt'),
    ('glacier_boulder','Glacial Erratic','Gletscherfindling'),
    ('vault_keystone','Vault Keystone','Tresorschlussstein'),
]
DESCRIPTIONS = [
    ("Cloudy quartz forms a pale band inside the massif.", "Milchiger Quarz bildet ein helles Band im Gebirge."),
    ("A seam of raw amethyst cuts through the gneiss.", "Eine Ader aus rohem Amethyst durchzieht den Gneis."),
    ("Dark mica glints on the polished rock face.", "Dunkler Glimmer glänzt auf der polierten Felsfläche."),
    ("Honey-gold auricite lies in a fractured quartz seam.", "Honiggoldener Aurizit sitzt in einer geborstenen Quarzader."),
    ("Glacial silt holds chips of quartz, slate and mica.", "Gletscherschluff bindet Splitter aus Quarz, Schiefer und Glimmer."),
    ("Long joints split this weathered alpine gneiss.", "Lange Klüfte spalten diesen verwitterten Alpengneis."),
    ("A violet quartz prism from a highland crystal crown.", "Ein violettes Quarzprisma aus einer Hochland-Kristallkrone."),
    ("Cold blue crystal grows beside the amethyst dikes.", "Eisblauer Kristall wächst entlang der Amethystgänge."),
    ("Courses of dressed lavender stone line the vault.", "Behauene lavendelfarbene Quader säumen den Tresor."),
    ("Gold wire traces a rune across the dressed stone.", "Golddraht zeichnet eine Rune auf dem behauenen Stein."),
    ("Overlapping violet slates shed snow from the tower roof.", "Überlappende violette Schieferplatten halten Schnee vom Turmdach ab."),
    ("Small quartz points sprout from a common matrix.", "Kleine Quarzspitzen wachsen aus einem gemeinsamen Muttergestein."),
    ("Amethyst and icy blue quartz meet in a radial crown.", "Amethyst und eisblauer Quarz bilden einen strahlenförmigen Kranz."),
    ("Facets of amber-gold auricite emerge from a gray matrix.", "Bernsteingoldene Aurizitfacetten ragen aus grauem Muttergestein."),
    ("Gold and violet minerals occupy the same fissure.", "Goldene und violette Minerale füllen dieselbe Kluft."),
    ("Pink saxifrage opens among wind-sheltered stones.", "Rosa Steinbrech blüht im Windschatten der Felsen."),
    ("Narrow blades bend away from the exposed ridge.", "Schmale Halme neigen sich im Wind des freiliegenden Grats."),
    ("A cushion of silver-tipped moss clings to the scree.", "Ein silbrig bereiftes Moospolster haftet am Geröll."),
    ("Weathered fragments gather at the foot of a cliff.", "Verwitterte Bruchstücke sammeln sich am Fuß der Klippe."),
    ("Glacial ice carried this fractured boulder to the ledge.", "Gletschereis hat diesen zerklüfteten Findling auf den Felsvorsprung getragen."),
    ("An amethyst seal crowns the ancient repository.", "Ein Amethystsiegel krönt die alte Schatzkammer."),
]
COLORS={
    'schist':(.32,.30,.39),'cliff':(.54,.52,.61),'mica':(.70,.68,.76),
    'indigo':(.33,.14,.63),'violet':(.62,.25,.88),'lilac':(.85,.55,.99),'pearl':(.92,.77,1.),
    'azure':(.19,.63,.91),'ice':(.49,.91,.98),'aqua':(.63,.99,1.),
    'umber':(.40,.29,.12),'amber':(.85,.49,.10),'honey':(1.,.76,.31),'glint':(1.,.90,.60),
    'stem':(.23,.32,.21),'leaf':(.41,.52,.26),'sage':(.62,.65,.38),'frost':(.75,.84,.81),
    'magenta':(.68,.17,.53),'heather':(.87,.42,.64),'ivory':(.97,.86,.78),
}

def face(slug, atlas):
    rock, gravel, ore, quartz = atlas
    def tone(texture,target,contrast=.96,chroma=.65):
        values=np.array(texture,dtype=float); light=values @ [.2126,.7152,.0722]
        pigment=values-light[...,None]
        return target + (light-light.mean())[...,None]*contrast+(pigment-pigment.mean((0,1)))*chroma
    if slug=='crystal_stone':
        result=tone(quartz,[166,148,187],.86,.70)
    elif slug in ('crystal_ore','auricite_ore'):
        result=tone(ore,[115,108,123],.93,.88)
        if slug=='auricite_ore':
            a=np.array(ore,dtype=float); intensity=np.clip((a[...,2]-a[...,1]-8)/60.,0,1)[...,None]
            result=result*(1-intensity)+(a.mean(2)[...,None]*.76+[107,62,-12])*intensity
    elif slug=='crystal_gravel':
        result=tone(gravel,[153,145,158],1.04,.84)
    elif slug in ('amethyst_spire','cyan_spire'):
        # Long prisms transmit cooler light near the core. Keep neighbouring
        # voxels continuous rather than painting a border around each meter.
        q=np.array(quartz.resize((256,128))); quartz2=Image.fromarray(np.concatenate((q,np.flip(q,axis=0)),axis=0))
        result=tone(quartz2,[185,137,218] if slug=='amethyst_spire' else [125,187,222],.85,.26)
        stripe=np.arange(256)[None,:]/256*math.tau
        result+=((np.cos(stripe*3+0.7)*.6+np.cos(stripe*5-1.1)*.34)*13)[...,None]
    elif slug in ('highland_cliff_stone','polished_highland_rock'):
        result=tone(rock,([133,126,142] if slug=='highland_cliff_stone' else [94,87,108]),1.12 if slug=='highland_cliff_stone' else .68,.70)
    else:
        color={'crystal_ashlar':[163,145,160],'gilded_runestone':[108,91,121],'violet_roof_slate':[93,70,125],'vault_keystone':[122,96,151]}[slug]
        result=tone(rock,color,.48,.38)
    img=Image.fromarray(np.uint8(np.clip(result,0,255)))
    draw=ImageDraw.Draw(img)
    if slug=='crystal_ore':
        # Cross-cutting aquamarine and lavender crystals in a gneiss matrix.
        # Repeat the diagonal fissures across tile borders, including the faces
        # of the stepped outcrops where the same geological seam surfaces.
        draw=ImageDraw.Draw(img,'RGBA')
        for lane,aquamarine in [(40,True),(164,False)]:
            rim=(48,154,217) if aquamarine else (141,97,208)
            lumen=(169,245,255) if aquamarine else (222,187,255)
            for shift in (-256,0,256):
                line=[(x,round(lane+shift-.34*x+7*math.sin(x*math.tau/128))) for x in range(-8,265,4)]
                draw.line(line,fill=(34,40,64,211),width=12)
                draw.line(line,fill=(*rim,217),width=7)
                draw.line([(x,y-1) for x,y in line],fill=(*lumen,225),width=2)
                for i in (3,9,13,25,36,42,58):
                    x,y=line[i];w=4+i%4;h=7+i%8
                    draw.polygon([(x,y+1),(x+w,y-2),(x+w+2,y-h),(x+2,y-6)],fill=(*rim,244))
                    draw.line([(x+w,y-2),(x+w+2,y-h)],fill=(*lumen,244),width=2)
    if slug in ('crystal_ashlar','violet_roof_slate'):
        roof=slug=='violet_roof_slate'; pitch=32 if roof else 64
        for j,y in enumerate(range(0,257,pitch)):
            draw.line((0,y,256,y),fill=(52,40,66) if roof else (94,81,100),width=3)
            draw.line((0,y+3,256,y+3),fill=(141,108,170) if roof else (196,174,186),width=2)
            for x in range(-64+(j%2)*64,257,128):
                draw.line((x,y,x-(10 if roof else 0),y+pitch),fill=(65,50,78) if roof else (99,85,104),width=3)
    if slug in ('vault_keystone','gilded_runestone'):
        for coords,c,w in [
            ([(128,12),(222,130),(128,244),(34,130),(128,12)],(81,59,91),10),
            ([(128,12),(222,130),(128,244),(34,130),(128,12)],(229,186,115),5),
            ([(128,30),(206,130),(128,226),(50,130),(128,30)],(171,125,92),3),
            ([(128,46),(128,90),(166,129),(128,182),(90,129),(128,90)],(245,194,255),7),
            ([(128,182),(128,208)],(235,177,251),5)]:
            draw.line(coords, fill=c, width=w,joint='curve')
        if slug=='vault_keystone':
            draw.polygon([(128,72),(169,124),(128,192),(92,128)],fill=(130,57,191))
            draw.polygon([(128,72),(169,124),(129,137)],fill=(226,160,253))
            draw.polygon([(128,72),(129,137),(92,128)],fill=(178,98,235))
            draw.polygon([(129,137),(169,124),(128,192)],fill=(109,63,175))
    return img

def prism(m,x,y,z,h,r,dx,dz,keys,angle):
    """Six individually shaded faces and an asymmetric termininating pyramid."""
    ring=lambda f,rad:[(x+f*dx+rad*math.cos(angle+i*math.tau/6),y+f*h,z+f*dz+rad*math.sin(angle+i*math.tau/6)) for i in range(6)]
    low=ring(0.,r*.82); body=ring(.16,r); neck=ring(.70,r*.93)
    tip=(x+dx+.12*r,y+h,z+dz-.22*r)
    for i in range(6):
        j=(i+1)%6; c=keys[i%len(keys)]
        m.polygon([low[i],body[i],body[j],low[j]],c)
        m.polygon([body[i],neck[i],neck[j],body[j]],c)
        m.polygon([neck[i],tip,neck[j]],keys[min(i%len(keys)+1,len(keys)-1)])
    m.polygon(low,keys[0],normal=(0,-1,0))

def mesh(slug):
    m=Mesh('stone')
    for key,c in COLORS.items():
        m.materials[key]=(list(c),None,0,.31 if key in ('indigo','violet','lilac','pearl','azure','ice','aqua','honey','glint') else .84)
    gems.palette(m)
    rng=random.Random(slug)
    def rock(x,y,z,rx,ry,rz,key):
        ellipsoid(m,[x,y,z],[rx,ry,rz],key,7,3)
    amet=['indigo','violet','lilac','violet','pearl','lilac']
    gold=['umber','amber','honey','glint','honey','amber']
    if slug in ('crystal_shard_cluster','crystal_crown','auricite_deposit','mixed_crystal_vein'):
        # Broad chips of the same rough matrix as the cliff, with short points
        # growing along one fissure. Keep the silhouette legible within a voxel.
        for index, (x,z,w,d,h) in enumerate([
                (.19,.46,.27,.32,.125),(.46,.50,.34,.43,.21),
                (.69,.55,.29,.30,.16),(.82,.36,.16,.18,.10)]):
            m.box([x-w/2,0.,z-d/2], [x+w/2,h,z+d/2], ['schist','cliff','mica','cliff'][index], .016)
        raw_palette(m, False)
        raw_palette(m, True, 'aqua_')
        blue_faces = ['aqua_'+k for k in gems.GEMS]
        ore = slug in ('auricite_deposit','mixed_crystal_vein')
        crown = slug == 'crystal_crown'
        for i, (x,z,h,r,dx,dz) in enumerate([
                (.47,.50,.69,.14,.035,.015),(.31,.47,.36,.106,-.10,.025),
                (.63,.52,.40,.104,.13,.03),(.59,.64,.25,.083,.07,.08)]):
            if i == 3 and not crown:
                continue
            keys = blue_faces if i != 1 else gems.GEMS
            if slug == 'auricite_deposit' or slug == 'mixed_crystal_vein' and i == 1:
                keys = gold
            scale = 1. if crown else .59 if ore else .50
            prism(m,x,.12,z,h*scale,r,dx,dz,keys,.15+i*.13)
    elif slug in ('highland_scree','glacier_boulder'):
        large=slug=='glacier_boulder'
        for i in range(4 if large else 8):
            a=i*2.4;e=0 if large and i==0 else rng.uniform(.12,.32);r=.35 if large and i==0 else rng.uniform(.068,.142)
            x=.50+e*math.cos(a);z=.50+e*math.sin(a)
            rock(x,r*.62,z,r,r*(1.03 if large else .71),r*.90,['cliff','mica','schist'][i%3])
            if large and i==0:prism(m,x-.08,.21,z+.16,.24,.031,.082,.03,amet,a)
    elif slug=='highland_tussock':
        for i in range(36):
            a=i*2.4;e=rng.uniform(.015,.235);x=.5+math.cos(a)*e;z=.5+math.sin(a)*e
            tip=rng.uniform(.21,.56);curl=rng.uniform(.065,.19);w=rng.uniform(.014,.028);dx,dz=math.cos(a),math.sin(a)
            mid=[x+dx*curl*.53,tip*.72,z+dz*curl*.53]; end=[x+dx*curl,tip,z+dz*curl]
            left=[x-dz*w,.016,z+dx*w];right=[x+dz*w,.016,z-dx*w]
            m.polygon([left,mid,right],'leaf');m.polygon([right,mid,left],'stem')
            m.polygon([left,end,mid],'sage');m.polygon([mid,end,left],'leaf')
    else:
        moss=slug=='frosted_moss';count=21 if moss else 17
        for i in range(count):
            a=i*2.4;e=.31*math.sqrt(i/count);x=.5+math.cos(a)*e;z=.5+math.sin(a)*e
            rock(x,.052,z,.095 if moss else .11,.068 if moss else .057,.098,['stem','sage','leaf'][i%3])
            if moss:
                rock(x+.014,.108,z,.072,.041,.063,['frost','ivory','sage'][i%3])
            else:
                height=.38+.36*rng.random();top=[x+math.cos(a)*.06,height,z+math.sin(a)*.06]
                cylinder(m,[x,.038,z],top,.014,'stem',5,end_radius=.009)
                for k in range(5):
                    ang=a+k*math.tau/5
                    rock(top[0]+.043*math.cos(ang),height+.01,top[2]+.043*math.sin(ang),.051,.040,.049,'heather' if (i+k)%3 else 'magenta')
                rock(top[0],height+.035,top[2],.023,.020,.023,'glint')
    low=min(v[1] for part in m.parts.values() for v in part['position'])
    for part in m.parts.values():part['position']=[[x,y-low,z] for x,y,z in part['position']]
    if slug in ('crystal_shard_cluster','crystal_crown','mixed_crystal_vein','auricite_deposit'):
        return gems.export_gem(m,slug,raw_growth=True)
    voxelize(m, resolution=16)
    return export(m,slug,('pearl','aqua'))

def raw_quartz_textures():
    """Weathered outcrops share the broken mineral grain of their cliff host."""
    size = 48
    with Image.open(MASTER) as master:
        grain = master.crop((512, 512, 1024, 1024)).convert('L').resize(
            (size, size), Image.Resampling.BOX)
    grain = np.asarray(grain, dtype=float)
    grain = (grain - grain.mean()) / max(grain.std(), 1.)
    y, x = np.mgrid[0:size, 0:size]
    strata = ((y + x // 5 + 2 * (x // 15)) % 24) / 24.
    # Broad cloudy zones crossed by a few blunt, pixel-scale fracture lines.
    # The little islands of host rock take their colour from the highland gneiss.
    # Interrupted strata glow inside the raw mineral. Keeping most of the mask
    # dark lets daylight and the host rock still determine the overall value.
    clefts = []
    for offset, start, end in [(13, 6, 30), (34, 22, 40)]:
        clefts.append([(x, offset - x // 6 + (1 if x % 21 > 14 else 0))
                       for x in range(start, end)])
    mask = Image.fromarray(np.uint8(np.clip(36. + 9. * grain, 15., 64.)))
    trace = ImageDraw.Draw(mask)
    for seam in clefts:
        trace.line(seam, fill=151, width=2)
        trace.line(seam[2:len(seam)//2], fill=232)
    mask.resize((192,192), Image.Resampling.NEAREST).save(FOLDER/'raw_growth_glow.png', optimize=True)
    for blue, base, lighter in [
            (True, [69, 124, 144], [160, 216, 223]),
            (False, [108, 88, 143], [200, 171, 215])]:
        color = np.array(base)
        mix = np.clip(.32 + .19 * grain + .19 * (strata > .60), .02, .91)[..., None]
        rgb = color + (np.array(lighter) - color) * mix
        quartz = tuple(lighter)
        image = Image.fromarray(rgb.astype('uint8'))
        draw = ImageDraw.Draw(image, 'RGBA')
        rng = random.Random(614)
        for i in range(13):
            px, py = rng.randrange(size), rng.randrange(size)
            width = rng.randrange(2, 7)
            for offset in (-size, 0, size):
                x0 = px + offset
                draw.line([(x0, py), (x0+width//2, py), (x0+width//2+1, py-1),
                           (x0+width, py-1)], fill=(*quartz, 160))
                if i % 3 == 0:
                    draw.rectangle((x0+1, py+1, x0+width//2+1, py+2), fill=(83, 77, 88, 179))
        for seam in clefts:
            draw.line(seam, fill=(*quartz, 158), width=2)
            draw.line(seam[2:len(seam)//2], fill=(194, 234, 235, 188) if blue else (222, 194, 241, 188))
        name = 'raw_aquamarine' if blue else 'raw_amethyst'
        image.resize((192, 192), Image.Resampling.NEAREST).save(FOLDER/(name+'.png'), optimize=True)


def raw_palette(mesh, blue, prefix=''):
    texture = 'crystal_highlands/' + ('raw_aquamarine.png' if blue else 'raw_amethyst.png')
    for key, factor, rough in [('body', [.95, .97, .99], .70),
                                ('cleavage', [.83, .87, .95], .78),
                                ('inclusion', [1., .99, 1.], .62)]:
        mesh.materials[prefix+key] = (factor, texture, .025, rough)


def raw_mineral(mesh, blue):
    raw_palette(mesh, blue)
    prism(mesh,.50,0.,.50,.995,.405,.035,-.02,gems.GEMS,.26)


def main():
    FOLDER.mkdir(parents=True,exist_ok=True)
    with Image.open(MASTER) as source:
        atlas=[source.crop((x,y,x+512,y+512)).resize((256,256),Image.Resampling.LANCZOS) for y in (0,512) for x in (0,512)]
    locale=[dict(source='Crystal Highlands',en='Crystal Highlands',de='Kristallhochland'),dict(source='Crystal Highlands · Cold',en='Crystal Highlands · Cold',de='Kristallhochland · Kalt')]
    for id,(slug,en,de) in enumerate(CATALOG,739):
        solid=id<=749 or id==759
        description, de_description = DESCRIPTIONS[id-739]
        block=dict(schema_version=1,id=id,name=en,description=description,creative_category='nature',mining_level=0,required_tool=None,hardness=2.1 if solid else .65,resistance=4. if solid else 1.6)
        if solid:
            path='textures/crystal_highlands/'+slug+'.png'; f=face(slug,atlas);f.save(BLOCKS/path,optimize=True)
            l=np.asarray(f.convert('L'),float)/255.
            norms=np.stack([(np.roll(l,1,k)-np.roll(l,-1,k))*(1.1 if id in (745,746) else 1.8) for k in (1,0)]+[np.ones_like(l)],axis=-1)
            norms/=np.linalg.norm(norms,axis=-1,keepdims=True)
            norm=path.replace('.png','_normal.png');Image.fromarray(np.uint8((norms*.5+.5)*255)).save(BLOCKS/norm,optimize=True)
            polished=id in (739,741,745,746,759)
            material=dict(normal_texture=norm,roughness=.31 if polished else .84,reflectance=.43 if polished else .14)
            if id in (745,746):
                # Very soft fluorescence leaves the illumination of each prism face readable.
                material['emission']=[.069,.036,.112] if id==745 else [.029,.071,.105]
            block.update(render_kind='cube',bevel_width=0,textures={'side':path},materials={'side':material},face_materials=['side']*6)
            if id in (745,746):
                # Cool open facets and paler termination planes catch the sky.
                block['textures'].update(edge=path, cap=path)
                block['materials']['edge']=dict(material,base_color=[.76,.81,.93,1.],roughness=.23)
                block['materials']['cap']=dict(material,base_color=[1.,.98,1.,1.],roughness=.26)
                block['face_materials']=['side','edge','edge','cap','side','edge']
        else:
            block.update(render_kind='custom_mesh',model_layout='scene',model='models/'+slug+'.glb',struct_block=True,textures=mesh(slug),collision_boxes=[])
        if slug=='glacier_boulder':
            block['collision_boxes']=[dict(minimum=[.22,0,.23],maximum=[.78,.60,.76])]
        save_json(BLOCKS/(slug+'.json'),block)
        locale.extend([dict(source=en,en=en,de=de),dict(source=description,en=description,de=de_description)])
    for en, de in [('Crystal Mine', 'Kristallmine'), ('Mage Tower', 'Magierturm'), ('Crystal Vault', 'Kristallgewölbe')]:
        locale.append(dict(source=en, en=en, de=de))
    minerals = [
        ('massive_amethyst','Massive Amethyst','Massiver Amethyst'),
        ('massive_aquamarine','Massive Aquamarine','Massiver Aquamarin'),
        ('amethyst_lance','Amethyst Lance','Amethystlanze'),
        ('aquamarine_lance','Aquamarine Lance','Aquamarinlanze'),
    ]
    gems.textures()
    raw_quartz_textures()
    for index,(slug,name,german) in enumerate(minerals,770):
        blue=index%2==1
        mineral_mesh=Mesh('quartz')
        raw_mineral(mineral_mesh,blue)
        textures=gems.export_gem(mineral_mesh,slug,blue,raw_growth=True)
        save_json(BLOCKS/(slug+'.json'), dict(schema_version=1,id=index,name=name,description=name,
            creative_category='nature',render_kind='custom_mesh',model_layout='scene',
            model='models/'+slug+'.glb',struct_block=True,textures=textures,hardness=2.7,
            resistance=4.,required_tool=None,mining_level=0,
            collision_boxes=[dict(minimum=[.19,0.,.19],maximum=[.81,.93,.81])]))
        locale.append(dict(source=name,en=name,de=german))
    save_json(ROOT/'src-ui/locales/source/crystal-highlands.json',locale)

if __name__=='__main__':main()
