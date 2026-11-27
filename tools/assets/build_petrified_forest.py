"""Author the petrified forest's mineral grain, eroded masonry and excavation props."""
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from build_ashlands import stone_fields
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from build_spirit_blossom import export
from voxel_props.geometry import VoxelSculpt
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

ROOT = Path(__file__).resolve().parents[2]
# Stable saved block IDs. The two cut faces also form the metre-wide growth
# bands on giant stumps, where a repeated end-grain tile would lose the rings.
CATALOG = [
    ('dusty_ground', 'Dusty Ground', 'Staubboden'),
    ('cracked_stone_soil', 'Cracked Stone-Soil', 'Rissiger Steinboden'),
    ('petrified_log', 'Petrified Log', 'Versteinerter Stamm'),
    ('petrified_planks', 'Petrified Planks', 'Versteinerte Bretter'),
    ('mineral_encrusted_stone', 'Mineral-Encrusted Stone', 'Mineralverkrusteter Stein'),
    ('weathered_ruin_stone', 'Weathered Ruin Stone', 'Verwitterter Ruinenstein'),
    ('carved_temple_stone', 'Carved Temple Stone', 'Behauener Tempelstein'),
    ('petrified_dead_root', 'Dead Root', 'Abgestorbene Wurzel'),
    ('broken_petrified_stump', 'Broken Petrified Stump', 'Gebrochener versteinerter Stumpf'),
    ('shattered_stone_root', 'Shattered Stone Root', 'Geborstene Steinwurzel'),
    ('petrified_mineral_deposit', 'Mineral Deposit', 'Mineralvorkommen'),
    ('small_dry_shrub', 'Small Dry Shrub', 'Kleiner Trockenstrauch'),
    ('large_dry_shrub', 'Large Dry Shrub', 'Großer Trockenstrauch'),
    ('petrified_rubble', 'Loose Ruin Rubble', 'Loser Ruinenschutt'),
    ('excavation_crate', 'Excavation Crate', 'Grabungskiste'),
    ('excavation_awning', 'Canvas Awning', 'Segeltuchdach'),
    ('excavation_standard', 'Excavation Standard', 'Grabungsbanner'),
    ('petrified_sapwood', 'Petrified Sapwood', 'Versteinertes Splintholz'),
    ('petrified_heartwood', 'Petrified Heartwood', 'Versteinerter Holzkern'),
    ('excavated_rib', 'Excavated Rib Cage', 'Freigelegter Rippenbogen'),
    ('excavated_skull', 'Excavated Skull', 'Freigelegter Schädel'),
]
DESCRIPTIONS = {
    'excavated_rib': ('A pair of curved ivory ribs rises above a fossil vertebra.', 'Ein Paar gebogener elfenbeinfarbener Rippen ragt über einem fossilen Wirbel auf.'),
    'excavated_skull': ('A long-toothed fossil skull with hollow eye sockets.', 'Ein langzahniger fossiler Schädel mit hohlen Augenhöhlen.'),
    'dusty_ground': ('Fine limestone dust settles over hard grey soil.', 'Feiner Kalkstaub bedeckt den harten grauen Boden.'),
    'cracked_stone_soil': ('Flat plates of shale split along rust-coloured seams.', 'Flache Schieferplatten brechen entlang rostfarbener Adern.'),
    'petrified_log': ('Silica preserves the fluted bark and ochre growth rings.', 'Kieselsäure bewahrt die gefurchte Rinde und die ockerfarbenen Jahresringe.'),
    'petrified_planks': ('Mineral bands run lengthwise through the sawn boards.', 'Mineralbänder ziehen sich längs durch die gesägten Bretter.'),
    'mineral_encrusted_stone': ('Turquoise and amber crystals fill pockets in the bedrock.', 'Türkise und bernsteinfarbene Kristalle füllen Hohlräume im Fels.'),
    'weathered_ruin_stone': ('Weather and lichen have rounded the old masonry.', 'Wetter und Flechten haben die Kanten des alten Mauerwerks abgerundet.'),
    'carved_temple_stone': ('A branching fossil tree is cut into the face of the stone.', 'Auf der Steinfläche ist ein verzweigter fossiler Baum eingemeißelt.'),
    'petrified_dead_root': ('A tangle of roots slowly turned to stone.', 'Ein Wurzelgeflecht hat sich über die Zeit in Stein verwandelt.'),
    'broken_petrified_stump': ('The snapped crown exposes concentric mineral rings.', 'An der Bruchfläche zeichnen sich konzentrische Mineralringe ab.'),
    'shattered_stone_root': ('A stone root arches above the eroded ground.', 'Eine Steinwurzel wölbt sich über den ausgewaschenen Boden.'),
    'petrified_mineral_deposit': ('Turquoise and citrine prisms grow from one stone matrix.', 'Türkis- und Citrinprismen wachsen aus demselben Muttergestein.'),
    'small_dry_shrub': ('Ochre seed husks cling to brittle twigs.', 'Ockerfarbene Samenhülsen hängen an spröden Zweigen.'),
    'large_dry_shrub': ('Rust-red stems branch above the dust.', 'Rostrote Triebe verzweigen sich über dem Staub.'),
    'petrified_rubble': ('Small angular fragments collect at the foot of the shelves.', 'Kantige Bruchstücke sammeln sich am Fuß der Felsstufen.'),
    'excavation_crate': ('Sorted mineral samples and a spade left by the dig crew.', 'Sortierte Mineralproben und ein Spaten blieben an der Grabung zurück.'),
    'excavation_awning': ('Heavy linen sags between the shelter posts.', 'Schweres Leinen hängt zwischen den Pfosten des Unterstands.'),
    'excavation_standard': ('A faded sun emblem marks an abandoned trench.', 'Ein verblichenes Sonnenzeichen markiert einen verlassenen Grabungsschnitt.'),
    'petrified_sapwood': ('A pale silica band from the outer wood of a fossil trunk.', 'Ein helles Kieselband aus dem äußeren Holz eines fossilen Stamms.'),
    'petrified_heartwood': ('Iron oxides colour the ancient heartwood amber.', 'Eisenoxide färben das uralte Kernholz bernsteinfarben.'),
}
COLORS = {
    'bark': (.40,.35,.34), 'stone': (.51,.50,.50), 'facet': (.68,.64,.60),
    'ironwood': (.57,.29,.15), 'amberwood': (.76,.45,.22), 'grain': (.83,.67,.54),
    'basalt': (.28,.25,.31), 'copper': (.76,.36,.17), 'ochre': (.90,.57,.25),
    'amethyst': (.20,.62,.63), 'quartz': (.57,.89,.84), 'citrine': (1.,.67,.24),
    'ivory': (.91,.79,.61), 'bone': (.75,.62,.46), 'enamel': (.99,.89,.75),
    'canvas': (.84,.68,.48), 'cut': (.46,.30,.20), 'iron': (.30,.28,.29),
}


