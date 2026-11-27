"""Orchestration of the three scores. Positions and lengths are in eighth notes."""

from math import sin, pi


def dark(bar):
    """Low antiphonal ritual in three; chorus answers the horn from across the hall."""
    root, tenor, alto, soprano = bar.voicing
    section, i = bar.section, bar.index
    bar.note("bass", root, 0, 11.8, 0.57, swell=0.82)
    if section == "procession":
        bar.note("trombone", root + 12, 0, 4.4, 0.74)
        for at in (6, 8, 10):
            bar.note("bow", tenor - 12, at, 1.25, 0.52)
        if i in (0, 2):
            bar.note("gong", 46, 0, 15, 0.29)
        if i % 2:
            bar.note("viola", alto, 4, 7.7, 0.31, swell=1.37)
        return
    if section == "eclipse":
        # A held choir above a displaced, shortening low-string figure.
        bar.chorus((tenor, alto, soprano), 0.2, 8.3, 103, gallery=True)
        bar.note("trombone", root + 12, 0, 5.4, 0.85)
        for at in (0, 3, 6, 8, 9.5, 11):
            bar.note("bow", tenor - 12 if at in (3, 8) else root + 12, at, 1, 0.66)
        bar.note("violin", soprano + 12, 5.9, 5.5, 0.61, swell=1.45)
        if i in (1, 4):
            bar.rise(6, 6, 0.67)
        if i in (0, 3, 5):
            bar.note("gong", 45, 0, 13, 0.66)
        return
    if section == "vigil":
        bar.note("viola", alto, 0.15, 11.5, 0.36, swell=1.25)
        bar.note("harp", tenor, 0, 7, 0.35)
        bar.note("harp", soprano + 12, 7.5, 4, 0.26)
        if i in (1, 3, 5):
            bar.note("soft_horn", tenor - 12, 6, 5.5, 0.43)
        return
    if section == "threshold":
        bar.note("viola", alto, 0, 11.5, 0.39)
        if i >= 2:
            bar.note("kettle", root + 12, 7, 4.5, 0.29)
        return

    # The repeated cell contracts from long bows to an uneven 3+3+2+2+2.
    pattern = (0, 3, 6, 8, 10) if section == "ascent" else (0, 2, 4, 6, 7, 9, 10, 11)
    notes = [root + 12, tenor - 12, alto - 12, tenor - 12]
    for j, pos in enumerate(pattern):
        bar.note("bow", notes[j % 4], pos, 0.9, 0.60 if pos in (0, 6) else 0.31)
    bar.note("viola", alto, 0, 5.5, 0.42, swell=1.18)
    bar.note("viola", soprano, 6, 5.7, 0.39, swell=0.78)
    if section == "ascent":
        bar.note("tremolo", soprano + 12, 1.5, 10.2, 0.37, swell=1.6)
        for pos in (0, 6, 9):
            bar.note("kettle", root + 12, pos, 3, 0.47)
        if i == 3:
            bar.rise(4, 8, 0.55)
    else:
        if i % 2 == 0:
            bar.note("drum", 46, 0, 7, 0.70)
        else:
            bar.note("cello", tenor - 12, 0, 5.7, 0.55)
            bar.note("cello", alto, 6, 5.7, 0.51, swell=1.21)
        if i in (2, 5, 7):
            bar.note("kettle", root + 12, 8, 4, 0.48)
    if section == "coronation":
        counter = [
            "A3 D4",
            "G3 Bb3",
            "A3 D4",
            "G3 C#4",
            "Bb3 D4",
            "Bb3 F4",
            "G3 Bb3",
            "A3 C#4",
        ][i].split()
        for j, note in enumerate(counter):
            bar.note("horn", note, j * 6 + 0.15, 5.8, 0.68)
        bar.note("trombone", tenor - 12, 0, 3.1, 0.50)
        bar.note("tremolo", alto + 12, 5.7, 6.1, 0.33, swell=1.33)
        if i in (0, 4):
            bar.note("cymbal", 43, 0, 15, 0.41)
        if i in (3, 6):
            bar.rise(7, 5, 0.44)


