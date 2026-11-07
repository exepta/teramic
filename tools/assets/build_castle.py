#!/usr/bin/env python3
"""Deterministic authored castle meshes, mechanics and crafting definitions.

All dimensions are meters in one voxel. Collision conservatively approximates the solid structure; curved arches
have their own continuous surface. Other details use shared structural boxes; stone joints/bevels, foliage, rivets and heraldry are detail.
Uses existing project textures; no external art/model dependencies.
"""
from pathlib import Path
import json
import math
import struct
import castle_details as details

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'
MODELS = BLOCKS / 'models'
RECIPES = ROOT / 'assets/crafting'
DEFINITIONS = [
    (173, 'bergfried', 'Bergfried', 'keep', 'royal'),
    (174, 'castle_stone_wall', 'Steinmauer', 'wall', 'stone'),
    (175, 'bergfried_wall', 'Bergfried Mauer', 'crown', 'stone'),
    (176, 'castle_cracked_wall', 'Rissige Mauer', 'wall', 'cracked'),
    (177, 'castle_overgrown_wall', 'Bewachsene Mauer', 'wall', 'overgrown'),
    (178, 'castle_moss_wall', 'Moos Mauer', 'wall', 'moss'),
    (179, 'castle_iron_wall', 'Verstaerkte Mauer', 'wall', 'iron'),
    (180, 'castle_battlement_straight', 'Zinnenmauer', 'straight', 'stone'),
    (181, 'castle_battlement_corner', 'Zinnen Ecke', 'corner', 'stone'),
    (182, 'castle_battlement_pillar', 'Zinnen Saeule', 'pillar', 'stone'),
    (183, 'castle_arch_left', 'Mauerbogen Links', 'arch_left', 'stone'),
    (184, 'castle_arch_right', 'Mauerbogen Rechts', 'arch_right', 'stone'),
    (185, 'castle_arrow_slit', 'Mauer mit Schiessscharte', 'slit', 'stone'),
    (186, 'castle_window', 'Mauer mit Fenster', 'window', 'stone'),
    (187, 'castle_sandstone_wall', 'Sandstein Mauer', 'wall', 'sandstone'),
    (188, 'castle_desert_wall', 'Wuestenmauer mit Muster', 'wall', 'desert'),
    (189, 'castle_snow_wall', 'Schneemauer', 'wall', 'snow'),
    (190, 'castle_blackstone_wall', 'Schwarzer Stein', 'wall', 'blackstone'),
    (191, 'castle_royal_wall', 'Koenigsmauer mit Wappen', 'wall', 'royal'),
    (192, 'castle_timber_wall', 'Holzverstaerkte Mauer', 'wall', 'timber'),
]

def unit(v):
    length = math.sqrt(sum(x*x for x in v))
    return [x/length for x in v]

def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]

