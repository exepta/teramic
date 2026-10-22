#!/usr/bin/env python3
"""Optional KDE/Wayland OS-pointer acceptance tool; requires existing /dev/uinput access.

Reads JSON commands from stdin: {"move":[physical_x,physical_y],"size":[960,640]},
{"button":1}, {"button":0}, {"wheel":-3}, {"key":[linux_key_code,1]}.
No Bevy/TiltUI events are injected. The temporary input device is removed on exit.
"""
import fcntl
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def window():
    name = "teramic-pointer-acceptance"
    command("qdbus6", "org.kde.KWin", "/Scripting", "unloadScript", name)
    script = command("qdbus6", "org.kde.KWin", "/Scripting", "loadScript",
                     str(Path(__file__).with_name("wayland-window.js").resolve()), name)
    started = int(time.time() * 1000)
    command("qdbus6", "org.kde.KWin", f"/Scripting/Script{script}", "run")
    for _ in range(12):
        lines = command("journalctl", "--user", "-u", "plasma-kwin_wayland.service",
                        "-n", "40", "--no-pager", "-o", "cat").splitlines()
        for line in reversed(lines):
            if line.startswith("TERAMIC_POINTER "):
                result = json.loads(line.removeprefix("TERAMIC_POINTER "))
                if result.get("stamp", 0) < started:
                    continue
                if "error" in result:
                    raise RuntimeError(result["error"])
                return result
        time.sleep(0.03)
    raise RuntimeError("KWin pointer probe produced no output")


class Mouse:
    def __enter__(self):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        # Linux uinput ABI: enable keyboard/buttons and relative X/Y/wheel.
        for event in (1, 2):
            fcntl.ioctl(self.fd, 0x40045564, event)
        for key in list(range(1, 256)) + [272, 273, 274]:
            fcntl.ioctl(self.fd, 0x40045565, key)
        for axis in (0, 1, 8):
            fcntl.ioctl(self.fd, 0x40045566, axis)
        descriptor = struct.pack("80sHHHHi", b"TERAMIC acceptance pointer", 3, 0x1234, 0x5678, 1, 0)
        os.write(self.fd, descriptor + bytes(64 * 4 * 4))
        fcntl.ioctl(self.fd, 0x5501)
        time.sleep(1)
        return self

    def event(self, kind, code, value):
        os.write(self.fd, struct.pack("llHHi", 0, 0, kind, code, value))
        os.write(self.fd, struct.pack("llHHi", 0, 0, 0, 0, 0))

    def move(self, point, size):
        # Wayland may not enter a newly activated window beneath a stationary cursor.
        self.event(2, 0, 5)
        time.sleep(0.04)
        probe = window()
        rect = probe["client"]
        x = rect["x"] + point[0] * rect["width"] / size[0]
        y = rect["y"] + point[1] * rect["height"] / size[1]
        # Close the loop using compositor coordinates, independent of acceleration/scaling.
        for _ in range(24):
            cursor = probe["cursor"]
            dx, dy = x - cursor["x"], y - cursor["y"]
            if abs(dx) <= 3 and abs(dy) <= 3:
                return
            for axis, delta in enumerate((dx, dy)):
                if abs(delta) >= 1:
                    self.event(2, axis, round(max(-80, min(80, delta * 0.65))))
            time.sleep(0.04)
            probe = window()
        raise RuntimeError(f"Pointer did not reach target: {probe['cursor']} versus {(x, y)}")

    def __exit__(self, *_):
        self.event(1, 272, 0)
        self.event(1, 273, 0)
        self.event(1, 274, 0)
        fcntl.ioctl(self.fd, 0x5502)
        os.close(self.fd)
        command("qdbus6", "org.kde.KWin", "/Scripting", "unloadScript", "teramic-pointer-acceptance")


with Mouse() as mouse:
    print("READY", flush=True)
    for line in sys.stdin:
        item = json.loads(line)
        if "move" in item:
            mouse.move(item["move"], item.get("size", [960, 640]))
        if "button" in item:
            mouse.event(1, 272, item["button"])
        if "wheel" in item:
            mouse.event(2, 8, item["wheel"])
        if "key" in item:
            mouse.event(1, *item["key"])
        time.sleep(item.get("wait", 0.15))
        print("OK " + json.dumps(item), flush=True)
