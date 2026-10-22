#!/usr/bin/env python3
"""Summarize measured soak intervals; never interpolate missing checkpoints."""
import argparse
import csv
from datetime import datetime
import json
import math
from pathlib import Path
import re
import sys


ANSI = re.compile(r"\x1b\[[0-9;]*m")
TIMESTAMP = re.compile(r"^\s*(\d{4}-\d\d-\d\dT[\d:.]+(?:Z|[+-]\d\d:\d\d))")
FIELDS = re.compile(
    r'(?<!\S)([a-zA-Z_]\w*)=("(?:\\.|[^"\\])*"|Some\([^)]*\)|'
    r'[a-zA-Z_]\w*\s*\{[^}]*\}|\[[^]]*\]|[^\s]+)'
)
EVENTS = {
    "interval": "interval",
    "world resources": "world",
    "image resources; descriptor estimates are not measured VRAM": "image_resources",
    "UI font cache": "ui_font_cache",
    "generation": "generation",
    "terrain": "terrain",
    "props": "props",
    "water rendering": "water",
    "upload budgets": "uploads",
    "journal": "journal",
    "clock": "clock",
    "world clock": "clock",
    "weather": "weather",
    "environment": "environment",
    "replicated population": "replicated_population",
    "replicated furnaces": "replicated_furnaces",
    "latest fluid step": "latest_fluid_step",
    "latest draw submission": "latest_draw_submission",
    "latest render pass diagnostic": "render_pass_latest_ms",
    "CPU scope interval": "cpu_scopes",
}


def scalar(value):
    if value == "None":
        return None
    if value.startswith("Some(") and value.endswith(")"):
        return scalar(value[5:-1])
    try:
        result = json.loads(value)
    except (ValueError, TypeError):
        return value
    return None if isinstance(result, float) and not math.isfinite(result) else result


def parse_fields(payload):
    return {key: scalar(value) for key, value in FIELDS.findall(payload)}


def timestamp(line):
    match = TIMESTAMP.match(line)
    if match:
        try:
            return datetime.fromisoformat(match[1].replace("Z", "+00:00")).timestamp()
        except ValueError:
            pass
    return None


def nearest_sample(samples, target, tolerance, not_later=False):
    eligible = [sample for sample in samples
                if abs(sample["elapsed_seconds"] - target) <= tolerance
                and (not not_later or sample["elapsed_seconds"] <= target)]
    return min(eligible, key=lambda sample: (
        abs(sample["elapsed_seconds"] - target), sample["elapsed_seconds"] > target,
        sample["elapsed_seconds"]), default=None)


