"""Render the engraved dragon themes to the game's stereo PCM loop assets.

Run samples.py once to populate the pinned VSCO library. NumPy, ffmpeg and
FluidSynth are authoring tools. Pass a path to GeneralUser 1.47 for the choir.
"""

import functools
import math
from pathlib import Path
import random
import re
import subprocess
import sys
import wave

import numpy as np

from compositions import SCORES, VOICINGS
from orchestration import ARRANGE, antiphon
from samples import CACHE, OUT, RATE, read, recordings


SEATS = {
    "bass": 0.58,
    "cello": 0.68,
    "bow": 0.65,
    "viola": 0.53,
    "violin": 0.26,
    "short_violin": 0.25,
    "tremolo": 0.36,
    "solo": 0.41,
    "harp": 0.24,
    "horn": 0.36,
    "horn_stab": 0.37,
    "soft_horn": 0.35,
    "trombone": 0.66,
    "low_stab": 0.67,
    "trumpet": 0.56,
    "flute": 0.46,
    "oboe": 0.55,
    "gong": 0.31,
    "kettle": 0.54,
    "snare": 0.64,
    "rim": 0.65,
    "cymbal": 0.73,
    "roll": 0.72,
    "drum": 0.48,
    "drum2": 0.56,
    "glock": 0.72,
}
LONG = {
    "horn",
    "soft_horn",
    "trumpet",
    "trombone",
    "bass",
    "cello",
    "violin",
    "solo",
    "viola",
    "tremolo",
    "oboe",
    "flute",
}
FAMILY = {
    "dark": (721, 2.3),
    "earth": (507, 1.65),
    "water": (816, 2.05),
}
BANK = {}
for desk, base, take, path in recordings():
    BANK.setdefault(desk, {}).setdefault(take, []).append((base, path))


@functools.cache
def pitch(name):
    match = re.fullmatch(r"([A-G])([#b]?)(-?\d+)", name)
    if not match:
        raise ValueError(f"Invalid written pitch {name}")
    letter, accidental, octave = match.groups()
    natural = dict(C=0, D=2, E=4, F=5, G=7, A=9, B=11)
    return (
        12 * (int(octave) + 1) + natural[letter] + {"#": 1, "b": -1, "": 0}[accidental]
    )


