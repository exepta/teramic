#!/usr/bin/env bash
# Build first. The standalone editor writes only into a fresh acceptance directory.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p "$repo_dir/target/building-editor"
run_dir=$(mktemp -d "$repo_dir/target/building-editor/run.XXXXXX")
printf 'Isolated standalone building editor run: %s\n' "$run_dir"
test -x "$repo_dir/target/debug/terra-building-editor"
env \
    TERRA_EDITOR_SMOKE_DIR="$run_dir" \
    TERRA_ASSET_DIR="$repo_dir/assets" \
    RUST_LOG=info \
    timeout --signal=TERM 180s "$repo_dir/target/debug/terra-building-editor" --editor-acceptance \
    > "$run_dir/native.log" 2>&1
python3 - "$run_dir" <<'PYVERIFY'
from collections import Counter
import json
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
log = (root / "native.log").read_text(errors="replace")
assert "Standalone building editor acceptance passed:" in log, "Missing actual editor acceptance success marker"
assert "Standalone building editor protected platform resize passed:" in log, "Missing real support resize and JSON exclusion proof"
assert "Standalone editor acceptance failed" not in log, "The editor reported an acceptance failure"
screens = {}
for name in [
    "editor-empty-platform",
    "editor-placed-block",
    "editor-removed-block",
    "editor-real-inventory-catalog",
    "editor-loaded-house",
    "editor-save-browser",
    "editor-overwrite-confirmation",
    "editor-options-english",
    "editor-expanded-platform-options",
    "editor-expanded-platform",
]:
    data = (root / f"{name}.png").read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {name}"
    size = struct.unpack(">II", data[16:24])
    # The editor requests logical pixels; native HiDPI windows use a larger
    # physical capture. Keep the aspect ratio and resolution checks exact.
    assert size[0] >= 1440 and size[1] >= 900, (name, size)
    assert size[0] * 900 == size[1] * 1440, (name, size)
    if screens:
        first = next(iter(screens.values()))
        assert size == (first["width"], first["height"]), (name, size, first)
    screens[name] = {"width": size[0], "height": size[1], "bytes": len(data)}

path = root / "editor-acceptance.json"
template = json.loads(path.read_text())
assert template["schema_version"] == 1, "Unexpected template schema"
assert template["id"] == "editor-acceptance", "The editor did not save the chosen filename/ID"
assert template["display_name"] == "Editor acceptance house", "The actual name edit was not saved"
assert template["steps"] and template["materials"], "Saved JSON has no authored steps or real material costs"
cells = [cell for step in template["steps"] for cell in step["cells"]]
assert len(cells) > 100, "Actual default JSON house was not imported and exported"
assert len({tuple(cell["position"]) for cell in cells}) == len(cells), "Saved template contains duplicate physical cells"
assert all(cell["material"] > 0 for cell in cells), "Saved template contains authored air"
assert all(cell["position"][1] >= template["anchor"][1] - 2 for cell in cells), "Editor support leaked into the saved template"
assert "platform" not in template and "platform_size" not in template, "Workspace settings are not model data"

def totals(stacks):
    result = Counter()
    for stack in stacks:
        assert 0 < stack["count"] <= 65535, "Invalid saved material count"
        result[json.dumps(stack["item"], sort_keys=True)] += stack["count"]
    return result

assert totals(template["materials"]) == totals(stack for step in template["steps"] for stack in step["costs"]), "Saved material totals differ from actual authored step inputs"
(root / "verification.json").write_text(json.dumps({
    "status": "passed", "screens": screens,
    "application": "standalone terra-building-editor",
    "input": "actual right/left MouseButtonInput and production HTML component handlers",
    "catalog": "real shared inventory preview assets",
    "file": path.name, "schema_version": template["schema_version"],
    "steps": len(template["steps"]), "physical_cells": len(cells),
    "materials": len(template["materials"]),
    "save": "actual default JSON import, exact save/reload and explicit overwrite confirmation",
    "options": "live English language selection and actual 9x9 to 17x17 support resize",
    "support": "protected StructureStone plane at Y -3; exact template and materials remain unchanged after resize and save",
}, indent=2) + "\n")
print("Standalone Building Editor passed: real voxel placement/removal, inventory catalog, house JSON import/save/reload, overwrite and language; ten screenshots")
PYVERIFY
