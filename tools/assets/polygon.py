"""Triangulate planar authored faces without crossing concave boundaries."""


def triangles(points, normal):
    axis = max(range(3), key=lambda i: abs(normal[i]))
    u, v = (axis + 1) % 3, (axis + 2) % 3
    flat = [(point[u], point[v]) for point in points]
    sign = 1 if normal[axis] > 0 else -1

    def turn(a, b, c):
        return sign * ((b[0]-a[0])*(c[1]-a[1]) - (b[1]-a[1])*(c[0]-a[0]))

    # Preserve the original fan for ordinary convex faces, including all boxes.
    if all(turn(flat[i-1], flat[i], flat[(i+1) % len(flat)]) >= -1e-12 for i in range(len(flat))):
        return [(0, index, index+1) for index in range(1, len(flat)-1)]
    remaining = list(range(len(points)))
    result = []
    while len(remaining) > 3:
        for j, b in enumerate(remaining):
            a, c = remaining[j-1], remaining[(j+1) % len(remaining)]
            if turn(flat[a], flat[b], flat[c]) <= 1e-12:
                continue
            if any(all(turn(flat[x], flat[y], flat[k]) >= -1e-12 for x,y in [(a,b),(b,c),(c,a)])
                   for k in remaining if k not in (a,b,c)):
                continue
            result.append((a,b,c))
            remaining.pop(j)
            break
        else:
            raise ValueError('Face is not a simple planar polygon with matching winding')
    result.append(tuple(remaining))
    return result