def earth(bar):
    """Five-beat brass march. Slabs of low brass and answering upper strings."""
    root, tenor, alto, soprano = bar.voicing
    section, i = bar.section, bar.index
    if section == "earthquake":
        # The brass chorus marks five broad beats against a marching snare.
        for at, key in ((0, root + 12), (4, tenor - 12), (6, root + 12)):
            bar.note("low_stab", key, at, 1.55, 0.92)
            bar.note("drum", 45, at, 3.3, 0.72)
        bar.note("horn", alto, 0.08, 5.82, 0.71)
        bar.note("horn", soprano, 6.06, 3.75, 0.79)
        bar.chorus((tenor, alto, soprano), 0.21, 5.35, 111, gallery=True)
        bar.note("tremolo", alto + 12, 1.8, 7.9, 0.48, swell=1.55)
        for at, strength in ((2.75, 0.30), (3, 0.69), (7, 0.63), (8.5, 0.38), (9, 0.53)):
            bar.note("snare", 48, at, 0.51, strength)
        if i in (0, 3):
            bar.note("cymbal", 48, 0, 10, 0.60)
        return
    if section == "fault":
        bar.note("bass", root, 0, 9.7, 0.50)
        bar.note("viola", alto, 0.1, 9.4, 0.43, swell=1.28)
        bar.note("soft_horn", tenor, 4.1, 5.6, 0.38, swell=1.18)
        if i in (1, 4):
            bar.note("oboe", soprano + 12, 5.3, 4.3, 0.44)
        if i == 5:
            bar.rise(6.5, 3.4, 0.28)
        return
    if section == "bedrock":
        bar.note("bass", root, 0, 9.7, 0.59)
        bar.note("viola", alto, 0.2, 7.3, 0.35)
        bar.note("drum2", 46, 0, 8, 0.37)
        bar.note("kettle", root + 12, 6, 4, 0.41)
        return

    # Trade weight between 3+2 and 2+3; the ground attack leaves an audible gap.
    accents = (0, 4, 7) if i % 4 == 2 else (0, 3, 6)
    for j, pos in enumerate(accents):
        bar.note(
            "bass", root if j != 1 else root + 7, pos, 1.9 if j == 0 else 1.4, 0.60
        )
        bar.note("bow", root + 12 if j != 1 else tenor - 12, pos + 0.04, 1, 0.72)
        bar.note(
            "drum" if j == 0 else "drum2", 47 + j, pos, 4, 0.69 if j == 0 else 0.37
        )
    bar.note("kettle", root + 12, 0, 4, 0.45)
    if section == "footfall":
        bar.note("viola", alto, 3, 6.6, 0.33, swell=1.35)
        bar.note("horn", alto, 0.08, 3.4, 0.65)
        bar.note("rim", 48, 6.75, 0.65, 0.41)
        bar.note("snare", 48, 7, 1.1, 0.53)
        if i > 1:
            bar.note("horn_stab", tenor, 7, 1.6, 0.51)
        return
    # The high string answer moves in contrary motion to the fanfare.
    if section in ("rampart", "summit"):
        counter = [
            "Ab4 G4",
            "F4 Eb4",
            "Eb4 C4",
            "D4 E4",
            "Ab4 G4",
            "F4 Db4",
            "F4 G4",
            "G4 E4",
        ][i].split()
        for j, pitch in enumerate(counter):
            bar.note("violin", pitch, 1 + j * 5, 3.85, 0.49, swell=1.18)
        bar.note("low_stab", tenor - 12, 3, 1.7, 0.64)
        bar.note("low_stab", root + 12, 7, 1.6, 0.55)
    if section == "hammer":
        for j, pos in enumerate((0, 2, 3.5, 5, 6, 7, 8.5)):
            pitch = (tenor, soprano, alto)[j % 3]
            bar.note(
                "short_violin", pitch + 12, pos, 0.72, 0.56 if j % 3 == 0 else 0.40
            )
        bar.note("horn_stab", alto, 4, 1.8, 0.66)
        bar.note("horn_stab", soprano, 7, 2.3, 0.54)
    if section == "summit":
        bar.note("cello", tenor, 0, 9.6, 0.50)
        if i in (0, 4):
            bar.note("cymbal", 46, 0, 14, 0.61)
        bar.note("horn", soprano, 0, 5.4, 0.60, swell=1.13)
    fills = (7.5, 8, 8.5, 9, 9.5) if i % 4 == 3 else (5, 8.5)
    for j, pos in enumerate(fills):
        bar.note("snare" if j % 2 == 0 else "rim", 48, pos, 1.5, 0.23 + 0.045 * j)


