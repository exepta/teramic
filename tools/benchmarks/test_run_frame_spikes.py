import argparse
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "run_frame_spikes", Path(__file__).with_name("run-frame-spikes.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)

CONFIG = ('INFO Effective benchmark configuration (graphics file loading is explicit and opt-in) '
          'benchmark_mode="integrated" benchmark_ui=true\n')
MEASURE = json.dumps({"command": ["client", "--smoke-test-benchmark"], "exit_code": 0,
                      "wall_seconds": 35, "peak_rss_kib": 2000000,
                      "peak_drm_memory_vram_kib": 3000000}) + "\n"


def wall(seconds=24):
    return (f'INFO Native smoke benchmark (CPU wall frames; screenshot-adjacent frames excluded) '
            f'sample_frames=1000 sample_window_seconds={seconds} average_fps=60 '
            'p95_frame_ms=20 p99_frame_ms=30 worst_frame_ms=50 loaded_sections=1200\n')


class FrameSpikeRunnerTests(unittest.TestCase):
    def test_wall_summary_keeps_actual_resources_and_rejects_missing_or_wrong_measurements(self):
        summary = runner.parse_summary(CONFIG + wall() + MEASURE, "clear")
        self.assertEqual(summary["native_wall"]["average_fps"], 60)
        self.assertEqual(summary["render_counts"]["loaded_sections"], 1200)
        self.assertEqual(summary["process"]["peak_drm_memory_vram_kib"], 3000000)
        self.assertIn("not GPU frame timings", summary["measurement_kind"])
        optional = runner.parse_summary(
            CONFIG + wall().replace("worst_frame_ms=50", "worst_frame_ms=Some(50.125)")
            + "INFO initial_load_seconds=Some(12.5)\n" + MEASURE, "clear")
        self.assertEqual(optional["native_wall"]["worst_frame_ms"], 50.125)
        for bad in [CONFIG + MEASURE, CONFIG + wall(12) + MEASURE,
                    CONFIG + wall() + MEASURE.replace('"exit_code": 0', '"exit_code": 1'),
                    CONFIG + wall() + MEASURE.replace('"peak_rss_kib": 2000000', '"peak_rss_kib": null'),
                    wall() + MEASURE, CONFIG + wall().replace("average_fps=60", "average_fps=NaN") + MEASURE]:
            with self.assertRaises(ValueError):
                runner.parse_summary(bad, "clear")

    def test_inventory_requires_all_phases_and_actual_open_close_transitions(self):
        phases = "".join(
            f'INFO Inventory benchmark phase: CPU wall frames, unchanged scene and quality '
            f'phase={phase} window_seconds=12.01 sample_frames=720 dropped_samples=0 '
            'fps=60 p95_ms=20 p99_ms=30 max_ms=40\n'
            for phase in ["Closed", "Open", "ClosedAgain"])
        transitions = "".join(
            f'INFO Inventory benchmark real UI transition; opening includes settled visible slot layout '
            f'stage={stage} request_to_visible_ms=33 frames=3 max_ms=20\n'
            for stage in ["Opening", "Closing"])
        contents = CONFIG + phases + transitions + runner.COMPLETE_INVENTORY + "\n" + MEASURE
        self.assertEqual(len(runner.parse_summary(contents, "inventory")["inventory_phases"]), 3)
        for bad in [contents.replace("phase=Open ", "phase=Closed "),
                    contents.replace("window_seconds=12.01", "window_seconds=2"),
                    contents.replace("dropped_samples=0", "dropped_samples=1"),
                    contents.replace("stage=Closing", "stage=Opening")]:
            with self.assertRaises(ValueError):
                runner.parse_summary(bad, "inventory")

    def test_rapid_summary_requires_balanced_acknowledgements_and_restoration(self):
        complete = ('INFO Rapid authoritative edits complete; benchmark continues idle '
                    'cycles=10 acknowledged_edits=20 mean_ack_replication_ms=23 restored=true\n')
        contents = CONFIG + wall(35) + complete + MEASURE
        self.assertEqual(runner.parse_summary(contents, "placement")["rapid_edits"]["cycles"], 10)
        for bad in [contents.replace("restored=true", "restored=false"),
                    contents.replace("acknowledged_edits=20", "acknowledged_edits=19"),
                    contents.replace("cycles=10 acknowledged_edits=20", "cycles=0 acknowledged_edits=0")]:
            with self.assertRaises(ValueError):
                runner.parse_summary(bad, "placement")

    def test_rapid_grass_decay_is_explicit_and_other_ground_changes_fail(self):
        complete = ('INFO Rapid authoritative edits complete; benchmark continues idle '
                    'cycles=10 acknowledged_edits=20 mean_ack_replication_ms=23 restored=true '
                    'restoration_scope="source_cell" source_restored=true '
                    'ground_before=1 ground_after=2 natural_grass_conversion=true\n')
        contents = CONFIG + wall(35) + complete + MEASURE
        summary = runner.parse_summary(contents, "placement")
        self.assertTrue(summary["rapid_edits"]["natural_grass_conversion"])
        for bad in [contents.replace("ground_after=2", "ground_after=3"),
                    contents.replace("source_restored=true", "source_restored=false"),
                    contents.replace("natural_grass_conversion=true", "natural_grass_conversion=false")]:
            with self.assertRaises(ValueError):
                runner.parse_summary(bad, "placement")

    def test_frontier_requires_streaming_samples_and_authoritative_arrival(self):
        route = 'INFO Real-input performance route phase phase="outbound" position=[8, 32, 4]\n'
        movement = 'INFO Frontier flight progress phase="outbound" surface_probes=25\n'
        arrival = 'INFO Frontier flight progress phase="settle" authoritative_x=3608.0\n'
        common = CONFIG + wall(105) + route + MEASURE + "\n"
        result = runner.parse_summary(common + movement + arrival, "frontier")
        self.assertEqual(len(result["frontier_samples"]), 2)
        for incomplete in [movement, arrival, movement + arrival.replace("3608", "308")]:
            with self.assertRaises(ValueError):
                runner.parse_summary(common + incomplete, "frontier")

    def test_optional_frontier_altitude_profile_is_validated_and_isolated(self):
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp) / "heights.json"
            for invalid in ["[]", "[0]", "[1, false]", "[15,345]", "[{},2]", "[NaN,20]"]:
                profile.write_text(invalid)
                with self.assertRaises(argparse.ArgumentTypeError):
                    runner.flight_heights(profile)
            profile.write_text("[75, 82, 76]")
            heights = runner.flight_heights(profile)
            self.assertEqual(heights, "[75,82,76]")
            stale = {"TERRA_FRONTIER_HEIGHTS": "[330,330]"}
            for scenario in ["clear", "flight", "frontier"]:
                env, _ = runner.scenario_environment(stale, scenario, Path(tmp), Path(tmp))
                self.assertNotIn("TERRA_FRONTIER_HEIGHTS", env)
            env, _ = runner.scenario_environment(stale, "frontier", Path(tmp), Path(tmp),
                                                  frontier_camera="8,90,32", frontier_heights=heights)
            self.assertEqual((env["TERRA_BENCH_CAMERA"], env["TERRA_FRONTIER_HEIGHTS"]), ("8,90,32", heights))

    def test_configs_are_frozen_and_frontier_uses_menu_world_generation(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            source, frozen = root / "source", root / "frozen"
            source.mkdir()
            (source / "world.toml").write_text("seed = 42\n")
            (source / "graphics.toml").write_text("width = 2560\nrender_distance = 20\n")
            (source / "server.toml").write_text('title = "Benchmark"\n[player_permissions]\n')
            fingerprints = runner.freeze_configs(source, frozen)
            self.assertEqual(runner.freeze_configs(source, frozen), fingerprints)
            overrides = runner.prepare_case_configs(frozen, root / "placement", "placement")
            self.assertEqual(overrides["server.toml.game_mode"]["effective"], "creative")
            for name in ["world.toml", "graphics.toml"]:
                self.assertEqual((frozen / name).read_bytes(), (root / "placement" / name).read_bytes())
            overrides = runner.prepare_case_configs(frozen, root / "frontier", "frontier")
            self.assertEqual(overrides["world.toml.bounded_world"], {"source": False, "effective": True})
            self.assertIn("bounded_world = true", (root / "frontier/world.toml").read_text())
            self.assertEqual(runner.config_fingerprints(frozen), fingerprints)
            (source / "world.toml").write_text("seed = 43\n")
            with self.assertRaises(ValueError):
                runner.freeze_configs(source, frozen)

    def test_stale_environment_cannot_enable_other_routes_or_gpu_timing(self):
        base = {"PATH": "/bin", "TERRA_SOAK_ROUTE": "walk",
                "TERRA_BENCH_CAMERA": "1,2,3", "TERRA_RENDER_DIAGNOSTICS": "1",
                "TERRA_SCOPE_TIMELINE": "1", "TERRA_SCREENSHOT": "/tmp/unwanted"}
        environment, _ = runner.scenario_environment(base, "clear", Path("/configs"), Path("/case"))
        self.assertEqual(environment["PATH"], "/bin")
        self.assertEqual(environment["TERRA_BENCH_WEATHER"], "clear")
        for name in ["TERRA_SOAK_ROUTE", "TERRA_BENCH_CAMERA", "TERRA_RENDER_DIAGNOSTICS",
                     "TERRA_SCOPE_TIMELINE", "TERRA_SCREENSHOT"]:
            self.assertNotIn(name, environment)

    def test_nested_physical_dimensions_and_actual_adapter_are_not_truncated(self):
        line = ('physical_resolution=Some((2562, 1440)) '
                'configured_resolution=(2562, 1440) '
                'requested_physical_resolution=Some([2562, 1440])')
        for name, value in runner.native_fields(line).items():
            self.assertEqual(value, [2562, 1440], name)
        adapter = runner.adapter_info(
            'INFO AdapterInfo { name: "AMD Radeon RX 6950 XT (RADV NAVI21)", '
            'vendor: 4098, device: 29605, device_type: DiscreteGpu, '
            'driver: "radv", driver_info: "Mesa 26.2.5", backend: Vulkan }')
        self.assertEqual(adapter["backend"], "Vulkan")
        self.assertEqual(adapter["driver_info"], "Mesa 26.2.5")
        self.assertEqual(adapter["vendor"], 4098)
        self.assertIsNone(runner.adapter_info("No supported adapter information"))

    def test_rain_camera_is_explicit_and_gpu_environment_is_allowlisted(self):
        environment, _ = runner.scenario_environment(
            {"WGPU_BACKEND": "vulkan", "UNRELATED_SECRET": "never-record-this"},
            "rain", Path("/configs"), Path("/case"),
            rain_camera="-585.5,33.62,976.5", rain_look_at="-583.5,36.6,981.5")
        self.assertEqual(environment["TERRA_BENCH_CAMERA"], "-585.5,33.62,976.5")
        self.assertEqual(environment["TERRA_BENCH_LOOK_AT"], "-583.5,36.6,981.5")
        snapshot = runner.gpu_environment_snapshot(environment)
        self.assertEqual(snapshot["WGPU_BACKEND"], "vulkan")
        self.assertIsNone(snapshot["WGPU_ADAPTER_NAME"])
        self.assertNotIn("UNRELATED_SECRET", snapshot)


if __name__ == "__main__":
    unittest.main()
