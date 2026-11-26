"""Finite local RIFE source streamed to the export encoder by VSPipe."""
import os
import runpy
from fractions import Fraction
from pathlib import Path
import vapoursynth as vs

app = Path(__file__).resolve().parents[1]
vs.core.std.LoadPlugin(path=str(app / "providers/rife-export/bestsource.dll"))
source = vs.core.bs.VideoSource(
    source=os.environ["RTX_EXPORT_SOURCE"], cachemode=0, showprogress=False,
    exporttimestamps=True, maxdecoders=2, cachesize=64,
)
clip = source["clip"]
ticks = source["timestamps"]
timebase = Fraction(source["timebasenum"], source["timebaseden"])
if not ticks or abs(ticks[0] * timebase) > Fraction(1, 1000) or not clip.fps:
    raise ValueError("RIFE export requires constant-rate video with a zero-based video timeline.")
# Matroska can round a constant cadence to milliseconds. Compare each timestamp
# against its position on the original grid to detect drift and duration changes.
step = 1 / (clip.fps * timebase)
tolerance = min(step.denominator, Fraction(step.denominator, 1000) / timebase)
if any(abs((t - ticks[0]) * step.denominator - n * step.numerator) > tolerance
       for n, t in enumerate(ticks)):
    raise ValueError("This source has variable frame timing. Choose Original FPS to preserve it.")
rate = int(os.environ["RTX_EXPORT_FPS"])
factor = Fraction(rate, 1) / clip.fps
if not 1 < factor <= 6:
    raise ValueError("Choose a target above the source rate and at most six times that rate.")
runpy.run_path(str(app / "filters/rife.vpy"), init_globals={
    "video_in": clip, "user_data": str(factor), "finite_export": True,
})
