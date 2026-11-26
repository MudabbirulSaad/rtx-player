"""Run with the optional provider's embedded Python and the installed filter script."""
from fractions import Fraction
from pathlib import Path
import runpy
import sys
import vapoursynth as vs


def verify(script):
    core = vs.core
    src = core.std.BlankClip(width=256, height=144, length=16, fpsnum=24,
                             format=vs.YUV420P8, color=[16, 128, 128])
    def paint(n, f):
        frame = f.copy()
        y = frame[0]
        if n >= 8:
            for row in range(144):
                for col in range(256):
                    y[row, col] = 225
        else:
            for row in range(44, 92):
                for col in range(24+4*n, 64+4*n):
                    y[row, col] = 75 + 150 * ((col-4*n+row) % 7 < 4)
        frame.props["_DurationNum"] = 1
        frame.props["_DurationDen"] = 24
        return frame

    original = core.std.ModifyFrame(src, clips=src, selector=paint)
    runpy.run_path(str(script), init_globals={"video_in": original, "user_data": "2"})
    out = vs.get_output(0)[0]
    frames = [out.get_frame(i) for i in range(3)]
    def center(frame):
        luma = frame[0]
        weights = [sum((v-16) if (v := luma[y,x]) > 40 else 0 for y in range(40,96)) for x in range(256)]
        return sum(i*w for i,w in enumerate(weights))/sum(weights)

    a, mid, b = map(center, frames)
    assert abs(mid - (a+b)/2) < 0.6, (a,mid,b)
    assert abs(b-a-4) < 0.2, (a,b)
    # Blending endpoint pixels can also match the centroid. Check the moving texture itself.
    planes = [f[0] for f in frames]
    delta = sum(abs(planes[1][y,x] - (planes[0][y,x]+planes[2][y,x])/2)
                for y in range(46,90) for x in range(30,62)) / (44*32)
    assert delta > 5, f"Intermediate frame resembles an endpoint blend: {delta}"
    assert out.fps == 48 and out.num_frames == 2*original.num_frames
    for frame in frames:
        assert Fraction(frame.props["_DurationNum"], frame.props["_DurationDen"]) == Fraction(1,48)
    cut, held = out.get_frame(14), out.get_frame(15)
    assert bytes(cut[0]) == bytes(held[0]), "Cut was interpolated"
    print(f"Object centers {a:.2f}, {mid:.2f}, {b:.2f}; mean difference from blending {delta:.2f}; cut held")
    runpy.run_path(str(script), init_globals={"video_in": original, "user_data": "2.5"})
    fractional = vs.get_output(0)[0]
    assert fractional.fps == 60 and fractional.num_frames == 40
    for i in range(6):
        frame = fractional.get_frame(i)
        assert Fraction(frame.props["_DurationNum"],frame.props["_DurationDen"]) == Fraction(1,60)
        assert abs(center(frame) - (a + 1.6*i)) < 0.8
    print("24 to 60 fps: six correctly timed motion samples")
    runpy.run_path(str(script), init_globals={"video_in": original, "user_data": "1.25"})
    custom = vs.get_output(0)[0]
    assert custom.fps == 30 and custom.num_frames == 20
    for i in range(4):
        frame = custom.get_frame(i)
        assert Fraction(frame.props["_DurationNum"],frame.props["_DurationDen"]) == Fraction(1,30)
        assert abs(center(frame) - (a + 3.2*i)) < 0.8
    print("24 to 30 fps: four correctly timed motion samples")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve(strict=True))
