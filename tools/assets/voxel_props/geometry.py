"""Offline, one-cell voxel sculpture with one material per occupied microvoxel.

Shapes describe occupied cells, never curved render surfaces. The exporter only
emits their outer boundary and greedily joins coplanar faces of one material.
There are no coincident faces between touching cubes or material layers.
"""
import math


class VoxelSculpt:
    def __init__(self, resolution=24):
        if not 4 <= resolution <= 64:
            raise ValueError('Voxel prop resolution must be between 4 and 64')
        self.resolution = resolution
        self.cells = {}
        self.collision = []

    def _range(self, low, high):
        n = self.resolution
        return [range(max(0, math.floor(low[k] * n)),
                      min(n, math.ceil(high[k] * n))) for k in range(3)]

    def fill(self, low, high, material, contains):
        axes = self._range(low, high)
        n = self.resolution
        count = 0
        for x in axes[0]:
            for y in axes[1]:
                for z in axes[2]:
                    if contains([(v + .5) / n for v in (x, y, z)]):
                        count += 1
                        if material is None:
                            self.cells.pop((x, y, z), None)
                        else:
                            self.cells[x, y, z] = material
        return count

    def box(self, low, high, material, bevel=0, solid=False):
        # The shared authoring call keeps its collider contract; voxel art has
        # intentionally sharp edges, regardless of an old bevel parameter.
        if solid:
            self.collision.append({'minimum': low, 'maximum': high})
        self.fill(low, high, material, lambda _: True)

    def blob(self, center, radius, material):
        # Cell-centre occupancy creates a stepped silhouette. A narrow petal or
        # stem gets at least one cube in its thin dimension instead of vanishing.
        radius = [max(r, .55 / self.resolution) for r in radius]
        low = [c - r for c, r in zip(center, radius)]
        high = [c + r for c, r in zip(center, radius)]
        count = self.fill(low, high, material,
                          lambda p: sum(((v-c)/r)**2 for v, c, r in zip(p, center, radius)) <= 1)
        if not count:
            cell = tuple(min(self.resolution-1, max(0, int(v*self.resolution))) for v in center)
            self.cells[cell] = material

    def branch(self, start, end, radius, material, end_radius=None):
        end_radius = radius if end_radius is None else end_radius
        delta = [b-a for a, b in zip(start, end)]
        length = math.sqrt(sum(v*v for v in delta))
        steps = max(1, math.ceil(length * self.resolution * 2))
        for i in range(steps + 1):
            t = i / steps
            center = [a+t*d for a, d in zip(start, delta)]
            r = max(.55/self.resolution, radius+(end_radius-radius)*t)
            self.blob(center, [r]*3, material)
            # A diagonal one-voxel twig must remain face-connected, not leave
            # disconnected corner-touching beads along its centre line.
            cell = tuple(min(self.resolution-1, max(0, int(v*self.resolution))) for v in center)
            if i:
                walk = list(previous)
                for axis in range(3):
                    while walk[axis] != cell[axis]:
                        walk[axis] += 1 if cell[axis] > walk[axis] else -1
                        self.cells[tuple(walk)] = material
            self.cells[cell] = material
            previous = cell

    def arch(self, center, inner, outer, depth, material):
        x, y, z = center
        self.fill([x-outer, y, z-depth/2], [x+outer, y+outer, z+depth/2], material,
                  lambda p: inner**2 <= (p[0]-x)**2+(p[1]-y)**2 <= outer**2)

    def emit(self, mesh):
        n = self.resolution
        mesh.collision.extend(self.collision)
        for axis in range(3):
            u, v = (axis+1)%3, (axis+2)%3
            for sign in (-1, 1):
                slices = {}
                for cell, material in self.cells.items():
                    other = list(cell)
                    other[axis] += sign
                    if tuple(other) in self.cells:
                        continue
                    slices.setdefault(cell[axis], {})[cell[u], cell[v]] = material
                for plane in sorted(slices):
                    faces = slices[plane]
                    while faces:
                        a, b = min(faces)
                        material = faces[a, b]
                        width = 1
                        while faces.get((a+width, b)) == material:
                            width += 1
                        height = 1
                        while all(faces.get((a+i, b+height)) == material for i in range(width)):
                            height += 1
                        for i in range(width):
                            for j in range(height):
                                del faces[a+i, b+j]
                        points = []
                        for du, dv in ((0, 0), (width, 0), (width, height), (0, height)):
                            p = [0., 0., 0.]
                            p[axis] = (plane + (sign > 0)) / n
                            p[u] = (a+du) / n
                            p[v] = (b+dv) / n
                            points.append(p)
                        normal = [0., 0., 0.]
                        normal[axis] = sign
                        mesh.polygon(points, material, normal)
        return mesh
