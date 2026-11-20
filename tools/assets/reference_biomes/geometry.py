"""Closed voxel botany with distinct silhouettes, surfaces and visible petals."""
import math
from voxel_props.geometry import VoxelSculpt

def flower(s, center, size, material='petal'):
    x,y,z=center
    # Two alternating rings keep flower heads readable from eye level and above.
    for i in range(6):
        a=i*math.tau/6
        s.blob([x+math.cos(a)*size*.58,y+(i%2)*size*.14,z+math.sin(a)*size*.58],
               [size*.44,size*.18,size*.44],material)
    s.blob([x,y+size*.20,z],[size*.25,size*.20,size*.25],'gold')

def stalk(s,x,z,h,material='stem',bend=.045):
    s.branch([x,.02,z],[x+bend,h*.55,z+.025],.014,material)
    s.branch([x+bend,h*.55,z+.025],[x,h,z],.012,material)
    for sign,y in [(-1,h*.30),(1,h*.55)]:
        s.branch([x,y,z],[x+sign*.11,y+.065,z+.03],.016,'leaf')
        s.blob([x+sign*.10,y+.06,z+.03],[.10,.025,.045],'leaf_light')

def sculpture(material):
    s=VoxelSculpt(32)
    if material==423:
        for i,(x,z,h) in enumerate([(.23,.30,.45),(.50,.48,.70),(.77,.67,.52)]):
            stalk(s,x,z,h)
            flower(s,[x,h,z],.15,'petal' if i!=1 else 'pink')
    elif material==424:
        for x,z,h in [(.30,.36,.87),(.66,.61,.66)]:
            stalk(s,x,z,h)
            for j in range(7):
                y=h*.45+j*h*.067;r=.10*(1-j*.085)
                for i in range(4):
                    a=i*math.tau/4+j*.7
                    s.blob([x+math.cos(a)*r,y,z+math.sin(a)*r],[r*.65,.035,r*.65],
                           'violet' if j%3 else 'pale')
    elif material==425:
        for i in range(7):
            a=i*math.tau/7
            tip=[.5+math.cos(a)*.40,.32+(i%3)*.10,.5+math.sin(a)*.40]
            start=[.5,.025,.5]
            s.branch(start,tip,.014,'stem')
            for j in range(1,7):
                t=j/7;c=[start[k]+(tip[k]-start[k])*t for k in range(3)]
                for side in (-1,1):
                    length=.13*(1-t*.65)
                    end=[c[0]+math.cos(a+side*1.1)*length,c[1]+.035,
                         c[2]+math.sin(a+side*1.1)*length]
                    s.branch(c,end,.022,'leaf' if (i+j)%2 else 'leaf_light')
    elif material==426:
        junction = [.5,.18,.5]
        s.branch([.5,0,.5],junction,.03,'wood',.02)
        for i,(x,z,h) in enumerate([(.27,.3,.43),(.65,.29,.52),(.32,.65,.58),(.68,.66,.45)]):
            elbow = [(x+.5)*.5,h*.58,(z+.5)*.5]
            s.branch(junction,elbow,.02,'wood',.015)
            s.branch(elbow,[x,h,z],.015,'stem',.012)
            s.blob([elbow[0]-.055,elbow[1],elbow[2]],[.07,.024,.035],'leaf')
            s.blob([elbow[0]+.055,elbow[1]+.035,elbow[2]],[.07,.024,.035],'leaf_light')
            flower(s,[x,h,z],.085,'petal' if i%2 else 'pink')
    elif material==427:
        for i in range(5):
            x=.2+i*.15;z=.3+(i%2)*.3;h=.43+(i%3)*.07
            stalk(s,x,z,h,'wood',.02)
            for j in range(4):
                flower(s,[x+.025*(j%2),h*.50+j*.075,z],.065,'pink' if i%2 else 'pale')
    elif material==428:
        for i in range(9):
            a=i*math.tau/9;r=.25+(i%3)*.035;h=.20+(i%3)*.10
            end=[.5+math.cos(a)*r,h,.5+math.sin(a)*r]
            s.branch([.5,.02,.5],end,.016,'wood')
            for j in range(1,4):
                t=j/4;c=[.5+(end[0]-.5)*t,end[1]*t,.5+(end[2]-.5)*t]
                s.blob(c,[.045,.075,.035],'sand_leaf')
    elif material==429:
        for i in range(7):
            x=.2+(i%3)*.22;z=.22+(i//3)*.25;h=.58+(i%3)*.14
            s.branch([x,0,z],[x+.02,h,z],.012,'reed')
            if i%2==0:s.box([x-.025,h-.12,z-.025],[x+.055,h+.065,z+.025],'seed')
            for side in (-1,1):
                s.branch([x,h*.38,z],[x+side*.16,h*.74,z+.03],.018,'leaf')
    else:
        raise ValueError(material)
    return s
