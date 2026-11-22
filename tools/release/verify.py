"""Run acceptance against the extracted binaries, from an unrelated working directory."""
import argparse
import contextlib
import os
from pathlib import Path
import re
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
    # Separate reader and writer handles keep their offsets independent on Windows and Unix.
    with (tempfile.TemporaryDirectory() as logs,
          (Path(logs) / "server.log").open("wb") as output,
          (Path(logs) / "server.log").open("rb") as log):
        proc = subprocess.Popen([binary, "--bind", "127.0.0.1:0", "--save-dir", saved],
                                stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT,
                                env=dict(env, RUST_LOG="info", NO_COLOR="1"), cwd=cwd)
        try:
            deadline = time.monotonic() + 120
            tail = b""
            while time.monotonic() < deadline and proc.poll() is None:
                batch = tail + log.read(65536)
                # Some Windows tracing subscribers color field names even in redirected output.
                plain = re.sub(rb"\x1b\[[0-9;:]*m", b"", batch)
                ready = re.search(rb"Authoritative Terra server listening\s+address=Some\((127\.0\.0\.1:[1-9][0-9]{0,4})\)", plain)
                if ready:
                    break
                tail = batch[-512:]
                time.sleep(0.1)
            else:
                raise RuntimeError("Packaged server did not report a bound TCP/UDP loopback endpoint")
            yield ready[1].decode("ascii")
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