def mineral_noise(rng, x, y, cells):
    """Tileable, gently interpolated patches of sediment at a chosen scale."""
    grid = rng.uniform(-1., 1., (cells, cells))
    u, v = x * cells / x.shape[1], y * cells / y.shape[0]
    i, j = u.astype(int), v.astype(int)
    u, v = u - i, v - j
    u, v = u*u*(3.-2.*u), v*v*(3.-2.*v)
    left = (1.-v)*grid[j, i] + v*grid[(j+1) % cells, i]
    right = (1.-v)*grid[j, (i+1) % cells] + v*grid[(j+1) % cells, (i+1) % cells]
    return (1.-u)*left + u*right


def shale_chips(color, relief, x, y, rng):
    # Embedded shards range from dark ironstone to pale, freshly broken quartz.
    n = x.shape[0]
    for _ in range(114):
        cx, cy = rng.uniform(0., n, 2)
        dx, dy = (x-cx+n/2) % n-n/2, (y-cy+n/2) % n-n/2
        a = rng.uniform(0., math.tau)
        u = (dx*math.cos(a) + dy*math.sin(a))/rng.uniform(.9, 4.8)
        v = (dy*math.cos(a) - dx*math.sin(a))/rng.uniform(.7, 2.8)
        shape = np.maximum(np.abs(u)*.78 + np.abs(v)*.57, np.abs(v))
        chip = shape < 1.
        tone = rng.choice([-26., -18., 13., 21., 28.])
        color[chip] += tone*np.array([.95, .94, 1.05])
        relief[chip] += (1.-shape[chip])*11. - v[chip]*7.

