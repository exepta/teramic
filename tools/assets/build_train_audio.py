#!/usr/bin/env python3
"""Deterministic original railway Foley: filtered steam, rods and rail joints."""
import math, random, wave, struct
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2] / 'assets/audio/trains'
ROOT.mkdir(parents=True, exist_ok=True)
def write(name, samples):
    peak=max(abs(x) for x in samples) or 1
    with wave.open(str(ROOT/name),'wb') as out:
        out.setparams((1,2,24000,0,'NONE','not compressed'))
        out.writeframes(b''.join(struct.pack('<h',int(max(-1,min(1,x/peak*.72))*32767)) for x in samples))
randomizer=random.Random(219)
steam=[]; rolling=[]; low=0.; rumble=0.
for i in range(48000):
    t=i/24000; noise=randomizer.uniform(-1,1)
    low += .09*(noise-low); rumble += .018*(noise-rumble)
    edge=min(1,i/720,(47999-i)/720)
    steam.append(edge*(low*.85 + noise*.10 + math.sin(t*math.tau*52)*.018))
    joint=t%0.5
    knock=math.exp(-joint*62)*(math.sin(joint*math.tau*165)+.45*math.sin(joint*math.tau*367))
    second=(t-.055)%0.5
    knock+=.6*math.exp(-second*85)*math.sin(second*math.tau*235)
    rolling.append(edge*(rumble*1.1+noise*.018+knock*.3))
write('locomotive-steam.wav',steam)
write('rail-rolling.wav',rolling)
