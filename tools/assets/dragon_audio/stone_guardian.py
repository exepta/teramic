"""Render the Petrified Forest guardian from the project's recorded stone and gravel Foley."""
from creature_foley import LOCAL, Shot
from samples import ROOT

DEST = ROOT / "assets/audio/golem"
LOCAL.update(slab="footsteps/detail/stone-2", heel="footsteps/detail/stone-6",
             rubble="footsteps/detail/gravel-3", chips="footsteps/detail/gravel-5")


def render(name, seconds, strikes, grinding=False):
    shot = Shot(seconds)
    if grinding:
        shot.add("gravel", .025, .015, seconds * .66, .48, .50, 65, 1950, .11, .19)
        shot.add("creak", .08, .03, seconds * .80, .65, .18, 60, 890, .08, .23)
    for at, material, speed, weight in strikes:
        shot.add(material, at, 0., .86, speed, weight, 45, 2700, .008, .10)
        shot.add("chips", at + .055, .012, .55, speed * 1.37, weight*.29, 145, 4250,.015,.16)
    shot.save(DEST / (name+".wav"), .74)


def render_all():
    DEST.mkdir(exist_ok=True)
    render("wake", 1.85, [(.19,"slab",.63,.74),(.63,"rubble",.83,.48),(1.06,"heel",.68,.60)], True)
    render("hurt", .68, [(.01,"slab",.87,.92),(.12,"chips",1.13,.53)])
    render("punch", .88, [(.01,"rock",.65,1.),(.074,"slab",.90,.59),(.28,"rubble",.85,.29)])
    render("slam", 1.53, [(.012,"heel",.46,1.14),(.069,"rock",.59,1.),
                           (.16,"rubble",.75,.67),(.47,"gravel",.82,.34),(.72,"chips",1.2,.25)])
    render("fall", 2.15, [(.03,"rubble",.68,.53),(.42,"slab",.74,.50),
                           (.91,"heel",.45,1.1),(1.04,"rock",.66,.79),(1.36,"chips",.88,.40)], True)
    render("step-left", .79, [(.015,"slab",.65,.88),(.095,"rubble",.86,.53)])
    render("step-right", .76, [(.012,"heel",.69,.88),(.10,"gravel",.77,.47)])


if __name__ == "__main__":
    render_all()
