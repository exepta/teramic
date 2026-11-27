"""Build the volcanic province's seamless rock faces and voxel scenery."""
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image

from build_relic_biomes import field_face
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

ROOT = Path(__file__).resolve().parents[2]
BLOCKS_DATA = [
    ("lava", "Molten Lava", "Flüssige Lava", "Viscous molten rock carrying small fragments of crust.", "Zähflüssige Gesteinsschmelze mit kleinen Krustenstücken."),
    ("ash", "Loose Volcanic Ash", "Lose Vulkanasche", "Fine grey cinders on the floor of an old lava valley.", "Feine graue Asche auf dem Grund eines alten Lavatals."),
    ("compacted_ash", "Compacted Ash", "Verdichtete Asche", "Compressed ash in smoky layers.", "Verdichtete Asche in rauchgrauen Schichten."),
    ("scorched_dirt", "Scorched Dirt", "Versengte Erde", "Rust-coloured soil split by the heat.", "Rostbraune, von der Hitze aufgerissene Erde."),
    ("columnar_basalt", "Columnar Basalt", "Säulenbasalt", "Black volcanic rock fractured into pillars.", "Schwarzer Vulkanfels mit säulenförmigen Klüften."),
    ("dark_volcanic_stone", "Dark Volcanic Stone", "Dunkler Vulkanstein", "Dense purple-black rock with a chipped surface.", "Dichtes schwarzviolettes Gestein mit abgesplitterter Oberfläche."),
    ("volcanic_gravel", "Volcanic Gravel", "Vulkankies", "Loose clinker and fragments of cooled basalt.", "Lockere Schlacke und Bruchstücke aus erkaltetem Basalt."),
    ("ember_crystal_ore", "Ember Crystal Cluster", "Glutkristallerz", "A seam of orange crystals enclosed in dark rock.", "Eine Ader aus orangefarbenen Kristallen in dunklem Stein."),
    ("charred_log", "Charred Wood Log", "Verkohlter Stamm", "Blackened bark surrounds a burnt umber heart.", "Schwarze Borke umschließt einen dunkelbraunen Holzkern."),
    ("charred_planks", "Charred Planks", "Kohlholzbretter", "Boards cut from the heart of a charred trunk.", "Bretter aus dem Kern eines verkohlten Stamms."),
    ("lava_crust", "Lava-Crust Rock", "Lavakrustengestein", "Cooling plates still glow along their fractures.", "Zwischen den erkaltenden Platten glühen die Bruchlinien."),
]
SCENERY = [
    ("ash_bush","Dead Ash Bush","Toter Aschebusch","Dry branches tipped with pale ash.","Trockene Zweige mit hellen Aschespitzen."),
    ("burnt_grass","Burnt Grass Tufts","Versengtes Büschelgras","Brittle ochre stalks on burnt ground.","Brüchige ockerfarbene Halme auf verbranntem Boden."),
    ("blackened_stump","Blackened Tree Stump","Verkohlter Baumstumpf","Burnt roots and a ringed stump.","Verkohlte Wurzeln und Jahresringe im Stumpf."),
    ("charred_sapling","Charred Sapling","Verkohlter Setzling","A small burned tree with ember-tipped branches.","Ein kleiner verbrannter Baum mit glimmenden Astspitzen."),
    ("ember_crystals","Ember Crystals","Glutkristalle","Amber prisms rise through the volcanic crust.","Bernsteinfarbene Prismen wachsen durch die Vulkankruste."),
    ("fumarole","Lava Fumarole","Dampfschlot","Scalding steam escapes through this burning vent.","Verbrühend heißer Dampf entweicht aus diesem glühenden Schlot."),
    ("fossil_bones","Fossil Bones","Fossile Knochen","Ribs and a weathered skull preserved in ash.","Rippen und ein verwitterter Schädel, in Asche erhalten."),
    ("ash_mushrooms","Ash Mushrooms","Rußpilze","Dark caps shelter warm orange gills.","Dunkle Hüte schützen orange glühende Lamellen."),
]
PALETTE = {
    'coal':[.18,.14,.17], 'rock':[.29,.24,.28], 'stone':[.40,.34,.36], 'ash':[.60,.52,.52],
    'wood':[.24,.15,.13], 'ring':[.39,.24,.18], 'grass':[.54,.44,.50],
    'ember':[.95,.20,.025], 'gold':[1.,.55,.04], 'crystal':[.90,.28,.035],
    'tip':[1.,.65,.18], 'bone':[.73,.61,.49], 'ivory':[.87,.77,.61],
    'garnet':[.44,.055,.038], 'amber':[.79,.20,.04], 'plum':[.46,.16,.50], 'amethyst':[.68,.35,.66], 'hot_facet':[.76,.14,.018],
}