class Mesh:
    def __init__(self, finish):
        self.finish = finish
        self.parts = {}
        self.collision = []
        self.materials = {}
        for index, factor in enumerate([.93,.97,1.0,.985]):
            # The game binds its ordinary procedural Stone/Sandstone palette.
            self.materials[f'stone_{index}'] = ([factor]*3, None, 0, .99)
        self.materials['arch_join'] = ([.99]*3, None, 0, .99)
        self.materials['heraldry_cloth'] = ([1,1,1], None, 0, .95)
        for icon in details.ICONS:
            self.materials['heraldry_icon_'+icon] = ([1,1,1], None, .12, .72)
        self.materials.update({
            'mortar': ([.23,.23,.22], None, 0, 1),
            'crack': ([.12,.115,.105], None, 0, 1),
            'iron': ([0.37,0.40,0.44], 'machinery-iron-hd.png', .82, .48),
            'rivet': ([0.69,0.72,0.74], 'machinery-iron-hd.png', .8, .4),
            'oak': ([0.72,0.44,0.20], 'machinery-oak-hd.png', 0, .88),
            'gold': ([0.97,0.66,0.15], None, .7, .37),
            'blue': ([0.04,0.17,0.52], None, 0, .93),
            'ivy': ([.13,.27,.045], None, 0, .95),
            'leaf': ([.26,.43,.075], None, 0, .94),
            'leaf_light': ([.38,.52,.13], None, 0, .95),
            'moss': ([.32,.40,.10], None, 0, 1),
            'snow': ([0.93,0.97,1], None, 0, .9),
            # Match the existing wooden windows rather than an opaque amber plate.
            'glass': ([.68,.85,.91,.12], None, 0, .14),
        })

    def polygon(self, points, material, normal=None):
        a,b,c=points[:3]
        calculated=cross([b[i]-a[i] for i in range(3)], [c[i]-a[i] for i in range(3)])
        if sum(x*x for x in calculated)<1e-16: return
        if normal is None: normal=unit(calculated)
        elif sum(calculated[i]*normal[i] for i in range(3))<0: points=list(reversed(points))
        group=self.parts.setdefault(material, {'position':[], 'normal':[], 'uv':[], 'tangent':[], 'indices':[]})
        offset=len(group['position'])
        tangent=unit(cross([0,1,0] if abs(normal[1])<.9 else [0,0,1], normal))
        other=cross(normal,tangent)
        for point in points:
            group['position'].append(point)
            group['normal'].append(normal)
            group['uv'].append([sum(point[i]*tangent[i] for i in range(3)),sum(point[i]*other[i] for i in range(3))])
            group['tangent'].append(tangent+[1])
        for index in range(1,len(points)-1): group['indices'] += [offset,offset+index,offset+index+1]

    def box(self, low, high, material, bevel=.014, solid=False):
        if solid: self.collision.append({'minimum':low,'maximum':high})
        center=[(low[i]+high[i])/2 for i in range(3)]
        h=[(high[i]-low[i])/2 for i in range(3)]
        if min(h)<=0: raise ValueError((low,high))
        b=min(bevel,min(h)*.22)
        def emit(points,n): self.polygon([[center[i]+p[i] for i in range(3)] for p in points],material,n)
        # Six inset faces, twelve bevel strips, eight triangular corner caps.
        for axis in range(3):
            u,v=(axis+1)%3,(axis+2)%3
            for sign in [-1,1]:
                points=[]
                for a,c in [(-1,-1),(1,-1),(1,1),(-1,1)]:
                    p=[0.,0.,0.];p[axis]=sign*h[axis];p[u]=a*(h[u]-b);p[v]=c*(h[v]-b);points.append(p)
                n=[0.,0.,0.];n[axis]=sign;emit(points,n)
        if b==0: return
        for along in range(3):
            u,v=(along+1)%3,(along+2)%3
            for su in [-1,1]:
                for sv in [-1,1]:
                    points=[]
                    for sa,outer in [(-1,0),(1,0),(1,1),(-1,1)]:
                        p=[0.,0.,0.];p[along]=sa*(h[along]-b)
                        p[u]=su*(h[u]-(b if outer else 0));p[v]=sv*(h[v]-(0 if outer else b));points.append(p)
                    n=[0.,0.,0.];n[u]=su/math.sqrt(2);n[v]=sv/math.sqrt(2);emit(points,n)
        for sx in [-1,1]:
            for sy in [-1,1]:
                for sz in [-1,1]:
                    s=[sx,sy,sz]
                    emit([[s[i]*(h[i]-(0 if i==axis else b)) for i in range(3)] for axis in range(3)],unit(s))

    def masonry(self, low, high, solid=True):
        if solid: self.collision.append({'minimum':low,'maximum':high})
        # A recessed mortar core and individually bevelled ashlar courses.
        self.box([low[i]+.007 for i in range(3)],[high[i]-.007 for i in range(3)],'mortar',.005)
        for axis in [0,2]:
            other=2-axis
            for sign in [0,1]:
                rows=max(1,min(3,round((high[1]-low[1])/.31)))
                for row in range(rows):
                    y0=low[1]+(high[1]-low[1])*row/rows
                    y1=low[1]+(high[1]-low[1])*(row+1)/rows
                    breaks=([0,1] if high[other]-low[other]<.4 else
                            ([0,.5,1] if row%2==0 else [0,.25,.75,1]))
                    for index,(a,b) in enumerate(zip(breaks,breaks[1:])):
                        p=list(low);q=list(high)
                        p[other]=low[other]+(high[other]-low[other])*a+.004
                        q[other]=low[other]+(high[other]-low[other])*b-.004
                        p[1]=y0+.004;q[1]=y1-.004
                        p[axis]=low[axis]+.002 if sign==0 else high[axis]-.065
                        q[axis]=low[axis]+.065 if sign==0 else high[axis]-.002
                        if self.finish=='cracked' and row==1 and index==1:
                            q[other]-=.032
                        self.box(p,q,f'stone_{(row+index+axis+sign)%4}')
        for a in range(2):
            for b in range(2):
                p=[low[0]+(high[0]-low[0])*a/2+.004,high[1]-.06,low[2]+(high[2]-low[2])*b/2+.004]
                q=[low[0]+(high[0]-low[0])*(a+1)/2-.004,high[1]-.002,low[2]+(high[2]-low[2])*(b+1)/2-.004]
                self.box(p,q,f'stone_{(a+b)%4}')

    def banner(self, side=0, keep=False):
        details.banner(self, side, keep)

    def foliage(self, dense):
        details.foliage(self, dense)

    def export(self, slug):
        data=bytearray();views=[];accessors=[];primitives=[]
        materials=[];images=[];textures=[];texture_map={}
        names=list(self.parts)
        for name in names:
            color,texture,metal,rough=self.materials[name]
            rgba=color if len(color)==4 else color+[1.0]
            pbr={'baseColorFactor':rgba,'metallicFactor':metal,'roughnessFactor':rough}
            if texture:
                pbr['baseColorTexture']={'index':len(textures)};textures.append({'source':len(images)})
                images.append({'uri':'../textures/'+texture});texture_map[name]='textures/'+texture
            material={'name':name,'pbrMetallicRoughness':pbr,'doubleSided':name in ['ivy','leaf','leaf_light','crack','glass']}
            if rgba[3]<1:material['alphaMode']='BLEND'
            materials.append(material)
        def accessor(values,components,kind):
            while len(data)%4:data.append(0)
            offset=len(data);flat=[n for row in values for n in row] if components>1 else values
            data.extend(struct.pack('<'+('I' if kind==5125 else 'f')*len(flat),*flat))
            view=len(views);views.append({'buffer':0,'byteOffset':offset,'byteLength':len(data)-offset})
            result={'bufferView':view,'componentType':kind,'count':len(values),'type':{1:'SCALAR',2:'VEC2',3:'VEC3',4:'VEC4'}[components]}
            if components==3:
                result['min']=[min(v[i] for v in values) for i in range(3)];result['max']=[max(v[i] for v in values) for i in range(3)]
            accessors.append(result);return len(accessors)-1
        for material,name in enumerate(names):
            p=self.parts[name];attributes={}
            for key,field,count in [('POSITION','position',3),('NORMAL','normal',3),('TEXCOORD_0','uv',2),('TANGENT','tangent',4)]:
                attributes[key]=accessor(p[field],count,5126)
            primitives.append({'attributes':attributes,'indices':accessor(p['indices'],1,5125),'material':material})
        gltf={'asset':{'version':'2.0','generator':'Teramic authored castle builder'},'scene':0,'scenes':[{'nodes':[0]}],'nodes':[{'mesh':0}],
              'meshes':[{'primitives':primitives}],'materials':materials,'images':images,'textures':textures,
              'buffers':[{'byteLength':len(data),'uri':slug+'.bin'}],'bufferViews':views,'accessors':accessors}
        (MODELS/(slug+'.gltf')).write_text(json.dumps(gltf,separators=(',',':'))+'\n')
        (MODELS/(slug+'.bin')).write_bytes(data)
        return texture_map


