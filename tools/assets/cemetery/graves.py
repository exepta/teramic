"""Gravestones, bordered graves, sarcophagi and the small sculpted cemetery props."""
import math
from .geometry import cylinder,ellipsoid,cross,skull,leaf,crystal,candle

def bones(mesh,count=7):
    for i in range(count):
        a=(i*2.39)%math.tau;x=.27+(i%3)*.2;z=.24+(i//3)*.21
        begin=[x-math.cos(a)*.14,.065+(i%2)*.035,z-math.sin(a)*.14]
        end=[x+math.cos(a)*.14,.065+(i%2)*.035,z+math.sin(a)*.14]
        cylinder(mesh,begin,end,.027,'bone',8)
        for p in [begin,end]:ellipsoid(mesh,p,[.045,.035,.045],'bone',8,4)

def bone_pile(mesh):
    # Crossed long bones bear on one another; ribs and a half-buried skull
    # break up the silhouette above the wider, settled base.
    for level, count, reach in [(0, 4, .35), (1, 3, .31), (2, 2, .26)]:
        for i in range(count):
            a = .45 + level*1.22 + i*.21
            offset = (i - (count-1)*.5) * .155
            x, z = .5 - math.sin(a)*offset, .52 + math.cos(a)*offset
            low = [x-math.cos(a)*reach, .055+level*.105, z-math.sin(a)*reach]
            high = [x+math.cos(a)*reach, .080+level*.105, z+math.sin(a)*reach]
            cylinder(mesh, low, high, .037, 'bone', 6)
            for tip in (low,high):
                for side in [-1,1]:
                    knob = [tip[0] - side*math.sin(a)*.024,tip[1], tip[2] + side*math.cos(a)*.024]
                    ellipsoid(mesh, knob, [.042,.050,.044], 'bone', 6, 3)
    skull(mesh,[.47,.442,.51],.37)
    for i in range(3):
        x = .20 + i*.078
        a,b,c = [x,.12,.64],[x-.04,.25+i*.022,.57],[x+.025,.31+i*.023,.44]
        cylinder(mesh,a,b,.024,'bone',5)
        cylinder(mesh,b,c,.023,'bone',5,.014)


def build(mesh,family):
    if family.startswith('gravestone_') or family=='cross':
        h={'gravestone_small':.56,'gravestone_medium':.75,'gravestone_large':.94,'cross':.94}[family]
        mesh.box([.16,0,.26],[.84,.12,.74],'stone',.022,True)
        if family=='cross':cross(mesh,.5,.55,.5,.82,'trim')
        else:
            mesh.box([.25,.12,.40],[.75,h-.15,.62],'stone',.020,True)
            # Pointed/cusped pediment above a deep frame, unlike a flat slab.
            cylinder(mesh,[.5,h-.20,.41],[.5,h-.20,.61],.20,'stone',16)
            for x in [.28,.68]:mesh.box([x,.15,.37],[x+.04,h-.16,.41],'trim',.006)
            cross(mesh,.5,.28+h*.28,.36,h*.37,'trim')
            for i in range(3):mesh.box([.36,.18+i*.04,.37],[.64-i*.025,.19+i*.04,.395],'shadow',.001)
        mesh.collision.append({'minimum':[.25,.12,.4],'maximum':[.75,h,.62]})
        return True
    if family in ('bordered_grave','sarcophagus','open_sarcophagus'):
        mesh.box([.08,0,.06],[.92,.12,.94],'stone',.025,True)
        for x in [.1,.80]:mesh.box([x,.12,.10],[x+.1,.40 if family!='bordered_grave' else .23,.90],'trim',.015,True)
        for z in [.1,.80]:mesh.box([.20,.12,z],[.80,.40 if family!='bordered_grave' else .23,z+.1],'trim',.015,True)
        if family=='bordered_grave':
            mesh.box([.21,.12,.21],[.79,.17,.79],'soil',.004)
            for i in range(6):
                x=.32+(i%2)*.32;z=.27+(i//2)*.20
                leaf(mesh,[x,.23,z],.17)
                ellipsoid(mesh,[x,.29,z],[.045,.028,.045],'flower',8,4)
        elif family=='sarcophagus':
            mesh.box([.07,.40,.07],[.93,.50,.93],'trim',.023,True)
            mesh.box([.17,.50,.17],[.83,.56,.83],'stone',.020)
            cross(mesh,.5,.58,.5,.27,'gold')
            for z in [.25,.75]:mesh.box([.06,.34,z-.025],[.94,.51,z+.025],'gold',.008)
        else:
            mesh.box([.20,.14,.20],[.80,.17,.80],'shadow',.004)
            bones(mesh,4)
            mesh.box([.02,.51,.73],[.98,.64,.95],'trim',.020,True)
        return True
    if family in ('skeleton','bones'):
        if family=='bones':
            bone_pile(mesh)
        else:
            bones(mesh,5)
            skull(mesh,[.5,.16,.22],.25)
            cylinder(mesh,[.5,.09,.37],[.5,.09,.79],.035,'bone',8)
            for z in [.43,.50,.57]:
                for side in [-1,1]:cylinder(mesh,[.5,.09,z],[.5+side*.15,.10,z+.06],.019,'bone',8)
        return True
    if family=='skull':skull(mesh,[.5,.25,.5],.48);return True
    if family=='urn':
        ellipsoid(mesh,[.5,.38,.5],[.27,.31,.27],'stone',16,8)
        cylinder(mesh,[.5,.64,.5],[.5,.75,.5],.13,'trim',16)
        cylinder(mesh,[.5,.75,.5],[.5,.80,.5],.18,'gold',16)
        for y in [.18,.57]:cylinder(mesh,[.5,y,.5],[.5,y+.025,.5],.24,'gold',16)
        mesh.collision.append({'minimum':[.23,.08,.23],'maximum':[.77,.80,.77]})
        return True
    if family=='books':
        for i in range(4):
            x=.16+(i%2)*.06;z=.18+(i%3)*.035;y=i*.13
            for yy in [y,y+.11]:mesh.box([x,yy,z],[x+.65,yy+.025,z+.56],'cloth' if i%2 else 'leather',.010)
            mesh.box([x+.035,y+.025,z+.035],[x+.615,y+.11,z+.525],'wax',.006)
            for line in range(4):mesh.box([x+.03,y+.04+line*.015,z+.015],[x+.62,y+.042+line*.015,z+.037],'trim',.001)
        return True
    if family=='chest':
        mesh.box([.10,.05,.15],[.90,.54,.85],'cloth',.025,True)
        # Barrel-shaped hinged lid with multiple articulated-looking facets.
        for i in range(8):
            x=.12+i*.095;h=.14*math.sqrt(max(0,1-((x+.047-.5)/.4)**2))
            mesh.box([x,.54,.14],[x+.095,.58+h,.86],'cloth',.008,True)
        for x in [.15,.77]:
            mesh.box([x,.06,.13],[x+.075,.66,.17],'gold',.012)
            mesh.box([x,.06,.83],[x+.075,.66,.87],'gold',.012)
        for z in [.16,.79]:mesh.box([.1,.08,z],[.9,.14,z+.05],'gold',.008)
        mesh.box([.44,.36,.11],[.56,.56,.16],'gold',.012)
        mesh.box([.48,.40,.09],[.52,.48,.12],'shadow',.003)
        return True
    if family in ('crystal_small','crystal_large'):
        scale=.65 if family=='crystal_small' else 1
        for x,z,h,r in [(.5,.5,.92,.15),(.28,.38,.50,.10),(.70,.43,.60,.11),(.35,.70,.40,.08),(.69,.69,.35,.08)]:
            crystal(mesh,[x,0,z],r*scale,h*scale)
            mesh.collision.append({'minimum':[x-r*scale,0,z-r*scale],'maximum':[x+r*scale,h*scale,z+r*scale]})
        return True
    return False
