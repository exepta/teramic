"""Indexed analytic normals for solid-color Spirit flower petals only.

The latitude/longitude faces and their triangulation match the existing closed
ellipsoid. UVs are deliberately only suitable for an untextured color material;
a textured material must keep its authored seam/derivative treatment.
"""
import math
from build_castle import unit, cross


def untextured_ellipsoid(mesh, center, radius, material, segments=10, rings=5):
    if mesh.materials[material][1] is not None:
        raise ValueError('Smooth Spirit petals require an untextured material')
    if segments < 3 or rings < 2 or min(radius) <= 0:
        raise ValueError('Invalid flower ellipsoid')
    part = mesh.parts.setdefault(material, {'position': [], 'normal': [], 'uv': [], 'tangent': [], 'indices': []})
    vertices = {}

    def vertex(j, i):
        i %= segments
        if j in (0, rings): i = 0
        key = (j, i)
        if key in vertices: return vertices[key]
        a, b = math.tau * i / segments, math.pi * j / rings
        radial = [math.sin(b) * math.cos(a), math.cos(b), math.sin(b) * math.sin(a)]
        if j == 0: radial = [0., 1., 0.]
        if j == rings: radial = [0., -1., 0.]
        position = [center[k] + radius[k] * radial[k] for k in range(3)]
        normal = unit([radial[k] / radius[k] for k in range(3)])
        tangent = [1., 0., 0.] if j in (0, rings) else unit([-radius[0] * math.sin(a), 0., radius[2] * math.cos(a)])
        index = len(part['position']); vertices[key] = index
        part['position'].append(position)
        part['normal'].append(normal)
        part['uv'].append([i / segments, j / rings])
        part['tangent'].append(tangent + [1.])
        return index

    for j in range(rings):
        for i in range(segments):
            keys = [(j, i), (j + 1, i), (j + 1, i + 1), (j, i + 1)]
            if j == 0: keys = keys[1:]
            elif j == rings - 1: keys = [keys[0], keys[1], keys[3]]
            indices = [vertex(*key) for key in keys]
            points = [part['position'][index] for index in indices]
            ab = [points[1][k] - points[0][k] for k in range(3)]
            ac = [points[2][k] - points[0][k] for k in range(3)]
            outward = [sum(p[k] - center[k] for p in points) / radius[k] ** 2 for k in range(3)]
            if sum(a * b for a, b in zip(cross(ab, ac), outward)) < 0:
                indices.reverse()
            for t in range(1, len(indices) - 1):
                part['indices'].extend([indices[0], indices[t], indices[t + 1]])
