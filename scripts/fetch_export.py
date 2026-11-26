"""Install the pinned Windows FFmpeg export executable beside a player build."""
import argparse
import json
from pathlib import Path

from scripts.fetch_runtime import install_member


def install(player):
    manifest = json.loads((Path(__file__).resolve().parents[1] / "build/export-runtime.json").read_text())
    folder = player.resolve(strict=True).parent / "tools" / "export"
    folder.mkdir(parents=True, exist_ok=True)
    return install_member(manifest, folder / "ffmpeg.exe")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("player", type=Path)
    print(install(parser.parse_args().player))
