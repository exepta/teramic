"""Tileable broadleaf bark: long fibres, branch knots and restrained lichen."""
import math
from build_castle import BLOCKS
from build_spirit_blossom import png,noise

def generate():
    size=512;pixels=[]
    for y in range(size):
        v=y/size
        for x in range(size):
            u=x/size
            warped=u+(.021*math.sin(v*math.tau)+.012*math.sin(v*math.tau*3+u*math.tau))
            fibre=(warped*13+.08*math.sin(u*math.tau*4))%1
            shade=.82+.24*math.sin(fibre*math.pi)
            if fibre<.055:shade=.69
            elif fibre<.14:shade=.88
            # Knots bend a few fibres, rather than outlining masonry panels.
            dx=(u-.34+.5)%1-.5;dy=(v-.61+.5)%1-.5
            ring=math.hypot(dx/.095,dy/.17)
            if ring<1:
                shade=.82+.12*math.sin(ring*math.pi*6)
            fine=.97+.055*noise(x//2,y//2,527)
            color=[144,100,60]
            lichen=math.sin(u*math.tau*3+math.sin(v*math.tau*2))
            if lichen>.90 and noise(x//11,y//13,29)>.82:
                color=[128,133,83];shade=.92
            pixels.append([int(min(255,c*shade*fine)) for c in color]+[255])
    png(BLOCKS/'textures/wood.png',pixels,size)
