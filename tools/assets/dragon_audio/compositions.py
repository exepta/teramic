"""Original dragon encounter scores. Each row is a chord and one melodic bar.

Pitches use scientific notation. Slash values are eighth notes and a dash is
a rest. Each section specifies its tempo, opening/closing dynamics, and soloist.
Harmonic voicings are written bottom to top and include inversions and pedals.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class Section:
    name: str
    bpm: int
    intensity: tuple[float, float]
    solo: str
    music: str


# Bass, tenor, alto and soprano. The inner parts stay in place across a change
# of bass where possible, so the harmony can move underneath a sustained theme.
VOICINGS = {
    "Dm": "D2 A3 D4 F4",
    "Eb/D": "D2 Bb3 Eb4 G4",
    "Gm/D": "D2 Bb3 D4 G4",
    "Bb": "Bb1 Bb3 D4 F4",
    "A7": "A1 A3 C#4 G4",
    "Dm/F": "F2 A3 D4 F4",
    "Gm": "G2 Bb3 D4 G4",
    "F/A": "A1 A3 C4 F4",
    "Edim": "E2 Bb3 D4 G4",
    "Dsus": "D2 A3 D4 G4",
    "Eb": "Eb2 Bb3 Eb4 G4",
    "Fm": "F2 Ab3 C4 F4",
    "Db": "Db2 Ab3 Db4 F4",
    "Eb/G": "G2 Bb3 Eb4 G4",
    "C7": "C2 Bb3 C4 E4",
    "Fm/C": "C2 Ab3 C4 F4",
    "Bbm": "Bb1 Bb3 Db4 F4",
    "Ab": "Ab1 Ab3 C4 Eb4",
    "Eb/F": "F2 Bb3 Eb4 G4",
    "Db/F": "F2 Ab3 Db4 F4",
    "C/E": "E2 G3 C4 E4",
    "Gsus": "G2 G3 C4 D4",
    "Csus": "C2 G3 C4 F4",
    "Em9": "E2 B3 D4 F#4",
    "A/E": "E2 A3 C#4 E4",
    "D/F#": "F#2 A3 D4 F#4",
    "G": "G2 B3 D4 G4",
    "Bm": "B1 B3 D4 F#4",
    "A": "A2 A3 C#4 E4",
    "Cmaj7": "C2 B3 E4 G4",
    "B7": "B1 A3 D#4 F#4",
    "Em": "E2 B3 E4 G4",
    "G/B": "B1 B3 D4 G4",
    "Asus": "A2 A3 D4 E4",
    "Bm/D": "D2 B3 D4 F#4",
    "F#m7": "F#2 A3 C#4 E4",
}

DARK = [
    Section(
        "procession",
        84,
        (0.72, 0.86),
        "cello",
        """
Dm      D3/6 -/2 A3/3 Bb3
Eb/D    A3/4 Eb3/4 D3/3 -
Dm      -/3 D3/3 F3/2 E3 D3/3
A7      C#3/6 A2/4 -/2
""",
    ),
    Section(
        "invocation",
        80,
        (0.75, 0.97),
        "horn",
        """
Dm      D4/5 Eb4 D4/2 A3/3 C4
Eb/D    Bb3/6 G3/3 Bb3/2 C4
Dm/F    D4/3 A3/2 F3 G3/3 A3/3
A7      E4/5 D4 C#4/4 -/2
Gm      G3/3 Bb3/3 D4/4 F4/2
Bb      F4/4 Eb4/2 D4/3 Bb3/3
Edim    G3/3 E4/3 D4/4 Bb3/2
A7      A3/7 -/2 A3 C#4 E4
""",
    ),
    Section(
        "vigil",
        69,
        (0.50, 0.32),
        "solo",
        """
Dm/F    F4/6 A4/4 G4/2
F/A     E4/4 F4/4 C5/4
Gm      Bb4/5 A4 G4/3 D4/3
Dsus    A4/8 -/2 G4 F4
Eb      Eb4/4 G4/3 Bb4/5
A/E     A4/6 -/3 E4/3
""",
    ),
    Section(
        "ascent",
        85,
        (0.58, 1.08),
        "cello",
        """
Dm      D3/3 E3 F3 G3 A3/3 -/3
Gm/D    Bb3/3 A3 G3 F3 Eb3/3 G3/3
Eb/D    Bb3/3 G3/3 Bb3 C4 D4 Eb4/3
A7      E4/6 C#4/3 E4 F4 G4
""",
    ),
    Section(
        "coronation",
        84,
        (1.14, 0.98),
        "violin",
        """
Dm      D5/5 Eb5 D5/2 A4/3 C5
Eb/D    Bb4/6 G4/3 Bb4 C5 D5
Dm/F    F5/3 D5/2 A4 G4/3 A4/3
A7      E5/5 F5 E5/3 C#5/3
Gm      G4/2 Bb4/2 D5/2 G5/4 F5/2
Bb      D5/5 F5 Eb5/3 D5/3
Edim    Bb4/3 G4/3 E5/3 D5/3
A7      C#5/6 A4/3 G4/3
""",
    ),
    Section(
        "threshold",
        73,
        (0.77, 0.38),
        "soft_horn",
        """
Dm      F4/4 E4/2 D4/4 A3/2
Eb/D    G3/7 Eb3/3 -/2
A7      A3/4 Bb3 C#4 E4/3 G4/3
A7      E4/6 C#4/4 -/2
""",
    ),
]

EARTH = [
    Section(
        "footfall",
        114,
        (0.78, 0.90),
        "low_stab",
        """