from build_relic_biomes import field_face

def surface(slug):
    n = 128
    y, x = np.mgrid[:n, :n]
    rng = np.random.default_rng(38382 + sum(map(ord, slug)))
    phase = math.tau/n
    xx = x + 2.3*np.sin(y*phase*2)
    yy = y + 2*np.sin(x*phase*2)
    cell, gap = stone_fields(xx, yy, rng, 6, 6)
    color = np.zeros((n,n,3)) + [153,146,138]
    grains = mineral_noise(rng, x, y, 15)
    sediment = mineral_noise(rng, x, y, 4)
    relief = rng.normal(0,2,(n,n)) + grains*5. + sediment*4.
    fracture = np.exp(-gap*22)
    if slug == 'dusty_ground':
        color[:] = [161,150,158]
        color += sediment[:,:,None]*[6,4,1]
        relief += np.where((cell%8==0) & (gap>.39), 8, 0) - fracture*2
        shale_chips(color, relief, x, y, rng)
    elif slug == 'dusty_ground_side':
        # A ragged dust cap gives way to darker shale bands on an exposed ledge.
        depth = y + 5.*np.sin(x*phase) + 3.*sediment
        color[:] = [152,139,136]
        color[depth < 24.] = [180,177,173]
        relief += 6.*np.sin(depth*.21 + sediment) - 15.*np.exp(-((depth-22.)/3.)**2)
        relief += 7.*np.where(cell%3==1, 1., -1.) - fracture*14.
        shale_chips(color, relief, x, y, rng)
    elif slug in ('cracked_stone_soil','mineral_encrusted_stone'):
        relief += (cell%5)*3 - 50*fracture + 11*np.exp(-((gap-.11)/.08)**2)
        color += np.where((cell%3==0)[:,:,None], [7,1,-4], [-2,0,5])
        if slug.startswith('mineral'):
            color[(cell%5==1)&(gap>.30)] = [68,131,131]
            color[(cell%5==1)&(gap>.43)] = [124,192,176]
            color[(cell%7==0)&(gap>.24)] = [206,143,65]
    elif slug in ('weathered_ruin_stone','carved_temple_stone'):
        color[:]=[162,149,136]
        yy = (y+1.2*np.sin(x*phase*3))%32
        xx = (xx + np.floor(y/32)*32)%64
        joints = np.minimum.reduce((yy,32-yy,xx,64-xx))
        relief += 3*np.floor(y/32)-36*np.exp(-joints/1.9)-11*fracture
        color[(cell%7==1)&(gap>.41)] = [149,133,92]
    elif slug == 'petrified_log_side':
        # Upright columns of mineralized wood break across staggered cleavage planes.
        flute, seam = stone_fields(x + 2.8*sediment, y + 2.*grains, rng, 10, 2)
        bark = np.array([[135,114,125], [160,139,147], [174,153,156],
                         [173,117,78], [153,127,127], [153,133,145], [186,145,111]])
        color = bark[flute % len(bark)].astype(float)
        relief += 9.*np.sin(xx*phase*9 + sediment) + 7.*np.sin(xx*phase*18)
        relief += 13.*np.exp(-((seam-.10)/.07)**2) - 39.*np.exp(-seam*18.)
    elif slug == 'petrified_planks':
        xx = y+1.4*np.sin(x*phase)
        flutes = .5+.5*np.cos(xx*phase*5 + .40*np.sin(y*phase) + sediment*.38)
        relief += 19*flutes - 28*(1-flutes)**5
        color[:] = [141,115,126]
        color[(flutes>.64)&(cell%4!=1)] = [188,131,88]
        color[(flutes>.82)&(cell%5==1)] = [194,172,159]
        relief -= fracture*22
        relief -= 32*((y%32<2)|((x+(y//32)*43)%128<2))
    else:
        angle = np.arctan2(y-64,x-64)
        radius = np.hypot(x-64,y-64) + np.sin(angle*7)*1.5 + np.sin(angle*3)*2
        radius = radius if slug.endswith('end') else xx*.25+1.7*sediment
        ring = np.cos(radius*.49)
        color[:]=[175,133,98] if slug.endswith(('end','heartwood')) else [211,193,159]
        color += (ring*17)[:,:,None]*[1,.81,.64]
        relief += 11*ring-14*(ring<-.83) - 12*fracture
    pixels = np.uint8(np.clip(color+relief[:,:,None], 0, 255))
    if slug in ('dusty_ground', 'petrified_log_side'):
        pixels = field_face(int(slug == 'petrified_log_side'))
    if slug=='cracked_stone_soil':
        mineral=np.asarray(Image.open(BLOCKS/'textures/relics/turquoise_veined_stone.png').convert('RGB'),dtype=float)
        grey=mineral @ np.array([.213,.715,.072])
        pixels=np.uint8(np.clip(np.array([152,149,148])+(grey-grey.mean())[:,:,None]*.79,0,255))
    if slug=='carved_temple_stone':
        # Recessed panel around the fossil tree rune, with a rubbed ochre lip.
        panel=Image.fromarray(pixels)
        draw=ImageDraw.Draw(panel)
        draw.rounded_rectangle((17,11,111,116), radius=5, outline=(98,77,79),width=4)
        for shift,ink,width in [(2,(91,70,74),7),(0,(207,159,104),4)]:
            strokes=[[(64,99),(64,39),(73,29)],[(64,77),(43,58),(43,48)],[(64,66),(85,46),(85,40)],[(64,54),(52,42)]]
            for stroke in strokes:
                draw.line([(x+shift,y+shift) for x,y in stroke],fill=ink,width=width)
        pixels=np.asarray(panel)
    folder=BLOCKS/'textures/petrified_forest'
    folder.mkdir(exist_ok=True)
    path=folder/(slug+'.png')
    Image.fromarray(pixels).resize((256,256),Image.Resampling.NEAREST).save(path,optimize=True)
    # Gentle wraparound normals follow the same cracks and flutes as the albedo.
    heights = pixels @ np.array([.2126,.7152,.0722]) / 255.
    normal = np.stack([(np.roll(heights,1,axis)-np.roll(heights,-1,axis))*2.6
                       for axis in [1,0]] + [np.ones_like(heights)], axis=-1)
    normal /= np.linalg.norm(normal, axis=-1, keepdims=True)
    normal_pixels = np.uint8(np.rint((normal*.5+.5)*255))
    Image.fromarray(normal_pixels).resize((256,256),Image.Resampling.NEAREST).save(
        folder/(slug+'_normal.png'), optimize=True)
    return path.relative_to(BLOCKS).as_posix()


def fossil_root(mesh, arch):
    root = VoxelSculpt(24)
    crest = .58 if arch else .24
    # A forked spine spans open ground between two feet. Side shoots taper
    # along the soil and the ochre fracture exposes the fossil's broken tip.
    spine = [(.08,.028,.35), (.22,.12,.40), (.32,crest*.91,.48),
             (.49,crest,.50), (.66,crest*.72,.52), (.78,.12,.64), (.92,.026,.69)]
    widths = [.025,.052,.075,.090,.072,.048,.022]
    for a,b,ra,rb in zip(spine,spine[1:],widths,widths[1:]):
        root.branch(a,b,ra,'bark',rb)
    for start, bend, fork, toe in [
        (spine[2],(.24,.16,.66),(.17,.053,.78),(.08,.020,.85)),
        (spine[4],(.67,.16,.35),(.74,.052,.23),(.91,.022,.17)),
        (spine[5],(.66,.085,.74),(.59,.040,.81),(.61,.017,.93)),
    ]:
        root.branch(start,bend,.058,'stone',.044)
        root.branch(bend,fork,.045,'stone',.029)
        root.branch(fork,toe,.030,'facet',.014)
    root.branch(spine[3],(.53,crest+.10,.45),.067,'ironwood',.051)
    root.blob((.53,crest+.10,.45),(.047,.029,.049),'amberwood')
    root.emit(mesh)


def prop(slug):
    m=Mesh('stone')
    for k,color in COLORS.items():
        m.materials[k]=(list(color),None,0,.71 if k in ('quartz','citrine') else .92)
    def rod(a,b,start,end,color,sides=6):
        cylinder(m,a,b,start,color,sides,end_radius=end)
    def rock(x,y,z,r,h,color):
        ellipsoid(m,[x,y,z],[r,h,r*.81],color,7,3)
    if slug in ('petrified_dead_root','shattered_stone_root'):
        fossil_root(m, arch=slug == 'shattered_stone_root')
        return export(m,slug)
    if slug=='broken_petrified_stump':
        height=.65
        for i in range(7):
            angle=i*2.39996
            dx,dz=math.cos(angle),math.sin(angle)
            top=[.48+dx*.05,height-.035*(i%3),.50+dz*.05]
            knee=[.49+dx*.24,height*.38,.49+dz*.21]
            foot=[.5+dx*.43,.046,.5+dz*.43]
            rod(top,knee,.085,.072,'facet' if i%3==0 else 'stone')
            rod(knee,foot,.074,.035,'stone' if i%2 else 'bark')
            rod(foot,[foot[0]-dz*.06,.022,foot[2]+dx*.07],.038,.012,'bark')
        rod([.49,.09,.5],[.46,.71,.5],.28,.22,'ironwood',11)
        for i in range(13):
            dx,dz=math.cos(i*math.tau/13),math.sin(i*math.tau/13)
            rod([.49+dx*.246,.07,.5+dz*.246],[.46+dx*.208,.69+(i%4)*.033,.5+dz*.208],.054,.044,'facet' if i%3 else 'amberwood',5)
        for j,col in enumerate(['grain','ironwood','amberwood','grain','ironwood']):
            r=.209-j*.04
            rod([.46,.711+j*.003,.5],[.46,.714+j*.003,.5],r,r,col,12)
    elif slug=='excavated_rib':
        rod([.01,.13,.5],[.99,.13,.5],.092,.093,'bone',8)
        for x in [.22,.73]:
            ellipsoid(m,[x,.18,.51],[.165,.16,.124],'ivory',8,4)
            for sign in [-1,1]:
                ribs = [[x,.17,.50+sign*.04], [x+.036,.21,.50+sign*.32],
                        [x+.09,.41,.50+sign*.42], [x+.12,.62,.50+sign*.32],
                        [x+.14,.76,.50+sign*.10]]
                for j in range(4):
                    rod(ribs[j],ribs[j+1],.059-j*.006,.054-j*.007,'ivory' if j else 'bone',7)
    elif slug=='excavated_skull':
        ellipsoid(m,[.28,.46,.5],[.25,.19,.27],'ivory',9,5)
        for sign in [-1,1]:
            z=.5+sign*.255
            # Brow ridge, cheek arch, jaw and teeth leave genuinely open sockets.
            rod([.22,.55,z],[.60,.56,z],.085,.061,'ivory',7)
            rod([.61,.55,z],[.75,.34,.5+sign*.13],.06,.06,'ivory',6)
            rod([.28,.30,z],[.65,.32,z],.053,.045,'bone',6)
            rod([.28,.29,z],[.40,.53,z],.052,.046,'ivory',6)
            rod([.54,.34,z],[.87,.27,.5+sign*.094],.07,.05,'ivory',6)
            rod([.31,.15,z],[.91,.13,.5+sign*.09],.073,.04,'bone',7)
            for j in range(5):
                x=.5+j*.082
                edge=.5+sign*(.18-j*.017)
                rod([x,.286,edge],[x+.013,.185+.015*(j%2),edge],.030,.006,'enamel',6)
        ellipsoid(m,[.74,.342,.5],[.21,.084,.16],'ivory',8,4)
    elif slug=='petrified_mineral_deposit':
        for i,(x,z,h,r) in enumerate([(.47,.48,.89,.13),(.72,.43,.58,.11),(.32,.65,.45,.10),(.26,.41,.61,.10),(.65,.69,.32,.09)]):
            tint='amethyst' if i in (0,1,3) else 'citrine'
            lean=(x-.5)*.31
            rod([x,.05,z],[x+lean,h*.75,z],r*.88,r,tint,5)
            rod([x+lean,h*.75,z],[x+lean*1.3,h,z-.018],r,.005,'quartz' if tint=='amethyst' else 'ochre',5)
        for i in range(8):
            rock(.5+.27*math.cos(i*2.4),.063,.5+.29*math.sin(i*2.4),.12,.08,'stone')
    elif slug.endswith('dry_shrub'):
        height=.91 if slug.startswith('large') else .48
        for i in range(8):
            a=i*2.39996; dx,dz=math.cos(a), math.sin(a)
            knee=[.5+dx*.15,height*.39,.5+dz*.15]
            tip=[.5+dx*.38,height*(.62+.09*(i%4)),.5+dz*.38]
            rod([.5,.012,.5],knee,.052,.034,'bark')
            rod(knee,tip,.038,.013,'ironwood',5)
            fork=[tip[0]-dz*.079,tip[1]-.09,tip[2]+dx*.089]
            rod(knee,fork,.025,.009,'ironwood')
            for q in [tip,fork]:
                ellipsoid(m,q,[.049,.06,.039],'ochre' if i%2 else 'copper',5,3)
    elif slug=='petrified_rubble':
        for i in range(10):
            a=i*2.39996;r=.10+(i%4)*.079
            rock(.5+math.cos(a)*r,.062,.50+math.sin(a)*r,.067+(i%3)*.04,.08+(i%3)*.024,'stone' if i%3 else 'facet')
    elif slug=='excavation_crate':
        m.box([.1,.02,.19],[.79,.40,.84],'cut')
        for x in [.11,.72]:
            for z in [.19,.78]:
                m.box([x,.013,z],[x+.06,.47,z+.06],'iron')
        for y in [.05,.34]:
            m.box([.08,y,.17],[.81,y+.058,.86],'amberwood')
        for j in range(5):
            rock(.22+j*.112,.445,.53+.12*math.sin(j*2),.072,.068,'amethyst' if j%2 else 'grain')
        rod([.76,.04,.14],[.91,.78,.31],.021,.018,'ironwood')
        m.box([.716,.018,.103],[.831,.16,.16],'iron')
    elif slug=='excavation_awning':
        # Tension holds the selvage up while the woven field sags. A folded
        # return on the underside keeps the canvas readable from the trench.
        def cloth(x,z):
            arch=math.sin(math.pi*x)*math.sin(math.pi*z)
            crease=.014*math.sin(math.pi*4*x)*math.sin(math.pi*z)
            return [x,.166-.125*arch+crease,z]
        for row in range(5):
            for col in range(5):
                x,z=col/5,row/5
                corners=[cloth(x,z),cloth(x,z+.2),cloth(x+.2,z+.2),cloth(x+.2,z)]
                m.polygon(corners,'canvas')
                m.polygon([[u,v-.017,w] for u,v,w in reversed(corners)],'grain')
        for x in [.015,.978]:
            for j in range(5):
                a=cloth(x,.009+.982*j*.2);b=cloth(x,.009+.982*(j+1)*.2)
                rod(a,b,.009,.009,'cut',5)
        for z in [.012,.988]:
            rod([.035,.15,z],[.96,.15,z],.012,.012,'amberwood',5)
    elif slug=='excavation_standard':
        rod([.17,.005,.5],[.17,.97,.5],.035,.026,'ironwood')
        rod([.12,.91,.5],[.85,.91,.5],.03,.028,'amberwood')
        for j in range(8):
            lo=.23+.026*((j*7)%3)
            m.box([.25+j*.069,lo,.46],[.321+j*.069,.88,.50],'canvas',.003)
        for i in range(8):
            a=i*math.tau/8
            rod([.53,.55,.45],[.53+.115*math.cos(a),.55+.115*math.sin(a),.45],.012,.009,'cut',4)
    if slug != 'excavation_awning':
        low=min(y for part in m.parts.values() for _,y,_ in part['position'])
        for part in m.parts.values():
            part['position']=[[x,y-low,z] for x,y,z in part['position']]
    if slug in ('excavation_awning', 'excavation_standard', 'excavation_crate'):
        return export(m,slug)
    return export_voxels(m, slug, resolution=16)


def main():
    locale=[
        dict(source='Petrified Forest',en='Petrified Forest',de='Versteinerter Wald'),
        dict(source='Petrified Forest · Warm',en='Petrified Forest · Warm',de='Versteinerter Wald · Warm'),
    ]
    for i,(slug,en,de) in enumerate(CATALOG,703):
        description, translation = DESCRIPTIONS[slug]
        block=dict(schema_version=1,id=i,name=en,description=description,
                   creative_category='nature',mining_level=0,required_tool=None,hardness=1.3,resistance=2.)
        if i<=709 or i in (720,721):
            side_name = {703: 'dusty_ground_side', 705: 'petrified_log_side'}.get(i, slug)
            faces = {'side': surface(side_name)}
            cap = 'side'
            if i in (703, 705):
                cap = 'top' if i == 703 else 'end'
                faces[cap] = surface('dusty_ground' if i == 703 else 'petrified_log_end')
            block.update(render_kind='cube',bevel_width=0,textures=faces,
                         materials={k: {'roughness': .90, 'reflectance': .13,
                                        'normal_texture': path[:-4]+'_normal.png'}
                                    for k, path in faces.items()},
                         face_materials=['side','side','side' if i==703 else cap,cap,'side','side'])
            if i == 707:
                block['surface'] = 'rock'
            if i in (708, 709):
                block['bevel_width'] = .035
        else:
            block.update(render_kind='custom_mesh',model_layout='scene',model='models/'+slug+'.glb',
                         struct_block=True,textures=prop(slug),collision_boxes=[])
            if i==717:
                block['collision_boxes']=[dict(minimum=[.08,.0,.17],maximum=[.82,.47,.86])]
        save_json(BLOCKS/(slug+'.json'),block)
        locale.extend([dict(source=en,en=en,de=de),dict(source=block['description'],en=block['description'],de=translation)])
    save_json(ROOT/'src-ui/locales/source/petrified-forest.json',locale)
    folder=ROOT/'assets/crafting/biomes/petrified_forest'
    folder.mkdir(exist_ok=True)
    save_json(folder/'petrified_planks.json',dict(id='teramic::petrified_planks',name='Petrified Planks',
        station='workbench',output=dict(item=dict(block=706),count=4),
        pattern=dict(kind='shapeless',ingredients=[dict(item=dict(block=705),count=1)])))

if __name__=='__main__':
    main()
