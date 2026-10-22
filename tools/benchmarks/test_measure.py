import unittest
from measure import counter_delta, parse_drm_memory, parse_process_usage, parse_swap_kib


class DrmMemoryTest(unittest.TestCase):
    def test_driver_fields_are_not_invented_or_confused_with_other_units(self):
        self.assertIsNone(parse_drm_memory("pos: 0\nflags: 0"))
        client, memory = parse_drm_memory("drm-client-id:\t42\ndrm-pdev:\t0000:0c:00.0\ndrm-memory-vram:\t8192 KiB\ndrm-memory-gtt:\t4096 KiB")
        self.assertEqual(client, ("0000:0c:00.0", "42"))
        self.assertEqual(memory, {"vram": 8192, "gtt": 4096})
        self.assertEqual(parse_drm_memory("drm-client-id: 2\ndrm-memory-vram: 5 MiB")[1], {})


class ProcessUsageTest(unittest.TestCase):
    def test_cpu_ticks_and_current_resident_pages_survive_spaces_and_parentheses(self):
        fields = ["S"] + ["0"] * 21
        fields[11], fields[12], fields[21] = "123", "45", "32"
        fields[7], fields[9] = "678", "9"
        stat = "42 (Teramic client (test)) " + " ".join(fields)
        self.assertEqual(parse_process_usage(stat, 4096), {
            "rss_kib": 128, "user_cpu_ticks": 123, "system_cpu_ticks": 45,
            "minor_faults": 678, "major_faults": 9,
        })
        self.assertEqual(parse_process_usage(stat, 16384)["rss_kib"], 512)

    def test_missing_truncated_and_invalid_samples_are_unavailable(self):
        for contents in ("", "42 (Teramic) S 0 0", "42 (Teramic S 0 0"):
            self.assertIsNone(parse_process_usage(contents, 4096))
        fields = ["S"] + ["0"] * 21
        fields[11] = "invalid"
        self.assertIsNone(parse_process_usage("42 (Teramic) " + " ".join(fields), 4096))
        fields[11], fields[21] = "0", "-1"
        self.assertIsNone(parse_process_usage("42 (Teramic) " + " ".join(fields), 4096))

    def test_swap_is_distinct_from_rss_and_missing_data_is_not_reported_as_zero(self):
        self.assertEqual(parse_swap_kib("VmRSS:\t600000 kB\nVmSwap:\t712008 kB\n"), 712008)
        self.assertEqual(parse_swap_kib("VmSwap: 0 kB"), 0)
        for contents in ("", "VmRSS: 600000 kB", "VmSwap: 7 MiB", "VmSwap: -1 kB", "VmSwap: n/a"):
            self.assertIsNone(parse_swap_kib(contents))

    def test_fault_counter_deltas_do_not_invent_missing_or_reset_samples(self):
        self.assertEqual(counter_delta({"major_faults": 18}, {"major_faults": 4}, "major_faults"), 14)
        self.assertEqual(counter_delta({"major_faults": 0}, {"major_faults": 0}, "major_faults"), 0)
        self.assertIsNone(counter_delta({"major_faults": 1}, {"major_faults": 5}, "major_faults"))
        self.assertIsNone(counter_delta(None, {"major_faults": 5}, "major_faults"))
        self.assertIsNone(counter_delta({}, {}, "major_faults"))


if __name__ == "__main__":
    unittest.main()
