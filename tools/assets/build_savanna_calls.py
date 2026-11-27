"""Original short elephant and zebra voices for Teramic's 24 kHz spatial mixer."""
import array
import math
from pathlib import Path
import random
import wave

RATE = 24_000
OUT = Path(__file__).resolve().parents[2] / "assets/audio/animals"


def voice(name, seed, phrases, formants):
    noise = random.Random(seed)
    samples = [0.] * int((max(start + duration for start, duration, *_ in phrases) + 0.04) * RATE)
    for start, duration, first, peak, last, tremor in phrases:
        phase = slow_noise = breath = 0.
        for i in range(round(duration * RATE)):
            t = i / RATE
            u = t / duration
            pitch = first + (last - first) * u + (peak - (first + last) / 2) * math.sin(math.pi * u)
            slow_noise += 0.005 * (noise.uniform(-1, 1) - slow_noise)
            breath += 0.21 * (noise.uniform(-1, 1) - breath)
            pitch *= 1 + 0.025 * math.sin(t * tremor * math.tau) + 0.19 * slow_noise
            phase += pitch * math.tau / RATE
            envelope = math.sin(math.pi * u) ** 0.65 * min(1, t * 32, (duration - t) * 16)
            tract = 0.
            for k in range(1, 15):
                hz = k * pitch
                gain = 0.15 / k + sum(g * math.exp(-((hz - f) / w) ** 2) for f, w, g in formants)
                tract += gain * math.sin(k * phase + 0.08 * math.sin(phase * 0.49)) / (1 + 0.12 * k)
            samples[int(start * RATE) + i] += envelope * (tract * (0.88 + 0.12 * math.cos(0.5 * phase)) + 0.32 * breath)
    scale = 0.70 * 32767 / max(map(abs, samples))
    with wave.open(str(OUT / (name + ".wav")), "wb") as tape:
        tape.setparams((1, 2, RATE, 0, "NONE", "not compressed"))
        tape.writeframes(array.array("h", (round(s * scale) for s in samples)).tobytes())


if __name__ == "__main__":
    horn = [(430, 170, .79), (970, 220, .48), (1730, 470, .26)]
    bray = [(600, 160, .5), (1350, 330, .69), (2310, 430, .29)]
    voice("elephant-trumpet", 51, [(0, .88, 188, 420, 228, 11), (.86, .25, 202, 238, 130, 14)], horn)
    voice("elephant-contact", 91, [(0, 1.14, 82, 126, 64, 7)], [(230, 100, .85), (520, 190, .48), (1120, 290, .18)])
    voice("zebra-bark", 72, [(0, .22, 280, 520, 310, 12), (.26, .33, 360, 610, 190, 15)], bray)
    voice("zebra-bray", 183, [(0, .27, 222, 456, 295, 9), (.34, .44, 315, 565, 155, 12), (.81, .20, 219, 310, 143, 15)], bray)