def stone_fields(x,y,rng,columns,rows):
    n=x.shape[0]
    sites=rng.uniform(.17,.83,(rows,columns,2))
    d=[]
    for j,i in np.ndindex(rows,columns):
        cx,cy=(sites[j,i]+[i,j])*[n/columns,n/rows]
        dx=((x-cx+n/2)%n-n/2)*columns/n
        dy=((y-cy+n/2)%n-n/2)*rows/n
        d.append(dx*dx+dy*dy)
    d=np.array(d)
    pair=np.sqrt(np.partition(d,1,axis=0)[:2])
    return d.argmin(axis=0),pair[1]-pair[0]


def surface(i,face):
    n=128
    rng=np.random.default_rng(102491+i*11+(face=='end'))
    y,x=np.mgrid[:n,:n]
    phase=math.tau/n
    s=np.sin
    ripple=s(x*phase*2 + .8*s(y*phase))+s(y*phase*3+x*phase)+.65*s(y*phase-x*phase*2)
    fine=rng.normal(0,2.1,(n,n)); large=4*ripple
    colors=[[237,104,30],[143,130,132],[105,94,100],[117,78,69],[106,95,108],
            [105,91,103],[104,91,92],[77,66,74],[54,41,43],[73,53,48],[87,70,76]]
    rgb=np.broadcast_to(colors[i],(n,n,3)).astype(float).copy()
    xx=x+3*s(y*phase*2)+1.7*s((x+y)*phase*3)
    yy=y+2*s(x*phase*2)+s((x-y)*phase*5)
    cols,rows={0:(4,5),1:(8,9),2:(5,6),3:(5,4),4:(6,1 if face=='side' else 6),
               5:(4,5),6:(10,11),7:(5,5),10:(4,3)}.get(i,(4,4))
    cells,joint=stone_fields(xx,yy,rng,cols,rows)
    relief=8*np.tanh(joint*6)+large
    if i==0:
        phase_flow=x*phase+1.2*s(y*phase)+.4*s((2*x+y)*phase)
        swirls=s(phase_flow*3+y*phase)+.55*s(phase_flow*6-y*phase)
        rgb[:,:,1]=103+swirls*12+6*s((x+y)*phase)
        rgb[:,:,2]=24+np.maximum(0,swirls)*7
        clinker=(joint>.49)&(cells%7==0)&(swirls<-.3)
        rgb[clinker]=[130,61,45]; relief=large*.32+swirls
    elif i in (8,9):
        if i==8 and face=='end':
            dist=np.hypot(((x+1.6*s(y*phase))%n)-n/2,((y+1.4*s(x*phase))%n)-n/2)
            rings=s(dist*.61)+.25*s(dist*1.2)
            rgb[:]=[100,66,50]; relief=11*rings+large*.45
            relief[dist>55]-=22
        elif i==8:
            relief=13*s(xx*phase*9)+6*s(xx*phase*18)+large
            relief[(joint<.05)&(cells%3==0)]-=18
        else:
            relief=5*s(yy*phase*12)+large*.45
            joints=(y%32<=1)|((x+np.floor(y/32)*37)%128<2)
            relief[joints]-=24
    elif i==1:
        relief=large+np.where((joint<.13)&(cells%5==0),-8,0)+4*s((x*.4+y)*phase*6)
    else:
        # Eroded rock planes meet under layers of windblown ash. Some joints
        # disappear completely and the surviving edges have rounded shoulders.
        relief=large*.56 + np.choose(cells%4,[-4,0,5,2]) + 2*np.tanh(joint*5)
        joint_exposure=np.clip(.38 + ripple*.21 + (cells%3)*.10,0,1)
        relief-=10.5*np.exp(-joint*joint*135)*joint_exposure
        if i==2: relief+=1.6*s(yy*phase*9)
        if i in (7,10):
            molten=(joint<(.061 if i==10 else .12)) & ((cells%4!=0) if i==10 else (cells%3==0)) & (joint_exposure>.52)
            rgb[molten]=[182,78,37]
            rgb[molten&(joint<.026)]=[230,118,47]
    out=BLOCKS/'textures/ashlands'; out.mkdir(parents=True,exist_ok=True)
    slug=BLOCKS_DATA[i][0]+'_'+face
    pixels=np.clip(rgb+(relief+fine)[:,:,None],0,255).astype('uint8')
    if i in (1,8) and face=='side':
        pixels=field_face(2 if i==1 else 3)
        relief=(pixels @ np.array([.2126,.7152,.0722]))*.5
    Image.fromarray(pixels).resize((256,256),Image.Resampling.NEAREST).save(out/(slug+'.png'),optimize=True)
    strength=.020 if i in (8,9) else .012
    normal=np.stack([(np.roll(relief,1,1)-np.roll(relief,-1,1))*strength,
                     (np.roll(relief,1,0)-np.roll(relief,-1,0))*strength,
                     np.ones_like(relief)],axis=-1)
    normal/=np.linalg.norm(normal,axis=2,keepdims=True)
    Image.fromarray(np.clip((normal*.5+.5)*255,0,255).astype('uint8')).resize((256,256),Image.Resampling.NEAREST).save(out/(slug+'_normal.png'),optimize=True)
    return 'textures/ashlands/'+slug


