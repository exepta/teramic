"""Closed ornamental lights, fabric, vegetation and authored dungeon mechanisms."""
import math
from .geometry import cylinder,ellipsoid,cross,leaf,candle,crystal

def light(mesh,family):
    if family in ('lantern','torch','candles','chandelier','brazier'):
        if family=='candles':
            mesh.box([.14,0,.14],[.86,.045,.86],'stone',.015)
            for x,z,h in [(.30,.30,.32),(.53,.27,.58),(.70,.49,.36),(.32,.63,.47),(.55,.65,.28)]:candle(mesh,x,z,h)
        elif family=='chandelier':
            cylinder(mesh,[.5,.45,.5],[.5,1,.5],.025,'gold',10)
            for i in range(8):
                a=math.tau*i/8;b=math.tau*(i+1)/8
                p=[.5+.32*math.cos(a),.37,.5+.32*math.sin(a)];q=[.5+.32*math.cos(b),.37,.5+.32*math.sin(b)]
                cylinder(mesh,p,q,.025,'gold',8)
                cylinder(mesh,p,[.5,.75,.5],.009,'gold',6)
                cylinder(mesh,p,[p[0],.55,p[2]],.028,'wax',10)
                ellipsoid(mesh,[p[0],.59,p[2]],[.025,.055,.025],'flame',8,5)
        elif family=='brazier':
            mesh.box([.25,0,.25],[.75,.14,.75],'stone',.025,True)
            cylinder(mesh,[.5,.14,.5],[.5,.50,.5],.14,'stone',12,end_radius=.22)
            cylinder(mesh,[.5,.50,.5],[.5,.58,.5],.31,'iron',12,end_radius=.34)
            mesh.collision.extend([{'minimum':[.28,.14,.28],'maximum':[.72,.50,.72]}, {'minimum':[.16,.50,.16],'maximum':[.84,.58,.84]}])
            for i in range(7):
                a=math.tau*i/7
                ellipsoid(mesh,[.5+.16*math.cos(a),.67+(i%2)*.04,.5+.16*math.sin(a)],[.09,.18,.09],'soul',8,6)
            ellipsoid(mesh,[.5,.77,.5],[.11,.23,.11],'soul',10,6)
        elif family=='torch':
            mesh.box([.30,.12,.80],[.70,.70,.94],'iron',.025)
            cylinder(mesh,[.5,.20,.85],[.5,.62,.42],.045,'iron',10)
            cylinder(mesh,[.5,.45,.42],[.5,.68,.42],.10,'iron',10,end_radius=.14)
            ellipsoid(mesh,[.5,.79,.42],[.11,.19,.11],'flame',10,6)
        else:
            mesh.box([.24,0,.24],[.76,.10,.76],'trim',.022,True)
            cylinder(mesh,[.5,.10,.5],[.5,.40,.5],.055,'iron',12)
            mesh.box([.27,.38,.27],[.73,.44,.73],'iron',.016,True)
            mesh.box([.32,.44,.32],[.68,.78,.68],'flame',.018)
            for x in [.27,.69]:
                for z in [.27,.69]:mesh.box([x,.43,z],[x+.04,.81,z+.04],'iron',.006)
            for y,w in [(.80,.52),(.86,.42),(.91,.28)]:mesh.box([.5-w/2,y,.5-w/2],[.5+w/2,y+.055,.5+w/2],'iron',.012)
            cylinder(mesh,[.5,.96,.5],[.5,.99,.5],.05,'gold',10)
            mesh.collision.extend([{'minimum':[.445,.10,.445],'maximum':[.555,.40,.555]}, {'minimum':[.24,.44,.24],'maximum':[.76,.97,.76]}])
        return True
    return False

def fabric(mesh,family):
    if family not in ('banner','torn_banner','curtain','chain'):return False
    if family=='chain':
        for i in range(6):
            y=.115+i*.153
            for n in range(12):
                a=math.tau*n/12;b=math.tau*(n+1)/12
                if i%2:p=[.5+.065*math.cos(a),y+.095*math.sin(a),.5];q=[.5+.065*math.cos(b),y+.095*math.sin(b),.5]
                else:p=[.5,y+.095*math.sin(a),.5+.065*math.cos(a)];q=[.5,y+.095*math.sin(b),.5+.065*math.cos(b)]
                cylinder(mesh,p,q,.018,'iron',6)
        return True
    cylinder(mesh,[.07,.94,.78],[.93,.94,.78],.03,'gold',12)
    for x in [.08,.92]:ellipsoid(mesh,[x,.94,.78],[.06,.055,.055],'gold',10,6)
    if family=='curtain':
        for side in [-1,1]:
            for i in range(5):
                x=.5+side*(.13+i*.065)
                mesh.box([x-.045,.06,.65+math.sin(i)*.025],[x+.045,.92,.73+math.sin(i)*.025],'cloth',.012)
            cylinder(mesh,[.5+side*.16,.42,.62],[.5+side*.39,.43,.62],.022,'gold',8)
    else:
        for i in range(12):
            x=.17+i*.055;low=.12 if family=='banner' else .08+(i%3)*.065
            mesh.box([x,low,.67+math.sin(i*.6)*.025],[x+.055,.92,.705+math.sin(i*.6)*.025],'cloth',.003)
        for x in [.17,.80]:mesh.box([x,.14,.64],[x+.025,.91,.67],'gold',.004)
        cross(mesh,.5,.56,.65,.43,'gold')
    return True

