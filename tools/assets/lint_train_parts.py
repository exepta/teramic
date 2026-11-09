#!/usr/bin/env python3
"""Validate native train assets without starting Bevy or using a graphics device.

Run from any directory:
    python tools/assets/lint_train_parts.py --log target/verification213/asset-lint.log

This reads shipped assets and catalogs only. It never regenerates them.
"""

import argparse
import ast
import base64
import json
import math
from pathlib import Path
import re
import struct
import sys


ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / "assets/terra/blocks"
COMPONENTS = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2),
              5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
WIDTHS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4,
          "MAT2": 4, "MAT3": 9, "MAT4": 16}


def dot(first, second):
    return sum(a * b for a, b in zip(first, second))


def subtract(first, second):
    return tuple(a - b for a, b in zip(first, second))


def cross(first, second):
    a, b, c = first
    x, y, z = second
    return b * z - c * y, c * x - a * z, a * y - b * x


def unit(vector):
    length = math.sqrt(dot(vector, vector))
    return tuple(value / length for value in vector)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def expected_parts():
    """Compare new names against actual Rust descriptors, not just asset copies."""
    basic = ast.parse((ROOT / "tools/assets/build_train_parts.py").read_text())
    rows = next(ast.literal_eval(node.value) for node in basic.body
                if isinstance(node, ast.Assign)
                and any(isinstance(target, ast.Name) and target.id == "PARTS"
                        for target in node.targets))
    names = {material: (slug, name) for material, slug, name in rows}
    source = (ROOT / "crates/terra-core/src/locomotive_parts.rs").read_text()
    descriptors = re.findall(
        r'Definition\s*\{\s*id:\s*(\d+),\s*slug:\s*"([^"]+)",\s*name:\s*"([^"]+)"',
        source,
    )
    last = int(re.search(r"pub const LAST: u16 = (\d+);", source)[1])
    require(len(descriptors) == last - 218 + 1, "expected all Rust BR52 descriptors")
    for material, slug, name in descriptors:
        require(int(material) not in names, f"duplicate descriptor {material}")
        names[int(material)] = (slug, name)
    require(set(names) == set(range(210, last + 1)), "missing native train IDs")
    return names


def valid_coordinates(values, material):
    for axis, value in enumerate(values):
        low = -0.36 if material == 232 else -0.61 if material == 211 and axis != 1 else -.14 if material in (224, 225, 226) and axis == 1 else 0
        high = 1.76 if material == 232 else 1.61 if material == 211 and axis != 1 else 2 if material == 248 and axis == 1 else 1
        require(math.isfinite(value), f"{material}: non-finite coordinate")
        require(low - 1e-5 <= value <= high + 1e-5,
                f"{material}: coordinate {axis}={value} outside owner bounds")


def load_buffers(path, model):
    buffers = []
    for descriptor in model["buffers"]:
        uri = descriptor["uri"]
        data = (base64.b64decode(uri.split(",", 1)[1], validate=True)
                if uri.startswith("data:") else (path.parent / uri).read_bytes())
        require(len(data) == descriptor["byteLength"], f"{path.name}: buffer length")
        buffers.append(data)
    for view in model["bufferViews"]:
        require(0 <= view["buffer"] < len(buffers), f"{path.name}: buffer-view index")
        start, length = view.get("byteOffset", 0), view["byteLength"]
        require(start >= 0 and length > 0 and start + length <= len(buffers[view["buffer"]]),
                f"{path.name}: buffer-view range")
    return buffers


def accessor_rows(path, model, buffers, accessor):
    require("sparse" not in accessor, f"{path.name}: unexpected sparse native accessor")
    require(accessor["componentType"] in COMPONENTS and accessor["type"] in WIDTHS,
            f"{path.name}: accessor type")
    require(0 <= accessor["bufferView"] < len(model["bufferViews"]),
            f"{path.name}: accessor buffer-view index")
    code, size = COMPONENTS[accessor["componentType"]]
    width = WIDTHS[accessor["type"]]
    view = model["bufferViews"][accessor["bufferView"]]
    packed = width * size
    stride = view.get("byteStride", packed)
    offset, count = accessor.get("byteOffset", 0), accessor["count"]
    require(count > 0 and offset >= 0 and offset % size == 0 and stride >= packed,
            f"{path.name}: accessor alignment/count/stride")
    require(offset + (count - 1) * stride + packed <= view["byteLength"],
            f"{path.name}: accessor outside buffer view")
    data = buffers[view["buffer"]]
    start = view.get("byteOffset", 0) + offset
    rows = [struct.unpack_from("<" + code * width, data, start + index * stride)
            for index in range(count)]
    require(all(math.isfinite(value) for row in rows for value in row),
            f"{path.name}: non-finite accessor value")
    for bound, comparison in [("min", min), ("max", max)]:
        if bound in accessor:
            declared = accessor[bound]
            require(len(declared) == width and all(math.isfinite(value) for value in declared),
                    f"{path.name}: invalid accessor {bound}")
            actual = [comparison(row[axis] for row in rows) for axis in range(width)]
            require(all(abs(a - b) <= 1e-5 * max(1, abs(a), abs(b))
                        for a, b in zip(actual, declared)),
                    f"{path.name}: incorrect accessor {bound}")
    return rows


