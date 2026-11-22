"""Render-time orchestral library. Only the mixed performances ship in the game."""

from concurrent.futures import ThreadPoolExecutor
from functools import cache
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlopen
import hashlib
import json
import subprocess
import numpy as np

ROOT = Path(__file__).resolve().parents[3]
CACHE = ROOT / "target/audio-revision"
OUT = ROOT / "assets/audio/dragons"
RATE = 44100
REV = "440300901dfe9275fd84e0b7763af1f8443ae62e"

# VSCO labels middle C as C3. The harp and solo violin use scientific pitch.
# A separate entry for each bow, velocity and retake preserves real attacks.
DESKS = {
    "cello": ("Strings/Cello Section/susvib/susvib_{}_v3_1", "C1 G1 D2 A2 C3 G3", 2),
    "bow": ("Strings/Cello Section/spic/spic_{}_v2_RR{}", "C1 G1 D2 A2 C3", 2),
    "violin": (
        "Strings/Violin Section/susVib/VlnEns_susVib_{}_v2",
        "G2 D3 A3 C4 G4 D5",
        2,
    ),
    "tremolo": ("Strings/Violin Section/Trem/VlnEns_Trem_{}_v1", "G2 D3 A3 C4 G4", 2),
    "bass": (
        "Strings/Solo Contrabass/SusVib/BKCtbss_SusVib_{}_v3_rr1",
        "G0 C1 F#1 C#2",
        2,
    ),
    "horn": ("Brass/F Horn/sus/MOHorn_sus_{}_v3_1", "C1 F2 A2 C3", 2),
    "trombone": ("Brass/Tenor Trombone/sus/tenortbn_sus_{}_v3_1", "F1 D2 F2 C3", 2),
    "harp": ("Strings/Harp/KSHarp_{}_mf", "A2 C3 G3 D4 A4 C5", 1),
    "flute": ("Woodwinds/Flute/susvib/LDFlute_susvib_{}_v1_1", "C3 A3 E4 A4", 2),
    "soft_horn": ("Brass/F Horn/sus/MOHorn_sus_{}_v2_1", "G1 D2 F2 A2 C3", 2),
    "horn_stab": ("Brass/F Horn/stac/MOHorn_stac_{}_v3_rr{}", "D2 F2 A2 C3 D4", 2),
    "low_stab": (
        "Brass/Tenor Trombone/stac/tenortbn_stac_{}_v3_rr{}",
        "F1 A#1 F2 A#2 D3",
        2,
    ),
    "trumpet": (
        "Brass/Trumpet/susvib/Sum_SHTrumpet_susvib_{}_v2_rr1",
        "F3 A#3 D4 F4",
        2,
    ),
    "short_violin": (
        "Strings/Violin Section/Spic/VlnEns_Spic_{}_v2_rr{}",
        "D3 F#3 A3 C4 E4 G4",
        2,
    ),
    "viola": (
        "Strings/Viola Section/susvib/ViolaEns_susvib_{}_v2_1",
        "G2 D3 F3 A3 C4",
        2,
    ),
    "solo": ("Strings/Solo Violin/Arco Vib/LLVln_ArcoVib_{}_p", "G3 C4 G4 C5 G5", 1),
    "oboe": ("Woodwinds/Oboe/Vib/Oboe_Vib_{}_v1_Main", "D3 F3 A#3 D4 F4", 2),
    "glock": ("Percussion/Glock/glock_medium_{}", "G4 C5 G5 C6", 2),
}
DRUMS = {
    "drum": "VSCO 1 Percussion/drums/bass/bdrum_ff_1",
    "drum2": "VSCO 1 Percussion/drums/bass/bdrum_f_2",
    "snare": "VSCO 1 Percussion/drums/snare/drum3_marching/snare3_f_1",
    "rim": "VSCO 1 Percussion/drums/snare/drum3_marching/snare3_rimshot_f_1",
    "cymbal": "VSCO 1 Percussion/varMetal/Cymbals/clash/crash_hit_ff_loose",
    "roll": "VSCO 1 Percussion/varMetal/Cymbals/susp/susp_hit_softmall_roll2_cresc",
    "gong": "VSCO 1 Percussion/varMetal/Gong/gong_hit_f",
    "kettle": "Percussion/Timpani/Timpani2_Hit_v4_rr1_Sum",
}


def recordings():
    for desk, (pattern, pitches, octave) in DESKS.items():
        for p in pitches.split():
            note, number = p[:-1], int(p[-1]) + octave
            key = number * 12 + ("C C# D D# E F F# G G# A A# B".split()).index(note)
            for take in [1, 2] if pattern.count("{}") == 2 else [1]:
                yield desk, key, take, pattern.format(p, take) + ".wav"
    for desk, sample in DRUMS.items():
        yield desk, 48, 1, sample + ".wav"


def fetch():
    CACHE.mkdir(parents=True, exist_ok=True)
    url = f"https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees/{REV}?recursive=1"
    with urlopen(url, timeout=40) as res:
        tree = {x["path"]: x["sha"] for x in json.load(res)["tree"]}

    def one(entry):
        path = entry[3]
        expected = tree[path]
        dest = CACHE / "vsco" / path

        def digest(b):
            return hashlib.sha1(f"blob {len(b)}\0".encode() + b).hexdigest()

        if dest.exists() and digest(dest.read_bytes()) == expected:
            return
        url = (
            f"https://raw.githubusercontent.com/sgossner/VSCO-2-CE/{REV}/{quote(path)}"
        )
        with urlopen(url, timeout=55) as res:
            data = res.read()
        if digest(data) != expected:
            raise ValueError(path)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)

    with ThreadPoolExecutor(max_workers=6) as workers:
        list(workers.map(one, recordings()))


@cache
def read(path, channels=2, rate=RATE):
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-ar",
            str(rate),
            "-ac",
            str(channels),
            "-f",
            "f32le",
            "-",
        ]
    )
    return np.frombuffer(raw, dtype="<f4").reshape(-1, channels)


if __name__ == "__main__":
    fetch()