def fumarole_texture():
    # Fractured basalt slopes into a dark central throat. A few narrow cracks
    # expose molten rock at the lip of the opening.
    n=128
    y,x=np.mgrid[:n,:n]
    rng=np.random.default_rng(17629)
    dx,dz=(x-65)/64,(y-62)/64
    angle=np.arctan2(dz,dx)
    r=np.maximum(abs(dx),abs(dz))+.058*np.sin(angle*5)+.027*np.sin(angle*9)
    cells,gap=stone_fields(x,y,rng,6,6)
    value=59+5*(cells%4)+rng.normal(0,2,(n,n)) - 18*np.exp(-gap*20)
    value+=20*np.exp(-((r-.45)/.08)**2)
    rgb=np.stack((value*1.05,value*.91,value),axis=2)
    rgb[r<.32]*=.36
    lit=((r>.25)&(r<.40)&(gap<.085))|((r>.12)&(r<.28)&(cells%3==0))
    rgb[lit]=[216,77,23]
    rgb[lit&(gap<.035)]=[255,162,51]
    rgb[r<.12]=[22,14,17]
    path=BLOCKS/'textures/ashlands/fumarole_top.png'
    Image.fromarray(np.uint8(np.clip(rgb,0,255))).resize((256,256),Image.Resampling.NEAREST).save(path)
    return 'textures/ashlands/fumarole_top.png'


