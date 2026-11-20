#!/usr/bin/env python3
"""Validate reproducible tar assets, seamless textures and the paid bucket recipe."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

import build_roads


def pixels(path):
    source = path.read_bytes()
    assert source.startswith(b"\x89PNG\r\n\x1a\n")
    offset, compressed = 8, bytearray()
    width = height = 0
    while offset < len(source):
        size = struct.unpack_from(">I", source, offset)[0]
        kind = source[offset + 4:offset + 8]
        body = source[offset + 8:offset + 8 + size]
        crc = struct.unpack_from(">I", source, offset + 8 + size)[0]
        assert zlib.crc32(kind + body) == crc
        if kind == b"IHDR":
            width, height, depth, color, *_ = struct.unpack(">IIBBBBB", body)
            assert (depth, color) == (8, 6)
        elif kind == b"IDAT":
            compressed.extend(body)
        offset += 12 + size
    data = zlib.decompress(compressed)
    stride = 1 + width * 4
    rows = []
    for y in range(height):
        row = data[y * stride:(y + 1) * stride]
        assert row[0] == 0
        rows.append(row[1:])
    return width, height, rows


class TarAssets(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.blocks = self.root / "assets/terra/blocks"
        with patch.object(build_roads, "ROOT", self.root), patch.object(build_roads, "BLOCKS", self.blocks):
            build_roads.tar_assets()

    def test_generator_reproduces_shipped_assets_exactly(self):
        files = sorted(path for path in self.root.rglob("*") if path.is_file())
        self.assertEqual(len(files), 6)
        for generated in files:
            shipped = build_roads.ROOT / generated.relative_to(self.root)
            self.assertEqual(hashlib.sha256(generated.read_bytes()).digest(),
                             hashlib.sha256(shipped.read_bytes()).digest(), str(shipped))

    def test_film_rests_on_path_substrate_and_has_distinct_wet_dry_material(self):
        wet = json.loads((self.blocks / "wet_tar.json").read_text())
        dry = json.loads((self.blocks / "dried_tar.json").read_text())
        self.assertEqual((wet["id"], dry["id"]), (437, 438))
        for definition in (wet, dry):
            ground, film = definition["collision_boxes"]
            self.assertEqual(ground["minimum"], [0, 0, 0])
            self.assertEqual(ground["maximum"], [1, 15 / 16, 1])
            self.assertEqual(film["minimum"], [0, 15 / 16, 0])
            self.assertEqual(film["maximum"], [1, 1, 1])
            self.assertEqual(definition["face_materials"][3], "Tar surface")
            self.assertEqual(definition["materials"]["Packed earth"]["texture_pattern"], "path_side")
        self.assertLess(wet["materials"]["Tar surface"]["roughness"], 0.25)
        self.assertGreater(dry["materials"]["Tar surface"]["roughness"], 0.85)

    def test_textures_repeat_without_edge_seams(self):
        for slug in ("wet_tar", "dried_tar"):
            width, height, rows = pixels(self.blocks / f"textures/{slug}.png")
            self.assertEqual((width, height), (256, 256))
            self.assertEqual(rows[0], rows[-1])
            for row in rows:
                self.assertEqual(row[:4], row[-4:])

    def test_bucket_recipe_requires_binder_and_empty_bucket(self):
        recipe = json.loads((self.root / "assets/crafting/building/roads/tar_bucket.json").read_text())
        self.assertEqual(recipe["station"], "workbench")
        self.assertEqual(recipe["output"], {"item": {"tool": "tar_bucket"}, "count": 1})
        self.assertEqual(recipe["pattern"]["cells"], [
            {"item": "bitumen", "count": 4},
            {"item": {"tool": "bucket"}, "count": 1},
        ])


if __name__ == "__main__":
    unittest.main()
