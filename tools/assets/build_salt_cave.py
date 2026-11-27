"""Build the halite cave palette and five placeable mineral formations.

The checked-in tile maps are the production inputs. To resample them from the
artist's supplied 1448 × 1086 salt palette, pass its path with --palette.
"""
import argparse
import json
import math
import random

from build_castle import BLOCKS, Mesh
from voxel_props.surface import export_voxels

TILES = BLOCKS / 'textures/salt_cave'
# Vertices of the illustrated cubes: left, rear, right, front, front foot, right foot.
SWATCHES = {
    'base': [(96,235),(194,189),(295,226),(202,271),(202,381),(295,339)],
    'crust': [(361,234),(456,189),(559,225),(466,270),(466,380),(557,340)],
    'compact': [(632,232),(727,190),(828,226),(732,268),(732,381),(825,342)],
    'wet': [(899,233),(997,191),(1095,225),(1003,269),(1003,380),(1095,341)],
    'dust': [(1160,245),(1264,198),(1369,237),(1268,279),(1268,381),(1366,340)],
    'deep': [(116,485),(221,443),(333,478),(229,523),(229,628),(332,589)],
    'clear': [(436,488),(537,445),(642,477),(545,522),(545,632),(642,591)],
    'accent': [(762,481),(861,441),(973,478),(874,523),(874,631),(973,592)],
    'layer': [(1083,486),(1189,441),(1303,475),(1204,518),(1204,632),(1304,589)],
}


def sample_swatches(path, tiles=TILES, swatches=SWATCHES):
    import numpy as np
    from PIL import Image

    source = Image.open(path).convert('RGB')
    if source.size != (1448, 1086):
        raise ValueError('Expected a 1448 × 1086 cave block palette')
    tiles.mkdir(parents=True, exist_ok=True)
    for name, (left, rear, right, front, foot, far_foot) in swatches.items():
        # Sample within the silhouette so there is never sky or cast shadow in a tile.
        for side, corners in [('top', [rear,left,front,right]), ('side', [front,foot,far_foot,right])]:
            if name == 'accent' and side == 'top':
                continue  # The ore matrix has one binding beneath its mineral geometry.
            points = np.array(corners)
            center = points.mean(axis=0)
            inset = 0.88 if name in ('crust', 'accent') else 0.91
            face = source.transform((128,128), Image.Transform.QUAD,
                                    tuple((center+(points-center)*inset).ravel()), Image.Resampling.BICUBIC)
            data = np.asarray(face, dtype=float)
            # Retain mineral veins in the interior. Crossfade only a thin strip
            # from the opposite edge so the mip chain samples continuous rock.
            for axis in (0, 1):
                data = np.swapaxes(data, axis, 0)
                for i in range(8):
                    opposite = 0.5*(1-i/8)**2
                    a, b = data[i].copy(), data[-1-i].copy()
                    data[i] = a*(1-opposite) + b*opposite
                    data[-1-i] = b*(1-opposite) + a*opposite
                data = np.swapaxes(data, axis, 0)
            # The directional key light from the illustration shades the flank;
            # remove this broad exposure difference before native game lighting.
            data = np.clip(data*(1.16 if side == 'side' else 1.02),0,255)
            Image.fromarray(data.astype('uint8')).save(tiles/f'{name}_{side}.png', optimize=True)
            height = data @ [0.2126,0.7152,0.0722] / 255
            nx = (np.roll(height,1,1)-np.roll(height,-1,1))*2.1
            ny = (np.roll(height,-1,0)-np.roll(height,1,0))*2.1
            normal = np.stack([nx,ny,np.ones_like(height)],axis=-1)
            normal /= np.linalg.norm(normal,axis=-1)[...,None]
            Image.fromarray(((normal*.5+.5)*255+.5).astype('uint8')).save(
                tiles/f'{name}_{side}_normal.png', optimize=True)


def save_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n')


