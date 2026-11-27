#!/usr/bin/env python3
"""Run finite, isolated native frame-time scenarios at the supplied graphics quality."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import shutil
import subprocess
import sys
import tomllib

from summarize_soak import ANSI, parse_fields

SCENARIOS = ("clear", "rain", "village", "flight", "inventory", "placement")
SECONDS = {"clear": 24, "rain": 24, "village": 24, "flight": 35,
           "inventory": 90, "placement": 35}
ROOT = Path(__file__).resolve().parents[2]
MEASURE = Path(__file__).with_name("measure.py")
WALL_MARKER = "Native smoke benchmark (CPU wall frames;"
PHASE_MARKER = "Inventory benchmark phase: CPU wall frames,"
COMPLETE_INVENTORY = "Inventory benchmark completed all three real closed/open/closed phases"
COMPLETE_RAPID = "Rapid authoritative edits complete;"
REMOVED_PREFIXES = ("TERRA_BENCH", "TERRA_SOAK", "TERRA_SCREENSHOT", "TERRA_MEASURE",
                    "TERRA_INVENTORY", "TERRA_MENU_SMOKE")
REMOVED_KEYS = {"TERRA_RENDER_DIAGNOSTICS", "TERRA_SCOPE_TIMELINE",
                "TERRA_CONFIG_DIR", "TERRA_MENU_CONFIG_DIR", "TERRA_PROFILE_FILE",
                "TERRA_WORLD_DIR", "TERRA_CAPTURE_DIR", "TERRA_CPU_SCOPES",
                "TERRA_NATIVE_OS_POINTER"}
GPU_ENVIRONMENT_KEYS = (
    "WGPU_BACKEND", "WGPU_ADAPTER_NAME", "WGPU_POWER_PREF", "WGPU_DX12_COMPILER",
    "WGPU_GLES_MINOR_VERSION", "VK_ICD_FILENAMES", "VK_DRIVER_FILES",
    "MESA_VK_DEVICE_SELECT", "DRI_PRIME", "__NV_PRIME_RENDER_OFFLOAD",
    "__VK_LAYER_NV_optimus", "TERRA_WGPU_MEMORY_BLOCK_MIB", "TERRA_MESH_SLAB_MIB",
    "TERRA_PACKED_TERRAIN", "TERRA_PACKED_ROOTED", "TERRA_FAR_BATCHES",
    "TERRA_MESH_SLAB_LEGACY_GROWTH",
)


def gpu_environment_snapshot(environment):
    return {key: environment.get(key) for key in GPU_ENVIRONMENT_KEYS}


def native_fields(line):
    fields = parse_fields(line)
    # The shared soak parser intentionally handles simple Some(value), while
    # Bevy emits physical dimensions as a nested Some((width, height)).
    for match in re.finditer(
            r"(?<!\w)(physical_resolution|requested_physical_resolution|configured_resolution)="
            r"(?:Some\()?\s*[\[(]\s*(\d+)\s*,\s*(\d+)\s*[\])](?:\))?", line):
        fields[match[1]] = [int(match[2]), int(match[3])]
    return fields


def adapter_info(line):
    if "AdapterInfo {" not in line:
        return None
    result = {}
    for match in re.finditer(
            r'\b(name|driver|driver_info|device_pci_bus_id):\s*("(?:\\.|[^"\\])*")', line):
        result[match[1]] = json.loads(match[2])
    for match in re.finditer(r"\b(vendor|device):\s*(\d+)", line):
        result[match[1]] = int(match[2])
    for match in re.finditer(r"\b(backend|device_type):\s*(\w+)", line):
        result[match[1]] = match[2]
    return result if result.get("name") and result.get("backend") else None


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def config_fingerprints(directory):
    directory = Path(directory)
    files = {str(path.relative_to(directory)): sha256(path)
             for path in sorted(directory.rglob("*.toml")) if path.is_file()}
    if not {"world.toml", "graphics.toml", "server.toml"} <= files.keys():
        raise ValueError("Configs must include world.toml, graphics.toml and server.toml")
    return files


def freeze_configs(source, destination):
    source, destination = Path(source), Path(destination)
    fingerprints = config_fingerprints(source)
    if destination.exists():
        if config_fingerprints(destination) != fingerprints:
            raise ValueError("Existing frozen configs differ from the supplied configs; use a fresh output")
    else:
        destination.mkdir(parents=True)
        for name in fingerprints:
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / name, target)
    return fingerprints


def prepare_case_configs(frozen, destination, scenario):
    shutil.copytree(frozen, destination)
    server_path = Path(destination) / "server.toml"
    server = tomllib.loads(server_path.read_text())
    overrides = {}
    if scenario == "placement":
        # The rapid edit validator requires Creative. This explicit gameplay
        # override changes neither visual quality nor immutable world settings.
        lines = server_path.read_text().splitlines(keepends=True)
        first_table = next((index for index, line in enumerate(lines)
                            if line.lstrip().startswith("[")), len(lines))
        existing = next((index for index, line in enumerate(lines[:first_table])
                         if re.match(r"^\s*game_mode\s*=", line)), None)
        assignment = 'game_mode = "creative"\n'
        if existing is None:
            lines.insert(0, assignment)
        else:
            lines[existing] = assignment
        server_path.write_text("".join(lines))
        overrides["server.toml.game_mode"] = {"source": server.get("game_mode", "survival"),
                                               "effective": "creative"}
    return overrides


def point(value):
    try:
        values = tuple(float(piece) for piece in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("Point must be finite x,y,z") from error
    if len(values) != 3 or not all(math.isfinite(number) and abs(number) <= 1_000_000
                                   for number in values):
        raise argparse.ArgumentTypeError("Point must be finite x,y,z within world bounds")
    return ",".join(format(number, ".12g") for number in values)


def scenario_environment(base, scenario, frozen, case, village_camera=None, village_look_at=None,
                         rain_camera=None, rain_look_at=None):
    environment = {key: value for key, value in base.items()
                   if key not in REMOVED_KEYS and not key.startswith(REMOVED_PREFIXES)}
    overrides = {
        "TERRA_CONFIG_DIR": str(frozen),
        "TERRA_MENU_CONFIG_DIR": str(case / "menu-configs"),
        "TERRA_PROFILE_FILE": str(case / "profile.toml"),
        "TERRA_WORLD_DIR": str(case / "worlds"),
        "TERRA_BENCH_SAVE_DIR": str(case / "worlds" / "benchmark"),
        "TERRA_BENCH_MODE": "integrated",
        "TERRA_BENCH_UI": "1",
        "TERRA_BENCH_GRAPHICS": "1",
        "TERRA_BENCH_SECONDS": str(SECONDS[scenario]),
        "TERRA_BENCH_LOAD_SECONDS": "180",
        "TERRA_BENCH_WEATHER": "rain" if scenario == "rain" else "clear",
        "TERRA_CPU_SCOPES": "1",
        "TERRA_MEASURE_GPU_MEMORY": "1",
        "TERRA_MEASURE_TIMELINE": "1",
        "TERRA_SOAK_REPORT": "1",
        "RUST_LOG_STYLE": "never",
    }
    if scenario == "rain":
        if rain_camera is not None:
            overrides["TERRA_BENCH_CAMERA"] = rain_camera
        if rain_look_at is not None:
            overrides["TERRA_BENCH_LOOK_AT"] = rain_look_at
    elif scenario == "village":
        overrides["TERRA_BENCH_REQUIRE_VILLAGERS"] = "1"
        if village_camera is not None:
            overrides["TERRA_BENCH_CAMERA"] = village_camera
        if village_look_at is not None:
            overrides["TERRA_BENCH_LOOK_AT"] = village_look_at
    elif scenario == "flight":
        overrides.update(TERRA_SOAK_ROUTE="explore", TERRA_SOAK_ROUTE_SPEED="20")
    elif scenario == "inventory":
        overrides["TERRA_BENCH_INVENTORY"] = "1"
    elif scenario == "placement":
        overrides.update(TERRA_SOAK_EDITS="1", TERRA_SOAK_EDIT_MODE="rapid",
                         TERRA_SOAK_EDIT_CYCLES="120")
    environment.update(overrides)
    return environment, overrides


def positive(fields, names, context):
    for name in names:
        value = fields.get(name)
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) or value <= 0:
            raise ValueError(f"{context} has no valid positive {name}")


def parse_summary(contents, scenario):
    wall, phases, transitions, measurements, resources = [], [], [], [], []
    rapid, routes, configuration, scopes, residents = [], [], [], {}, []
    completed_inventory = False
    adapters = []
    for raw in contents.splitlines():
        line = ANSI.sub("", raw).strip()
        if line.startswith("{"):
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if record.get("kind") == "resource_timeline":
                resources.append(record)
            elif "command" in record and "exit_code" in record:
                measurements.append(record)
            continue
        adapter = adapter_info(line)
        if adapter is not None and adapter not in adapters:
            adapters.append(adapter)
        fields = native_fields(line)
        if WALL_MARKER in line:
            wall.append(fields)
        elif PHASE_MARKER in line:
            phases.append(fields)
        elif "Inventory benchmark real UI transition;" in line:
            transitions.append(fields)
        elif COMPLETE_INVENTORY in line:
            completed_inventory = True
        elif COMPLETE_RAPID in line:
            rapid.append(fields)
        elif "Real-input performance route phase" in line:
            routes.append(fields)
        elif "Grounded performance route progress" in line:
            routes.append({**fields, "phase": "walk"})
        elif "Grounded performance route completed a real return" in line:
            routes.append({**fields, "phase": "walk_return"})
        elif "Effective benchmark configuration (" in line:
            configuration.append(fields)
        elif "CPU scope within native sampling window" in line:
            if isinstance(fields.get("scope"), str):
                scopes[fields["scope"]] = fields
        elif "Generated settlement residents verified" in line:
            residents.append(fields)
    if len(measurements) != 1 or measurements[0].get("exit_code") != 0:
        raise ValueError("Missing successful measure.py exit summary")
    measurement = measurements[0]
    positive(measurement, ("wall_seconds", "peak_rss_kib"), "Process measurement")
    if len(configuration) != 1 or configuration[0].get("benchmark_mode") != "integrated" or configuration[0].get("benchmark_ui") is not True:
        raise ValueError("Native effective configuration did not prove integrated gameplay with UI")
    if scenario == "inventory":
        if not completed_inventory or [phase.get("phase") for phase in phases] != ["Closed", "Open", "ClosedAgain"]:
            raise ValueError("Inventory did not complete the three required real UI phases")
        for phase in phases:
            positive(phase, ("window_seconds", "sample_frames", "fps", "p95_ms", "p99_ms", "max_ms"), "Inventory phase")
            if phase["window_seconds"] < 11.9 or phase.get("dropped_samples") != 0:
                raise ValueError("Inventory phase was truncated or dropped frame samples")
        if {transition.get("stage") for transition in transitions} != {"Opening", "Closing"}:
            raise ValueError("Inventory opening/closing transitions were not measured")
        for transition in transitions:
            positive(transition, ("request_to_visible_ms", "frames", "max_ms"), "Inventory transition")
    else:
        if len(wall) != 1:
            raise ValueError("Missing unique native wall-frame summary")
        positive(wall[0], ("sample_frames", "sample_window_seconds", "average_fps",
                          "p95_frame_ms", "p99_frame_ms", "worst_frame_ms"), "Native summary")
        if wall[0]["sample_window_seconds"] != SECONDS[scenario]:
            raise ValueError("Native sampling duration differs from the requested scenario")
    if scenario == "placement":
        if len(rapid) != 1 or rapid[0].get("restored") is not True:
            raise ValueError("Rapid edits did not prove final terrain restoration")
        positive(rapid[0], ("cycles", "acknowledged_edits", "mean_ack_replication_ms"), "Rapid edits")
        if rapid[0]["acknowledged_edits"] != rapid[0]["cycles"] * 2:
            raise ValueError("Rapid placement/removal acknowledgements are incomplete")
        if rapid[0].get("restoration_scope") == "source_cell":
            before, after = rapid[0].get("ground_before"), rapid[0].get("ground_after")
            natural_grass_conversion = before == 1 and after == 2
            if rapid[0].get("source_restored") is not True or not isinstance(before, int) or not isinstance(after, int):
                raise ValueError("Rapid edits did not demonstrate their source/ground postconditions")
            if before != after and not natural_grass_conversion:
                raise ValueError("Rapid edits changed unrelated ground")
            if rapid[0].get("natural_grass_conversion") != natural_grass_conversion:
                raise ValueError("Rapid grass-decay metadata contradicts the observed materials")
    if scenario == "flight" and not any(route.get("phase") == "outbound" and route.get("acknowledged_input", 0) > 0 for route in routes):
        raise ValueError("Flight did not demonstrate real acknowledged outbound movement")
    if scenario == "village" and not any(row.get("residents", 0) > 0 and row.get("residents") == row.get("rendered_residents") for row in residents):
        raise ValueError("Village did not prove replicated and rendered residents")
    render_fields = ("loaded_sections", "rendered_sections", "occluded_sections", "mesh_assets",
                     "visible_mesh_entities", "point_lights", "shadowed_point_lights",
                     "loaded_active_props", "represented_props", "prop_batches", "water_tiles",
                     "terrain_composition_jobs", "reused_shadow_meshes", "reused_shadow_bytes",
                     "reused_edit_meshes", "reused_edit_bytes",
                     "far_groups", "far_source_batches", "far_cache_bytes",
                     "peak_admitted_upload_vertices", "peak_admitted_upload_bytes",
                     "deferred_upload_batches")
    native = wall[0] if wall else None
    return {
        "valid": True,
        "scenario": scenario,
        "measurement_kind": "CPU wall-frame intervals; GPU fields below are memory allocations, not GPU frame timings",
        "native_wall": native,
        "effective_configuration": configuration[0],
        "adapters": adapters,
        "render_counts": {key: native[key] for key in render_fields if native and key in native},
        "inventory_phases": phases,
        "inventory_transitions": transitions,
        "rapid_edits": rapid[0] if rapid else None,
        "real_input_route": routes,
        "village_residents": residents,
        "process": measurement,
        "resource_timeline": resources,
        "cpu_scopes": scopes,
    }


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def run_native(argv, environment, log_path, timeout):
    with Path(log_path).open("w") as log:
        process = subprocess.Popen(argv, cwd=ROOT, env=environment, stdout=log,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            return process.wait(timeout=timeout)
        except BaseException:
            # measure.py owns a child client. Reap the complete isolated process
            # group on timeout/interruption so no invisible benchmark stays alive.
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            raise


def options():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--configs", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--scenarios", nargs="+", choices=SCENARIOS, default=list(SCENARIOS))
    parser.add_argument("--village-camera", type=point,
                        help="x,y,z from cargo run -p terra-world --example performance_scene -- --configs DIR")
    parser.add_argument("--village-look-at", type=point)
    parser.add_argument("--rain-camera", type=point,
                        help="Optional warm low-altitude rain location; avoids cold-region snowfall")
    parser.add_argument("--rain-look-at", type=point)
    arguments = parser.parse_args()
    if len(set(arguments.scenarios)) != len(arguments.scenarios):
        parser.error("Each scenario can occur only once per invocation")
    if arguments.village_look_at and not arguments.village_camera:
        parser.error("--village-look-at requires --village-camera")
    if arguments.rain_look_at and not arguments.rain_camera:
        parser.error("--rain-look-at requires --rain-camera")
    return arguments


def main():
    arguments = options()
    binary, source, output = (path.resolve() for path in
                              (arguments.binary, arguments.configs, arguments.output))
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("--binary must name an existing executable")
    fingerprints = config_fingerprints(source)
    for scenario in arguments.scenarios:
        if (output / scenario).exists():
            raise ValueError(f"Refusing to overwrite existing scenario {output / scenario}; choose a fresh output")
    output.mkdir(parents=True, exist_ok=True)
    binary_hash = sha256(binary)
    manifest_path = output / "matrix.json"
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        if manifest.get("binary_sha256") != binary_hash or manifest.get("config_sha256") != fingerprints:
            raise ValueError("Existing matrix belongs to a different binary/config snapshot; use a fresh output")
    else:
        manifest = {"binary": str(binary), "binary_sha256": binary_hash,
                    "config_source": str(source), "config_sha256": fingerprints, "scenarios": {}}
    gpu_environment = gpu_environment_snapshot(os.environ)
    if "gpu_environment" in manifest and manifest["gpu_environment"] != gpu_environment:
        raise ValueError("GPU/backend environment differs from the existing matrix; use a fresh output")
    if "gpu_environment" not in manifest:
        manifest["gpu_environment"] = gpu_environment
        manifest["gpu_environment_recorded_utc"] = datetime.now(timezone.utc).isoformat()
    frozen = output / "configs"
    freeze_configs(source, frozen)
    manifest["graphics"] = tomllib.loads((frozen / "graphics.toml").read_text())
    manifest["world_seed"] = tomllib.loads((frozen / "world.toml").read_text()).get("seed")
    write_json(manifest_path, manifest)
    for scenario in arguments.scenarios:
        case = output / scenario
        case.mkdir()
        case_configs = case / "configs"
        scenario_overrides = prepare_case_configs(frozen, case_configs, scenario)
        shutil.copytree(case_configs, case / "menu-configs")
        environment, overrides = scenario_environment(
            os.environ, scenario, case_configs, case,
            arguments.village_camera, arguments.village_look_at,
            arguments.rain_camera, arguments.rain_look_at)
        argv = [sys.executable, str(MEASURE), str(binary), "--smoke-test-benchmark"]
        record = {
            "scenario": scenario, "status": "running",
            "started_utc": datetime.now(timezone.utc).isoformat(),
            "binary_sha256": binary_hash, "config_sha256": fingerprints,
            "scenario_config_sha256": config_fingerprints(case_configs),
            "scenario_overrides": scenario_overrides,
            "game_mode": tomllib.loads((case_configs / "server.toml").read_text()).get("game_mode", "survival"),
            "environment_overrides": overrides,
            "gpu_environment": gpu_environment_snapshot(environment),
            "removed_environment_keys": sorted(set(os.environ) - set(environment)),
            "argv": argv,
        }
        write_json(case / "manifest.json", record)
        print(f"Running {scenario}: {case / 'native.log'}", flush=True)
        try:
            code = run_native(argv, environment, case / "native.log", SECONDS[scenario] + 240)
            if code:
                raise ValueError(f"Native scenario exited with status {code}")
            summary = parse_summary((case / "native.log").read_text(errors="replace"), scenario)
            if config_fingerprints(frozen) != fingerprints:
                raise ValueError("Frozen base configuration was changed during the scenario")
            write_json(case / "summary.json", summary)
            record["status"] = "passed"
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            record.update(status="failed", error=str(error))
            raise
        except KeyboardInterrupt:
            record.update(status="interrupted", error="Interrupted by operator")
            raise
        finally:
            record["finished_utc"] = datetime.now(timezone.utc).isoformat()
            write_json(case / "manifest.json", record)
            manifest["scenarios"][scenario] = record
            write_json(manifest_path, manifest)
        print(f"Passed {scenario}: {case / 'summary.json'}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f"frame-spikes: {error}", file=sys.stderr)
        raise SystemExit(1)
