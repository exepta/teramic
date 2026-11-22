"""Build a grounded sheaf of bent, overlapping straw culms with loose wheat ears."""
import math
import random
from build_castle import Mesh
from build_spirit_blossom import export
from cemetery.geometry import cylinder, ellipsoid


def build():
    rng = random.Random(464)
    mesh = Mesh("straw")
    for material, rgb in [('ochre', [.55,.34,.12]), ('wheat', [.83,.61,.26]), ('stalk', [.72,.48,.18])]:
        mesh.materials[material] = (rgb, None, 0, .94)
    for i in range(23):
        angle = .72 + rng.uniform(-.66,.66) + (i % 4 == 0) * 1.32
        radius = rng.uniform(.25,.45)
        x = .5 + rng.uniform(-.15,.15)
        z = .5 + rng.uniform(-.13,.13)
        dx = math.cos(angle)*radius
        dz = math.sin(angle)*radius
        start = [max(.015,min(.985,x-dx)), .016, max(.015,min(.985,z-dz))]
        bend = [x, .038 + .14*(1 - abs(x-.5)*5) + rng.uniform(0,.022), z]
        end = [max(.02,min(.98,x+dx)), .024 + (i%3)*.035, max(.02,min(.98,z+dz))]
        material = ('wheat','stalk','ochre')[i%3]
        cylinder(mesh, start, bend, .016, material, 4)
        cylinder(mesh, bend, end, .016, material, 4, .009)
        if i % 4 == 0:
            ear = [(a*.18+b*.82) for a,b in zip(bend,end)]
            ellipsoid(mesh, ear, [.028,.021,.053], 'wheat', 6, 3)
    export(mesh, 'nest_straw', ())


if __name__ == '__main__':
    build()
