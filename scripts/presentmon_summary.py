"""Summarize the DXGI display intervals from one PresentMon 2.x swap chain."""
import argparse
import csv
import json
import math
import statistics
from pathlib import Path


def number(value):
    if value in (None, "", "NA"):
        return None
    result = float(value)
    if not math.isfinite(result) or result < 0:
        raise ValueError(f"Unexpected timing value: {value}")
    return result


def distribution(samples):
    if not samples:
        return None
    values = sorted(samples)
    return {"median": statistics.median(values),
            "p95": values[math.ceil(0.95 * len(values)) - 1],
            "p99": values[math.ceil(0.99 * len(values)) - 1],
            "max": values[-1]}


def summarize(rows, chain=None):
    rows = list(rows)
    if len({r["ProcessID"] for r in rows}) != 1:
        raise ValueError("Capture a single player process")
    chains = {r["SwapChainAddress"] for r in rows}
    if chain is None:
        if len(chains) != 1:
            raise ValueError(f"Select a video swap chain from {sorted(chains)}")
        chain = next(iter(chains))
    rows = [r for r in rows if r["SwapChainAddress"] == chain]
    intervals = [v for r in rows if (v := number(r["MsBetweenDisplayChange"])) is not None and v > 0]
    if not intervals:
        raise ValueError("No positive displayed-frame intervals on the selected swap chain")
    gpu = [v for r in rows if (v := number(r["MsGPUBusy"])) is not None]
    latency = [v for r in rows if (v := number(r["MsUntilDisplayed"])) is not None]
    return {"present_calls": len(rows), "unshown": len(rows) - len(latency),
            "display_intervals": len(intervals), "displayed_seconds": sum(intervals)/1000,
            "displayed_fps": 1000/statistics.mean(intervals),
            "display_interval_ms": distribution(intervals),
            "present_to_display_ms": distribution(latency),
            "gpu_busy_ms": distribution(gpu)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--chain")
    args = parser.parse_args()
    with args.trace.open(encoding="utf-8-sig", newline="") as file:
        print(json.dumps(summarize(csv.DictReader(file), args.chain), indent=2))
