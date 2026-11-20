"""Tiled masonry and carved cemetery architecture; each piece fits one voxel."""
import math
from .geometry import cylinder,arch,cross,skull

def ornament(mesh,y=.5,z=.145):
    for r in [.21,.15]:
        for i in range(12):
            a=math.tau*i/12;b=math.tau*(i+1)/12
            cylinder(mesh,[.5+r*math.cos(a),y+r*math.sin(a),z],[.5+r*math.cos(b),y+r*math.sin(b),z],.012,'trim',6)
    cross(mesh,.5,y,z-.012,.25,'gold')

def build(mesh,family):
    if family.endswith('_slab') or family=='slab':
        mesh.box([0,0,0],[1,.25,1],'stone',.018,True)
        for x in [0,.5]:
            for z in [0,.5]:mesh.box([x+.008,.25,z+.008],[x+.492,.29,z+.492],'trim' if family=='mosaic_slab' else 'stone',.012)
        if family=='ornate_slab':
            for i in range(4):
                a=i*math.pi/2;b=(i+1)*math.pi/2
                cylinder(mesh,[.5+.29*math.cos(a),.305,.5+.29*math.sin(a)],[.5+.29*math.cos(b),.305,.5+.29*math.sin(b)],.018,'trim',6)
        if family=='inscribed_slab':
            skull(mesh,[.5,.33,.37],.19)
            for i in range(4):mesh.box([.29,.294,.58+i*.055],[.71-i*.025,.302,.60+i*.055],'shadow',.001)
        if family=='cracked_slab':
            for i in range(5):mesh.box([.34+i*.045,.292,.1+i*.13],[.35+i*.045,.300,.26+i*.13],'shadow',.001)
        return True
    if family in ('column','ornate_column','capital','base'):
        if family=='capital':
            mesh.box([.12,.61,.12],[.88,1,.88],'trim',.028,True)
            mesh.box([.23,.36,.23],[.77,.62,.77],'stone',.026,True)
            cylinder(mesh,[.5,0,.5],[.5,.37,.5],.22,'stone',16)
            mesh.collision.append({'minimum':[.28,0,.28],'maximum':[.72,.37,.72]})
        elif family=='base':
            mesh.box([.10,0,.10],[.90,.22,.90],'trim',.028,True)
            mesh.box([.18,.22,.18],[.82,.40,.82],'stone',.024,True)
            cylinder(mesh,[.5,.4,.5],[.5,1,.5],.22,'stone',16)
            mesh.collision.append({'minimum':[.28,.4,.28],'maximum':[.72,1,.72]})
        else:
            cylinder(mesh,[.5,.10,.5],[.5,.90,.5],.23,'stone',16)
            for y in [0,.87]:mesh.box([.17,y,.17],[.83,y+.13,.83],'trim',.020,True)
            for i in range(12):
                a=math.tau*i/12
                cylinder(mesh,[.5+math.cos(a)*.23,.18,.5+math.sin(a)*.23],[.5+math.cos(a)*.23,.82,.5+math.sin(a)*.23],.014,'shadow',6)
            mesh.collision.append({'minimum':[.27,.1,.27],'maximum':[.73,.9,.73]})
            if family=='ornate_column':
                for y in [.14,.81]:cylinder(mesh,[.5,y,.5],[.5,y+.045,.5],.27,'gold',16)
                for i in range(4):
                    a=math.pi*i/2
                    mesh.box([.5+math.cos(a)*.24-.04,.45,.5+math.sin(a)*.24-.04],[.5+math.cos(a)*.24+.04,.55,.5+math.sin(a)*.24+.04],'gold',.012)
        return True
    if family in ('arch','moss_arch'):
        for x in [0,.82]:mesh.box([x,0,.28],[x+.18,.52,.72],'stone',.014,True)
        arch(mesh,[.5,.51,.5],.32,.49,.44,'trim',16)
        for x in [0,.82]:mesh.collision.append({'minimum':[x,.5,.28],'maximum':[x+.18,1,.72]})
        mesh.collision.append({'minimum':[.18,.84,.28],'maximum':[.82,1,.72]})
        if family=='moss_arch':
            from .geometry import leaf
            for i in range(8):leaf(mesh,[.08+i*.11,.87+math.sin(i*.65)*.05,.26],.16,'moss')
        return True
    if family in ('relief','skull_relief','cross_relief','niche'):
        mesh.box([.03,0,.50],[.97,1,.94],'stone',.025,True)
        if family=='niche':
            for x in [.03,.82]:mesh.box([x,.03,.14],[x+.15,.66,.51],'trim',.012,True)
            arch(mesh,[.5,.54,.32],.32,.46,.36,'trim',14)
            from .geometry import candle
            for x,h in [(.35,.30),(.50,.45),(.65,.27)]:candle(mesh,x,.30,h)
        else:
            for x in [.08,.84]:mesh.box([x,.04,.37],[x+.08,.96,.52],'trim',.012)
            for y in [.06,.88]:mesh.box([.08,y,.37],[.92,y+.07,.52],'trim',.012)
            if family=='skull_relief':skull(mesh,[.5,.56,.40],.51)
            elif family=='cross_relief':cross(mesh,.5,.51,.39,.74,'trim')
            else:ornament(mesh,.52,.39)
        return True
    return False
