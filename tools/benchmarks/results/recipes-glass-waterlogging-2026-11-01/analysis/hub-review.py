import math,json
R=.23; B=.15; N=16;TAU=math.tau
D=[[-1,0,0],[1,0,0],[0,-1,0],[0,1,0],[0,0,-1],[0,0,1]]
def add(a,b): return tuple(x+y for x,y in zip(a,b))
def scale(a,b): return tuple(x*b for x in a)
def cross2(a,b):return a[0]*b[1]-a[1]*b[0]
def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def point(side,ring,sector,active):
    axis=side//2;out=D[side];u=D[((axis+1)%3)*2+1];v=D[((axis+2)%3)*2+1]
    angle=sector*TAU/N;s=math.sin(angle);c=math.cos(angle)
    boundary=R/math.sqrt(1+max(abs(c),abs(s))**2)
    start=B if active else 0
    radius=start+(boundary-start)*ring/3
    return add(scale(out,math.sqrt(max(0,R*R-radius*radius))),add(scale(u,c*radius),scale(v,s*radius)))
def triangles(mask):
    out=[]
    for side in range(6):
        active=bool(mask&(1<<side))
        for ring in range(3):
            for sector in range(N):
                q=[point(side,ring,sector,active),point(side,ring+1,sector,active),point(side,ring+1,sector+1,active),point(side,ring,sector+1,active)]
                out.extend([[q[0],q[1],q[2]],[q[0],q[2],q[3]]])
        if active:
            cap=scale(D[side],math.sqrt(R*R-B*B))
            out.extend([[cap,point(side,0,i,True),point(side,0,i+1,True)] for i in range(N)])
    return out

def hull(points):
    ps=sorted(set(points));h=[]
    for p in ps:
        while len(h)>=2 and cross2(sub(h[-1],h[-2]),sub(p,h[-1]))<=1e-16:h.pop()
        h.append(p)
    lo=len(h)
    for p in reversed(ps[:-1]):
        while len(h)>lo and cross2(sub(h[-1],h[-2]),sub(p,h[-1]))<=1e-16:h.pop()
        h.append(p)
    return h[:-1]
def area(p):return abs(sum(cross2(a,b) for a,b in zip(p,p[1:]+p[:1])))/2

def section(tris,axis,plane):
    uv=[i for i in range(3) if i!=axis]; graph={};coords={}
    def key(p):return tuple(round(p[i],10) for i in uv)
    for tri in tris:
        hits=[]
        for a,b in zip(tri,tri[1:]+tri[:1]):
            da=a[axis]-plane;db=b[axis]-plane
            if abs(da)<1e-12:hits.append(a)
            if da*db<0:
                t=da/(da-db);hits.append(add(a,scale(sub(b,a),t)))
        hits=list({key(p):p for p in hits}.values())
        if len(hits)!=2:continue
        a,b=map(key,hits)
        if a==b:continue
        graph.setdefault(a,set()).add(b);graph.setdefault(b,set()).add(a)
    if not graph:return None
    bad={p:len(ns) for p,ns in graph.items() if len(ns)!=2}
    if bad:return {'bad':len(bad),'n':len(graph)}
    start=next(iter(graph));poly=[start];prev=None;p=start
    for _ in range(len(graph)):
        nxt=next(q for q in graph[p] if q!=prev)
        if nxt==start:break
        poly.append(nxt);prev,p=p,nxt
    a=area(poly);ha=area(hull(list(graph)))
    return {'area':a,'hull_area':ha,'gap_area':ha-a,'vertices':len(poly)}

bad=[];nonconvex=[];examined=0
planes=[-.1999+i*.00999 for i in range(41)]
for mask in range(3,64):
    if mask.bit_count()==2:continue
    tris=triangles(mask)
    for axis in range(3):
        for plane in planes:
            v=section(tris,axis,plane)
            examined+=1
            if v is None:continue
            if 'bad' in v:bad.append([mask,axis,plane,v]);continue
            if v['gap_area']>1e-9:nonconvex.append([mask,axis,plane+.5,v])
nonconvex.sort(key=lambda v:v[-1]['gap_area'],reverse=True)
print(json.dumps({'examined':examined,'bad_contours':len(bad),'nonconvex':len(nonconvex),'worst':nonconvex[:8],'bad_examples':bad[:3]},indent=2))
