#!/usr/bin/env python3
"""Build deterministic Fluent catalogs and migrate presentation-only template bindings."""
import argparse
import hashlib
import html
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
LOCALES = ROOT / "src-ui/locales"
FORMAT = re.compile(r"(?<!\{)\{([^{}]*)\}(?!\})")
TAG = re.compile(r'''<(?:"[^"]*"|'[^']*'|[^'">])*>''')
ROOTS = set()
for path in list((ROOT / "crates/terra-ui/src").rglob("*.rs")) + list((ROOT / "src-ui").rglob("*.rs")):
    source = path.read_text()
    for name in re.findall(r"#\[html_shared\][\s\S]*?pub struct (\w+)", source):
        if name != "LocalizedUi":
            ROOTS.add(name[0].lower() + name[1:])


def identifier(source):
    label = re.sub(r"[^a-z0-9]+", "_", source.lower()).strip("_")[:36]
    digest = hashlib.sha256(source.encode()).hexdigest()[:12]
    return f"text_{label}_{digest}"


def placeholders(source):
    literals, names, tokens = [], [], []
    offset = 0
    named = {}
    for match in FORMAT.finditer(source):
        literals.append(source[offset:match.start()].replace("{{", "{").replace("}}", "}"))
        token = match[1]
        variable = token.partition(":")[0]
        if variable and variable in named:
            name = named[variable]
        else:
            name = f"arg{len(names)}"
            if variable:
                named[variable] = name
        names.append(name)
        tokens.append(token)
        offset = match.end()
    literals.append(source[offset:].replace("{{", "{").replace("}}", "}"))
    return literals, names, tokens


def literal(value):
    # Fluent string expressions preserve leading/trailing spaces and newlines.
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    escaped = escaped.replace("\n", "\\u000A").replace("\r", "\\u000D").replace("\t", "\\u0009")
    return '{ "' + escaped + '" }' if value else ""


def fluent(value, names):
    pieces, _, _ = placeholders(value)
    found = list(FORMAT.finditer(value))
    if len(found) != len(names):
        raise ValueError(f"Changed formatting arguments: {value!r}")
    return "".join(literal(piece) + (f"{{ ${names[index]} }}" if index < len(names) else "") for index, piece in enumerate(pieces)) or '{ "" }'