def model(slug):
    m=Mesh('stone')
    for k,v in PALETTE.items(): m.materials[k]=(v,None,0,.45 if k in ('crystal','tip') else .85)
    def prism(a,b,r,key,top,sides=6): cylinder(m,a,b,r,key,sides,end_radius=top)
    if slug in ('ash_bush','charred_sapling','burnt_grass'):
        twig=slug!='burnt_grass'; tree=slug=='charred_sapling'
        count=7 if twig else 17
        for i in range(count):
            a=i*2.39996; dx,dz=math.cos(a),math.sin(a)
            root=[.5+dx*(.02 if twig else .12),0,.5+dz*(.02 if twig else .12)]
            base=.10 if tree else .02
            knee=[.51+dx*.12,.25+base*(i%3),.50+dz*.12]
            end=[.51+dx*(.28 if tree else .33),.56+(i%4)*.095,.50+dz*.32]
            prism(root,knee,.052 if twig else .018,'coal',.032 if twig else .014,5)
            prism(knee,end,.035 if twig else .014,'wood' if twig else 'grass',.017 if twig else .003,5)
            if twig:
                tip=[end[0]+dx*.04,end[1]+.08,end[2]+dz*.04]
                prism(end,tip,.020,'ember' if tree else 'ash',.010,4)
                fork=[end[0]-dz*.11,end[1]-.06,end[2]+dx*.11]
                prism(knee,fork,.019,'wood',.007,4)
        if tree: prism([.5,0,.5],[.51,.78,.49],.083,'coal',.032)
    elif slug=='blackened_stump':
        for i in range(7):
            a=i*math.tau/7
            prism([.5+math.cos(a)*.17,.28,.5+math.sin(a)*.17],
                  [.5+math.cos(a)*.45,.022,.5+math.sin(a)*.45],.09,'coal',.027,5)
        prism([.50,.025,.5],[.48,.60,.5],.29,'wood',.25,10)
        for i in range(10):
            a=i*math.tau/10
            prism([.50+math.cos(a)*.255,.05,.50+math.sin(a)*.255],
                  [.48+math.cos(a)*.224,.59+(i%3)*.014,.50+math.sin(a)*.224],.055,'coal' if i%3 else 'rock',.045,5)
        for n,key in enumerate(['ring','wood','ring','wood','ring']):
            prism([.48,.603+n*.002,.50],[.48,.606+n*.002,.50],.219-n*.04,key,.219-n*.04,10)
    elif slug=='ember_crystals':
        for i,(dx,dz,h,r) in enumerate([(0,0,.96,.16),(-.22,-.09,.55,.095),(.14,.19,.63,.11),(.20,-.12,.40,.09),(-.18,.18,.33,.10)]):
            # Each facet has a hot foot and a dark termination. Shard axes
            # fan out so the silhouette reads as a cluster from either bank.
            angles=[math.tau*k/5+i*.48 for k in range(5)]
            rings=[]
            for t,scale in [(0.,.70),(.18,1.),(.71,.86)]:
                reach=.70+t*.76
                rings.append([[.50+dx*reach+r*scale*math.cos(a), .015+h*t,
                               .50+dz*reach+r*scale*math.sin(a)] for a in angles])
            tip=[.50+dx*1.50,h,.50+dz*1.50]
            for k in range(5):
                j=(k+1)%5
                base='hot_facet' if (i+k)%3 else 'ember'
                middle=['amber','garnet','hot_facet','amber','crystal'][(i+k)%5]
                cap=['garnet','amber','crystal','garnet','hot_facet'][(i+k)%5]
                if i%4==1: base,middle,cap='plum','amethyst','plum'
                for low,high,material in [(rings[0],rings[1],base),(rings[1],rings[2],middle)]:
                    m.polygon([low[j],low[k],high[k],high[j]],material)
                m.polygon([rings[2][j],rings[2][k],tip],cap)
            m.polygon(rings[0], 'garnet', [0,-1,0])
        for i in range(9):
            a=i*2.39996
            ellipsoid(m,[.5+math.cos(a)*.30,.055,.5+math.sin(a)*.30],[.10,.055,.085],'rock',6,3)
    elif slug=='fossil_bones':
        prism([.13,.075,.50],[.77,.10,.50],.055,'bone',.052)
        for i in range(4):
            for direction in (-1,1):
                x=.20+i*.115
                pts=[[x,.08,.50],[x-.02,.17,.50+direction*.17],[x-.012,.31,.50+direction*.18],[x+.026,.42,.50+direction*.09]]
                for j,(a,b) in enumerate(zip(pts,pts[1:])): prism(a,b,.040-j*.005,'bone' if j==0 else 'ivory',.037-j*.007,5)
        ellipsoid(m,[.79,.16,.51],[.14,.12,.13],'ivory',8,4)
        ellipsoid(m,[.89,.10,.51],[.085,.065,.08],'bone',6,3)
        for sign in (-1,1):
            ellipsoid(m,[.82,.17,.50+sign*.111],[.047,.046,.018],'coal',5,3)
    else:
        for i,(x,z,h,r) in enumerate([(.34,.43,.65,.22),(.69,.61,.43,.17),(.64,.25,.28,.13)]):
            prism([x,.01,z],[x+.025,h,z],r*.20,'ember',r*.15)
            ellipsoid(m,[x,h,z],[r,r*.28,r],'gold',9,4)
            ellipsoid(m,[x,h+r*.16,z],[r,r*.47,r*.96],'garnet' if i%2 else 'amber',9,4)
            for j in range(5):
                a=j*math.tau/5
                ellipsoid(m,[x+math.cos(a)*r*.56,h+r*.51,z+math.sin(a)*r*.56],[r*.13,r*.055,r*.12],'ember',4,3)
    # Slanted prisms extend below their endpoint by part of the radius. Seat
    # every prop on the actual ground plane before baking its shared geometry.
    lowest = min(y for part in m.parts.values() for _,y,_ in part["position"])
    for part in m.parts.values():
        part["position"] = [[x,y-lowest,z] for x,y,z in part["position"]]
    return export_voxels(m,slug,('gold','ember','tip','hot_facet'))