def stone(number, slug, title, description, style, roughness, reflectance, alpha=1, directory="salt_cave"):
    path = BLOCKS/f'{slug}.json'
    data = json.loads(path.read_text()) if path.exists() else dict(
        schema_version=1, id=number, name=title, description=description, creative_category='terrain',
        hardness=1.0, resistance=8.0, required_tool='pickaxe', mining_level=1)
    flank = {'salt_formation':'Salt body', 'salt_stone':'Mineral rock',
             'pink_salt_stone':'Mineral rock', 'dripstone':'Dripstone', 'dripstone_formation':'Dripstone'}.get(slug, 'Cleavage')
    data.update(render_kind='cube', surface='rock', bevel_width=0,
                face_materials=[flank,flank,flank,'Bedding',flank,flank])
    data['materials'], data['textures'] = {}, {}
    for material, side in [(flank,'side'),('Bedding','top')]:
        stem = f'textures/{directory}/{style}_{side}'
        data['materials'][material] = dict(base_color=[1,1,1,alpha], roughness=roughness,
            reflectance=reflectance, normal_texture=stem+'_normal.png')
        data['textures'][material] = stem+'.png'
    if slug == 'salt_formation':
        data['crystal_material'] = flank
    save_json(path,data)


def column(mesh, x, z, width, depth, height, angle, lean, materials, hanging=False, base=0):
    """Bevelled square halite prism with offset cleavage band and asymmetric termination."""
    ca, sa = math.cos(angle), math.sin(angle)
    outline = [(-1,-.7),(-.7,-1),(.7,-1),(1,-.7),(1,.7),(.7,1),(-.7,1),(-1,.7)]
    if height < .28:
        outline = [(-1,-1),(1,-1),(1,1),(-1,1)]
    def ring(t, radius):
        result = []
        for a,b in outline:
            dx,dz=a*width*radius,b*depth*radius
            y=base+t*height
            result.append([x+dx*ca-dz*sa+lean[0]*t, 1-y if hanging else y,
                           z+dx*sa+dz*ca+lean[1]*t])
        return result
    profile = [(0,.78),(.11,1),(.67,.96),(.91,.96),(1,.64)] if height >= .28 else [(0,.80),(.64,1),(1,.58)]
    rings = [ring(t,r) for t,r in profile]
    def polygon(points,material):
        mesh.polygon(list(reversed(points)) if hanging else points, material)
    for layer, (low,high) in enumerate(zip(rings,rings[1:])):
        for i in range(len(outline)):
            j=(i+1)%len(outline)
            key=materials[1] if i%2==0 or layer in (2,4) else materials[0]
            polygon([low[i],high[i],high[j],low[j]], key)
    mesh.polygon(rings[-1],materials[1],[0,-1 if hanging else 1,0])
    mesh.polygon(rings[0],materials[0],[0,1 if hanging else -1,0])