def make(shape,finish):
    m=Mesh(finish)
    if shape=='wall':m.masonry([.004,0,.004],[.996,1,.996])
    elif shape in ['crown','keep']:
        m.masonry([.004,0,.004],[.996,.70,.996])
        for x in [.004,.716]:
            for z in [.004,.716]:m.masonry([x,.70,z],[x+.28,1,z+.28])
        if shape=='keep':
            for side in range(4):m.banner(side,True)
            m.box([.04,.66,.04],[.96,.705,.96],'gold',.008)
    elif shape=='straight':
        m.masonry([0,0,.25],[1,.68,.75])
        for x in [0,.70]:m.masonry([x,.68,.25],[x+.30,1,.75])
    elif shape=='corner':
        connected_masonry(m, 5) # Two crowns, one on each arm of the L.
    elif shape=='pillar':
        m.masonry([.20,0,.20],[.80,.70,.80]);m.box([.10,.65,.10],[.90,.78,.90],'stone_1',.018,True)
        for x in [.10,.62]:
            for z in [.10,.62]:m.masonry([x,.78,z],[x+.28,1,z+.28])
    elif shape in ['arch_left','arch_right']:
        details.arch(m,shape=='arch_right')
    elif shape in ['slit','window']:
        x0,x1=(.44,.56) if shape=='slit' else (.23,.77)
        y0,y1=(.22,.82) if shape=='slit' else (.18,.85)
        for low,high in [([0,0,.25],[x0,1,.75]),([x1,0,.25],[1,1,.75]),([x0,0,.25],[x1,y0,.75]),([x0,y1,.25],[x1,1,.75])]:m.masonry(low,high)
        if shape=='window':
            for low,high in [([x0-.02,y0,.28],[x0+.025,y1,.72]),([x1-.025,y0,.28],[x1+.02,y1,.72]),([.48,y0,.44],[.52,y1,.56]),([x0,.48,.44],[x1,.53,.56])]:m.box(low,high,'oak',.006,True)
            # Four panes sit inside the crossbars, without overlapping the wood.
            for left,right in [(x0+.025,.48),(.52,x1-.025)]:
                for bottom,top in [(y0,.48),(.53,y1)]:
                    m.box([left,bottom,.495],[right,top,.505],'glass',0,True)
    if finish in ['iron','timber']:
        for z in [.005,.94]:
            for y in [.17,.76]:m.box([.025,y,z],[.975,y+.07,z+.055],'iron' if finish=='iron' else 'oak',.008)
            for x in [.10,.83]:
                if finish=='timber':m.box([x,.08,z],[x+.07,.93,z+.055],'oak',.008)
                for y in [.18,.77]:m.box([x+.02,y+.015,max(0,z-.005)],[x+.05,y+.045,z+.060],'rivet',.006)
        # The reinforcement wraps around all sides, not just the front plane.
        for x in [.005,.94]:
            for y in [.17,.76]:m.box([x,y,.025],[x+.055,y+.07,.975],'iron' if finish=='iron' else 'oak',.008)
    elif finish=='cracked':details.cracks(m)
    elif finish in ['overgrown','moss']:m.foliage(finish=='overgrown')
    elif finish=='snow':
        m.box([.015,.91,.015],[.985,1,.985],'snow',.022)
        for x in [.04,.28,.58,.83]:m.box([x,.83+(x%0.1),.012],[min(.98,x+.12),.96,.08],'snow',.015)
    elif finish=='royal' and shape!='keep':
        for side in range(4):m.banner(side)
    elif finish=='desert':
        for side in range(4):
            def turn(p):
                x,y,z=p
                return [[x,y,z],[1-z,y,x],[1-x,y,1-z],[z,y,1-x]][side]
            m.polygon([turn(p) for p in [[.02,.48,.001],[.98,.48,.001],[.98,.64,.001],[.02,.64,.001]]],'blue')
            for i in range(7):
                x=.065+i*.145
                m.polygon([turn(p) for p in [[x,.49,0],[x+.06,.56,0],[x,.63,0],[x-.06,.56,0]]],'gold')
    return m