def build():
    locale=[dict(source='Ashlands',en='Ashlands',de='Aschelande'),
            dict(source='Lava Bucket',en='Lava Bucket',de='Lavaeimer'),
            dict(source='A heatproof bucket carrying one source of molten rock.',en='A heatproof bucket carrying one source of molten rock.',de='Ein hitzefester Eimer mit einer Quelle geschmolzenen Gesteins.')]
    for i,(slug,en,de,description,translation) in enumerate(BLOCKS_DATA+SCENERY):
        ident=i+613
        block=dict(schema_version=1,id=ident,name=en,description=description,creative_category='nature',
                   mining_level=0,required_tool=None,hardness=1.25,resistance=2.0)
        if i<len(BLOCKS_DATA):
            faces=['side','end'] if i in (4,8) else ['side']
            paths={f:surface(i,f) for f in faces}
            mats={f:dict(roughness=.61 if i==0 else .84,normal_texture=p+'_normal.png') for f,p in paths.items()}
            if i in (0,7,10):
                for mat in mats.values(): mat['emission']={0:[1.05,.15,.009],7:[.10,.018,.002],10:[.14,.025,.004]}[i]
            end=faces[-1]
            block.update(render_kind='cube',bevel_width=0,materials=mats,textures={f:p+'.png' for f,p in paths.items()},
                         face_materials=['side','side',end,end,'side','side'])
            # The generic inset rock geometry stamps rectangular slots across
            # every connected face. Volcanic strata use their joint normal map.
        elif slug=='fumarole':
            block.update(render_kind='cube',bevel_width=0,
                         textures=dict(side='textures/ashlands/dark_volcanic_stone_side.png',top=fumarole_texture()),
                         materials=dict(side=dict(roughness=.89,normal_texture='textures/ashlands/dark_volcanic_stone_side_normal.png'),
                                        top=dict(roughness=.81)),
                         face_materials=['side','side','side','top','side','side'],
                         light=dict(color=[1.,.28,.06],intensity=85,range=4.,shadows=False))
        else:
            block.update(render_kind='custom_mesh',model_layout='scene',model='models/'+slug+'.glb',
                         struct_block=True,textures=model(slug),collision_boxes=[])
            if slug=='blackened_stump':
                block['collision_boxes']=[dict(minimum=[.23,0,.23],maximum=[.77,.59,.77])]
            if slug=='ember_crystals':
                block['light']=dict(color=[1.,.28,.06],intensity=85,range=4.,shadows=False)
        save_json(BLOCKS/(slug+'.json'),block)
        for src,tr in [(en,de),(description,translation)]:
            locale.append(dict(source=src,en=src,de=tr))
    save_json(ROOT/'src-ui/locales/source/ashlands.json',locale)
    recipe=dict(id='teramic::charred_planks',name='Charred Planks',station='workbench',
                output=dict(item=dict(block=622),count=4),pattern=dict(kind='shapeless',ingredients=[dict(item=dict(block=621),count=1)]))
    folder=ROOT/'assets/crafting/biomes/ashlands'; folder.mkdir(parents=True,exist_ok=True)
    save_json(folder/'charred_planks.json',recipe)


if __name__=='__main__':
    build()