def vegetation(mesh,family):
    if family not in ('grass','thorn','ivy','moss','mushrooms','glow_mushrooms','vines'):return False
    if family=='grass':
        for i in range(13):
            a=i*2.39;r=.08+(i%3)*.08;x=.5+math.cos(a)*r;z=.5+math.sin(a)*r;h=.2+(i%5)*.08
            cylinder(mesh,[x,.02,z],[x+math.cos(a)*.15,h,z+math.sin(a)*.15],.018,'dry',6,end_radius=.003)
    elif family=='thorn':
        for i in range(9):
            a=i*2.39
            p=[.5,.03,.5];q=[.5+math.cos(a)*.32,.25+(i%4)*.13,.5+math.sin(a)*.32]
            cylinder(mesh,p,q,.027,'wood',8,end_radius=.010)
            for j in [1,2]:
                c=[p[k]+(q[k]-p[k])*j/3 for k in range(3)]
                cylinder(mesh,c,[c[0]+.09,c[1]+.08,c[2]-.04],.017,'wood',6,end_radius=.001)
    elif family in ('ivy','vines'):
        for n in range(3):
            x=.23+n*.26
            for i in range(7):
                y=.09+i*.128;xx=x+math.sin(i*.7+n)*.065
                cylinder(mesh,[xx,y,.84],[x+math.sin((i+1)*.7+n)*.065,y+.13,.84],.010,'wood',6)
                leaf(mesh,[xx+(-1)**i*.055,y+.05,.79],.17,'leaf' if i%2 else 'moss')
                if family=='vines' and i%2==0:
                    for j in range(5):ellipsoid(mesh,[xx+math.cos(j*math.tau/5)*.047,y+math.sin(j*math.tau/5)*.047,.74],[.04,.04,.015],'flower',8,4)
    elif family=='moss':
        for i in range(9):ellipsoid(mesh,[.20+(i%3)*.30,.046,.20+(i//3)*.30],[.20,.045,.20],'moss',8,4)
    else:
        glow=family=='glow_mushrooms'
        for x,z,h,r in [(.30,.33,.28,.15),(.63,.49,.54,.23),(.40,.71,.20,.12)]:
            cylinder(mesh,[x,0,z],[x,h,z],.033,'bone',10)
            ellipsoid(mesh,[x,h,z],[r,r*.55,r],'soul' if glow else 'cloth',12,6)
            for i in range(5):
                a=i*math.tau/5
                ellipsoid(mesh,[x+math.cos(a)*r*.65,h+r*.32,z+math.sin(a)*r*.65],[r*.13,.015,r*.13],'flower',8,4)
    return True

def mechanism(mesh,family):
    if family not in ('plate','lever','switch','portcullis','gate','spikes','arrows'):return False
    if family=='plate':
        mesh.box([.08,0,.08],[.92,.10,.92],'stone',.022,True)
        mesh.box([.16,.10,.16],[.84,.14,.84],'trim',.016)
    elif family in ('lever','switch'):
        mesh.box([.26,0,.28],[.74,.12,.72],'stone',.020,True)
        if family=='lever':
            cylinder(mesh,[.5,.13,.5],[.5,.66,.29],.045,'wood',12)
            cylinder(mesh,[.38,.66,.29],[.62,.66,.29],.065,'gold',12)
        else:
            mesh.box([.30,.12,.43],[.70,.66,.88],'iron',.022,True)
            mesh.box([.39,.25,.35],[.61,.53,.45],'gold',.016)
    elif family in ('portcullis','gate'):
        for x in [.08,.25,.42,.59,.76,.93]:
            mesh.box([x-.025,0,.42],[x+.025,1,.48],'iron' if family=='portcullis' else 'wood',.006,True)
        for y in [.15,.79]:mesh.box([.04,y,.39],[.97,y+.08,.51],'iron',.008,True)
        if family=='gate':
            mesh.box([.40,.37,.33],[.63,.67,.44],'gold',.020)
            mesh.box([.49,.43,.31],[.54,.57,.35],'shadow',.004)
    elif family=='spikes':
        mesh.box([.04,0,.04],[.96,.12,.96],'stone',.018,True)
        for x in [.22,.50,.78]:
            for z in [.22,.5,.78]:cylinder(mesh,[x,.12,z],[x,.65,z],.065,'iron',8,end_radius=.001)
    else:
        mesh.box([.10,0,.62],[.90,1,.95],'stone',.020,True)
        for y in [.25,.5,.75]:
            mesh.box([.26,y-.045,.60],[.74,y+.045,.65],'shadow',.006)
            cylinder(mesh,[.5,y,.61],[.5,y,.20],.018,'wood',8)
            cylinder(mesh,[.5,y,.26],[.5,y,.10],.05,'iron',4,end_radius=.001)
    return True
