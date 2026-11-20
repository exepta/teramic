#!/usr/bin/env python3
"""Export compact SVG block-plan previews from the actual authored JSON cells.

These schematics use voxel occupancy and block metadata colors; runtime GLB
bevels and the fine geometry of stairs/furniture belong to the game renderer.
No hand-authored example building or inferred material costs are substituted.
"""
import html
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
OUTPUT = ROOT / "src-ui/assets/buildings"
NAMES = {
    "village-house-temperate-v1": "temperate-house.svg",
    "spruce-cottage-v1": "spruce-cottage.svg",
}


def palette():
    colors = {}
    for path in sorted((ASSETS / "terra/blocks").glob("*.json")):
        block = json.loads(path.read_text())
        first = next(iter(block.get("materials", {}).values()), {})
        colors[block.get("id")] = first.get("base_color", [0.50, 0.42, 0.32])[:3]
    return colors


def project(point):
    x, y, z = point
    return ((x - z) * 12.0, (x + z) * 6.0 - y * 14.0)


def render(template, colors):
    cells = {tuple(cell["position"]): cell["material"]
             for step in template["steps"] for cell in step["cells"]}
    faces = []
    # Only outward visible cube faces are emitted. Ordering is deterministic.
    sides = [((1, 0, 0), [(1, 0, 0), (1, 0, 1), (1, 1, 1), (1, 1, 0)], 0.72),
             ((0, 0, 1), [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)], 0.87),
             ((0, 1, 0), [(0, 1, 0), (1, 1, 0), (1, 1, 1), (0, 1, 1)], 1.08)]
    for (x, y, z), material in cells.items():
        for side, offsets, shade in sides:
            neighbor = (x + side[0], y + side[1], z + side[2])
            if neighbor in cells:
                continue
            points = [project((x + a, y + b, z + c)) for a, b, c in offsets]
            rgb = colors.get(material, [0.5, 0.42, 0.32])
            fill = "#" + "".join(f"{round(max(0.0, min(1.0, value * shade)) * 255):02x}" for value in rgb)
            faces.append((x + z, y, x, side, points, fill))
    faces.sort(key=lambda face: face[:4])
    points = [point for face in faces for point in face[4]]
    left, right = min(p[0] for p in points), max(p[0] for p in points)
    top, bottom = min(p[1] for p in points), max(p[1] for p in points)
    margin = 10
    polygons = []
    for *_order, points, fill in faces:
        coords = " ".join(f"{x-left+margin:.2f},{y-top+margin:.2f}" for x, y in points)
        polygons.append(f'<polygon points="{coords}" fill="{fill}" stroke="#21170e" stroke-width="0.35"/>')
    title = html.escape(template.get("display_name", template["id"]))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {right-left+margin*2:.2f} {bottom-top+margin*2:.2f}">\n'
            f'<title>{title}</title>\n' + "\n".join(polygons) + '\n</svg>\n')


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    colors = palette()
    written = 0
    for path in sorted((ASSETS / "buildings/templates").rglob("*.json")):
        template = json.loads(path.read_text())
        if template["id"] in NAMES:
            (OUTPUT / NAMES[template["id"]]).write_text(render(template, colors))
            written += 1
    if written != len(NAMES):
        raise SystemExit("Expected both shipped building templates")
    print(f"Generated {written} SVG block-plan previews from actual templates")


if __name__ == "__main__":
    main()