Fm      F3/2 - C3 F3/2 - Ab3 G3 F3
Db/F    Db3/3 F3/2 Ab3/2 -/3
Eb/F    Eb3/2 - Bb2 Eb3/2 F3 G3/2 -
C7      C3/3 -/2 G3/2 E3/2 -
""",
    ),
    Section(
        "rampart",
        131,
        (0.88, 1.06),
        "horn",
        """
Fm      F3/3 C4/2 Db4 C4 Ab3/2 F3
Db      Ab3/2 Db4/2 F4/3 Eb4 Db4 C4
Ab      C4/3 - Ab3 C4/2 Eb4/3
C7      E4/2 D4 C4 G3/4 -/2
Fm      F3/2 Ab3 C4/2 F4/2 - Ab4/2
Bbm     F4/4 Db4/2 Bb3/3 C4
Db      Db4/2 Ab3/2 F3/2 Ab3/2 Db4/2
C7      E4/3 G4/2 E4/2 -/3
""",
    ),
    Section(
        "fault",
        108,
        (0.45, 0.50),
        "cello",
        """
Db      F3/6 Ab3/2 Db4/2
Ab      C4/3 Eb4/2 C4/3 -/2
Bbm     Bb3/4 F3/3 Ab3/3
Fm/C    C4/6 Ab3/2 G3/2
Db      F3/4 G3 Ab3 Db4/4
Csus    G3/6 -/2 C3/2
""",
    ),
    Section(
        "hammer",
        137,
        (0.79, 1.10),
        "low_stab",
        """
Fm      F3/2 - F3 C3/2 - Eb3 F3/2
Eb/G    G3/2 - G3 Bb3/2 - F3 G3/2
Db      Ab3/2 - Ab3 Db4/2 - Ab3 C4/2
C/E     G3/2 C4 E4/2 - G4/2 -/2
Bbm     F3 Db4 C4 Bb3 - F3 Bb3/2 Db4/2
C7      C4/2 E4/2 G4/2 -/4
""",
    ),
    Section(
        "summit",
        133,
        (1.16, 1.02),
        "trumpet",
        """
Fm      F4/3 C5/2 Db5 C5 Ab4/2 F4
Db      Ab4/2 Db5/2 F5/3 Eb5 Db5 C5
Ab      C5/3 - Ab4 C5/2 Eb5/3
C7      E5/4 C5/2 G4/2 E4/2
Bbm     F4/2 Bb4 Db5/2 F5/3 Eb5 Db5
Db/F    Db5/4 Ab4/2 F4/3 Ab4
Gsus    D5/3 C5/2 G4/3 D4/2
C7      E4/4 G4/2 C5/2 -/2
""",
    ),
    Section(
        "bedrock",
        119,
        (0.73, 0.56),
        "trombone",
        """
Fm      F3/6 -/2 C3/2
C7      G2/4 - C3/2 E3/2 -
""",
    ),
]

WATER = [
    Section(
        "undertow",
        100,
        (0.67, 0.81),
        "oboe",
        """
Em9     B4/5 A4 G4/3 F#4/2 E4
A/E     C#5/3 B4/2 A4 E4/4 -/2
Em9     G4/3 B4/3 F#5/4 E5/2
Bm      D5/6 F#4/3 -/3
""",
    ),
    Section(
        "current",
        118,
        (0.74, 0.94),
        "solo",
        """
Em      E5/4 B4 G4 A4/3 B4/3
D/F#    F#5/4 E5 D5 A4/3 D5/3
G       B4/3 D5/2 G5 F#5/4 D5/2
A       C#5/6 A4/4 B4/2
Em9     G5/4 F#5 E5 B4/3 D5/3
Bm/D    F#5/3 D5/3 B4/4 A4/2
A       A4/2 C#5 E5/3 F#5/3 E5/3
B7      D#5/5 B4 F#4/3 -/3
""",
    ),
    Section(
        "depths",
        96,
        (0.60, 0.34),
        "cello",
        """
Cmaj7   E3/6 G3/4 B3/2
G/B     D4/6 B3/3 G3/3
A       C#4/5 B3 A3/3 E3/3
Em      G3/9 -/3
Cmaj7   G3/3 B3/2 C4 E4/6
B7      D#4/7 F#3/3 -/2
""",
    ),
    Section(
        "whitecaps",
        125,
        (0.69, 1.05),
        "flute",
        """
Em      -/2 B4 E5/2 F#5 G5/2 A5 G5/2 E5
D/F#    -/2 A4 D5/2 E5 F#5/2 G5 F#5/2 D5
G       B4 D5 G5 B5/3 A5/3 G5/3
A       E5/3 C#5/2 E5 A5/4 G5/2
F#m7    F#5/3 E5/3 C#5/3 A4/3
B7      D#5/6 -/2 F#5 G5 F#5 D#5
""",
    ),
    Section(
        "crest",
        119,
        (1.09, 0.99),
        "violin",
        """
Em9     E5/4 B4 G4 A4/3 B4/3
D/F#    F#5/4 E5 D5 A4/3 D5/3
G       B4/3 D5/2 G5 B5/4 A5/2
A       E5/5 F#5 E5/3 C#5/3
Em      G5/3 F#5/3 E5 B4 G4 B4/3
G/B     D5/3 B4/2 G4 D5/4 G5/2
Asus    A5/6 G5/3 E5/3
B7      F#5/6 D#5/4 B4/2
""",
    ),
    Section(
        "foam",
        104,
        (0.70, 0.38),
        "oboe",
        """
Em9     G5/4 F#5 E5 B4/4 G4/2
A/E     A4/6 C#5/4 B4/2
Em      E5/6 G4/3 B4/3
B7      F#4/6 D#4/4 -/2
""",
    ),
]

# Meter in eighth notes and the number of conducting beats per bar.
SCORES = {"dark": (12, 3, DARK), "earth": (10, 5, EARTH), "water": (12, 4, WATER)}