def summarize(lines, source=None, tolerance=5.0, ready_offset_seconds=None):
    windows = {}
    resources = []
    readiness = []
    final_measurement = None
    for raw in lines:
        line = ANSI.sub("", raw).strip()
        if line.startswith("{"):
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if record.get("kind") == "resource_timeline":
                resources.append(record)
            elif "command" in record and "exit_code" in record:
                final_measurement = record
            continue
        if "Performance soak sampling started" in line:
            if (at := timestamp(line)) is not None:
                readiness.append(at)
            continue
        for event, group in EVENTS.items():
            marker = f"Performance soak {event} "
            if marker not in line:
                continue
            fields = parse_fields(line.split(marker, 1)[1])
            elapsed = fields.pop("elapsed_seconds", None)
            if not isinstance(elapsed, (int, float)):
                break
            window = windows.setdefault(elapsed, {"elapsed_seconds": elapsed})
            if group == "interval":
                window.update(fields)
                window["wall_unix_seconds"] = timestamp(line)
            elif group == "cpu_scopes":
                name = fields.pop("scope", None)
                if name is not None:
                    window.setdefault(group, {})[name] = fields
            elif group == "render_pass_latest_ms":
                name = fields.get("diagnostic")
                if name is not None:
                    window.setdefault(group, {})[name] = fields.get("value_ms")
            else:
                window[group] = fields
            break
    intervals = sorted((window for window in windows.values() if "fps" in window),
                       key=lambda window: window["elapsed_seconds"])
    ready = readiness[0] if len(readiness) == 1 else None
    aligned = []
    for resource in resources:
        if ready_offset_seconds is not None:
            elapsed = resource.get("elapsed_seconds")
            offset = elapsed - ready_offset_seconds if isinstance(elapsed, (int, float)) else None
            alignment = "explicit_ready_offset"
        elif ready is not None and isinstance(resource.get("wall_unix_seconds"), (int, float)):
            offset = resource["wall_unix_seconds"] - ready
            alignment = "absolute_wall_clock"
        else:
            offset = None
            alignment = "unavailable_no_common_clock"
        resource["soak_elapsed_seconds"] = offset
        resource["alignment"] = alignment
        if offset is not None:
            aligned.append({"elapsed_seconds": offset, "resource": resource})
    legacy_entities = False
    for window in intervals:
        if "entities" in window and "allocated_entity_indices" not in window:
            window["legacy_entity_indices"] = window.pop("entities")
            window["entity_count_kind"] = "legacy_allocated_indices_not_live"
            legacy_entities = True
        elif "allocated_entity_indices" in window:
            window["entity_count_kind"] = "explicit_fields"
        if isinstance(window.get("rss_bytes"), (int, float)):
            window["rss_mib"] = window["rss_bytes"] / (1024 * 1024)
        sample = nearest_sample(aligned, window["elapsed_seconds"], tolerance, not_later=True)
        if sample:
            window["resource_timeline"] = sample["resource"].copy()
            window["resource_timeline"]["sample_lag_seconds"] = (
                window["elapsed_seconds"] - sample["elapsed_seconds"])
    checkpoints = []
    for minute in (1, 5, 10, 15, 30):
        target = minute * 60
        sample = nearest_sample(intervals, target, tolerance)
        checkpoint = {"minute": minute, "target_elapsed_seconds": target,
                      "status": "measured" if sample else "not_measured"}
        if sample:
            checkpoint.update({"sample_elapsed_seconds": sample["elapsed_seconds"],
                               "sample_offset_seconds": sample["elapsed_seconds"] - target})
            for key in ("fps", "median_ms", "p95_ms", "p99_ms", "max_ms", "rss_mib"):
                checkpoint[key] = sample.get(key)
            if "resource_timeline" in sample:
                checkpoint["resource_timeline"] = sample["resource_timeline"]
        checkpoints.append(checkpoint)
    notes = [
        "FPS and frame-time percentiles summarize the stated interval, not the full run.",
        "Render-pass diagnostics are latest samples, not interval means or total GPU frame time.",
        "CPU scope totals/calls are interval values; cumulative_max_ms is a run maximum. Nested scopes must not be summed.",
        "Checkpoints use the nearest measured interval within the tolerance, without interpolation; actual sample times are retained.",
        "Resource joins use only preceding samples within tolerance and a common or explicitly supplied clock; unaligned samples remain separate.",
        "RSS excludes swapped-out process pages. A falling RSS does not prove lower memory use; consult measured swap_kib and major-fault deltas where available.",
    ]
    if legacy_entities:
        notes.append("Legacy entities= used Bevy allocated entity indices, not live entities; reported as legacy_entity_indices.")
    return {"source": source, "tolerance_seconds": tolerance, "soak_ready_unix_seconds": ready,
            "ready_offset_seconds": ready_offset_seconds, "notes": notes,
            "windows": intervals, "checkpoints": checkpoints, "resources": resources,
            "final_measurement": final_measurement}


def write_csv(summary, stream):
    fields = ("elapsed_seconds", "interval_seconds", "frames", "fps", "median_ms", "p95_ms",
              "p99_ms", "max_ms", "rss_mib", "legacy_entity_indices", "entities",
              "allocated_entity_indices", "live_entities", "entity_count_kind", "mesh_entities",
              "visible_mesh_entities", "mesh_assets", "standard_material_assets", "image_assets",
              "upload_vertices", "resource_sample_lag_seconds", "drm_memory_vram_kib",
              "drm_memory_gtt_kib", "process_cpu_percent", "swap_kib", "minor_faults",
              "major_faults", "minor_faults_delta", "major_faults_delta")
    writer = csv.DictWriter(stream, fieldnames=fields, extrasaction="ignore")
    writer.writeheader()
    for window in summary["windows"]:
        row = window.copy()
        resource = row.get("resource_timeline", {})
        row.update(resource_sample_lag_seconds=resource.get("sample_lag_seconds"),
                   drm_memory_vram_kib=resource.get("drm_memory_vram_kib"),
                   drm_memory_gtt_kib=resource.get("drm_memory_gtt_kib"),
                   process_cpu_percent=resource.get("cpu_percent"))
        row.update((key, resource.get(key)) for key in (
            "swap_kib", "minor_faults", "major_faults", "minor_faults_delta", "major_faults_delta"))
        writer.writerow(row)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    formats = parser.add_mutually_exclusive_group()
    formats.add_argument("--json", action="store_const", dest="format", const="json")
    formats.add_argument("--csv", action="store_const", dest="format", const="csv")
    parser.set_defaults(format="json")
    parser.add_argument("--tolerance-seconds", type=float, default=5.0)
    parser.add_argument("--ready-offset-seconds", type=float)
    args = parser.parse_args()
    for value in (args.tolerance_seconds, args.ready_offset_seconds):
        if value is not None and (not math.isfinite(value) or value < 0):
            parser.error("time offsets and tolerance must be finite and nonnegative")
    try:
        with args.log.open(encoding="utf-8", errors="replace") as log:
            summary = summarize(log, str(args.log), args.tolerance_seconds, args.ready_offset_seconds)
    except OSError as error:
        parser.error(str(error))
    if args.format == "csv":
        write_csv(summary, sys.stdout)
    else:
        print(json.dumps(summary, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