def preview_coverage(path, model, rows):
    """Rasterize alpha coverage with the actual orthographic preview camera.

    This checks front-face winding, culling and camera framing on the native
    imported triangles. It deliberately makes no claim about GPU lighting.
    The shipped train nodes are identity transforms; refuse an unsupported
    transform rather than silently projecting geometry in the wrong space.
    """
    size, height = 128, 1.95
    eye, target = (2.9, 2.5, 3.5), (0.5, 0.5, 0.5)
    towards_camera = unit(subtract(eye, target))
    right = unit(cross((0, 1, 0), towards_camera))
    up = cross(towards_camera, right)
    pixels, front_faces = set(), 0

    def edge(first, second, point):
        return ((second[0] - first[0]) * (point[1] - first[1])
                - (second[1] - first[1]) * (point[0] - first[0]))

    def visit(index):
        nonlocal front_faces
        node = model["nodes"][index]
        require(not any(key in node for key in ("translation", "rotation", "scale", "matrix")),
                f"{path.name}: extend CPU projection for transformed native nodes")
        if "mesh" in node:
            for primitive in model["meshes"][node["mesh"]]["primitives"]:
                material = model["materials"][primitive["material"]]
                alpha = material.get("pbrMetallicRoughness", {}).get("baseColorFactor", [1] * 4)[3]
                if alpha <= 8 / 255:
                    continue
                positions = rows[primitive["attributes"]["POSITION"]]
                indices = rows[primitive["indices"]]
                for offset in range(0, len(indices), 3):
                    points = [positions[indices[offset + corner][0]] for corner in range(3)]
                    normal = cross(subtract(points[1], points[0]), subtract(points[2], points[0]))
                    if not material.get("doubleSided", False) and dot(normal, towards_camera) <= 0:
                        continue
                    if not all(0.01 < dot(subtract(eye, point), towards_camera) < 30 for point in points):
                        continue
                    projected = [(size * (0.5 + dot(subtract(point, target), right) / height),
                                  size * (0.5 - dot(subtract(point, target), up) / height))
                                 for point in points]
                    area = edge(projected[0], projected[1], projected[2])
                    if abs(area) < 1e-10:
                        continue
                    front_faces += 1
                    sign = 1 if area > 0 else -1
                    x0 = max(0, math.floor(min(point[0] for point in projected)))
                    x1 = min(size - 1, math.ceil(max(point[0] for point in projected)))
                    y0 = max(0, math.floor(min(point[1] for point in projected)))
                    y1 = min(size - 1, math.ceil(max(point[1] for point in projected)))
                    for y in range(y0, y1 + 1):
                        for x in range(x0, x1 + 1):
                            point = (x + 0.5, y + 0.5)
                            if all(sign * edge(projected[index], projected[(index + 1) % 3], point) >= -1e-8
                                   for index in range(3)):
                                pixels.add((x, y))
        for child in node.get("children", []):
            visit(child)

    for index in model["scenes"][model.get("scene", 0)]["nodes"]:
        visit(index)
    require(front_faces > 0 and len(pixels) > 64,
            f"{path.name}: no sufficient front-facing preview alpha coverage ({len(pixels)} pixels)")
    return front_faces, len(pixels)


def check_model(path, material):
    model = read_json(path)
    require(model["asset"]["version"] == "2.0", f"{path.name}: GLTF version")
    buffers = load_buffers(path, model)
    rows = [accessor_rows(path, model, buffers, accessor) for accessor in model["accessors"]]
    triangles = 0
    for mesh in model["meshes"]:
        for primitive in mesh["primitives"]:
            attributes = primitive["attributes"]
            require(all(isinstance(index, int) and 0 <= index < len(rows)
                        for index in [*attributes.values(), primitive["indices"]]),
                    f"{path.name}: invalid accessor reference")
            positions = rows[attributes["POSITION"]]
            require(all(len(point) == 3 for point in positions), f"{path.name}: positions")
            for point in positions:
                valid_coordinates(point, material)
            for semantic, index in attributes.items():
                require(len(rows[index]) == len(positions), f"{path.name}: {semantic} count")
            index_accessor = model["accessors"][primitive["indices"]]
            require(index_accessor["type"] == "SCALAR"
                    and index_accessor["componentType"] in (5121, 5123, 5125),
                    f"{path.name}: unsigned scalar indices required")
            indices = rows[primitive["indices"]]
            require(primitive.get("mode", 4) == 4 and len(indices) % 3 == 0,
                    f"{path.name}: native triangle topology")
            require(all(0 <= index[0] < len(positions) for index in indices),
                    f"{path.name}: mesh index outside vertex buffer")
            require(0 <= primitive["material"] < len(model["materials"]),
                    f"{path.name}: material index")
            normals = rows[attributes["NORMAL"]]
            for offset in range(0, len(indices), 3):
                corners = [indices[offset + corner][0] for corner in range(3)]
                a, b, c = [positions[index] for index in corners]
                face = cross(subtract(b, a), subtract(c, a))
                area = math.sqrt(dot(face, face))
                require(area > 1e-12, f"{path.name}: degenerate triangle")
                require(all(dot(face, normals[index]) > 0 for index in corners),
                        f"{path.name}: triangle winding opposes its vertex normals")
            triangles += len(indices) // 3
    require(triangles > 0, f"{path.name}: empty mesh")
    for image in model.get("images", []):
        if "uri" in image and not image["uri"].startswith("data:"):
            require((path.parent / image["uri"]).is_file(), f"{path.name}: missing texture")
    front_faces, pixels = preview_coverage(path, model, rows)
    return sum(map(len, buffers)), triangles, front_faces, pixels


