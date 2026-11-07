from pathlib import Path
s=Path('/tmp/teramic173-hub-review.py').read_text().split('bad=[];nonconvex=[];examined=0')[0]
n={};exec(s,n)
import math,json
D=n['D'];N=n['N'];R=.15;add=n['add'];sub=n['sub'];scale=n['scale']
cross=lambda a,b:(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def path(a,b):
    pivot=scale(add(a,b),.25);p=[(scale(a,.5),scale(b,-1))]
    for step in range(9):
        t=step*math.pi/16;rad=add(scale(b,-math.cos(t)),scale(a,-math.sin(t)))
        p.append((add(pivot,scale(rad,.25)),rad))
    p.append((scale(b,.5),scale(a,-1)))
    return p

def prism(pair,binormal):
    rings=[]
    for center,normal in pair:
        rings.append([add(center,scale(add(scale(normal,math.cos(i*math.tau/N)),scale(binormal,math.sin(i*math.tau/N))),R)) for i in range(N)])
    tris=[]
    for i in range(N):
        j=(i+1)%N;q=[rings[0][i],rings[1][i],rings[1][j],rings[0][j]]
        tris.extend([[q[0],q[1],q[2]],[q[0],q[2],q[3]]])
        for k in range(2):tris.append([pair[k][0],rings[k][i],rings[k][j]])
    return tris
bad=[];nonconvex=[];examined=0
for sa in range(6):
  for sb in range(sa+1,6):
    if sa//2==sb//2:continue
    a,b=D[sa],D[sb];p=path(a,b);binormal=cross(a,b)
    for part,pair in enumerate(zip(p,p[1:])):
      tris=prism(pair,binormal)
      for axis in range(3):
        for i in range(41):
          plane=-.1999+i*.00999;v=n['section'](tris,axis,plane);examined+=1
          if v is None:continue
          if 'bad' in v:bad.append([sa,sb,part,axis,plane,v]);continue
          if v['gap_area']>1e-9:nonconvex.append([sa,sb,part,axis,plane+.5,v])
print(json.dumps({'examined':examined,'bad_contours':len(bad),'nonconvex':len(nonconvex),'worst':sorted(nonconvex,key=lambda x:x[-1]['gap_area'],reverse=True)[:5],'bad_examples':bad[:3]},indent=2))
