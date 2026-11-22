"""Build 24 kHz spatial one-shots from CC0 field recordings and edited Foley."""

from functools import cache
import hashlib
from urllib.request import urlopen
import numpy as np
from samples import CACHE, OUT, ROOT, read
from scores import write_wav

RATE = 24_000
SOURCES = {
    "tiger": (
        "263/263115_672090",
        "38a0c9603e5dd819f8140dd637d0521d52609c2304654a28dedd481fd36f974a",
    ),

}
LOCAL = {
    "air": "animals/stalker-airway",
    "feathers": "animals/sheep-shake",
    "leaves": "environment/leaves",
    "surf": "environment/ocean-surf",
    "swim": "animals/swim-stroke-3",
    "splash": "animals/splash",
    "wood": "footsteps/detail/wood-4",
    "sand": "footsteps/sand",
    "dunes": "footsteps/sand-2",
    "gravel": "footsteps/detail/gravel-8",
    "rock": "footsteps/detail/stone-5",
    "ice": "footsteps/detail/glass-3",
    "shard": "footsteps/detail/glass-4",
    "wind": "environment/wind-ambience",
    "creak": "objects/door-open",
    "slosh": "animals/swim-stroke-4",
}


def fetch():
    CACHE.mkdir(parents=True, exist_ok=True)
    for name, (source, checksum) in SOURCES.items():
        path = CACHE / (name + ".mp3")
        if not path.exists():
            url = f"https://cdn.freesound.org/previews/{source}-hq.mp3"
            with urlopen(url, timeout=40) as response:
                path.write_bytes(response.read())
        if hashlib.sha256(path.read_bytes()).hexdigest() != checksum:
            raise ValueError(f"Unexpected recording for {name}")


@cache
def source(name):
    path = (
        CACHE / (name + ".mp3")
        if name in SOURCES
        else ROOT / "assets/audio" / (LOCAL[name] + ".wav")
    )
    return read(path, 1, RATE).ravel()


def band(x, low, high):
    # Pad both ends so filtering cannot join unrelated ends of an excerpt.
    padded = np.pad(x, 1024)
    freq = np.fft.rfftfreq(len(padded), 1 / RATE)
    response = (1 + (low / np.maximum(1.0, freq)) ** 6) ** -0.5 * (1 + (freq / high) ** 8) ** -0.5
    return np.fft.irfft(np.fft.rfft(padded) * response, n=len(padded))[1024:-1024]


class Shot:
    def __init__(self, seconds):
        self.mix = np.zeros(round(seconds * RATE))

    def add(self, name, at, offset, seconds, speed, gain, low, high, attack=0.02, tail=0.14):
        # Speed is a tape ramp, so a caller can open its throat then fall away
        # without a fixed-pitch oscillator or a cycling piece of a short sample.
        start = round(at * RATE)
        n = min(round(seconds * RATE), len(self.mix) - start)
        if n <= 1:
            return
        take = source(name)
        speed = np.broadcast_to(speed, (2,))
        index = offset * RATE + np.cumsum(np.linspace(*speed, n))
        n = min(n, np.searchsorted(index, len(take) - 1))
        if n <= 1:
            return
        x = band(np.interp(index[:n], np.arange(len(take)), take), low, high)
        local = np.sqrt(np.mean(x * x))
        x *= min(12, 0.17 / max(0.008, local))
        t = np.arange(n) / RATE
        env = np.minimum(1, t / attack) * np.minimum(1, (t[-1] - t) / tail)
        self.mix[start : start + n] += x * env * gain

    def save(self, path, peak):
        x = band(self.mix, 38, 10700)
        # Sparse early reflections. The positional room processor supplies the
        # larger room tail at playback according to the player's surroundings.
        dry = x.copy()
        for t, g in ((0.027, 0.10), (0.064, 0.07), (0.102, 0.04)):
            off = round(t * RATE)
            x[off:] += dry[:-off] * g
        x = np.tanh(x * 1.45)
        x *= peak / max(1e-6, abs(x).max())
        n = round(0.025 * RATE)
        x[:n] *= np.linspace(0, 1, n)
        x[-n:] *= np.linspace(1, 0, n)
        write_wav(path, x[:, None], RATE)


