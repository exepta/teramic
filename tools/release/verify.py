"""Run acceptance against the extracted binaries, from an unrelated working directory."""
import argparse
import contextlib
import os
from pathlib import Path
import socket
import subprocess
import tarfile
import tempfile
import time
import zipfile

from package import checksum


def run(args, cwd, env, marker, timeout=180):
    proc = subprocess.run(args, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, encoding="utf-8", errors="replace", timeout=timeout)
    if proc.returncode or marker not in proc.stdout:
        raise RuntimeError(f"{args[0]} exited {proc.returncode}, expected {marker!r}\n{proc.stdout[-14000:]}")
    print(marker, flush=True)


@contextlib.contextmanager
def server(binary, cwd, env, saved):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    address = f"127.0.0.1:{port}"
    with tempfile.TemporaryFile() as log:
        proc = subprocess.Popen([binary, "--bind", address, "--save-dir", saved],
                                stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=cwd)
        try:
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline and proc.poll() is None:
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=0.5):
                        break
                except OSError:
                    time.sleep(0.25)
            else:
                raise RuntimeError("Packaged server did not listen on loopback")
            yield address
        finally:
            if proc.poll() is None:
                try:
                    proc.communicate(input=b"stop\n", timeout=30)
                except (subprocess.TimeoutExpired, BrokenPipeError):
                    proc.kill()
                    proc.wait()
            if proc.returncode:
                log.seek(0)
                raise RuntimeError(f"Packaged server exited {proc.returncode}: {log.read()[-12000:].decode(errors='replace')}")


def verify(archive, desktop=False):
    line = archive.with_name(archive.name + ".sha256").read_text().strip()
    if line != f"{checksum(archive)}  {archive.name}":
        raise ValueError("Release archive checksum mismatch")
    with tempfile.TemporaryDirectory(prefix="teramic-release-") as temp:
        scratch = Path(temp)
        unpacked = scratch / "unpacked"
        work = scratch / "empty"
        work.mkdir()
        if archive.name.endswith(".zip"):
            with zipfile.ZipFile(archive) as z:
                z.extractall(unpacked)
        else:
            with tarfile.open(archive) as t:
                t.extractall(unpacked, filter="data")
        root, = unpacked.iterdir()
        for item in ("TERAMIC-VERSION", "LICENSE-MIT", "LICENSE-APACHE", "licenses/dependencies.json",
                     "src-ui/index.html", "apps/building-editor/ui/index.html", "assets/audio/CREDITS.md"):
            if not (root / item).is_file():
                raise FileNotFoundError(item)
        env = {k: v for k, v in os.environ.items() if not k.startswith("TERRA_")}
        game = root / ("terra-client.exe" if os.name == "nt" else "terra-client")
        host = root / ("terra-server.exe" if os.name == "nt" else "terra-server")
        editor = root / ("terra-building-editor.exe" if os.name == "nt" else "terra-building-editor")
        saved = scratch / "world"
        with server(host, work, env, saved):
            pass
        manifest = saved / "world.toml"
        if not manifest.is_file():
            raise FileNotFoundError("Packaged server did not persist its world manifest")
        building = root / "assets/buildings/templates/village/residential.json"
        with server(host, work, env, saved) as address:
            run([game, "--verify-package", "--connect", address], work, env, "Package client verified")
            if desktop:
                # Limit the smoke camera, preserving the distributed world and binary.
                settings = root / "configs/client.toml"
                import re
                settings.write_text(re.sub(r"(?m)^view_distance\s*=.*$", "view_distance = 2",
                                            settings.read_text(encoding="utf-8")), encoding="utf-8")
                run([game, "--connect", address, "--smoke-test-network"], work,
                    dict(env, TERRA_SCREENSHOT=str(scratch / "client.png")), "Native network smoke completed")
        run([editor, "--verify-package", "--load", building], work, env, "Package editor verified")
        if desktop:
            run([editor, "--editor-acceptance"], work, dict(env, TERRA_EDITOR_SMOKE_DIR=str(scratch / "editor")),
                "Standalone building editor acceptance passed", 220)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--desktop", action="store_true")
    args = parser.parse_args()
    verify(args.archive.resolve(), args.desktop)
