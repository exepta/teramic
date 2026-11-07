"""Authored castle surface detail and continuous arch meshes (no bitmap editing)."""
import math
import random

ICONS = ('castle', 'crown', 'tree', 'mountain', 'wheat', 'anchor', 'hammer', 'star')


def turn(point, side):
    x, y, z = point
    return ([x, y, z], [1-z, y, x], [1-x, y, 1-z], [z, y, 1-x])[side]


def plaque(mesh, points, material, side, depth, thickness=0.001):
    front = [turn([x, y, depth], side) for x, y in points]
    back = [turn([x, y, depth + thickness], side) for x, y in points]
    mesh.polygon(front, material)
    mesh.polygon(list(reversed(back)), material)
    for i in range(len(points)):
        j = (i + 1) % len(points)
        mesh.polygon([front[i], back[i], back[j], front[j]], material)


def segment(mesh, a, b, width, material, side, depth):
    dx, dy = b[0]-a[0], b[1]-a[1]
    length = math.hypot(dx, dy)
    if length < 1e-8:
        return
    nx, ny = -dy/length*width, dx/length*width
    plaque(mesh, [(a[0]-nx,a[1]-ny), (b[0]-nx,b[1]-ny),
                  (b[0]+nx,b[1]+ny), (a[0]+nx,a[1]+ny)], material, side, depth)


def emblem(mesh, icon, side, cx, cy, scale):
    material = 'heraldry_icon_' + icon
    def poly(points):
        plaque(mesh, [(cx+x*scale, cy+y*scale) for x,y in points], material, side, 0.0002, 0.0007)
    def bar(x, y, w, h):
        poly([(x,y),(x+w,y),(x+w,y+h),(x,y+h)])
    if icon == 'castle':
        bar(-.75,-.60,1.5,.70)
        for x in [-.85,.30]:
            bar(x,-.60,.55,1.25)
            for dx in [0,.36]: bar(x+dx,.63,.19,.24)
        bar(-.25,-.1,.5,.90)
        for x in [-.25,.08]: bar(x,.77,.17,.20)
    elif icon == 'crown':
        bar(-.78,-.55,1.56,.32)
        for a,b,peak in [(-.78,-.26,(-.80,.75)),(-.36,.36,(0,1)),(.26,.78,(.80,.75))]:
            poly([(a,-.26),(b,-.26),peak])
    elif icon == 'tree':
        bar(-.12,-.86,.24,.85)
        for y,w in [(-.15,.84),(.18,.65),(.48,.44)]:
            poly([(-w,y),(w,y),(0,y+.63)])
    elif icon == 'mountain':
        poly([(-.95,-.60),(.35,-.60),(-.35,.90)])
        poly([(-.10,-.60),(.95,-.60),(.42,.55)])
    elif icon == 'wheat':
        bar(-.055,-.9,.11,1.8)
        for y in [-.52,-.15,.22,.59]:
            for sign in [-1,1]:
                poly([(0,y),(.55*sign,y+.08),(.65*sign,y+.42),(.18*sign,y+.27)])
    elif icon == 'anchor':
        bar(-.07,-.55,.14,1.25)
        bar(-.58,.17,1.16,.11)
        for i in range(12):
            a=math.tau*i/12;b=math.tau*(i+1)/12
            poly([(.25*math.cos(a),.78+.25*math.sin(a)),(.25*math.cos(b),.78+.25*math.sin(b)),
                  (.13*math.cos(b),.78+.13*math.sin(b)),(.13*math.cos(a),.78+.13*math.sin(a))])
        for sign in [-1,1]:
            poly([(0,-.68),(.57*sign,-.30),(.68*sign,.0),(.95*sign,-.08),(.72*sign,-.62),(0,-.87)])
    elif icon == 'hammer':
        bar(-.11,-.89,.22,1.48)
        bar(-.80,.33,1.60,.51)
    elif icon == 'star':
        ring=[(.37*math.sin(math.tau*i/5),.37*math.cos(math.tau*i/5)) for i in range(5)]
        poly(ring)
        for i in range(5):
            a=math.tau*i/5
            poly([ring[(i-1)%5],(math.sin(a)*.95,math.cos(a)*.95),ring[i]])


def banner(mesh, side=0, keep=False):
    top = .68 if keep else .82
    bottom = .11 if keep else .13
    plaque(mesh, [(.25,top),(.75,top),(.75,bottom+.10),(.50,bottom),(.25,bottom+.10)], 'gold', side, .0027)
    plaque(mesh, [(.28,top-.025),(.72,top-.025),(.72,bottom+.12),(.50,bottom+.035),(.28,bottom+.12)], 'heraldry_cloth', side, .0014)
    for icon in ICONS:
        emblem(mesh, icon, side, .50, (top+bottom)/2+.015, .16)


def cracks(mesh):
    for side in range(4):
        rng=random.Random(0xC4AC+side*73)
        for _ in range(2):
            x=rng.uniform(.23,.77)
            points=[]
            for i in range(10):
                x=max(.08,min(.92,x+rng.gauss(0,.037)))
                points.append((x,.035+i*.098+rng.uniform(-.009,.009)))
            for i,(a,b) in enumerate(zip(points,points[1:])):
                segment(mesh,a,b,rng.uniform(.0012,.0023),'crack',side,.0008)
                if i in [2,5,7]:
                    sign=rng.choice([-1,1])
                    tip=(max(.035,min(.965,b[0]+sign*rng.uniform(.07,.16))),b[1]+rng.uniform(.04,.12))
                    mid=((b[0]+tip[0])/2+rng.uniform(-.018,.018),(b[1]+tip[1])/2)
                    segment(mesh,b,mid,.0011,'crack',side,.0009)
                    segment(mesh,mid,tip,.0007,'crack',side,.0010)