def rasp(shot, at, duration, seed, throat, grain, gain):
    """Unvoiced air tearing across an irregular reed or a ring of teeth.

    Each pulse has a different spacing, damping and formant. This avoids the
    sustained harmonic fundamental of a pitched mammal sample. A noise floor
    links the individual grains into a single exhalation.
    """
    rng = np.random.default_rng(seed)
    start = round(at * RATE)
    n = min(round(duration * RATE), len(shot.mix) - start)
    t = np.arange(n) / RATE
    u = t / duration
    noise = rng.normal(size=n)
    breath = band(noise, 530, 4300)
    grit = np.zeros(n)
    pos = 0
    while pos < n:
        openness = np.sin(np.pi * pos / n)
        step = rng.uniform(0.55, 1.6) / (grain * (0.72 + 0.76 * openness))
        size = min(n - pos, round(RATE * rng.uniform(0.017, 0.054)))
        v = np.arange(size) / RATE
        decay = np.exp(-v * rng.uniform(90, 200))
        ring = rng.uniform(0.74, 1.32) * throat
        tooth = np.sin(2 * np.pi * ring * v)
        tooth += 0.47 * np.sin(2 * np.pi * (2.56 * ring * v + 410 * v * v))
        attack = np.minimum(1, v * 2600)
        grit[pos : pos + size] += rng.uniform(0.65, 1.2) * tooth * decay * attack
        pos += max(1, round(step * RATE))
    # The first contraction opens abruptly; the back of the throat frays as
    # air pressure drops. There is no copied loop or continuous sung pitch.
    contractions = np.arange(0, duration + 0.06, 0.06)
    flow = np.interp(t, contractions, rng.uniform(0.56, 1.12, size=len(contractions)))
    env = np.minimum(1, t / 0.036) * np.minimum(1, (t[-1] - t) / min(0.24, duration * 0.38))
    air = 0.14 * breath * (0.57 + u) + 0.25 * band(noise, 54, 205)
    shot.mix[start : start + n] += gain * (0.43 * grit + air) * env * flow


def dragon_voice(species, cue):
    # Distinct jaw attacks, then a fraying tail of the dragon's own element.
    durations = {"roar": (2.72, 3.15, 2.77), "hurt": (0.76, 0.89, 0.78), "death": (3.72, 3.93, 3.51)}
    duration = durations[cue][species]
    shot = Shot(duration)
    seed = 701 + species * 31 + ("roar", "hurt", "death").index(cue)
    is_hurt = cue == "hurt"
    start = 0.052 if is_hurt else 0.21
    length = duration * (0.69 if cue == "death" else 0.82)
    shot.add("air", 0, 0.01, start + 0.15, (1.62, 1.12), 0.62, 760, 6800, 0.016, 0.07)
    shot.add("wood", start - 0.018, 0, 0.13, 1.23, 0.64, 200, 3900, 0.002, 0.045)
    rasp(shot, start, length, seed, (730, 475, 1080)[species], (69, 44, 86)[species], 1.0)
    # Use brief high-passed snarls as transients. Weight comes from moving
    # air and rock, leaving the nasal 150–400 Hz animal call out of the body.
    for j, phase in enumerate((0.0, 0.24, 0.52)):
        shot.add("tiger", start + phase * length, 1.24 + 0.16 * j,
                 min(length * 0.44, 0.38), (1.45 - 0.12 * j, 1.12),
                 0.49 - j * 0.09, 630 + 115 * j, 5900, 0.025, 0.085)
    if species == 0:
        shot.add("wind", start, 2.1, length, (1.27, 0.58), 0.54, 58, 420, 0.05, 0.24)
        for j, phase in enumerate((0.08, 0.42, 0.69)):
            shot.add("creak", start + length * phase, 0.20, 0.48, (1.8, 1.15),
                     0.47 - 0.08 * j, 1050, 6400, 0.012, 0.12)
    elif species == 1:
        shot.add("surf", start, 6.82, length, (0.75, 0.57), 0.76, 48, 320, 0.045, 0.33)
        for j, phase in enumerate((0.04, 0.29, 0.52, 0.72)):
            shot.add("gravel", start + length * phase, 0, 0.54, (0.56 + j * 0.09, 0.69),
                     0.62 - 0.08 * j, 310, 4100, 0.006, 0.12)
    else:
        shot.add("surf", start, 6.9, length, (1.32, 0.70), 0.85, 180, 5400, 0.08, 0.21)
        for j, phase in enumerate((0.12, 0.23, 0.58)):
            shot.add("shard", start + phase * length, 0, 0.30, 0.9 + 0.23 * j,
                     0.39 - 0.07 * j, 1500, 8700, 0.003, 0.06)
    if cue == "death":
        shot.add("air", duration * 0.60, 0.035, 0.90, 0.79, 0.56, 340, 4100, 0.07, 0.26)
        shot.add(("wood", "rock", "splash")[species], duration * 0.81, 0, 0.7,
                 0.59, 0.78, 50, 3400, 0.005, 0.18)
    return shot