def outcrop(shape, seed):
    rng = random.Random(seed)
    mesh = Mesh('stone')
    crystal = ('Halite','Cleavage edge')
    stone = ('Rock','Chipped facet')
    mesh.materials.update({
        'Halite':([1,.94,.97], 'salt_cave/clear_side.png', .02,.23),
        'Cleavage edge':([1,.97,.91], 'salt_cave/compact_side.png', .01,.33),
        'Chipped facet':([.86,.79,.86], 'salt_cave/crust_top.png', 0,.76),
        'Rock':([.81,.77,.86], 'salt_cave/base_side.png',0,.88),
        'Warm core':([.84,.65,.40], 'salt_cave/clear_side.png',0,.29)})
    hanging=shape=='stalactite'
    if shape in ('stalactite', 'stalagmite'):
        # A common cleavage plane ties a fan together. Each daughter prism
        # has its own offset, termination and height. An overhanging cornice
        # carries the pendants; the upright habit has a low salt-scree skirt.
        angle = rng.uniform(-.45,.45)
        if hanging:
            shoots=[(.46,.47,.98,.116),(.38,.53,.77,.092),(.56,.51,.63,.105),
                    (.49,.60,.51,.090),(.30,.42,.51,.081),(.62,.43,.42,.082),
                    (.37,.63,.36,.067),(.59,.60,.30,.084)]
        else:
            shoots=[(.51,.47,.97,.132),(.38,.44,.71,.092),(.59,.55,.62,.103),
                    (.48,.60,.54,.095),(.32,.55,.43,.086),(.64,.42,.42,.081),
                    (.40,.67,.29,.07),(.67,.60,.23,.072),(.37,.30,.30,.066)]
        for i,(x,z,h,w) in enumerate(shoots):
            tilt=.05 if hanging else .20
            column(mesh,x,z,w,w*rng.uniform(.74,.96),h,angle+rng.uniform(-.18,.18),
                   (tilt*(x-.5),tilt*(z-.5)),
                   ('Warm core','Cleavage edge') if i in (1,3,6) else crystal,hanging)
        for i in range(26):
            a=rng.random()*math.tau
            r=.38*math.sqrt(rng.random())
            w=rng.uniform(.036,.087)
            column(mesh,.5+r*math.cos(a),.5+r*math.sin(a),w,w*.84,
                   rng.uniform(.055,.22)*(1.2-r),angle+rng.uniform(-.35,.35),(0,0),
                   stone if i%5==0 else crystal,hanging)
        return mesh
    if shape == 'pillar_fragment':
        for x,z,h,w in [(.43,.48,.92,.19),(.6,.56,.84,.14),(.36,.61,.74,.12),(.62,.38,.71,.10)]:
            column(mesh,x,z,w,w*.88,h,.2,(.015,.02),stone)
    if shape == 'rubble':
        for i in range(20):
            a=rng.random()*math.tau; r=.37*math.sqrt(rng.random())
            w=rng.uniform(.045,.11)
            column(mesh,.5+r*math.cos(a),.5+r*math.sin(a),w,w*.88,
                   .09+.24*(1-r/.4),a,(.018,-.02),stone if i%3 else crystal)
    else:
        radius={'crystal_cluster':.31,'stalagmite':.22,'stalactite':.32,'pillar_fragment':.31}[shape]
        for i in range(13):
            r=radius*math.sqrt(i/13); a=i*2.4+rng.uniform(-.13,.13)
            x,z=.5+r*math.cos(a),.5+r*math.sin(a)
            h=(.93-.61*r/radius)*rng.uniform(.77,1.02)
            if shape=='pillar_fragment':
                x+=.09; h*=.42
            if shape=='stalagmite':
                h*=1.04
            w=(.14 if i==0 else rng.uniform(.053,.093))*(1.13 if hanging else 1)
            lean=.14*r/radius if shape=='crystal_cluster' else .032
            column(mesh,x,z,w,w*.87,h,a,(lean*math.cos(a),lean*math.sin(a)),
                   ('Warm core','Cleavage edge') if i%4==1 else crystal,hanging)
    for i in range(19):
        a=rng.random()*math.tau; r=.43*math.sqrt(rng.random()); w=rng.uniform(.014,.047)
        column(mesh,.5+r*math.cos(a),.5+r*math.sin(a),w,w*.91,rng.uniform(.035,.11),
               a,(0,0),stone if i%4==1 else crystal,hanging)
    return mesh


