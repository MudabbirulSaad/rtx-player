# mpv supplies video_in and user_data when it creates a VapourSynth filter.
from pathlib import Path
from fractions import Fraction
import vapoursynth as vs

factor = Fraction(user_data)
if not (1 < factor <= 6) or (not globals().get("finite_export") and factor not in (2, Fraction(5, 2), 3, 4, 5, 6)):
    raise ValueError("Unsupported RIFE multiplier")
root = Path(__file__).resolve().parents[1] / "providers/rife"
core = vs.core
if not hasattr(core, "rife"):
    core.std.LoadPlugin(path=str(root / "librife.dll"))
# Keep the source frame durations. mpv can deliver variable-duration frames.
matrix = "709" if video_in.width >= 1280 or video_in.height > 576 else "170m"
rgb = core.resize.Bicubic(video_in, format=vs.RGBS, matrix_in_s=matrix)
luma = core.std.ShufflePlanes(video_in, planes=0, colorfamily=vs.GRAY)
stats = core.std.PlaneStats(luma, luma[1:])
def mark_cut(n, f):
    out = f[0].copy()
    out.props["_SceneChangeNext"] = int(f[1].props["PlaneStatsDiff"] > 0.22)
    return out
marked = core.std.ModifyFrame(rgb, clips=[rgb, stats], selector=mark_cut)
out = core.rife.RIFE(marked, model_path=str(root / "models/rife-v4.25-lite_ensembleFalse"),
                     factor_num=factor.numerator, factor_den=factor.denominator, gpu_thread=2, sc=True)
core.resize.Bicubic(out, format=video_in.format.id, matrix_s=matrix).set_output()
