"""Author three portable dragon eggs as closed, faceted shell plates over a luminous core."""
import json
import math
from build_castle import Mesh, BLOCKS
from voxel_props.surface import export_voxels

SHELLS = [
    ("dark", "Dunkel", [0.115,0.055,0.17], [0.72,0.15,0.95]),
    ("earth", "Erd", [0.54,0.28,0.09], [1.0,0.62,0.16]),
    ("water", "Wasser", [0.18,0.52,0.8], [0.48,0.92,1.0]),
]
DESCRIPTION = "Collect the egg, then place it on a straw block with room around it to hatch it."
FIRE_DESCRIPTION = "Place the egg on straw or cooled lava crust with room around it to hatch it."
DE_FIRE_DESCRIPTION = "Setze das Ei zum Ausbrüten auf Stroh oder abgekühlte Lavakruste mit Platz ringsum."
DE_DESCRIPTION = "Nimm das Ei mit und setze es zum Ausbrüten auf einen Strohblock mit Platz ringsum."

def ring(y, radius, count, twist):
    return [[0.5+radius*math.cos(i*math.tau/count+twist), y,
             0.5+radius*math.sin(i*math.tau/count+twist)] for i in range(count)]

def build(slug, number, shell, light):
    model = Mesh('stone')
    model.materials['vein'] = (light, None, 0.12, 0.38)
    for i, gain in enumerate([0.78, 0.95, 1.1, 1.22]):
        model.materials['shell'+str(i)] = ([min(1,x*gain) for x in shell], None, 0.15, 0.54)
    levels = [(0.0,.16),(.09,.265),(.25,.345),(.43,.37),(.6,.335),(.75,.27),(.87,.19),(.95,.09),(.975,.035)]
    count = 12
    rings = [ring(h,r,count,0.1*(j%2)) for j,(h,r) in enumerate(levels)]
    model.polygon(rings[0], 'shell0')
    model.polygon(rings[-1][::-1], 'shell2')
    for j, (lower, upper) in enumerate(zip(rings,rings[1:])):
        for i in range(count):
            n=(i+1)%count
            face = [lower[i],upper[i],upper[n],lower[n]]
            center = [sum(p[k] for p in face)/4 for k in range(3)]
            inset = [[v*.96+center[k]*.04 for k,v in enumerate(p)] for p in face]
            ridge = [0.5+(center[0]-.5)*1.055, center[1]+.006, 0.5+(center[2]-.5)*1.055]
            shell_key='shell'+str((i*7+j*3+(i//3))%4)
            for k in range(4):
                end=(k+1)%4
                model.polygon([inset[k],inset[end],ridge],shell_key)
                model.polygon([face[k],face[end],inset[end],inset[k]],
                              'vein' if (i+j//2)%5 == 0 or j==3 and i%3==0 else 'shell0')
    export_voxels(model, slug, ('vein',), resolution=16)
    name=slug.replace('_',' ').title()
    bounds=[dict(minimum=[.24,.0,.24],maximum=[.76,.13,.76]),
            dict(minimum=[.16,.13,.16],maximum=[.84,.63,.84]),
            dict(minimum=[.23,.63,.23],maximum=[.77,.82,.77]),
            dict(minimum=[.33,.82,.33],maximum=[.67,.975,.67])]
    description = FIRE_DESCRIPTION if slug == 'fire_dragon_egg' else DESCRIPTION
    definition=dict(schema_version=1,id=number,name=name,description=description,creative_category='nature',
                    hardness=.45,resistance=1,required_tool=None,mining_level=0,render_kind='custom_mesh',
                    model_layout='scene',model=f'models/{slug}.glb',struct_block=True,
                    textures={},collision_boxes=bounds)
    (BLOCKS/f'{slug}.json').write_text(json.dumps(definition,indent=2)+'\n')
    return name

if __name__ == '__main__':
    entries=[]
    for number,(key,de,shell,light) in enumerate(SHELLS,449):
        name=build(key+'_dragon_egg',number,shell,light)
        entries.append(dict(source=name,en=name,de=de+'drachenei'))
    name = build("fire_dragon_egg", 702, [0.24, 0.07, 0.034], [1.0, 0.29, 0.018])
    entries.append(dict(source=name, en=name, de="Feuerdrachenei"))
    entries.append(dict(source=FIRE_DESCRIPTION, en=FIRE_DESCRIPTION, de=DE_FIRE_DESCRIPTION))
    entries.append(dict(source=DESCRIPTION,en=DESCRIPTION,de=DE_DESCRIPTION))
    path=BLOCKS.parents[2]/'src-ui/locales/source/dragon-eggs.json'
    path.write_text(json.dumps(entries,ensure_ascii=False,indent=2)+'\n')
