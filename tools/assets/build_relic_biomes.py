"""Author volcanic and fossil grove scenery. Stone faces use the painted PNG masters."""
import math
from pathlib import Path

import numpy as np
from PIL import Image
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from build_spirit_blossom import export
from voxel_props.geometry import VoxelSculpt
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

ROOT = Path(__file__).resolve().parents[2]
CATALOG = [
    ('fossiliferous_stone','Fossil-Streaked Stone','Fossilienstein','An ammonite lies embedded in the ancient seabed.','Ein Ammonit liegt im alten Meeresboden.'),
    ('turquoise_veined_stone','Cracked Mineral Stone','Rissiger Mineralstein','Turquoise crystals thread the limestone fractures.','Türkise Kristalle durchziehen die Kalksteinklüfte.'),
    ('root_bound_stone','Root-Bound Stone','Durchwurzelter Stein','Ropes of fossil wood bind the stone.','Stränge aus fossilem Holz umschließen den Stein.'),
    ('temple_architrave','Inlaid Temple Frieze','Eingelegter Tempelfries','A turquoise and brass frieze crowns a weathered archway.','Ein Fries aus Türkis und Messing krönt den verwitterten Bogen.'),
    ('scoria_bricks','Ruined Basalt Masonry','Verwittertes Basaltmauerwerk','Ancient masons dressed the cinderstone into dark courses.','Steinmetze schichteten den Schlackenstein zu dunklen Mauern.'),
    ('pyric_rune_stone','Ember Rune Stone','Glutrunenstein','Two burning diamonds mark the path to a fire shrine.','Zwei glühende Rauten markieren den Weg zum Feuerschrein.'),
    ('volcanic_obsidian','Volcanic Obsidian','Vulkanischer Obsidian','Violet light catches the sharp fractures in this volcanic glass.','Violettes Licht bricht sich in den scharfkantigen Spalten des Vulkanglases.'),
    ('geyser_vent','Geyser Vent','Geysirmündung','Mineral rings surround a hot spring that periodically erupts.','Mineralringe umgeben eine heiße Quelle, die schubweise ausbricht.'),
    ('ember_bloom','Ember Flowers','Glutblüten','Clusters of orange flowers grow in warm ash.','Büschel orangefarbener Blüten wachsen in warmer Asche.'),
    ('fallen_charred_log','Fallen Burnt Log','Gestürzter Brandstamm','An ember seam glows beneath the cracked bark.','Eine Glutader leuchtet unter der geborstenen Rinde.'),
    ('cinder_root','Dead Burnt Roots','Verkohlte Wurzeln','A knot of roots exposed in a lava valley.','Ein Geflecht freigelegter Wurzeln in einem Lavatal.'),
    ('mineral_strata','Layered Mineral','Gebänderter Mineralbrocken','Alternating belts of turquoise and amber cross the broken slab.','Türkise und bernsteinfarbene Bänder durchziehen die gebrochene Platte.'),
    ('archaeology_urn','Excavation Urn','Grabungsurne','A painted field jar with a brush for cleaning fossils.','Ein bemaltes Feldgefäß mit einem Pinsel zum Freilegen der Fossilien.'),
    ('ashen_brazier','Basalt Brazier','Basaltfeuerschale','Golden fire burns above a carved basalt pedestal.','Goldene Flammen brennen auf einem behauenen Basaltsockel.'),
    ('temple_pennant','Temple Pennant','Tempelwimpel','An old mineral-dyed banner carries the tree of the sanctuary.','Ein mit Mineralfarben gefärbtes Banner trägt den Baum des Heiligtums.'),
]
PALETTE={'stone':(.36,.31,.36),'obsidian':(.19,.14,.21),'bark':(.24,.15,.14),'ash':(.59,.50,.55),
         'ember':(.98,.18,.025),'flame':(1.,.65,.08),'garnet':(.67,.09,.12),'copper':(.66,.41,.19),
         'turquoise':(.19,.69,.69),'glass':(.54,.88,.84),'gold':(.91,.70,.34),'sand':(.75,.66,.55),'ochre':(.62,.32,.17),
         'teal':(.12,.41,.41),'linen':(.88,.80,.65)}

def field_face(index):
    atlas = Image.open(Path(__file__).with_name('relics') / 'field-faces.jpg')
    x, y = index % 2 * 256, index // 2 * 256
    pixels = np.asarray(atlas.crop((x, y, x + 256, y + 256)).convert('RGB'), dtype=float)
    luma = pixels @ np.array([.213,.715,.072])
    # The mineralized surfaces reflect neutral sky light. Warm iron and cool
    # silica remain in the fractures without turning the entire grove ochre.
    target = np.array([(180,177,176),(174,172,162),(133,116,130),(69,57,66)][index])
    tone = (luma-luma.mean()) * (1.30 if index==1 else 1.04)
    chroma = pixels-luma[:,:,None]
    result=target + tone[:,:,None] + (chroma-chroma.mean((0,1)))*(.59 if index<3 else 1.28)
    return np.uint8(np.clip(result,0,255))

