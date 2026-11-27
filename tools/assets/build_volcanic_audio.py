"""Synthesize original mono 24 kHz Foley for molten rock and pressurized steam."""
import math
import random
import struct
import wave
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / 'assets/audio/environment'
RATE = 24000


def record(name, duration, voice):
    rng = random.Random(4599)
    samples = []
    low = mid = prev = 0.
    for i in range(round(RATE * duration)):
        t = i / RATE
        white = rng.uniform(-1, 1)
        low += .027 * (white-low)
        mid += .25 * (white-mid)
        sample = voice(t, low, mid, white-prev)
        prev = white
        edge = min(1., t/.006, (duration-t)/.018)
        samples.append(sample*edge)
    gain = 24500 / max(map(abs, samples))
    with wave.open(str(OUT/name), 'wb') as wav:
        wav.setparams((1, 2, RATE, 0, 'NONE', 'not compressed'))
        wav.writeframes(struct.pack('<'+'h'*len(samples), *(round(s*gain) for s in samples)))


def thump(t, base, sweep, decay):
    """A heavy pressure release falls in pitch as the cavity opens."""
    if t < 0:
        return 0.
    phase = math.tau * (base*t + sweep*.045*(1-math.exp(-t/.045)))
    return math.sin(phase) * (1-math.exp(-120*t)) * math.exp(-t*decay)


def bubble(t, low, mid, high):
    # A heavy gas pocket opens, pushes a thick fold aside, and falls back.
    # Short, noisy pressure fronts avoid the pure, high sliding tones of a water drop.
    swell = (1-math.exp(-34*t))*math.exp(-7.5*t)
    front = thump(t-.036, 57, 39, 15) + .32*thump(t-.23, 73, 21, 16)
    return .41*front + swell*(3.8*low+.32*mid) + .3*low*math.exp(-4.7*t)


def crust(t, low, mid, high):
    sound = 0.
    for delay, gain, decay in [(0.,.6,49),(.041,.36,35),(.123,.18,29),(.218,.11,18)]:
        u = t-delay
        if u >= 0:
            sound += gain*(1.-math.exp(-260*u))*math.exp(-decay*u)*(1.8*low+.66*(mid-low))
    return sound + .12*thump(t-.015, 81, 32, 13)


def lava_bed(t, low, mid, high):
    # Broadband stone movement rolls underneath the individual gas breaches.
    # Low, uneven pulses add weight while remaining audible on small speakers.
    drift = .68+.16*math.sin(.93*t+.8*math.sin(1.81*t))+.13*math.sin(2.77*t+.61)
    grit = .5+.5*math.sin(3.16*t+.82*math.sin(1.55*t))
    return (2.65*low + .13*mid + .08*(mid-low)*grit)*drift


def eruption(t, low, mid, high):
    front = thump(t, 34, 93, 1.8) + .56*thump(t-.29, 41, 71, 2.1)
    air = (1-math.exp(-32*t))*math.exp(-.83*t)
    fracture = sum(.22*thump(t-d, 70, 53, 7) for d in [.10,.51,.82,1.37])
    return math.tanh(front*1.4 + fracture + (2.5*low+.17*mid)*air)


def deep_rumble(t, low, mid, high):
    # Gravel grinds over the bass fundamental in rolling, irregular surges.
    pulse = .37 + .17*math.sin(t*1.82+.8*math.sin(t*2.2)) + .16*math.sin(t*3.77)
    body = math.sin(math.tau*38*t + .8*math.sin(t*4.2))
    grit = math.tanh(2.2*low) * (.65+.25*math.sin(t*5.8+.9*math.sin(t*2.7)))
    return pulse * (.34*body+grit) + .17*thump(t%2.83, 30, 22, 2.1)


def steam(t, low, mid, high):
    envelope = (1-math.exp(-14*t))*math.exp(-1.9*t)
    breath = .8+.2*math.sin(14*t+math.sin(t*6))
    return envelope*breath*(mid+.20*high+.14*low)


if __name__ == '__main__':
    record('lava-bubble.wav', .83, bubble)
    record('lava-crust.wav', .54, crust)
    record('lava-flow.wav', 11.4, lava_bed)
    record('eruption-blast.wav', 5.3, eruption)
    record('eruption-rumble.wav', 8.5, deep_rumble)
    record('fumarole-hiss.wav', 2., steam)