def connected_masonry(m, mask, full_base=False):
    """Disjoint centered wall arms close each occupied port, without an extra corner tooth."""
    if full_base:
        m.masonry([0,0,0],[1,.68,1])
    else:
        m.masonry([.25,0,.25],[.75,.68,.75])
        for bit,lo,hi in [(1,[0,0,.25],[.25,.68,.75]),(2,[.75,0,.25],[1,.68,.75]),
                          (4,[.25,0,0],[.75,.68,.25]),(8,[.25,0,.75],[.75,.68,1])]:
            if mask & bit: m.masonry(lo,hi)
    for bit,lo,hi in [(1,[0,.68,.30],[.28,1,.70]),(2,[.72,.68,.30],[1,1,.70]),
                      (4,[.30,.68,0],[.70,1,.28]),(8,[.30,.68,.72],[.70,1,1])]:
        if mask & bit: m.masonry(lo,hi)
    if mask.bit_count()==1:
        m.masonry([.35,.68,.35],[.65,1,.65])


def connected_variants(shape, finish):
    """Seven reusable authored modules, five ordinary PBR groups each.

    Default, center, four arms and end-post share their geometry across all
    sixteen masks. The same module selection exports the physical profiles.
    """
    if shape=='pillar':
        return pillar_variants(finish)
    modules=[make(shape,finish)]
    center=Mesh(finish)
    center.masonry([0,0,0] if shape=='crown' else [.25,0,.25],
                   [1,.68,1] if shape=='crown' else [.75,.68,.75])
    modules.append(center)
    for lo,hi,tooth_lo,tooth_hi in [([0,0,.25],[.25,.68,.75],[0,.68,.30],[.28,1,.70]),
            ([.75,0,.25],[1,.68,.75],[.72,.68,.30],[1,1,.70]),
            ([.25,0,0],[.75,.68,.25],[.30,.68,0],[.70,1,.28]),
            ([.25,0,.75],[.75,.68,1],[.30,.68,.72],[.70,1,1])]:
        part=Mesh(finish)
        if shape!='crown': part.masonry(lo,hi)
        part.masonry(tooth_lo,tooth_hi)
        modules.append(part)
    end=Mesh(finish)
    end.masonry([.35,.68,.35],[.65,1,.65])
    modules.append(end)
    combined=Mesh(finish)
    combined.parts={}
    for index,part in enumerate(modules):
        assert set(part.parts)=={'mortar','stone_0','stone_1','stone_2','stone_3'},(shape,index,set(part.parts))
        for name in ['mortar','stone_0','stone_1','stone_2','stone_3']:
            key=f'battlement_{index:02}_{name}'
            combined.parts[key]=part.parts[name]
            combined.materials[key]=part.materials[name]
    profiles=[]
    for mask in range(16):
        chosen=([0] if mask==0 else [1]+[2+port for port in range(4) if mask & (1<<port)]
                +([6] if mask.bit_count()==1 else []))
        profiles.append([box for i in chosen for box in modules[i].collision])
    combined.collision=profiles[0]
    return combined,profiles


