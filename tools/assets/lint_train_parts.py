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
    require(len(descriptors) == 76, "expected all 76 Rust BR52 descriptors")
    for material, slug, name in descriptors:
        require(int(material) not in names, f"duplicate descriptor {material}")
        names[int(material)] = (slug, name)
    require(set(names) == set(range(210, 294)), "missing train IDs in 210..293")
    return names


def valid_coordinates(values, material):
    for axis, value in enumerate(values):
        low = -0.61 if material == 211 and axis != 1 else 0
        high = 1.61 if material == 211 and axis != 1 else 2 if material == 248 and axis == 1 else 1
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
            triangles += len(indices) // 3
    require(triangles > 0, f"{path.name}: empty mesh")
    for image in model.get("images", []):
        if "uri" in image and not image["uri"].startswith("data:"):
            require((path.parent / image["uri"]).is_file(), f"{path.name}: missing texture")
    return sum(map(len, buffers)), triangles


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
    binary_bytes = triangles = boxes = 0
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
        size, faces = check_model(BLOCKS / definition["model"], material)
        binary_bytes += size
        triangles += faces
        boxes += len(bounds)
    recipe_count, recipe_bytes = check_recipes(parts)
    return [
        "PASS: CPU-only native train asset lint; no regeneration, window or GPU.",
        f"Train IDs210..293: {len(parts)} complete JSON/model/recipe entries with matching names.",
        f"Collision: {boxes} finite bounded boxes; every part stays within the 128-box budget.",
        f"GLTF: {binary_bytes} binary bytes, {triangles} triangles; buffers, accessors, indices, bounds and textures valid.",
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