STONE = [
    (454,'salt_cave_stone','Salt Cave Stone','Salzhöhlengestein','base',.85,.16),
    (57,'salt_stone',None,None,'crust',.67,.24),
    (38,'salt_formation',None,None,'compact',.40,.32),
    (455,'wet_salt_stone','Wet Salt Stone','Nasses Salzgestein','wet',.17,.48),
    (456,'salt_dust_floor','Salt Dust Floor','Salzstaubboden','dust',.94,.15),
    (457,'deep_salt_stone','Deep Cave Stone','Tiefes Höhlengestein','deep',.82,.18),
    (458,'translucent_salt_block','Translucent Salt Crystal Block','Durchscheinender Salzkristallblock','clear',.24,.41),
    (58,'pink_salt_stone',None,None,'layer',.54,.29),
]
PROPS = [
    ('crystal_cluster','Salt Crystal Cluster','Salzkristallgruppe'),
    ('stalactite','Salt Stalactite','Salzstalaktit'),
    ('stalagmite','Salt Stalagmite','Salzstalagmit'),
    ('rubble','Salt Rubble','Salzgeröll'),
    ('pillar_fragment','Salt Pillar Fragment','Salzsäulenfragment'),
]
DESCRIPTIONS = {
    'base':('Fractured bedrock from the halite galleries.','Zerklüftetes Grundgestein aus den Salzgalerien.'),
    'wet':('Glossy salt stone lining underground pools.','Glänzendes Salzgestein vom Rand unterirdischer Becken.'),
    'dust':('Fine salt dust settles on dry ledges.','Feiner Salzstaub bedeckt die trockenen Felsvorsprünge.'),
    'deep':('Dark, dense rock from the lower salt galleries.','Dunkles, dichtes Gestein aus den unteren Salzgalerien.'),
    'clear':('Light passes through its pale cubic salt crystals.','Licht fällt durch die hellen, würfelförmigen Salzkristalle.'),
    'crystal_cluster':('A fan of warm halite crystals growing from a shared base.','Ein Fächer warmer Halitkristalle auf einem gemeinsamen Sockel.'),
    'stalactite':('Tapered salt crystals hanging from the ceiling.','Spitz zulaufende Salzkristalle, die von der Decke hängen.'),
    'stalagmite':('Upright halite shoots rise from a bed of salt chips.','Aufrechte Halitnadeln wachsen aus einem Bett aus Salzsplittern.'),
    'rubble':('Loose halite chips and fragments of cave stone.','Lose Halitsplitter und Bruchstücke aus Höhlengestein.'),
    'pillar_fragment':('The fluted remnant of a salt pillar with crystals at its foot.','Das geriffelte Fragment einer Salzsäule mit Kristallen am Fuß.'),
}


def build():
    translations = []
    for number,slug,en,de,style,roughness,reflectance in STONE:
        description = DESCRIPTIONS.get(style, (None,None))[0]
        stone(number,slug,en,description,style,roughness,reflectance,.78 if style=='clear' else 1)
        if en:
            translations.extend([(en,de),DESCRIPTIONS[style]])
    # Keep the salt ore's mining tier, mineral drops and crystal mesh binding.
    path = BLOCKS/'salt_ore.json'
    ore=json.loads(path.read_text())
    ore['materials']['Host rock']['normal_texture']='textures/salt_cave/accent_side_normal.png'
    ore['textures']['Host rock']='textures/salt_cave/accent_side.png'
    save_json(path,ore)
    for number,(shape,en,de) in enumerate(PROPS,459):
        slug='salt_'+shape
        mesh=outcrop(shape,number)
        textures=export_voxels(mesh, slug, ('Warm core',), resolution=16)
        low, high = (.73,1) if shape=='stalactite' else (0,.23 if shape=='rubble' else .29)
        boxes=[dict(minimum=[.10,low,.10],maximum=[.90,high,.90])]
        if shape!='rubble':
            boxes.append(dict(minimum=[.33,.07 if shape=='stalactite' else high,.33],
                              maximum=[.67,low if shape=='stalactite' else .94,.67]))
        definition=dict(schema_version=1,id=number,name=en,description=DESCRIPTIONS[shape][0],
                        creative_category='decoration',mining_level=0,hardness=.70,resistance=2,
                        struct_block=True,render_kind='custom_mesh',model_layout='scene',
                        model='models/'+slug+'.glb',textures=textures,collision_boxes=boxes)
        if shape=='crystal_cluster':
            definition['light']=dict(color=[1,.78,.56],intensity=4700,range=17,falloff='soft_core',
                flicker=0,cave_intensity=.8,shadows=False,offset=[.5,.34,.5])
        save_json(BLOCKS/(slug+'.json'),definition)
        translations.extend([(en,de),DESCRIPTIONS[shape]])
    save_json(BLOCKS.parents[2]/'src-ui/locales/source/salt-cave.json',
              [dict(source=en,en=en,de=de) for en,de in translations])


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--palette')
    args=parser.parse_args()
    if args.palette:
        sample_swatches(args.palette)
    build()
