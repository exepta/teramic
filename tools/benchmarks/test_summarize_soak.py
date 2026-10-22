import io
import json
import unittest

from summarize_soak import parse_fields, summarize, write_csv


READY = "2026-10-20T12:00:00Z INFO test: Performance soak sampling started after initial scene readiness\n"


def trace(message, elapsed=60.01, payload="", stamp="2026-10-20T12:01:00.01Z"):
    return f"{stamp} INFO test: Performance soak {message} elapsed_seconds={elapsed} {payload}\n"


class SoakSummaryTest(unittest.TestCase):
    def test_image_payload_estimates_remain_separate_from_measured_vram(self):
        window = summarize([
            trace("interval", payload="fps=90 image_assets=200"),
            trace("image resources; descriptor estimates are not measured VRAM",
                  payload="cpu_image_data_bytes=1048576 descriptor_texture_bytes_estimate=2097152 "
                          "font_atlas_texture_ids=1 font_atlas_font_size_min=Some(16.0)"),
            trace("UI font cache", payload="pinned_fonts=2 pinned_bytes=9000 evicted_fonts=0"),
        ])["windows"][0]
        self.assertEqual(window["image_assets"], 200)
        self.assertEqual(window["image_resources"]["cpu_image_data_bytes"], 1048576)
        self.assertEqual(window["image_resources"]["descriptor_texture_bytes_estimate"], 2097152)
        self.assertEqual(window["image_resources"]["font_atlas_texture_ids"], 1)
        self.assertEqual(window["image_resources"]["font_atlas_font_size_min"], 16.0)
        self.assertEqual(window["ui_font_cache"]["pinned_fonts"], 2)
        self.assertEqual(window["ui_font_cache"]["pinned_bytes"], 9000)
        self.assertNotIn("drm_memory_vram_kib", window)

    def test_structured_fields_and_option_values(self):
        parsed = parse_fields('scope="render/early prepass/elapsed_cpu" calls=3 total_ms=1.2e-3 '
                              'rss_bytes=Some(1048576) missing=None ready=true '
                              'fluid=FluidWorkCounts { active: 2, queued: 3 }')
        self.assertEqual(parsed["scope"], "render/early prepass/elapsed_cpu")
        self.assertEqual(parsed["rss_bytes"], 1048576)
        self.assertEqual(parsed["total_ms"], 0.0012)
        self.assertIsNone(parsed["missing"])
        self.assertIs(parsed["ready"], True)
        self.assertEqual(parsed["fluid"], "FluidWorkCounts { active: 2, queued: 3 }")

    def test_interval_scope_and_latest_diagnostics_have_distinct_semantics(self):
        summary = summarize([
            READY,
            trace("interval", payload="fps=96 frames=2880 interval_seconds=30 median_ms=10 p95_ms=14 p99_ms=18 max_ms=21 rss_bytes=Some(1048576) entities=32768 mesh_entities=12000"),
            trace("CPU scope interval", payload='scope="mapping.index" calls=150 total_ms=120 mean_ms=0.8 cumulative_max_ms=2.5 over_8_ms=0'),
            trace("latest render pass diagnostic", payload='diagnostic="render/early prepass/elapsed_gpu" value_ms=0.7'),
            trace("world resources", payload="sections=4393 stored_props=19000"),
        ])
        window = summary["windows"][0]
        self.assertEqual(window["rss_mib"], 1)
        self.assertNotIn("entities", window)
        self.assertEqual(window["legacy_entity_indices"], 32768)
        self.assertEqual(window["cpu_scopes"]["mapping.index"]["total_ms"], 120)
        self.assertEqual(window["render_pass_latest_ms"]["render/early prepass/elapsed_gpu"], 0.7)
        self.assertEqual(window["world"]["sections"], 4393)
        self.assertEqual(summary["checkpoints"][0]["status"], "measured")
        self.assertAlmostEqual(summary["checkpoints"][0]["sample_offset_seconds"], 0.01)
        self.assertEqual(summary["checkpoints"][1]["status"], "not_measured")
        stream = io.StringIO()
        write_csv(summary, stream)
        self.assertIn("legacy_entity_indices", stream.getvalue())
        self.assertIn("32768", stream.getvalue())

    def test_resource_join_requires_a_common_clock_and_rejects_future_samples(self):
        ready_unix = 1792497600
        resource = {"kind": "resource_timeline", "elapsed_seconds": 74.9, "rss_kib": 2048,
                    "wall_unix_seconds": ready_unix + 59.9, "drm_memory_vram_kib": 4000}
        future = dict(resource, elapsed_seconds=75.02, wall_unix_seconds=ready_unix + 60.02,
                      drm_memory_vram_kib=9999)
        lines = [READY, json.dumps(resource), json.dumps(future), trace("interval", payload="fps=96")]
        window = summarize(lines)["windows"][0]
        self.assertEqual(window["resource_timeline"]["drm_memory_vram_kib"], 4000)
        self.assertAlmostEqual(window["resource_timeline"]["sample_lag_seconds"], 0.11, places=5)
        resource.pop("wall_unix_seconds")
        old = [READY, json.dumps(resource), trace("interval", payload="fps=96")]
        self.assertNotIn("resource_timeline", summarize(old)["windows"][0])
        self.assertEqual(summarize(old)["resources"][0]["alignment"], "unavailable_no_common_clock")
        joined = summarize(old, ready_offset_seconds=15)["windows"][0]["resource_timeline"]
        self.assertEqual(joined["alignment"], "explicit_ready_offset")

    def test_missing_checkpoints_are_not_filled_with_distant_or_unfinished_windows(self):
        summary = summarize([trace("interval", elapsed=30, payload="fps=80"),
                             trace("interval", elapsed=400, payload="fps=90"),
                             trace("CPU scope interval", elapsed=600, payload='scope="partial" calls=1')])
        self.assertTrue(all(point["status"] == "not_measured" for point in summary["checkpoints"]))
        self.assertEqual(len(summary["windows"]), 2)

    def test_explicit_allocated_indices_are_not_relabelled_as_legacy(self):
        window = summarize([trace("interval", payload="fps=90 entities=123 allocated_entity_indices=32768")])["windows"][0]
        self.assertEqual(window["entities"], 123)
        self.assertEqual(window["entity_count_kind"], "explicit_fields")
        self.assertNotIn("legacy_entity_indices", window)


if __name__ == "__main__":
    unittest.main()
