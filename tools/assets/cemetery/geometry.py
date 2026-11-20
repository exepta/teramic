"""Closed low-poly ornament primitives with geometric normals and metre UVs."""
import math
from build_castle import unit

def ellipsoid(mesh, center, radius, material, segments=12, rings=6):
    def p(j,i):
        a=math.tau*i/segments; b=math.pi*j/rings
        return [center[0]+radius[0]*math.sin(b)*math.cos(a),center[1]+radius[1]*math.cos(b),center[2]+radius[2]*math.sin(b)*math.sin(a)]
    for j in range(rings):
        for i in range(segments):
            points=[p(j,i),p(j+1,i),p(j+1,i+1),p(j,i+1)]
            if j==0: points=points[1:]
            elif j==rings-1: points=[points[0],points[1],points[3]]
            normal=unit([sum(q[k]-center[k] for q in points)/radius[k]**2 for k in range(3)])
            mesh.polygon(points,material,normal)

def cylinder(mesh, low, high, radius, material, sides=12, end_radius=None):
    axis=unit([high[i]-low[i] for i in range(3)])
    from build_castle import cross
    u=unit(cross(axis,[0,0,1] if abs(axis[2])<.8 else [1,0,0]));v=cross(axis,u)
    end_radius=radius if end_radius is None else end_radius
    rings=[[ [c[k]+r*(u[k]*math.cos(math.tau*i/sides)+v[k]*math.sin(math.tau*i/sides)) for k in range(3)] for i in range(sides)] for c,r in [(low,radius),(high,end_radius)]]
    for i in range(sides):
        n=unit([u[k]*math.cos(math.tau*(i+.5)/sides)+v[k]*math.sin(math.tau*(i+.5)/sides) for k in range(3)])
        mesh.polygon([rings[0][i],rings[0][(i+1)%sides],rings[1][(i+1)%sides],rings[1][i]],material,n)
    mesh.polygon(rings[0],material,[-a for a in axis]);mesh.polygon(rings[1],material,axis)

def arch(mesh, center, inner, outer, depth, material, segments=14):
    # Semi-circular voussoir ring, closed at front/back/inside/outside and ends.
    x,y,z=center
    for i in range(segments):
        a=math.pi*i/segments+.006;b=math.pi*(i+1)/segments-.006
        def p(r,t,d):return[x+math.cos(t)*r,y+math.sin(t)*r,z+d]
        front=[p(inner,a,-depth/2),p(outer,a,-depth/2),p(outer,b,-depth/2),p(inner,b,-depth/2)]
        back=[p(inner,a,depth/2),p(outer,a,depth/2),p(outer,b,depth/2),p(inner,b,depth/2)]
        mesh.polygon(front,material,[0,0,-1]);mesh.polygon(back,material,[0,0,1])
        for j in range(4):mesh.polygon([front[j],back[j],back[(j+1)%4],front[(j+1)%4]],material)

def cross(mesh, x,y,z,size,material):
    mesh.box([x-size*.12,y-size*.48,z-size*.10],[x+size*.12,y+size*.48,z+size*.10],material,.012)
    mesh.box([x-size*.36,y+size*.06,z-size*.10],[x+size*.36,y+size*.24,z+size*.10],material,.012)

def candle(mesh,x,z,h=.3):
    cylinder(mesh,[x,.04,z],[x,h,z],.035,'wax',12)
    cylinder(mesh,[x,h,z],[x,h+.035,z],.008,'iron',8)
    ellipsoid(mesh,[x,h+.06,z],[.026,.060,.026],'flame',8,5)

def skull(mesh,center,size=.18):
    x,y,z=center
    ellipsoid(mesh,[x,y,z],[size*.47,size*.5,size*.39],'bone',12,6)
    mesh.box([x-size*.30,y-size*.42,z-size*.35],[x+size*.30,y-size*.20,z+size*.07],'bone',.009)
    for sign in [-1,1]:ellipsoid(mesh,[x+sign*size*.19,y+size*.06,z-size*.34],[size*.13,size*.17,size*.04],'shadow',8,4)
    for i in range(5):mesh.box([x-size*.23+i*size*.095,y-size*.39,z-size*.39],[x-size*.16+i*size*.095,y-size*.23,z-size*.34],'ivory',.002)
    mesh.polygon([[x,y-size*.02,z-size*.405],[x-size*.07,y-size*.15,z-size*.38],[x+size*.07,y-size*.15,z-size*.38]],'shadow',[0,0,-1])

def leaf(mesh,center,scale,material='leaf'):
    x,y,z=center
    a=[x-scale*.2,y-scale*.5,z];b=[x+scale*.5,y,z];c=[x+scale*.2,y+scale*.5,z];d=[x-scale*.5,y,z];m=[x,y,z-.025]
    for p,q in [(a,b),(b,c),(c,d),(d,a)]:mesh.polygon([p,q,m],material);mesh.polygon([q,p,[x,y,z+.004]],material)

def crystal(mesh,center,radius,height):
    x,y,z=center
    cylinder(mesh,[x,y,z],[x,y+height*.68,z],radius,'crystal',6,end_radius=radius*.8)
    cylinder(mesh,[x,y+height*.68,z],[x,y+height,z],radius*.8,'crystal',6,end_radius=.001)