def check_recipes(parts):
    source = (ROOT / "crates/terra-core/src/crafting/builtins.rs").read_text()
    paths = re.findall(r'include_str!\("../../../../([^"]+)"\)', source)
    recipes = [read_json(ROOT / path) for path in paths]
    require(415 <= len(recipes) <= 512, "incomplete or oversized embedded recipe catalog")
    by_id = {recipe["id"]: recipe for recipe in recipes}
    require(len(by_id) == len(recipes), "duplicate embedded recipe IDs")
    keys = set()
    for recipe in recipes:
        key = 0xcbf29ce484222325
        for byte in recipe["id"].encode():
            key = ((key ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
        require(key not in keys, "recipe key collision")
        keys.add(key)
    for material, (slug, name) in parts.items():
        recipe = by_id.get("teramic::" + slug)
        require(recipe is not None, f"missing embedded recipe for {slug}")
        require(recipe["name"] == name and recipe["output"]["item"] == {"block": material},
                f"{slug}: recipe name/output mismatch")
    payload = json.dumps({"schema_version": 1, "recipes": recipes}, ensure_ascii=False,
                         separators=(",", ":")).encode()
    require(len(payload) <= 1024 * 1024, "recipe catalog exceeds the 1MiB transport budget")
    return len(recipes), len(payload)


def lint():
    parts = expected_parts()
    binary_bytes = triangles = boxes = front_faces = 0
    coverage = []
    for material, (slug, name) in sorted(parts.items()):
        path = BLOCKS / (slug + ".json")
        definition = read_json(path)
        require(definition["id"] == material and definition["name"] == name,
                f"{slug}: JSON ID/name differs from its catalog descriptor")
        bounds = definition["collision_boxes"]
        require(0 < len(bounds) <= 128, f"{slug}: collision budget")
        for box in bounds:
            valid_coordinates(box["minimum"], material)
            valid_coordinates(box["maximum"], material)
            require(all(low < high for low, high in zip(box["minimum"], box["maximum"])),
                    f"{slug}: empty or inverted collision volume")
        require(definition["model"] == f"models/{slug}.gltf", f"{slug}: incorrect model path")
        size, faces, front, pixels = check_model(BLOCKS / definition["model"], material)
        binary_bytes += size
        triangles += faces
        boxes += len(bounds)
        front_faces += front
        coverage.append((pixels, material, slug))
    recipe_count, recipe_bytes = check_recipes(parts)
    affected = ", ".join(f"{material}: {pixels}px" for pixels, material, _ in sorted(coverage, key=lambda row: row[1])
                         if material in (249, 250, 261, 262, 271, 272))
    return [
        "PASS: CPU-only native train asset lint; no regeneration, window or GPU.",
        f"Train IDs210..{max(parts)}: {len(parts)} complete JSON/model/recipe entries with matching names.",
        f"Collision: {boxes} finite bounded boxes; every part stays within the 128-box budget.",
        f"GLTF: {binary_bytes} binary bytes, {triangles} triangles; buffers, accessors, indices, bounds and textures valid.",
        f"CPU preview projection: all {len(parts)} have >64 alpha-covered pixels at the actual 128x128 camera; {front_faces} front-facing triangles; winding agrees with every vertex normal.",
        f"Smallest projected coverage: {min(coverage)[0]} pixels ({min(coverage)[2]}).",
        f"Reported startup icons, projected alpha coverage: {affected}.",
        f"Embedded recipes: {recipe_count} unique IDs/keys; JSON payload {recipe_bytes} bytes within 1MiB.",
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", type=Path, help="also write this verification report to a file")
    arguments = parser.parse_args()
    status = 0
    try:
        lines = lint()
    except (ValueError, KeyError, IndexError, OSError, StopIteration, struct.error) as error:
        lines = [f"FAIL: {error}"]
        status = 1
    report = "\n".join(lines) + "\n"
    print(report, end="")
    if arguments.log:
        arguments.log.parent.mkdir(parents=True, exist_ok=True)
        arguments.log.write_text(report, encoding="utf-8")
    return status


if __name__ == "__main__":
    sys.exit(main())
