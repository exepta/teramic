"""Conservative closed-cuboid replacement preserving original UV mapping."""
import math
from collections import defaultdict
from build_castle import Mesh


def components(positions,indices):
    welded={};ids=[]
    for p in positions:
        key=tuple(round(x,6) for x in p);ids.append(welded.setdefault(key,len(welded)))
    parent=list(range(len(indices)//3))
    def root(a):
        while parent[a]!=a:parent[a]=parent[parent[a]];a=parent[a]
        return a
    edges={}
    for ti in range(len(parent)):
        t=[ids[i] for i in indices[ti*3:ti*3+3]]
        for a,b in zip(t,t[1:]+t[:1]):
            key=tuple(sorted((a,b)))
            if key in edges:parent[root(ti)]=root(edges[key])
            else:edges[key]=ti
    groups=defaultdict(list)
    for i in range(len(parent)):groups[root(i)].append(i)
    return groups.values()


def box_bounds(positions,indices,group):
    if len(group)!=12:return None
    vertices={positions[indices[t*3+i]] for t in group for i in range(3)}
    if len(vertices)!=8:return None
    lo=[min(p[i] for p in vertices) for i in range(3)];hi=[max(p[i] for p in vertices) for i in range(3)];size=[b-a for a,b in zip(lo,hi)]
    if min(size)<.035 or max(size)>min(size)*16:return None
    if any(any(abs(p[i]-lo[i])>1e-6 and abs(p[i]-hi[i])>1e-6 for i in range(3)) for p in vertices):return None
    # Every face must be an axis plane; ignore merely box-shaped sloped meshes.
    for t in group:
        triangle=[positions[indices[t*3+i]] for i in range(3)]
        if not any(max(p[k] for p in triangle)-min(p[k] for p in triangle)<1e-6 for k in range(3)):return None
    return lo,hi


def replacement(attributes,indices,group,bounds):
    lo,hi=bounds;mesh=Mesh('polish');mesh.box(lo,hi,'surface',max(.0035,min(b-a for a,b in zip(lo,hi))*.06))
    part=mesh.parts['surface'];old=attributes['POSITION'];source={}
    for t in group:
        ids=indices[t*3:t*3+3];points=[old[i] for i in ids]
        axis=next(i for i in range(3) if max(p[i] for p in points)-min(p[i] for p in points)<1e-6)
        sign=1 if abs(points[0][axis]-hi[axis])<1e-6 else -1
        source.setdefault((axis,sign),[]).append(ids)
    out={key:[] for key in attributes}
    for p,n in zip(part['position'],part['normal']):
        axis=max(range(3),key=lambda i:abs(n[i]));sign=1 if n[axis]>0 else -1
        u,v=[i for i in range(3) if i!=axis];selected=None
        for ids in source[(axis,sign)]:
            a,b,c=[old[i] for i in ids];den=(b[v]-c[v])*(a[u]-c[u])+(c[u]-b[u])*(a[v]-c[v])
            if abs(den)<1e-12:continue
            w0=((b[v]-c[v])*(p[u]-c[u])+(c[u]-b[u])*(p[v]-c[v]))/den
            w1=((c[v]-a[v])*(p[u]-c[u])+(a[u]-c[u])*(p[v]-c[v]))/den;weights=[w0,w1,1-w0-w1]
            if min(weights)>-1e-5:selected=(ids,weights);break
        if selected is None:raise ValueError('No source face covers bevel UV projection')
        ids,weights=selected
        for key,values in attributes.items():
            value=p if key=='POSITION' else n if key=='NORMAL' else tuple(sum(values[i][k]*w for i,w in zip(ids,weights)) for k in range(len(values[0])))
            if key=='TANGENT':
                dot=sum(value[k]*n[k] for k in range(3));xyz=[value[k]-dot*n[k] for k in range(3)];length=math.sqrt(sum(x*x for x in xyz))
                value=tuple(x/length for x in xyz)+(1 if value[3]>=0 else -1,)
            out[key].append(value)
    return out,part['indices']