def water(bar):
    """A twelve-eight current under a long solo line, then three against four."""
    root, tenor, alto, soprano = bar.voicing
    section, i = bar.section, bar.index
    bar.note("bass", root, 0, 11.7, 0.52, swell=0.83)
    if section in ("undertow", "depths", "foam"):
        arpeggio = (0, 2, 4, 7, 9) if i % 2 == 0 else (0, 3, 5, 8, 10)
    else:
        arpeggio = (0, 1, 2, 4, 5, 6, 7, 9, 10, 11)
    pitches = [tenor - 12, alto, soprano, tenor + 12, soprano, alto]
    for j, pos in enumerate(arpeggio):
        bar.note("harp", pitches[j % 6], pos, 4.5, (0.48 + 0.09 * sin(j * pi / 3)))
    if section == "maelstrom":
        # A rising wave in four crosses a three-beat horn answer.
        for j, at in enumerate((0, 3, 6, 9)):
            bar.note("cello", (root + 12, tenor, alto, soprano)[j], at, 2.8, 0.69)
            bar.note("short_violin", (soprano, alto, tenor, alto)[j] + 12, at + 1.6, 1.1, 0.57)
        for j, at in enumerate((0, 4, 8)):
            bar.note("horn", (tenor, alto, soprano)[j], at + 0.12, 3.6, 0.71)
        bar.chorus((alto, soprano, tenor + 12), 3.25, 8.25, 107, gallery=True)
        if i in (1, 4):
            bar.rise(4, 8, 0.70)
        if i in (0, 2, 5):
            bar.note("cymbal", 53, 0, 12, 0.57)
        return
    if section == "depths":
        bar.note("solo", soprano + 12, 1, 10.3, 0.36, swell=1.40)
        if i in (1, 3, 5):
            bar.note("glock", alto + 24, 7, 8, 0.20)
        return
    if section in ("undertow", "foam"):
        if section == "undertow":
            bar.note("horn", tenor, 0.1, 6.1, 0.62)
            bar.note("flute", alto + 12, 6.15, 5.0, 0.53, swell=1.47)
            bar.note("kettle", root + 12, 9, 3.4, 0.55)
        bar.note("viola", alto, 0.3, 10.8, 0.41, swell=1.35)
        if i % 2 == 0:
            bar.note("glock", alto + 24, 6, 8, 0.32)
        if i == 3 and section == "undertow":
            bar.rise(6, 6, 0.35)
        return
    bar.note("cello", tenor - 12, 0, 5.7, 0.49)
    bar.note("cello", alto - 12, 6, 5.8, 0.46, swell=1.19)
    if section in ("current", "crest"):
        answers = [
            "B4 A4",
            "D5 A4",
            "D5 B4",
            "A4 E5",
            "B4 D5",
            "D5 F#5",
            "A4 C#5",
            "F#5 D#5",
        ][i].split()
        desk = "flute" if section == "current" else "soft_horn"
        for j, pitch in enumerate(answers):
            key = bar.key(pitch) - (12 if desk == "soft_horn" else 0)
            bar.note(desk, key, 4.5 + 3.5 * j, 3.1, 0.46, swell=0.79)
    if section == "whitecaps":
        for j, pos in enumerate((0, 2, 4, 6, 8, 10)):
            bar.note(
                "short_violin", (soprano, alto, tenor)[j % 3] + 12, pos, 0.95, 0.55
            )
        bar.note("tremolo", soprano, 0.1, 11.4, 0.38, swell=1.29)
    if section == "crest":
        for j, pos in enumerate((0, 4, 8)):
            bar.note("bow", [root + 12, tenor - 12, alto - 12][j], pos, 1.7, 0.66)
        if i in (0, 4):
            bar.note("cymbal", 51, 0, 14, 0.43)
            bar.note("glock", soprano + 24, 0, 12, 0.26)
    bar.note("kettle", root + 12, 0, 5, 0.49)
    if i in (1, 3, 5, 7):
        bar.note("drum2", 46, 6, 7, 0.46)
    if i in (3, 6):
        bar.rise(6, 6, 0.30)