def pillar_variants(finish):
    """Original four-crown top, continuous stacked shafts and separate low connectors."""
    modules=[make('pillar',finish)]
    shaft=Mesh(finish)
    shaft.masonry([.20,0,.20],[.80,1,.80])
    modules.append(shaft)
    foot=Mesh(finish)
    foot.masonry([.10,0,.10],[.90,.12,.90])
    modules.append(foot)
    for lo,hi in [([0,0,.25],[.20,.68,.75]),([.80,0,.25],[1,.68,.75]),
                  ([.25,0,0],[.75,.68,.20]),([.25,0,.80],[.75,.68,1])]:
        arm=Mesh(finish)
        arm.masonry(lo,hi)
        modules.append(arm)
    combined=Mesh(finish)
    combined.parts={}
    for index,part in enumerate(modules):
        assert set(part.parts)=={'mortar','stone_0','stone_1','stone_2','stone_3'},('pillar',index,set(part.parts))
        for name in ['mortar','stone_0','stone_1','stone_2','stone_3']:
            key=f'battlement_{index:02}_{name}'
            combined.parts[key]=part.parts[name]
            combined.materials[key]=part.materials[name]
    profiles=[]
    for key in range(64):
        below,above=bool(key&16),bool(key&32)
        chosen=([1]+([] if below else [2])) if above else [0]
        chosen += [3+port for port in range(4) if key&(1<<port)]
        profiles.append([box for i in chosen for box in modules[i].collision])
    combined.collision=profiles[0]
    return combined,profiles


def ingredient(block=None,mineral=None,count=1):
    return {'item':{'block':block} if block is not None else {'mineral':mineral},'count':count}