def dragon_foley(species, cue):
    if cue in ("roar", "hurt", "death"):
        return dragon_voice(species, cue)
    s = Shot({"wing": 1.14, "land": 1.62, "cast": 2.28}[cue])
    material = ["creak", "gravel", "splash"][species]
    if cue == "wing":
        s.add("feathers", 0.02, 0.02, 0.61, (0.64, 1.20), 0.82, 85, 5200, 0.09, 0.12)
        s.add("leaves", 0.08, 1.12, 0.46, (1.65, 1.03), 0.39, 550, 6900, 0.09, 0.09)
        s.add("wind", 0.09, 2.4, 0.85, (1.27, 0.66), 0.65, 65, 980, 0.19, 0.32)
        s.add(material, 0.46, 0.00, 0.45, (0.72, 0.95), 0.26, 150, 4700, 0.025, 0.19)
    elif cue == "land":
        for j, (at, amp) in enumerate([(0.02, 1), (0.093, 0.86), (0.28, 0.49), (0.43, 0.27)]):
            s.add("rock", at, 0, 0.59, (0.49 + j * 0.07, 0.64), amp, 42, 2600, 0.006, 0.21)
            s.add(
                material, at + 0.016, 0, 0.61, 0.71 + j * 0.07, 0.47 * amp, 120, 4800, 0.007, 0.13
            )
        s.add("wind", 0.06, 1.3, 0.95, 0.60, 0.36, 44, 330, 0.08, 0.44)
    elif species == 0:
        s.add("air", 0.015, 0.05, 0.57, (0.79, 1.29), 0.71, 240, 6500, 0.10, 0.09)
        rasp(s, 0.24, 1.21, 831, 882, 55, 0.80)
        s.add("wind", 0.34, 1.32, 1.73, (1.32, 0.68), 0.82, 125, 3200, 0.22, 0.51)
        for j, t in enumerate([0.31, 0.58, 0.83]):
            s.add("shard", t, 0, 0.54, 0.73 - 0.07 * j, 0.24, 840, 6700)
    elif species == 1:
        rasp(s, 0.02, 0.66, 832, 480, 41, 0.75)
        for j, t in enumerate([0.17, 0.34, 0.43, 0.59, 0.85, 1.19]):
            s.add(
                ["wood", "rock", "gravel"][j % 3],
                t,
                0,
                0.88,
                (0.48 + j * 0.09, 0.69),
                1 - j * 0.11,
                50,
                5000,
                0.004,
                0.24,
            )
        s.add("dunes", 1.25, 0, 0.9, 0.63, 0.31, 160, 2800)
    else:
        s.add("tiger", 0.0, 1.04, 0.73, (0.74, 1.12), 0.45, 470, 4700)
        s.add("surf", 0.09, 4.5, 1.9, (1.38, 0.71), 1.01, 140, 6100, 0.19, 0.35)
        s.add("splash", 0.26, 0.02, 0.85, (1.12, 0.96), 0.73, 70, 3800, 0.02, 0.25)
        for j, t in enumerate([0.15, 0.43, 0.62, 0.95, 1.52]):
            s.add(
                ["ice", "shard"][j % 2], t, 0.0, 0.59, 1.11 + j * 0.12, 0.29 - j * 0.032, 720, 9200
            )
    return s


