"""Bake authored biome silhouettes to the same cube grid as the older flora."""
from collections import deque
from functools import lru_cache

import numpy as np

from build_spirit_blossom import export
from voxel_props.geometry import VoxelSculpt


@lru_cache(maxsize=256)
def barycentric_grid(steps):
    return np.array([(1 - (i+j)/steps, i/steps, j/steps)
                     for i in range(steps+1) for j in range(steps+1-i)])


def voxelize(mesh, resolution=24):
    sculpt = VoxelSculpt(resolution)
    bounds = np.concatenate([part['position'] for part in mesh.parts.values()]) * resolution
    low = np.floor(bounds.min(axis=0) + 1.e-6)
    high = np.maximum(low, np.ceil(bounds.max(axis=0) - 1.e-6) - 1)
    for material, part in mesh.parts.items():
        vertices = np.asarray(part['position']) * resolution
        for triangle in np.asarray(part['indices']).reshape(-1, 3):
            points = vertices[triangle]
            edges = np.roll(points, -1, axis=0) - points
            length = np.linalg.norm(edges, axis=1).max()
            normal = np.cross(edges[0], edges[1])
            normal /= max(np.linalg.norm(normal), 1.e-12)
            # Move a face exactly on a cell boundary towards the solid.
            samples = barycentric_grid(max(1, int(np.ceil(length * 2.5)))) @ points
            cells = np.clip(np.floor(samples - normal * .015), low, high).astype(int)
            sculpt.cells.update((tuple(cell), material) for cell in np.unique(cells, axis=0))

    # Seal the interior before greedy meshing. Open arches, eye sockets and
    # gaps between stems remain connected to the exterior.
    cells = np.array(list(sculpt.cells))
    lo, hi = cells.min(axis=0)-1, cells.max(axis=0)+1
    occupied = np.zeros(hi-lo+1, dtype=bool)
    occupied[tuple((cells-lo).T)] = True
    front = deque([(0, 0, 0)])
    visited = occupied.copy()
    visited[0, 0, 0] = True
    while front:
        p = front.popleft()
        for k in range(3):
            for delta in [-1, 1]:
                q = list(p)
                q[k] += delta
                q = tuple(q)
                if 0 <= q[k] < visited.shape[k] and not visited[q]:
                    visited[q] = True
                    front.append(q)
    inner_material = next(iter(sculpt.cells.values()))
    for cell in np.argwhere(~visited) + lo:
        sculpt.cells[tuple(cell)] = inner_material
    mesh.parts.clear()
    sculpt.emit(mesh)


def export_voxels(mesh, slug, luminous=(), *, resolution=24):
    voxelize(mesh, resolution)
    return export(mesh, slug, luminous)