def recipe(material,slug,name):
    if material in [174,175,187]:
        source={174:3,175:174,187:11}[material]
        pattern={'kind':'shaped','width':2,'height':2,'cells':[ingredient(source) for _ in range(4)]}
    elif material==173:
        pattern={'kind':'shaped','width':3,'height':3,'cells':[ingredient(x) if isinstance(x,int) else ingredient(mineral=x) for x in [175,'gold_ingot',175,174,191,174,174,174,174]]}
    else:
        inputs={176:[ingredient(174)],177:[ingredient(174),ingredient(7, count=2)],178:[ingredient(174),ingredient(13)],
                179:[ingredient(174),ingredient(mineral='iron_ingot',count=2)],180:[ingredient(175),ingredient(174,count=2)],
                181:[ingredient(175,count=2),ingredient(174)],182:[ingredient(175,count=4)],
                183:[ingredient(174,count=3),ingredient(3)],184:[ingredient(183)],185:[ingredient(174,count=3),{'item':'stick','count':1}],
                186:[ingredient(174,count=3),ingredient(68)],188:[ingredient(187),ingredient(121),ingredient(mineral='gold_ingot')],
                189:[ingredient(174),ingredient(5)],190:[ingredient(174),ingredient(mineral='coal',count=2)],
                191:[ingredient(174),ingredient(121),ingredient(mineral='gold_ingot')],192:[ingredient(174),ingredient(16,count=2),ingredient(mineral='iron_ingot')]}
        pattern={'kind':'shapeless','ingredients':inputs[material]}
    return {'id':'teramic::'+slug,'name':name,'station':'workbench','output':{'item':{'block':material},'count':1},'pattern':pattern}


def main():
    MODELS.mkdir(parents=True,exist_ok=True)
    profiles={}
    for material,slug,name,shape,finish in DEFINITIONS:
        if material in [175,180,181,182]:
            mesh,profiles[material]=connected_variants(shape,finish)
        else:
            mesh=make(shape,finish)
        textures=mesh.export(slug)
        assert 0<len(mesh.collision)<=128,(slug,len(mesh.collision))
        for box in mesh.collision:
            assert all(-.01<=box['minimum'][i]<box['maximum'][i]<=1.01 for i in range(3)),(slug,box)
        definition={'schema_version':1,'id':material,'name':name,
                    'description':('Legacy corner block. New battlement walls form corners automatically.' if material==181 else 'Four-crown masonry pillar. Adjacent pillars connect; stacked pillars share one continuous shaft.' if material==182 else 'Connected castle battlement; automatically forms ends, corners and junctions.' if material in [175,180] else f'Castle masonry: {shape.replace("_"," ")}, {finish} finish. Authored joints, relief and shared physical openings. Place against any face to orient; arch halves form a two-block span.'),
                    'creative_category':'building','render_kind':'custom_mesh','model_layout':'scene','model':'models/'+slug+'.gltf',
                    'struct_block':True,'hardness':2.8 if finish!='iron' else 4,'resistance':12 if finish!='iron' else 20,
                    'required_tool':'pickaxe','mining_level':1,'textures':textures,'collision_boxes':mesh.collision}
        pattern='sandstone' if finish in ['sandstone','desert'] else 'stone'
        tint=[.40,.40,.46] if finish=='blackstone' else [1,1,1]
        definition['materials']={name:{'texture_pattern':pattern,'base_color':[*(min(1,tint[i]*mesh.materials[name][0][i]) for i in range(3)),1], 'roughness':.99,'reflectance':.08}
                                 for name in mesh.parts if name.startswith('stone_') or '_stone_' in name or name=='arch_join'}
        (BLOCKS/(slug+'.json')).write_text(json.dumps(definition,indent=2)+'\n')
        recipe_path=RECIPES/(slug+'.json')
        if material==181:
            recipe_path.unlink(missing_ok=True) # Retained block ID, automatic corners replace this item.
        else:
            recipe_path.write_text(json.dumps(recipe(material,slug,name),indent=2)+'\n')
    (MODELS/'castle_connections.json').write_text(json.dumps(profiles,separators=(',',':'))+'\n')
    # Both halves can be converted without discarding their material cost.
    (RECIPES/'castle_arch_left_from_right.json').write_text(json.dumps({'id':'teramic::castle_arch_left_from_right','name':'Mauerbogen Links','station':'workbench','output':{'item':{'block':183},'count':1},'pattern':{'kind':'shapeless','ingredients':[ingredient(184)]}},indent=2)+'\n')
    print(f'Authored {len(DEFINITIONS)} castle meshes, block definitions and recipes.')

if __name__=='__main__':main()
