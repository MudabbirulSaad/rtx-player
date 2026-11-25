"""Install the pinned Windows FFmpeg export executable beside a player build."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request
import zipfile

from scripts.fetch_runtime import verified_copy


def install(player):
    manifest = json.loads((Path(__file__).resolve().parents[1] / "build/export-runtime.json").read_text())
    folder = player.resolve(strict=True).parent / "tools"
    folder.mkdir(exist_ok=True)
    target = folder / "ffmpeg.exe"
    if target.exists():
        with target.open("rb") as binary:
            if hashlib.file_digest(binary, "sha256").hexdigest() == manifest["member_sha256"]:
                return target
        raise FileExistsError(f"Another encoder already exists at {target}")
    with tempfile.TemporaryDirectory(dir=folder) as scratch:
        archive = Path(scratch) / "runtime.whl"
        staged = Path(scratch) / "ffmpeg.exe"
        with urllib.request.urlopen(manifest["source"], timeout=60) as response:
            verified_copy(response, archive, manifest["archive_bytes"], manifest["archive_sha256"])
        with zipfile.ZipFile(archive) as package, package.open(manifest["member"]) as binary:
            verified_copy(binary, staged, manifest["member_bytes"], manifest["member_sha256"])
        staged.rename(target)
    return target


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("player", type=Path)
    print(install(parser.parse_args().player))
