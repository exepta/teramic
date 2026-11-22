"""Faceted, self-lit crystal outcrops in the earth and tide dragon lairs."""
import json
import math
from build_castle import Mesh, BLOCKS
from build_spirit_blossom import export

def geode(number, slug, title, description, color):
    mesh = Mesh('stone')
    keys = ['mineral', 'facet', 'edge']
    for key,gain in zip(keys,[.35,.74,1.15]):
        mesh.materials[key]=([min(1,c*gain) for c in color],None,.16,.30)
    # Six independent prisms grow out of the same point, each with a slanted termination.
    for x,z,radius,height,lx,lz in [
        (.48,.50,.14,.96,.06,.06),(.37,.49,.12,.63,-.19,.12),(.64,.49,.11,.67,.17,-.10),
        (.49,.32,.12,.53,-.04,-.18),(.56,.67,.13,.50,.12,.14),(.31,.31,.08,.34,-.13,-.11)
    ]:
        def ring(t,r):
            return [[x+lx*t+r*math.cos(k*math.tau/6+.29), height*t,
                     z+lz*t+r*math.sin(k*math.tau/6+.29)] for k in range(6)]
        base,waist,cap=ring(0,radius*.68),ring(.20,radius),ring(.79,radius*.83)
        tip=[x+lx*1.08+.02,height,z+lz*1.08]
        for i in range(6):
            j=(i+1)%6
            mesh.polygon([base[i],waist[i],waist[j],base[j]],'mineral')
            mesh.polygon([waist[i],cap[i],cap[j],waist[j]],keys[i%3])
            mesh.polygon([cap[i],tip,cap[j]],keys[(i+1)%3])
    export(mesh,slug,('facet','edge'))
    definition=dict(schema_version=1,id=number,name=title,description=description,creative_category='decoration',
        hardness=1.2,resistance=3,mining_level=0,render_kind='custom_mesh',model_layout='scene',
        model='models/'+slug+'.glb',struct_block=True,textures={},
        collision_boxes=[dict(minimum=[.10,0,.09],maximum=[.88,.42,.86]),dict(minimum=[.38,.42,.37],maximum=[.71,.91,.71])],
        light=dict(color=color,intensity=920,range=12,falloff='soft_core',flicker=.016,
            cave_intensity=.93,shadows=False,offset=[.5,.57,.5]))
    (BLOCKS/(slug+'.json')).write_text(json.dumps(definition,indent=2)+'\n')

if __name__=='__main__':
    amber = 'Golden crystals cast a warm light across the earth dragon’s nest.'
    tide = 'These pale blue crystals light the pools in a water dragon’s grotto.'
    geode(452,'amber_geode','Amber Geode',amber,[1,.52,.08])
    geode(453,'tide_geode','Tide Geode',tide,[.14,.74,1])
    labels=[('Amber Geode','Bernsteindruse'),('Tide Geode','Gezeitendruse'),
            (amber,'Goldene Kristalle tauchen das Nest des Erddrachen in warmes Licht.'),
            (tide,'Diese hellblauen Kristalle erhellen die Wasserbecken in der Grotte des Wasserdrachen.')]
    (BLOCKS.parents[2]/'src-ui/locales/source/dragon-geodes.json').write_text(json.dumps([
        dict(source=en,en=en,de=de) for en,de in labels],ensure_ascii=False,indent=2)+'\n')
