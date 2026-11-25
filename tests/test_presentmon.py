import unittest
from scripts.presentmon_summary import summarize


class DisplayTraceTests(unittest.TestCase):
    def test_excludes_unshown_presents_and_separates_gpu_time(self):
        def row(interval, gpu, chain="A"):
            return {"ProcessID": "12", "SwapChainAddress": chain, "MsBetweenDisplayChange": interval,
                    "MsGPUBusy": gpu, "MsUntilDisplayed": "NA" if interval == "NA" else "2"}
        result = summarize([row("16", "1"), row("NA", "2"), row("24", "3")])
        self.assertEqual(result["displayed_fps"], 50)
        self.assertEqual(result["display_interval_ms"]["median"], 20)
        self.assertEqual(result["gpu_busy_ms"]["median"], 2)
        self.assertEqual(result["present_calls"], 3)
        self.assertEqual(result["unshown"], 1)

    def test_requires_explicit_swapchain_with_multiple_outputs(self):
        rows = [{"ProcessID": "2", "SwapChainAddress": c, "MsBetweenDisplayChange": "10",
                 "MsUntilDisplayed": "1", "MsGPUBusy": "0.2"} for c in ["A","B"]]
        with self.assertRaises(ValueError):
            summarize(rows)
        self.assertEqual(summarize(rows, "B")["displayed_fps"], 100)