@functools.cache
def sample(path):
    recording = read(CACHE / "vsco" / path)
    # A held orchestral note and a brief drum transient need comparable metering;
    # each recording keeps the performance's attack and natural dynamic contour.
    window = RATE // 16
    strength = max(
        np.sqrt(np.mean(segment * segment))
        for segment in np.array_split(recording, max(1, len(recording) // window))
    )
    return np.ascontiguousarray(
        recording * (0.20 / max(0.002, strength)), dtype=np.float32
    )


def write_wav(path, sound, rate=RATE):
    with wave.open(str(path), "wb") as file:
        file.setparams((sound.shape[1], 2, rate, len(sound), "NONE", "not compressed"))
        file.writeframes(np.asarray(sound * 32767, dtype="<i2").tobytes())


class Stage:
    def __init__(self, name, seconds):
        self.rng = random.Random(FAMILY[name][0])
        self.mix = np.zeros((round((seconds + 7) * RATE), 2), np.float32)
        self.round_robin = {}
        self.choir_notes = []

    def play(self, desk, key, start, duration, level, swell=1.0, connected=False):
        takes = BANK[desk]
        count = self.round_robin.get(desk, 0)
        self.round_robin[desk] = count + 1
        base, path = min(takes[1 + count % len(takes)], key=lambda r: abs(r[0] - key))
        if desk == "kettle":
            base = 46.72  # Recorded timpano's first strong mode is 121.5 Hz.
        source = sample(path)
        speed = 2 ** ((key - base + self.rng.uniform(-0.045, 0.045)) / 12)
        offset = round(self.rng.uniform(0.06, 0.12) * RATE) if connected else 0
        release = 0.17 if desk in LONG else 0.21
        # Keep sustaining pitches just under the conductor's beat on a change of bow/breath.
        lead = 0.038 if desk in LONG else 0.0
        start = max(0, round((start - lead + self.rng.uniform(-0.009, 0.009)) * RATE))
        length = min(
            round((duration + release) * RATE),
            len(self.mix) - start,
            int((len(source) - 1 - offset) / speed),
        )
        if length < 2:
            return
        cursor = offset + np.arange(length) * speed
        i = cursor.astype(np.int32)
        fraction = (cursor - i).astype(np.float32)[:, None]
        signal = source[i] + fraction * (source[i + 1] - source[i])
        position = np.linspace(0.0, 1.0, length, dtype=np.float32)
        expression = 1.0 + (swell - 1.0) * position
        if desk in LONG:
            expression *= 0.87 + 0.17 * np.sin(position * np.pi)
        if connected:
            n = min(length, round(0.045 * RATE))
            expression[:n] *= np.linspace(0, 1, n, dtype=np.float32)
        tail = min(length, round(release * RATE))
        expression[-tail:] *= np.linspace(1.0, 0.0, tail, dtype=np.float32) ** 1.5
        mid = (signal[:, 0] + signal[:, 1]) * 0.5
        signal = 0.73 * signal + 0.27 * mid[:, None]
        seat = SEATS[desk]
        balance = np.array([math.cos(seat * math.pi / 2), math.sin(seat * math.pi / 2)])
        signal *= math.sqrt(2) * balance * (level * self.rng.uniform(0.96, 1.045))
        self.mix[start : start + length] += signal * expression[:, None]


class Bar:
    key = staticmethod(pitch)

    def __init__(self, stage, section, index, start, duration, meter, gain, chord):
        self.stage = stage
        self.section = section.name
        self.index = index
        self.start = start
        self.pulse = duration / meter
        self.meter = meter
        self.level = gain
        self.voicing = tuple(map(pitch, VOICINGS[chord].split()))

    def note(self, desk, key, at, length, level, swell=1.0, connected=False):
        self.stage.play(
            desk,
            pitch(key) if isinstance(key, str) else key,
            self.start + at * self.pulse,
            length * self.pulse,
            level * self.level,
            swell,
            connected,
        )

    def chorus(self, parts, at, length, strength, gallery=False):
        for voice, key in enumerate(parts):
            # Each voice takes its own breath and releases consonants before the cadence.
            onset = self.start + at * self.pulse + 0.028 * voice
            velocity = min(116, round(strength * self.level**0.57))
            self.stage.choir_notes.append(
                (key, onset, length * self.pulse - voice * 0.018, velocity, gallery)
            )

    def rise(self, at, length, level):
        # The source is a nine-second suspended-cymbal crescendo. Unfold its
        # recorded swell towards this downbeat instead of replaying its quiet head.
        path = BANK["roll"][1][0][1]
        raw = sample(path)
        duration = length * self.pulse
        mono = np.mean(
            np.abs(raw[: len(raw) // 512 * 512].reshape(-1, 512, 2)), axis=(1, 2)
        )
        peak = (int(np.argmax(mono)) + 1) * 512
        count = min(peak, round(duration * RATE))
        begin = round((self.start + at * self.pulse) * RATE)
        tail = round(1.4 * RATE)
        count_out = min(
            count + tail, len(raw) - (peak - count), len(self.stage.mix) - begin
        )
        env = np.ones(count_out, np.float32)
        env[:count] *= np.linspace(0.0, 1.0, count, dtype=np.float32) ** 1.7
        env[count:] *= np.linspace(1.0, 0.0, count_out - count, dtype=np.float32) ** 2
        self.stage.mix[begin : begin + count_out] += raw[
            peak - count : peak - count + count_out
        ] * (level * self.level * env[:, None])

    def melody(self, desk, words, family):
        pos = 0.0
        connected = False
        for word in words:
            note, _, value = word.partition("/")
            ticks = float(value or 1)
            if note != "-":
                key = pitch(note)
                # The scored fanfare and its pizzicato-sized pickups need different
                # tongue/bow attacks. Sustained lines overlap between adjacent pitches.
                accent = 1.10 if pos == 0 or ticks >= 4 else 0.95
                sustained = desk in LONG
                legato = connected and sustained and desk not in ("trumpet", "horn")
                length = ticks * (0.97 if sustained else 0.82)
                self.note(
                    desk,
                    key,
                    pos,
                    length,
                    0.96 * accent,
                    1.13 if ticks > 3 else 0.95,
                    legato,
                )
                if self.section == "coronation" and ticks >= 2:
                    self.note("horn", key - 12, pos + 0.09, length * 0.94, 0.52)
                elif self.section == "summit":
                    self.note(
                        "violin", key, pos + 0.08, length * 0.98, 0.60, connected=legato
                    )
                elif self.section == "crest" and ticks >= 3:
                    self.note(
                        "flute",
                        key + (12 if key < 75 else 0),
                        pos + 0.14,
                        length * 0.92,
                        0.46,
                    )
            connected = note != "-"
            pos += ticks
        if abs(pos - self.meter) > 0.00001:
            raise ValueError(
                f"{family}, {self.section}, bar {self.index + 1}: {pos} eighth notes"
            )


def choir(stage, font):
    # Render directly to memory. Authoring never opens the desktop audio device.
    from ctypes import CDLL, POINTER, c_char_p, c_double, c_int, c_void_p

    lib = CDLL("libfluidsynth.so.3")

    def bind(name, result, args):
        fn = getattr(lib, name)
        fn.restype, fn.argtypes = result, args
        return fn

    void, integer = c_void_p, c_int
    bind("fluid_audio_driver_register", integer, [POINTER(c_char_p)])(
        (c_char_p * 1)(None)
    )
    settings = bind("new_fluid_settings", void, [])()
    bind("fluid_settings_setnum", integer, [void, c_char_p, c_double])(
        settings, b"synth.sample-rate", RATE
    )
    synth = bind("new_fluid_synth", void, [void])(settings)
    try:
        sfid = bind("fluid_synth_sfload", integer, [void, c_char_p, integer])(
            synth, bytes(font), 1
        )
        if sfid < 0:
            raise ValueError(f"Could not load choir soundfont {font}")
        program = bind(
            "fluid_synth_program_select",
            integer,
            [void, integer, integer, integer, integer],
        )
        # Opposite galleries answer in different registers and vowel colours.
        cc = bind("fluid_synth_cc", integer, [void, integer, integer, integer])
        for ch, (patch, pan) in enumerate(((52, 44), (53, 69), (52, 81), (52, 58))):
            program(synth, ch, sfid, 0, patch)
            cc(synth, ch, 10, pan)
            cc(synth, ch, 91, 12)

        on = bind("fluid_synth_noteon", integer, [void, integer, integer, integer])
        off = bind("fluid_synth_noteoff", integer, [void, integer, integer])
        render = bind(
            "fluid_synth_write_float",
            integer,
            [void, integer, void, integer, integer, void, integer, integer],
        )
        events = []
        for note, at, duration, velocity, gallery in stage.choir_notes:
            channel = 2 * int(gallery) + int(note >= 64)
            events.extend(
                [
                    (round(at * RATE), note, velocity, channel),
                    (round((at + duration) * RATE), note, 0, channel),
                ]
            )
        events.append((len(stage.mix), 0, 0, 0))
        last = 0
        for pos, note, vel, channel in sorted(events, key=lambda ev: (ev[0], ev[2])):
            if pos > last:
                block = np.zeros((pos - last, 2), dtype=np.float32)
                render(
                    synth, len(block), block.ctypes.data, 0, 2, block.ctypes.data, 1, 2
                )
                stage.mix[last:pos] += block * 3.8
            if note:
                if vel:
                    on(synth, channel, note, vel)
                else:
                    off(synth, channel, note)
            last = pos
    finally:
        bind("delete_fluid_synth", None, [void])(synth)
        bind("delete_fluid_settings", None, [void])(settings)


def hall(dry, decay):
    """Add two periodic room impulse responses in bounded overlap-add buffers."""
    frames = len(dry)
    mixed = dry.copy()
    ir_length = round(decay * RATE)
    t = np.arange(ir_length, dtype=np.float32) / RATE
    rng = np.random.default_rng(5621)
    block = 131072
    fft_size = 1 << (block + ir_length - 2).bit_length()
    for ch in range(2):
        noise = rng.normal(size=ir_length).astype(np.float32)
        soft = np.convolve(noise, np.ones(11, dtype=np.float32) / 11, mode="same")
        tail = soft * np.exp(-t * 4.8 / decay) + noise * 0.11 * np.exp(-t * 10 / decay)
        ir = tail * (0.0034 * np.clip((t - 0.021) / 0.065, 0, 1))
        for delay, amplitude in (
            (0.020, 0.125),
            (0.042, 0.10),
            (0.071, 0.073),
            (0.105, 0.047),
        ):
            ir[round((delay + 0.004 * ch) * RATE)] += amplitude
        kernel = np.fft.rfft(ir, fft_size)
        send = 0.65 * dry[:, ch] + 0.35 * dry[:, 1 - ch]
        for a in range(0, frames, block):
            part = send[a : a + block]
            reverb = np.fft.irfft(np.fft.rfft(part, fft_size) * kernel, fft_size)
            reverb = reverb[: len(part) + ir_length - 1]
            used = min(frames - a, len(reverb))
            mixed[a : a + used, ch] += reverb[:used]
            mixed[: len(reverb) - used, ch] += reverb[used:]
    return mixed


def finish(stage, frames, family):
    # Releases from the last phrase and the room response continue into bar one.
    dry = stage.mix[:frames].copy()
    dry[: len(stage.mix) - frames] += stage.mix[frames:]
    mixed = hall(dry, FAMILY[family][1])
    # Modest seat EQ removes rumble and leaves room for the in-game roar/hit effects.
    # Warm the filter history using the preceding half-second so the loop remains continuous.
    pre = RATE // 2
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "f32le",
            "-ar",
            str(RATE),
            "-ac",
            "2",
            "-i",
            "-",
            "-af",
            "highpass=f=28,bass=f=105:g=2.8,equalizer=f=470:t=q:w=1.1:g=-2.7,equalizer=f=2800:t=q:w=1.1:g=-1.2",
            "-f",
            "f32le",
            "-",
        ],
        input=np.concatenate([mixed[-pre:], mixed]).astype("<f4").tobytes(),
    )
    result = np.frombuffer(raw, dtype="<f4").reshape(-1, 2)[pre:].copy()
    result *= 0.14 / max(0.01, np.sqrt(np.mean(result * result)))
    # The soft knee touches isolated percussion peaks; quieter phrases retain their dynamics.
    excess = np.maximum(np.abs(result) - 0.72, 0.0)
    result = np.sign(result) * (
        np.minimum(np.abs(result), 0.72) + 0.18 * np.tanh(excess / 0.18)
    )
    return result * 0.9  # Leave true-peak headroom for downmixing.


def render(family, font):
    meter, beats, sections = SCORES[family]
    plan = []
    for section in sections:
        lines = [row.split() for row in section.music.strip().splitlines()]
        for i, (chord, *melody) in enumerate(lines):
            fraction = i / max(1, len(lines) - 1)
            gain = (1 - fraction) * section.intensity[0] + fraction * section.intensity[
                1
            ]
            # Agogic accents land at the end of four-bar phrases. Short pickups
            # anticipate the next section; the cadence gets an actual extra breath.
            rubato = 1 + 0.027 * math.sin(fraction * 2 * math.pi)
            if i == len(lines) - 1:
                rubato += 0.075
            elif i % 4 == 3:
                rubato += 0.025
            seconds = beats * 60 / section.bpm * rubato
            plan.append((section, i, chord, melody, gain, seconds))
    total = sum(row[-1] for row in plan)
    stage = Stage(family, total)
    at = 0.0
    for section, i, chord, notes, level, seconds in plan:
        bar = Bar(stage, section, i, at, seconds, meter, level, chord)
        ARRANGE[family](bar)
        antiphon(bar, family)
        bar.melody(section.solo, notes, family)
        at += seconds
    choir(stage, font)
    sound = finish(stage, round(at * RATE), family)
    write_wav(OUT / (family + "-theme.wav"), sound)
    print(
        f"{family}: {len(plan)} bars, {at:.2f}s, peak {abs(sound).max():.3f}",
        flush=True,
    )


if __name__ == "__main__":
    font = Path(
        sys.argv[1]
        if len(sys.argv) > 1
        else "/usr/share/minuet/soundfonts/GeneralUser-v1.47.sf2"
    )
    for family in SCORES:
        render(family, font)
