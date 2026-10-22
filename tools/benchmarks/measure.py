#!/usr/bin/env python3
"""Run an existing benchmark and report Linux peak RSS without external packages."""
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time


def parse_drm_memory(contents):
    fields = dict(line.split(":", 1) for line in contents.splitlines() if ":" in line)
    client = fields.get("drm-client-id")
    if client is None:
        return None
    memory = {}
    for kind in ("vram", "gtt"):
        words = fields.get(f"drm-memory-{kind}", "").split()
        if len(words) == 2 and words[0].isdigit() and words[1] == "KiB":
            memory[kind] = int(words[0])
    return (fields.get("drm-pdev", "").strip(), client.strip()), memory


def drm_memory(pid):
    clients = {}
    try:
        entries = list(Path(f"/proc/{pid}/fdinfo").iterdir())
    except OSError:
        return {}
    for entry in entries:
        try:
            result = parse_drm_memory(entry.read_text())
        except OSError:
            continue
        if result:
            client, memory = result
            clients[client] = memory
    return {kind: sum(memory[kind] for memory in clients.values() if kind in memory)
            for kind in ("vram", "gtt") if any(kind in memory for memory in clients.values())}


def parse_process_usage(contents, page_size):
    # comm may contain spaces and parentheses; fields after its final ')' start at state (3).
    _, delimiter, trailing = contents.rpartition(")")
    fields = trailing.split()
    if not delimiter or len(fields) < 22:
        return None
    try:
        user, system, pages, minor, major = (int(fields[index]) for index in (11, 12, 21, 7, 9))
    except ValueError:
        return None
    if min(user, system, pages, minor, major) < 0:
        return None
    return {"rss_kib": pages * page_size // 1024,
            "user_cpu_ticks": user, "system_cpu_ticks": system,
            "minor_faults": minor, "major_faults": major}


def parse_swap_kib(contents):
    for line in contents.splitlines():
        name, separator, value = line.partition(":")
        if separator and name == "VmSwap":
            words = value.split()
            if len(words) == 2 and words[0].isdigit() and words[1] == "kB":
                return int(words[0])
            return None
    return None


def process_usage(pid):
    try:
        usage = parse_process_usage(Path(f"/proc/{pid}/stat").read_text(), os.sysconf("SC_PAGE_SIZE"))
    except (OSError, ValueError):
        return None
    if usage is not None:
        try:
            usage["swap_kib"] = parse_swap_kib(Path(f"/proc/{pid}/status").read_text())
        except OSError:
            usage["swap_kib"] = None
    return usage


def counter_delta(current, previous, key):
    if current is None or previous is None:
        return None
    value, old = current.get(key), previous.get(key)
    if not isinstance(value, int) or not isinstance(old, int) or value < old:
        return None
    return value - old


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: measure.py PROGRAM [ARG ...]")
    started = time.monotonic()
    result = subprocess.Popen(sys.argv[1:])
    peak_gpu = {}
    timeline = os.environ.get("TERRA_MEASURE_TIMELINE") == "1"
    gpu_memory = timeline or os.environ.get("TERRA_MEASURE_GPU_MEMORY") == "1"
    previous_time = started
    previous_usage = process_usage(result.pid) if timeline else None
    next_sample = started + 30.0
    while result.poll() is None:
        gpu = drm_memory(result.pid) if gpu_memory else {}
        if gpu_memory:
            for kind, value in gpu.items():
                peak_gpu[kind] = max(peak_gpu.get(kind, 0), value)
        now = time.monotonic()
        if timeline and now >= next_sample:
            usage = process_usage(result.pid)
            ticks = None
            interval = now - previous_time
            if usage is not None and previous_usage is not None:
                ticks = sum(usage[key] - previous_usage[key]
                            for key in ("user_cpu_ticks", "system_cpu_ticks"))
                if ticks < 0:
                    ticks = None
            clock_ticks = os.sysconf("SC_CLK_TCK")
            print(json.dumps({
                "kind": "resource_timeline",
                "elapsed_seconds": round(now - started, 3),
                "wall_unix_seconds": time.time(),
                "rss_kib": usage["rss_kib"] if usage else None,
                "swap_kib": usage.get("swap_kib") if usage else None,
                "drm_memory_vram_kib": gpu.get("vram"),
                "drm_memory_gtt_kib": gpu.get("gtt"),
                "user_cpu_ticks": usage["user_cpu_ticks"] if usage else None,
                "system_cpu_ticks": usage["system_cpu_ticks"] if usage else None,
                "minor_faults": usage["minor_faults"] if usage else None,
                "major_faults": usage["major_faults"] if usage else None,
                "minor_faults_delta": counter_delta(usage, previous_usage, "minor_faults"),
                "major_faults_delta": counter_delta(usage, previous_usage, "major_faults"),
                "cpu_ticks_delta": ticks,
                "cpu_interval_seconds": round(interval, 3),
                "cpu_percent": round(ticks / clock_ticks / interval * 100.0, 3)
                    if ticks is not None and interval > 0.0 else None,
            }), flush=True)
            previous_time = now
            previous_usage = usage
            next_sample = now + 30.0
        time.sleep(0.25)
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    print(json.dumps({
        "command": sys.argv[1:],
        "wall_seconds": round(time.monotonic() - started, 3),
        "peak_rss_kib": usage.ru_maxrss,
        "user_cpu_seconds": round(usage.ru_utime, 3),
        "system_cpu_seconds": round(usage.ru_stime, 3),
        "peak_drm_memory_vram_kib": peak_gpu.get("vram"),
        "peak_drm_memory_gtt_kib": peak_gpu.get("gtt"),
        "exit_code": result.returncode,
    }), flush=True)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