def build(allow_pending=False):
    entries = {}
    pending = []
    for path in sorted((LOCALES / "source").glob("*.json")):
        for entry in json.loads(path.read_text()):
            source = entry["source"]
            if not all(isinstance(entry.get(language), str) for language in ["de", "en"]):
                pending.append((path.name, source))
                continue
            if source in entries and entries[source] != entry:
                raise ValueError(f"Conflicting duplicate source: {source!r}")
            entries[source] = entry
    if pending and not allow_pending:
        raise ValueError(f"{len(pending)} untranslated entries; first: {pending[:3]}")
    exact, messages, patterns, catalogs = {}, {}, [], {"de": [], "en": []}
    for source, entry in sorted(entries.items()):
        key = identifier(source)
        literals, names, tokens = placeholders(source)
        for language in ["de", "en"]:
            translated = entry[language]
            if [m[1] for m in FORMAT.finditer(translated)] != tokens:
                raise ValueError(f"Changed {language} placeholders in {source!r}")
            catalogs[language].append(f"{key} = {fluent(translated, names)}")
        if names:
            # Skip raw debug patterns without meaningful product wording.
            if len("".join(literals).strip()) >= 3 and any(c.isalpha() for c in "".join(literals)):
                numeric = [bool(re.search(r":.*\.\d+", token)) for token in tokens]
                patterns.append(dict(id=key, literals=literals, arguments=names, numeric=numeric))
                if source.startswith("\n") or source.endswith("\n"):
                    trimmed = literals.copy()
                    trimmed[0] = trimmed[0].lstrip("\n")
                    trimmed[-1] = trimmed[-1].rstrip("\n")
                    patterns.append(dict(id=key, literals=trimmed, arguments=names, numeric=numeric, trimmed=True))
        else:
            exact[source] = key
            messages[key] = key
    # Exact source contracts win over convenience aliases in the other language.
    for source, entry in sorted(entries.items()):
        if not FORMAT.search(source):
            for language in ["de", "en"]:
                exact.setdefault(entry[language], identifier(source))
    patterns.sort(key=lambda p: (-sum(map(len, p["literals"])), p["id"]))
    for language, tag in [("de", "de-DE"), ("en", "en-US")]:
        extras = ("workers_count = { $count ->\n    [one] Eine Arbeitskraft\n   *[other] { $count } Arbeitskräfte\n}\n" if language == "de" else
                  "workers_count = { $count ->\n    [one] One worker\n   *[other] { $count } workers\n}\n")
        extras += "compass-east = " + ("O" if language == "de" else "E") + "\n"
        (LOCALES / f"{tag}.ftl").write_text("# Generated by tools/localization/build_catalogs.py; edit source/*.json.\n" + "\n".join(catalogs[language]) + "\n" + extras)
    search = {}
    for source, entry in sorted(entries.items()):
        if not FORMAT.search(source):
            terms = sorted({source, entry["de"], entry["en"]})
            for alias in terms:
                search[alias] = "\n".join(sorted(set(search.get(alias, "").splitlines()) | set(terms)))
    index = dict(exact=exact, messages=messages, patterns=patterns, search=search)
    (LOCALES / "index.json").write_text(json.dumps(index, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n")
    print(f"{len(entries)} bilingual messages, {len(patterns)} parameterized patterns, {len(pending)} pending")
    return entries


def migrate(entries):
    def root_paths(expression):
        for root in sorted(ROOTS, key=len, reverse=True):
            expression = re.sub(rf"(?<![\w.]){root}(?=\.)", f"localizedUi.{root}", expression)
        return expression

    def expression(match):
        value = root_paths(match[1])
        def branch(m):
            source = html.unescape(m[2])
            return m[1] + f"localizedUi.messages.{identifier(source)}" if source in entries and not FORMAT.search(source) else m[0]
        value = re.sub(r"([?:]\s*)'([^']*)'", branch, value)
        return "{{" + value + "}}"

    def text(value):
        value = re.sub(r"\{\{(.*?)\}\}", expression, value, flags=re.S)
        # Angular control-flow delimiters are syntax; the text between them is product text.
        tokens = re.split(r"(\{\{.*?\}\}|@(?:if|for|else\s+if)\s*\([^)]*\)\s*\{|@else\s*\{|\})", value, flags=re.S)
        parts = []
        for part in tokens:
            if part.startswith(("{{", "@", "}")):
                if part.startswith("@for"):
                    part = re.sub(r"(\bof\s+)(\w+)(?=\.)", lambda m: m[1] + ("localizedUi." if m[2] in ROOTS else "") + m[2], part)
                parts.append(part)
                continue
            source = html.unescape(part.strip())
            if source in entries and not FORMAT.search(source):
                leading = part[:len(part) - len(part.lstrip())]
                trailing = part[len(part.rstrip()):]
                parts.append(leading + "{{ i18n." + identifier(source) + " }}" + trailing)
            else:
                parts.append(part)
        return "".join(parts)

    def tag(value):
        if "data-l10n=\"preserve\"" in value:
            return value
        def attribute(m):
            source = html.unescape(m[2])
            if source in entries and not FORMAT.search(source):
                return f'[{m[1]}]="i18n.{identifier(source)}"'
            return m[0]
        value = re.sub(r'(?<![\w\]-])(title|placeholder|aria-label)="([^"]*)"', attribute, value)
        # Only presentation properties receive translated mirrors. Input values remain canonical.
        value = re.sub(r'\[(text|title|placeholder|label|slot|row|item|card|button|stat|category)\]="([^"]+)"', lambda m: f'[{m[1]}]="{root_paths(m[2])}"', value)
        return value

    for path in (ROOT / "src-ui").rglob("*.html"):
        if path.name == "index.html":
            continue
        source = path.read_text()
        parts, cursor, preserved = [], 0, False
        for match in TAG.finditer(source):
            between = source[cursor:match.start()]
            parts.append(between if preserved else text(between))
            original_tag = match[0]
            parts.append(tag(original_tag))
            if original_tag.startswith("</"):
                preserved = False
            elif 'data-l10n="preserve"' in original_tag:
                preserved = True
            cursor = match.end()
        parts.append(text(source[cursor:]))
        target = "".join(parts)
        if target != source:
            path.write_text(target)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-pending", action="store_true")
    parser.add_argument("--migrate-templates", action="store_true")
    args = parser.parse_args()
    entries = build(args.allow_pending)
    if args.migrate_templates:
        migrate(entries)
