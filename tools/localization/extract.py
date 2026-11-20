#!/usr/bin/env python3
"""Extract product text for the initial bilingual Fluent migration (no network)."""
import html
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "src-ui/locales/source"
OUT.mkdir(parents=True, exist_ok=True)


def human(value):
    value = html.unescape(value).strip()
    return (len(value) > 1 and re.search(r"[A-Za-zÄÖÜäöüß]", value)
            and not any(token in value for token in ["{{", "@if", "@for", "@else", "=>", "::", "#[", "../", "https://", "texture:"])
            and not re.fullmatch(r"[a-z0-9_./:#-]+", value)
            and not re.fullmatch(r"[\w.-]+\.(rs|html|png|toml|json|glb|gltf|css)", value)
            and len(value) < 1200)


static = set()
for path in (ROOT / "src-ui").rglob("*.html"):
    source = path.read_text()
    for value in re.findall(r">([^<>]+)<", source):
        if any(c in value for c in "@{}"):
            continue
        if human(value):
            static.add(html.unescape(value).strip())
    for value in re.findall(r'\b(?:title|placeholder|aria-label)="([^"]+)"', source):
        if human(value):
            static.add(html.unescape(value).strip())
    for expression in re.findall(r"\{\{(.+?)\}\}", source):
        for value in re.findall(r"'([^']+)'", expression):
            if human(value):
                static.add(value)

assets = set()
for directory in ["assets/terra/blocks", "assets/terra/items", "assets/crafting", "assets/biome"]:
    for path in (ROOT / directory).rglob("*.json"):
        data = json.loads(path.read_text())
        if isinstance(data, dict):
            for key in ["name", "display_name", "description"]:
                value = data.get(key)
                if isinstance(value, str) and human(value):
                    assets.add(value)

runtime = set()
paths = list((ROOT / "crates/terra-ui/src").rglob("*.rs")) + list((ROOT / "src-ui").rglob("*.rs"))
paths += list((ROOT / "crates/terra-network/src").glob("server*.rs"))
paths += list((ROOT / "crates/terra-core/src").glob("*.rs"))
for path in paths:
    if "test" in path.stem or "tests" in path.parts:
        continue
    source = path.read_text().split("#[cfg(test)]")[0]
    for match in re.finditer(r'"(?:[^"\\]|\\.)*"', source):
        try:
            value = json.loads(match.group())
        except (ValueError, TypeError):
            continue
        if human(value) and (" " in value or value[0].isupper()):
            runtime.add(value)

for name, values in [("interface", static), ("content", assets - static), ("runtime", runtime - assets - static)]:
    if name == "runtime" and list(OUT.glob("runtime-*.json")):
        continue
    path = OUT / f"{name}.json"
    if not path.exists():
        path.write_text(json.dumps([{"source": s, "de": None, "en": None} for s in sorted(values)], ensure_ascii=False, indent=2) + "\n")
    print(name, len(values), path.relative_to(ROOT))
