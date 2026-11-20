#!/usr/bin/env python3
"""Move flat recipes without changing bytes; refresh deterministic embedded paths."""
import hashlib
from recipe_paths import CRAFTING, recipe_path, write_builtins


def main():
    moved = {}
    for source in sorted(CRAFTING.glob("*.json")):
        target = recipe_path(source.stem)
        if target.exists():
            raise SystemExit(f"Recipe destination already exists: {target}")
        target.parent.mkdir(parents=True, exist_ok=True)
        data = source.read_bytes()
        source.rename(target)
        assert hashlib.sha256(target.read_bytes()).digest() == hashlib.sha256(data).digest()
        moved[source.name] = target.relative_to(CRAFTING).as_posix()
    write_builtins()
    print(f"Moved {len(moved)} recipes without changing their bytes")


if __name__ == "__main__":
    main()
