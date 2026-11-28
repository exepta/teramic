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


def record_lava_bed():
    rng = random.Random(7441)
    duration = 18
    overlap = RATE
    count = duration * RATE
    releases = []
    clock = rng.uniform(0.4, 0.9)
    while clock < duration + 1:
        releases.append((clock, rng.uniform(0.08, .29), rng.uniform(0.12, 0.30)))
        clock += rng.uniform(0.16, 1.05)
    samples = []
    slow = low = mid = 0.
    next_release = 0
    for i in range(count + overlap):
        t = i / RATE
        white = rng.uniform(-1, 1)
        slow += .00065 * (white-slow)
        low += .018 * (white-low)
        mid += .071 * (white-mid)
        while next_release < len(releases) and t - releases[next_release][0] > .55:
            next_release += 1
        froth = 0.
        for onset, length, strength in releases[next_release:next_release+5]:
            age = t - onset
            if age < 0.:
                break
            froth += strength * (1-math.exp(-age/.027)) * math.exp(-age/length)
        samples.append(math.tanh(3.8*low + 0.4*mid + 71.*slow*low + froth*(15.*low + 2.*mid)))
    # Crossfade the tail into the first second so the loop has no silent seam.
    for i in range(overlap):
        blend = (1-math.cos(math.pi*i/(overlap-1))) * .5
        samples[i] = samples[count+i]*(1-blend) + samples[i]*blend
    with wave.open(str(OUT/'lava-flow.wav'), 'wb') as wav:
        wav.setparams((1, 2, RATE, 0, 'NONE', 'not compressed'))
        wav.writeframes(struct.pack('<'+'h'*count, *(round(s*23800) for s in samples[:count])))


def thump(t, base, sweep, decay):
    """A heavy pressure release falls in pitch as the cavity opens."""
    if t < 0:
        return 0.
    phase = math.tau * (base*t + sweep*.045*(1-math.exp(-t/.045)))
    return math.sin(phase) * (1-math.exp(-120*t)) * math.exp(-t*decay)


def bubble(t, low, mid, high):
    # A heavy gas pocket opens, pushes a thick fold aside, and falls back.
    # Short, noisy pressure fronts avoid the pure, high sliding tones of a water drop.
    swell = (1-math.exp(-29*t))*math.exp(-8*t)
    front = thump(t-.031, 48, 24, 28) + .24*thump(t-.144, 60, 18, 25)
    fold = max(t-.12, 0.)
    collapse = (1-math.exp(-27*fold))*math.exp(-13*fold)
    return .19*front + swell*(4.8*low+.25*mid) + collapse*(2.45*low+.13*mid)


def crust(t, low, mid, high):
    sound = 0.
    for delay, gain, decay in [(0.,.6,60),(.036,.24,43),(.111,.13,32),(.186,.08,24)]:
        u = t-delay
        if u >= 0:
            sound += gain*(1.-math.exp(-145*u))*math.exp(-decay*u)*(2.3*low+.34*(mid-low))
    return sound + .07*thump(t-.015, 66, 28, 24)


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
    record_lava_bed()
    record('eruption-blast.wav', 5.3, eruption)
    record('eruption-rumble.wav', 8.5, deep_rumble)
    record('fumarole-hiss.wav', 2., steam)
