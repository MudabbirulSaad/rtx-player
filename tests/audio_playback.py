"""Check for non-silent samples in the player's Windows audio session."""
import argparse
from pathlib import Path
import subprocess
import time

from pycaw.pycaw import AudioUtilities, IAudioMeterInformation


def verify(executable, fixture):
    with subprocess.Popen([executable, fixture]) as player:
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and player.poll() is None:
                for session in AudioUtilities.GetAllSessions():
                    if session.ProcessId == player.pid:
                        meter = session._ctl.QueryInterface(IAudioMeterInformation)
                        peak = meter.GetPeakValue()
                        if peak > 0.001:
                            print(f"Player audio session peak {peak:.6f}")
                            return
                time.sleep(0.1)
            raise RuntimeError("Expected non-silent audio from the fixture")
        finally:
            player.terminate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("fixture", type=Path)
    args = parser.parse_args()
    verify(args.executable.resolve(strict=True), args.fixture.resolve(strict=True))
