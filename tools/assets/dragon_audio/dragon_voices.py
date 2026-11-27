"""Original spatial dragon Foley. Render mono masters; the game places them at skeleton joints."""
from creature_foley import LOCAL, SOURCES, Shot, fetch
from samples import OUT

FAMILIES = ("dark", "earth", "water", "fire")
DURATIONS = {"roar": 2.85, "wing": 0.96, "land": 1.52, "cast": 1.84, "hurt": 0.79,
             "death": 3.48, "bite": 0.76, "tremor": 2.03, "ward": 1.99, "volley": 1.43,
             "surge": 1.58, "burrow": 1.97, "step": 0.69, "launch": 1.49, "breath": 1.46}
LOCAL.update(fire="environment/lava-crust", gas="environment/lava-bubble",
             magma="environment/lava-flow", steam="environment/fumarole-hiss")
# Each articulation uses a continuous performed vocal take, including its
# natural intake, glides and release. No pitched oscillator or repeated grains.
SOURCES.update(
    lion=("488/488348_10507382","8c0ca2c2dd6ecd755f3d105956d1120711d9d2f1124c83ba930551829ebbf6d8"),
    beast=("410/410343_1766049","cb311ab32e2816a006a8c43e8e765aaacb279e45df8ed15307553ff4568be179"),
)
# Source, roar, other articulation, tape speed, upper bandwidth and environmental Foley.
VOICES = (
    ("beast", 1.38, 2.16, .85, 3700, "wind", "creak", "gravel"),
    ("lion",  3.08, 6.19, .71, 2450, "dunes", "gravel", "rock"),
    ("tiger",  .90, 1.44, .91, 2950, "surf", "slosh", "swim"),
    ("lion",   .67, 10.45, .96, 4550, "steam", "fire", "gas"),
)


def vocal(shot, species, at, length, cue, gain):
    take, opening, close, speed, cutoff, *_ = VOICES[species]
    offset = opening if cue in ("roar", "death") else close
    pitch = speed * {"death": .87, "hurt": 1.14, "bite": 1.08}.get(cue, 1.)
    shot.add(take, at, offset, length, (pitch, pitch * .96), gain,
             55, cutoff, .033 if cue in ("hurt", "bite") else .095, min(.36,length*.28))


def layer(shot, name, at, length, speed, gain, lo=72, hi=6600, attack=.009, tail=.14):
    # The ambiences have an established body past their opening. Other source
    # files are individual transient recordings and begin at the impact.
    offset = {"wind": 2.68, "surf": 5.33, "leaves": 1.08, "magma": 1.64}.get(name, 0.)
    shot.add(name, at, offset, length, speed, gain, lo, hi, attack, tail)


def design(species, cue):
    lift = (1.0, .84, 1.12, 1.04)[species]
    *_, air, contact, snap = VOICES[species]
    duration = DURATIONS[cue] * (1 + (species - 1.5) * .035)
    shot = Shot(duration)

    if cue in ("roar", "death", "hurt", "breath"):
        start = .015 if cue == "hurt" else .075
        if cue == "breath":
            layer(shot, "air", .03, duration*.95, (.81,.73), 1., 90, 1960, .22,.32)
            layer(shot, air, .12, duration*.78, .78*lift, .20, 130, 2100, .20,.26)
        else:
            vocal(shot, species, start, duration * .90, cue, 1.)
            layer(shot, air, .04, duration*.88, .83*lift, .14, 80, 2300, .13,.26)
            if cue == "death":
                layer(shot, "splash" if species == 2 else "rock",
                      duration*.74, duration*.24, .57, .35, 52, 1700)
    elif cue in ("land", "step", "tremor", "burrow", "bite"):
        strikes = {
            "land": ((.02, 1.), (.106, .83), (.29, .53), (.49, .26)),
            "step": ((.018, .81), (.12, .49), (.20, .22)),
            "tremor": ((.02, 1.), (.23, .86), (.50, .61), (.88, .46), (1.26, .28)),
            "burrow": ((.03, .84), (.27, .61), (.61, .87), (.93, .53), (1.29, .42)),
            "bite": ((.11, .86), (.164, 1.), (.37, .26)),
        }[cue]
        for j, (at, gain) in enumerate(strikes):
            layer(shot, "wood" if cue == "bite" else "rock", at, .63, (.38 + j * .065) * lift, .78 * gain, 42, 2100)
            layer(shot, contact, at + .018, .66, (.71 + j * .085) * lift, gain, 90, 6700)
            layer(shot, snap, at + .037, .40, (.93 + j * .11) * lift, .24 * gain, 180, 4550)
        if cue in ("tremor", "burrow"):
            layer(shot, "magma" if species == 3 else air, .12, duration * .77, .66, .84, 38, 1010, .13, .42)
        if cue == "bite":
            vocal(shot, species, 0., .51, cue, .66)
        if cue == "land":
            layer(shot, "wind", .06, .90, .78, .64, 53, 770, .05, .28)

    elif cue in ("wing", "launch"):
        # Leading-edge pressure, sail tension, trailing wake.
        layer(shot, "wind", .01, duration * .92, (1.5 * lift, .63 * lift), .96, 55, 1740, .15, .28)
        layer(shot, "feathers", .13, duration * .64, (1.47 * lift, .96 * lift), .49, 200, 4000, .08, .12)
        layer(shot, contact, .26, .53, .85 * lift, .67, 115, 5000)
        layer(shot, air, .41, .61, .80 * lift, .34, 80, 3100)
        if cue == "launch":
            vocal(shot, species, .20, .77, cue, .46)
            layer(shot, "rock", .01, .57, .57, .79, 46, 1630)
            layer(shot, snap, .075, .46, .85, .57)
    else:
        # Close discharge, separate projectiles, enveloping shield or advancing front.
        rhythm = {
            "cast": ((.03, .77), (.23, 1.), (.49, .64)),
            "volley": ((.02, .94), (.27, .93), (.51, 1.)),
            "ward": ((.045, .47), (.34, .64), (.72, 1.), (1.10, .45)),
            "surge": ((.025, .48), (.19, .76), (.42, 1.), (.71, .72)),
        }[cue]
        vocal(shot, species, .01, .56 if cue != "ward" else 1.12, cue, .77)
        for j, (at, g) in enumerate(rhythm):
            p = (.74 + j * .12) * lift
            layer(shot, snap, at, .63, (p * 1.25, p * .76), .49 * g, 94, 3900)
            layer(shot, contact, at + .056, .70, p, .76 * g, 140, 4100)
        layer(shot, air, .10, duration * .85, (1.56 * lift, .62 * lift), 1.0, 65, 6200, .16, .31)
        if species == 3:
            layer(shot, "steam", .12, duration * .86, (1.37, .58), .48, 260, 4500, .07, .31)
            layer(shot, "magma", .18, duration * .72, .80, .90, 45, 820)
        elif species == 2:
            layer(shot, "splash", .23, 1.04, (.73, 1.20), .64, 150, 5100)
        elif species == 0:
            layer(shot, "wind", .13, duration * .82, .73, .48, 48, 1450, .20, .37)
        else:
            layer(shot, "gravel", .19, 1.05, (.46, .84), .96, 83, 3100)
    return shot


def render_all():
    fetch()
    for i, family in enumerate(FAMILIES):
        for cue in DURATIONS:
            peak = {"roar": .81, "breath": .56, "step": .66, "wing": .67}.get(cue, .76)
            design(i, cue).save(OUT / f"{family}-{cue}.wav", peak)


if __name__ == "__main__":
    render_all()