def foliage(mesh, dense):
    for side in range(4):
        rng=random.Random(0x1F711+side*977+(1 if dense else 0))
        for stem in range(rng.randint(3,5) if dense else rng.randint(1,2)):
            x=rng.uniform(.13,.87)
            y=rng.uniform(.83,.97)
            length=rng.uniform(.40,.85) if dense else rng.uniform(.18,.50)
            points=[(x,y)]
            end=max(.09,points[0][1]-length)
            while y>end+1e-6:
                y=max(end,y-rng.uniform(.055,.095))
                x=max(.06,min(.94,x+rng.gauss(0,.034)))
                points.append((x,y))
            for a,b in zip(points,points[1:]):
                segment(mesh,a,b,rng.uniform(.0015,.003),'ivy',side,.0015)
                for sign in [-1,1]:
                    if rng.random()<.22:continue
                    cx=max(.035,min(.965,b[0]+sign*rng.uniform(.025,.055)))
                    cy=b[1]+rng.uniform(-.012,.025)
                    w=rng.uniform(.019,.037);h=rng.uniform(.028,.048)
                    angle=sign*rng.uniform(.15,.65)
                    def leaf_point(u,v,z):
                        return turn([max(.004,min(.996,cx+u*math.cos(angle)-v*math.sin(angle))),max(.004,min(.996,cy+u*math.sin(angle)+v*math.cos(angle))),z],side)
                    ring=[leaf_point(0,h,.003),leaf_point(w,0,.0025),leaf_point(0,-h*.72,.003),leaf_point(-w,0,.0025)]
                    center=leaf_point(0,0,0)
                    mat=rng.choice(['leaf','leaf_light','ivy'])
                    for i in range(4):mesh.polygon([center,ring[i],ring[(i+1)%4]],mat)
        for _ in range(7 if dense else 4):
            x=rng.uniform(.03,.83);z=rng.uniform(.04,.82)
            mesh.box([x,.995,z],[min(.99,x+rng.uniform(.06,.15)),.999,min(.99,z+rng.uniform(.06,.15))],'moss',.001)


def arch(mesh, right):
    """Connected curved soffit/front/back; collision is a separate conservative approximation."""
    radius=.75
    segments=32
    def local(p):
        return [1-p[0],p[1],p[2]] if right else list(p)
    def poly(points, material, normal=None):
        if right:points=list(reversed(points))
        mesh.polygon([local(p) for p in points],material,
                     None if normal is None else ([-normal[0],normal[1],normal[2]] if right else normal))
    # Solid outside pier. Its join to the curved band is internal, so no exposed cut face.
    for z,n in [(0,[0,0,-1]),(1,[0,0,1])]:
        poly([[0,0,z],[.25,0,z],[.25,1,z],[0,1,z]],'stone_1',n)
    poly([[0,0,0],[0,0,1],[0,1,1],[0,1,0]],'stone_1',[-1,0,0])
    poly([[0,0,0],[.25,0,0],[.25,0,1],[0,0,1]],'stone_1',[0,-1,0])
    poly([[0,1,0],[1,1,0],[1,1,1],[0,1,1]],'stone_1',[0,1,0])
    poly([[.25,0,0],[.25,.15,0],[.25,.15,1],[.25,0,1]],'stone_1',[1,0,0])
    # Only this primitive can be culled when the matching second half is present.
    poly([[1,.9,0],[1,1,0],[1,1,1],[1,.9,1]],'arch_join',[1,0,0])
    mesh.collision.append({'minimum':local([0,0,0]) if not right else [0.75,0,0],
                           'maximum':[.25,1,1] if not right else [1,1,1]})
    for i in range(segments):
        a=math.pi-i*math.pi/2/segments
        b=math.pi-(i+1)*math.pi/2/segments
        x0,y0=1+radius*math.cos(a),.15+radius*math.sin(a)
        x1,y1=1+radius*math.cos(b),.15+radius*math.sin(b)
        for z,n in [(0,[0,0,-1]),(1,[0,0,1])]:
            poly([[x0,y0,z],[x1,y1,z],[x1,1,z],[x0,1,z]],'stone_1',n)
        before=len(mesh.parts['stone_1']['position'])
        middle=(a+b)/2
        poly([[x0,y0,0],[x0,y0,1],[x1,y1,1],[x1,y1,0]],'stone_1',[-math.cos(middle),-math.sin(middle),0])
        group=mesh.parts['stone_1']
        for j,p in enumerate(group['position'][before:],before):
            x=1-p[0] if right else p[0]
            angle=math.acos(max(-1,min(1,(x-1)/radius)))
            normal=[-math.cos(angle),-math.sin(angle),0]
            if right:normal[0]*=-1
            group['normal'][j]=normal
            group['uv'][j]=[p[2],radius*(math.pi-angle)]
            group['tangent'][j]=[0,0,1,1 if right else -1]
        low_x,high_x=(1-x1,1-x0) if right else (x0,x1)
        mesh.collision.append({'minimum':[low_x,min(y0,y1),0],'maximum':[high_x,1,1]})
