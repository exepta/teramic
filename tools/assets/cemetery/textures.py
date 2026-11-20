"""Individual tileable 128px masonry/clay textures, never a shared texture atlas."""
import math
from build_spirit_blossom import png,noise

def write(path,color,family):
    size=128;pixels=[]
    for y in range(size):
        for x in range(size):
            grain=.90+.12*noise(x//2,y//2,76)+.035*noise(x,y,11)
            brick=(y//32)%2
            seam=y%32<2 or (x+brick*32)%64<2
            bevel=y%32 in (2,3) or (x+brick*32)%64 in (2,3)
            f=grain*(.57 if seam else 1.08 if bevel else 1)
            rgb=[max(0,min(255,int(c*f))) for c in color]
            if 'cracked' in family and abs(x-(44+13*math.sin(y*.052)))<1.6:rgb=[round(c*.35) for c in rgb]
            if family in ('moss_stone','overgrown_stone'):
                growth=noise(x//12,y//12,313)
                if growth>(.62 if family=='overgrown_stone' else .78):rgb=[int(c*grain) for c in (66,91,43)]
            if family=='ornate_stone':
                r=max(abs(x-64),abs(y-64))
                if abs(r-40)<2 or abs(r-31)<2:rgb=[round(c*.60) for c in rgb]
            if family in ('rune','inactive_rune'):
                a=math.atan2(y-64,x-64);r=math.hypot(x-64,y-64)
                if abs(r-(a+math.pi)*6-8)<2.0:rgb=[205,101,247] if family=='rune' else [80,65,92]
            pixels.append(rgb+[255])
    png(path,pixels,size)