def scene(slug):
    m=Mesh('stone')
    for k,c in PALETTE.items():m.materials[k]=(list(c),None,0,.72 if k in ('glass','gold') else .89)
    def rod(a,b,r,q,col,n=7): cylinder(m,a,b,r,col,n,end_radius=q)
    def oval(center,r,col):ellipsoid(m,center,r,col,8,4)
    if slug=='ember_bloom':
        for i in range(5):
            a=i*2.39; x=.5+math.cos(a)*.31; z=.5+math.sin(a)*.31; y=.44+.063*(i%4)
            rod([.48,.01,.5],[x,y,z],.029,.016,'bark')
            for j in range(5):
                b=j*math.tau/5+a;dx=math.cos(b);dz=math.sin(b)
                oval([x+dx*.057,y+.028,z+dz*.057],[.061,.046,.056],'garnet')
                rod([x+dx*.055,y+.039,z+dz*.055],[x+dx*.091,y+.073,z+dz*.091],.038,.012,'ember',5)
            oval([x,y+.066,z],[.044,.036,.044],'flame')
    elif slug=='fallen_charred_log':
        rod([.12,.21,.43],[.85,.28,.61],.18,.143,'bark',10)
        for i in range(9):
            a=i*math.tau/9;dy=math.sin(a);dz=math.cos(a)
            rod([.15,.21+dy*.16,.43+dz*.16],[.86,.28+dy*.14,.61+dz*.14],.039,.031,'obsidian' if i!=2 else 'ember',5)
        rod([.43,.24,.47],[.54,.52,.32],.067,.024,'obsidian')
    elif slug=='cinder_root':
        root = VoxelSculpt(16)
        knot = (.44,.20,.46)
        for knee, toe, end in [
            ((.25,.14,.40),(.17,.065,.26),(.055,.025,.21)),
            ((.57,.12,.26),(.66,.052,.20),(.65,.018,.065)),
            ((.63,.15,.58),(.72,.065,.74),(.87,.022,.81)),
            ((.33,.13,.62),(.20,.059,.73),(.19,.021,.89)),
        ]:
            root.branch(knot,knee,.10,'bark',.074)
            root.branch(knee,toe,.070,'obsidian',.048)
            root.branch(toe,end,.048,'obsidian',.023)
        root.branch(knot,(.35,.37,.48),.10,'bark',.062)
        # A live coal sits inside the splintered end of the burnt wood.
        root.cells[5,5,7] = 'ember'
        root.emit(m)
        return export(m,slug,('ember',))
    elif slug=='mineral_strata':
        # Broad tilted seams terminate in a chipped, asymmetric slab.
        rock = VoxelSculpt(16)
        bands = ['stone', 'gold', 'turquoise', 'turquoise', 'glass', 'ochre', 'sand', 'sand']
        for y in range(8):
            for x in range(2, 14):
                for z in range(3, 13):
                    if x + z < 7 + y//3 or x - z > 8 - y//5 or x + z > 23 - y//4:
                        continue
                    top = 7 - (x < 4) * 2 - (z > 10) - (x + z > 18)
                    if y <= top:
                        band = min(len(bands)-1, max(0, y + (x-6)//6))
                        rock.cells[x, y, z] = bands[band]
        rock.emit(m)
        return export(m,slug)
    elif slug=='archaeology_urn':
        for y1,y2,r,q,k in [(0,.10,.14,.17,'ochre'),(.1,.25,.17,.25,'sand'),(.25,.44,.25,.238,'sand'),(.44,.50,.238,.17,'ochre'),(.5,.57,.17,.12,'sand'),(.57,.62,.12,.16,'ochre')]:
            rod([.50,y1,.48],[.50,y2,.48],r,q,k,12)
        for y in [.21,.29,.40]:
            rod([.50,y,.48],[.50,y+.018,.48],.244,.245,'teal' if y==.29 else 'copper',12)
        rod([.19,.02,.71],[.73,.16,.79],.019,.016,'bark');rod([.12,.018,.70],[.20,.035,.71],.047,.040,'linen')
    elif slug=='ashen_brazier':
        m.box([.15,0,.15],[.85,.13,.85],'stone');m.box([.29,.13,.29],[.71,.49,.71],'obsidian')
        for y in [.14,.39]:m.box([.255,y,.255],[.745,y+.047,.745],'copper')
        rod([.50,.47,.50],[.50,.61,.50],.21,.42,'stone',8)
        rod([.50,.61,.50],[.50,.648,.50],.42,.416,'gold',8)
        for i in range(8):
            a=i*2.4;dx,dz=math.cos(a),math.sin(a);h=.81+.15*(i%3)/2
            rod([.5+dx*.15,.63,.5+dz*.15],[.5+dx*.19-.028,h,.5+dz*.19],.11,.008,'ember' if i%3 else 'flame',5)
    elif slug=='temple_pennant':
        # Twin iron brackets cantilever from the stone jamb behind the cloth.
        for x in (.18,.82):
            m.box([x-.048,.83,0.],[x+.048,.98,.045],'stone')
            rod([x,.87,.03],[x,.94,.5],.027,.022,'copper')
        rod([.03,.94,.50],[.97,.94,.5],.026,.026,'gold')
        for j in range(12):
            x=.09+j*.068; low=.06+.19*abs(2*(j+.5)/12-1)
            z=.50+.024*math.sin(j*.67)
            m.box([x,low,z],[x+.07,.935,z+.035],'teal')
            m.box([x,low,z-.002],[x+.07,low+.06,z+.037],'gold')
        for z in [.464,.569]:
            rod([.5,.31,z],[.5,.76,z],.019,.015,'gold')
            for i in range(3):
                for q in [-1,1]:
                    rod([.5,.40+i*.10,z],[.5+q*(.16-i*.038),.55+i*.09,z],.018,.012,'gold',5)
    # All floor props rest on an actual supporting cell.
    if slug!='temple_pennant':
        low=min(y for part in m.parts.values() for _,y,_ in part['position'])
        for part in m.parts.values():part['position']=[[x,y-low,z] for x,y,z in part['position']]
    if slug == 'temple_pennant':
        return export(m,slug)
    return export_voxels(m, slug, ('ember','flame'), resolution=16)

def weathered_masonry():
    master = Path(__file__).with_name('relics') / 'scoria-masonry.png'
    face = np.asarray(Image.open(master).convert('RGB'), dtype=float)
    value = face @ np.array([.213, .715, .072])
    # Exposed ash-grey faces catch diffuse light while the recessed mortar
    # keeps its depth and the iron-rich fractures retain their red tint.
    relief = (value - value.mean()) * 1.47
    pigment = face - value[:, :, None]
    color = np.array([120, 112, 120]) + relief[:, :, None]
    color += (pigment - pigment.mean((0, 1))) * .68
    return Image.fromarray(np.uint8(np.clip(color, 0, 255)))

def build():
    folder=BLOCKS/'textures/relics'
    weathered_masonry().save(folder/'scoria_bricks.png', optimize=True)
    locale=[]
    for i,(slug,name,de,desc,de_desc) in enumerate(CATALOG,724):
        block=dict(schema_version=1,id=i,name=name,description=desc,creative_category='nature',mining_level=0,required_tool=None,hardness=1.4,resistance=3.)
        if i<=731:
            tex='textures/relics/'+slug+'.png'
            arr=np.asarray(Image.open(BLOCKS/tex).convert('L'),float)/255.
            normals=np.stack([(np.roll(arr,1,a)-np.roll(arr,-1,a))*1.8 for a in [1,0]]+[np.ones_like(arr)],axis=-1)
            normals/=np.linalg.norm(normals,axis=-1,keepdims=True)
            Image.fromarray(np.uint8((normals*.5+.5)*255)).save(folder/(slug+'_normal.png'),optimize=True)
            side=dict(roughness=.28 if i==730 else .88,reflectance=.28 if i==730 else .13,normal_texture=tex.replace('.png','_normal.png'))
            block.update(render_kind='cube',bevel_width=.018 if i in (727,728,729) else 0,textures=dict(side=tex),materials=dict(side=side),face_materials=['side']*6)
            if i in (729,731):
                block['textures']['sidewall']='textures/relics/scoria_bricks.png'
                block['materials']['sidewall']=dict(roughness=.87,normal_texture='textures/relics/scoria_bricks_normal.png')
                if i==731:block['face_materials']=['sidewall','sidewall','sidewall','side','sidewall','sidewall']
                else:block['face_materials']=['side','side','sidewall','sidewall','side','side']
        else:
            block.update(render_kind='custom_mesh',model_layout='scene',model='models/'+slug+'.glb',struct_block=True,textures=scene(slug),collision_boxes=[])
        if i in (729,737):
            block['light']=dict(color=[1.,.48,.14],intensity=680 if i==737 else 96,range=11 if i==737 else 4.,falloff='soft_core',flicker=.025,shadows=False)
        save_json(BLOCKS/(slug+'.json'),block)
        locale.extend([dict(source=name,en=name,de=de),dict(source=desc,en=desc,de=de_desc)])
    save_json(ROOT/'src-ui/locales/source/relic-biomes.json',locale)

if __name__=='__main__':build()
