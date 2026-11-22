"""Create a portable Teramic distribution from native, locked release binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PROGRAMS = ("terra-client", "terra-server", "terra-building-editor")
CONFIGS = ("client", "gameplay", "multiplayer", "server", "world")
TARGETS = {"x86_64-unknown-linux-gnu": "linux-x86_64",
           "aarch64-apple-darwin": "macos-arm64",
           "x86_64-pc-windows-msvc": "windows-x86_64",
           "x86_64-pc-windows-gnu": "windows-x86_64"}


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True, encoding="utf-8")


def checksum(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def licenses(root, metadata):
    """Include the shipped crates' license notices alongside their Cargo declarations."""
    notices = root / "licenses"
    notices.mkdir()
    index = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        directory = Path(package["manifest_path"]).parent
        if package["id"] in metadata["workspace_members"]:
            continue
        names = []
        locations = [directory, directory / "licenses"]
        if package["name"].startswith("tilt-ui") and not package["source"]:
            locations.append(directory.parent.parent)
        candidates = {p for loc in locations if loc.is_dir() for p in loc.iterdir()
                      if p.is_file() and p.name.lower().startswith(("license", "licence", "copying", "notice", "copyright"))}
        if package["name"] == "tilt-ui-icons":
            candidates.update(p for p in directory.iterdir() if p.is_file() and
                              any(word in p.name for word in ("LICENSE", "SOURCES", "DISCLAIMER")))
        if package["license_file"]:
            candidates.add(directory / package["license_file"])
        for path in sorted(candidates):
            name = checksum(path) + ".txt"
            if not (notices / name).exists():
                shutil.copyfile(path, notices / name)
            names.append({"file": path.name, "text": name})
        index.append({"crate": package["name"], "version": package["version"],
                      "license": package["license"], "repository": package["repository"], "notices": names})
    (notices / "dependencies.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")


def package(binary_dir, destination, tag, target=None):
    host = re.search(r"^host: (.+)$", output("rustc", "-vV"), re.M)[1].strip()
    target = target or host
    platform = TARGETS[target]
    windows = platform.startswith("windows-")
    binary_dir = binary_dir or (ROOT / "target" / (target if host != target else "") / "release")
    metadata = json.loads(output("cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target))
    versions = {p["version"] for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    if len(versions) != 1:
        raise SystemExit("Workspace packages have different versions")
    version = versions.pop()
    if tag and tag != "v" + version:
        raise SystemExit(f"Tag {tag!r} does not match workspace version {version}")
    name = f"Teramic-{version}-{platform}"
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary) / name
        root.mkdir()
        for program in PROGRAMS:
            filename = program + (".exe" if windows else "")
            shutil.copy2(binary_dir / filename, root / filename)
        for directory in ("assets", "src-ui", "apps/building-editor/ui"):
            shutil.copytree(ROOT / directory, root / directory)
        (root / "configs").mkdir()
        for config in CONFIGS:
            filename = f"{config}.toml"
            shutil.copyfile(ROOT / "configs" / filename, root / "configs" / filename)
        (root / "TERAMIC-VERSION").write_text(version + "\n", encoding="utf-8")
        shutil.copyfile(ROOT / "tools/release/PLAY.md", root / "README.md")
        shutil.copyfile(ROOT / f"docs/releases/v{version}.md", root / "RELEASE-NOTES.md")
        for license_name in ("LICENSE-MIT", "LICENSE-APACHE"):
            shutil.copyfile(ROOT / license_name, root / license_name)
        licenses(root, metadata)
        if target == "x86_64-pc-windows-gnu" and host.endswith("linux-gnu"):
            # These Debian packages supply the runtime and startup objects linked into the EXEs.
            for name in ("mingw-w64-common", "gcc-mingw-w64-x86-64-posix-runtime"):
                shutil.copyfile(Path("/usr/share/doc") / name / "copyright",
                                root / "licenses" / f"{name}-copyright.txt")
        if windows:
            archive = destination / f"{name}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as out:
                for file in sorted(root.rglob("*")):
                    if file.is_file():
                        out.write(file, file.relative_to(root.parent))
        else:
            archive = destination / f"{name}.tar.gz"
            with tarfile.open(archive, "w:gz", compresslevel=6) as out:
                out.add(root, arcname=name)
    archive.with_name(archive.name + ".sha256").write_text(
        f"{checksum(archive)}  {archive.name}\n", encoding="ascii")
    print(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tag")
    parser.add_argument("--target", choices=TARGETS)
    args = parser.parse_args()
    package(args.binary_dir, args.out, args.tag, args.target)