# Forceful syllables leave air between the two choirs. The countersubjects move
# against the bass instead of retriggering a pad on every chord change.
def fire(bar):
    """Embers gather under a 3+3+2 brass figure; the middle eight clear to oboe and harp."""
    root, tenor, alto, soprano = bar.voicing
    if bar.section == "caldera":
        for at, key in ((0, tenor), (3, alto), (6, soprano)):
            bar.note("harp", key, at, 3.7, 0.43)
        bar.note("tremolo", root + 12, 0, 7.8, 0.34, swell=1.28)
        return
    for at, key in ((0, root), (3, tenor-12), (6, root+7)):
        bar.note("bow", key+12, at, 1.05, 0.65)
        bar.note("drum", 44, at + 0.1, 2.2, 0.55)
    if bar.section == "embers":
        bar.note("soft_horn", alto, 2, 5.8, 0.42, swell=1.3)
    else:
        for i, key in enumerate((alto, soprano, tenor, alto)):
            bar.note("short_violin", key+12, i*2+0.85, 0.86, 0.66)
        bar.note("trombone", root+12, 0, 3.1, 0.60)
        if bar.index % 4 == 3:
            bar.rise(4, 4, 0.62)
        if bar.section == "firestorm":
            bar.note("horn_stab", soprano, 3.05, 1.85, 0.8)
            bar.note("cymbal", 53, 6.0, 3.8, 0.49)

CHANT = {
    "fire": (((0, 1.85, 0), (3, 2.3, 1), (6, 1.55, 0)),
             ((0, 2.8, 2), (3.15, 1.6, 3), (5.35, 2.35, 1)), (0, 3, 6)),
    "dark": (
        ((0, 2.8, 0), (4, 1.45, 1), (6, 4.25, 0)),
        ((1, 2.5, 2), (5, 3.5, 1), (9, 2.65, 3)),
        (0, 4, 8),
    ),
    "earth": (
        ((0, 1.45, 0), (2, 1.35, 0), (5, 3.7, 1)),
        ((0, 2.7, 3), (4, 1.4, 2), (6, 3.6, 1)),
        (0, 4, 6),
    ),
    "water": (
        ((0, 4.55, 1), (6, 2.15, 0), (9, 2.75, 0)),
        ((0, 2.5, 2), (3, 4.6, 3), (9, 2.5, 1)),
        (0, 3, 6, 9),
    ),
}


def antiphon(bar, family):
    bass, tenor, alto, soprano = bar.voicing
    call, answer, tread = CHANT[family]
    i = bar.index
    quiet = bar.section in ("vigil", "fault", "depths", "threshold", "bedrock", "foam", "caldera")
    peak = bar.section in ("coronation", "summit", "crest")
    if quiet:
        # Keep the hush before the renewed attack. The choir's last unresolved
        # note sounds into the first bar on the loop.
        if i % 2 == 0:
            bar.chorus([tenor - 12, alto], 0.12, bar.meter * 0.72, 76)
        else:
            bar.chorus([soprano], bar.meter * 0.46, bar.meter * 0.48, 82, gallery=True)
        return

    low = (bass + 12, tenor, alto, soprano)
    high = (tenor, alto, soprano, tenor + 12)
    if i % 2 == 0 or peak:
        for at, length, degree in call:
            # The low choir sings in octaves, with separate consonant attacks.
            bar.chorus([low[degree] - 12, low[degree]], at, length, 106)
    if i % 2 or peak:
        for at, length, degree in answer:
            bar.chorus([high[degree], high[degree] + 12], at + 0.11, length, 100, gallery=True)

    for j, at in enumerate(tread):
        if j == 0:
            bar.note("drum2", 40, at, 5.6, 1.12)
        bar.note("bass", bass, at, 1.45, 0.88 if j == 0 else 0.53)
        if i % 2 == 0 or peak:
            bar.note("kettle", bass + (12 if j == 0 else 19), at, 2.6, 0.84 if j == 0 else 0.49)
    if peak:
        bar.note("trombone", bass + 12, 0, bar.meter * 0.6, 0.72)
        bar.note("tremolo", alto + 12, bar.meter * 0.55, bar.meter * 0.43, 0.38, swell=1.55)


ARRANGE = {"dark": dark, "earth": earth, "water": water, "fire": fire}