def worm(cue, take):
    dur, peak = {
        "roar": (2.73, 0.67),
        "hurt": (0.78, 0.56),
        "death": (3.62, 0.64),
        "dig": (1.05, 0.44),
        "landing": (1.35, 0.62),
        "rumble": (1.80, 0.56),
        "bite": (0.72, 0.63),
        "spit": (1.13, 0.54),
        "breath": (1.32, 0.32),
    }[cue]
    s = Shot(dur + [0, 0.11, -0.07][take])
    v = [1.0, 1.105, 0.91][take]
    if cue in ("roar", "hurt", "death"):
        at = 0.038 if cue == "hurt" else 0.13
        s.add("sand", 0, 0.0, 0.47, 0.72 * v, 0.54, 200, 4700, 0.012, 0.09)
        s.add("air", 0.008, 0.01, 0.48, 1.18 * v, 0.55, 660, 6900, 0.025, 0.15)
        length = dur * (0.64 if cue == "death" else 0.83)
        seed = 300 + take * 17 + ("roar", "hurt", "death").index(cue)
        rasp(s, at, length, seed, 980 * v, 111 * v, 0.99)
        s.add("surf", at, 6.88 + take * 0.31, length, (0.59, 0.78),
              0.74, 39, 158, 0.055, 0.22)
        s.add("air", at + 0.01, 0.03, length * 0.64, (1.54, 1.23),
              0.53, 1160, 7100, 0.029, 0.18)
        # The ring of teeth closes in a wave of near, then distant clicks.
        for j, phase in enumerate((0.03, 0.095, 0.19, 0.31, 0.53, 0.74)):
            s.add(("ice", "wood", "gravel")[j % 3], at + phase * length * v,
                  0.0, 0.38, (1.32 - 0.11 * j, 1.21 - 0.09 * j),
                  0.62 - j * 0.055, 540, 7400, 0.002, 0.093)
        if cue == "death":
            for i, at in enumerate([1.86, 2.2, 2.67]):
                s.add(
                    ["wood", "rock", "dunes"][i],
                    at,
                    0,
                    1.15,
                    0.53 * v,
                    1 - i * 0.15,
                    48,
                    4300,
                    0.012,
                    0.4,
                )
    elif cue in ("dig", "rumble", "landing"):
        if cue == "rumble":
            s.add("surf", 0.02, 4.1 + take * 0.74, 1.65, (0.71, 0.93), 0.65, 42, 320, 0.39, 0.42)
            s.add("wind", 0.31, 2.7 + take, 1.17, (0.62, 0.78), 0.54, 42, 280, 0.31, 0.38)
        if cue == "landing":
            s.add("rock", 0.045, 0, 0.78, 0.39 * v, 0.96, 44, 1700, 0.005, 0.25)
        for j, at in enumerate([0.015, 0.135, 0.259, 0.411, 0.645, 0.80]):
            grit = ["sand", "gravel", "dunes", "wood", "sand", "gravel"][(j + take) % 6]
            s.add(
                grit,
                at * (0.79 if cue == "landing" else 1.18),
                0,
                0.60,
                (0.59 * v + j * 0.04, 0.82 * v),
                (0.77 - j * 0.073) * (1 if cue != "rumble" else 0.42),
                70,
                1900 if cue == "rumble" else 4400,
                0.012,
                0.17,
            )
    elif cue == "bite":
        rasp(s, 0.004, 0.22, 381 + take, 1330, 143, 0.67)
        s.add("wood", 0.132, 0, 0.51, 0.69 * v, 1.15, 75, 3600, 0.004, 0.13)
        s.add("ice", 0.138, 0, 0.33, 1.42 * v, 0.47, 1250, 8700, 0.004, 0.13)
        s.add("gravel", 0.259, 0, 0.42, 0.78, 0.44, 175, 4200)
    elif cue == "spit":
        s.add("air", 0.0, 0.015, 0.45, (1.26, 0.94), 0.62, 250, 4400, 0.026, 0.075)
        s.add("swim", 0.16, 0, 0.35, (0.92, 1.25), 0.88, 190, 5100, 0.012, 0.10)
        s.add("slosh", 0.26, 0, 0.62, (1.36, 0.83), 0.81, 440, 6700, 0.018, 0.14)
        s.add("splash", 0.49, 0.03, 0.66, 0.86 * v, 0.50, 125, 3600)
    else:
        s.add("air", 0.0, 0.02, 0.70, 0.72 * v, 0.68, 180, 3300, 0.11, 0.2)
        s.add("surf", 0.41, 6.52 + take * 0.13, 0.86, (0.75, 1.1), 0.54, 490, 5700, 0.18, 0.27)
        s.add("sand", 0.57, 0.04, 0.64, 0.8, 0.32, 660, 4700, 0.09, 0.24)
    return s, peak


if __name__ == "__main__":
    fetch()
    for i, family in enumerate(("dark", "earth", "water")):
        for cue in ("roar", "wing", "land", "cast", "hurt", "death"):
            dragon_foley(i, cue).save(OUT / f"{family}-{cue}.wav", 0.77 if cue == "roar" else 0.67)
    dest = ROOT / "assets/audio/sandworm"
    dest.mkdir(exist_ok=True)
    for take in range(3):
        for cue in ("roar", "hurt", "death", "dig", "landing", "rumble", "bite", "spit", "breath"):
            sound, peak = worm(cue, take)
            sound.save(dest / f"{cue}-{take + 1}.wav", peak)
