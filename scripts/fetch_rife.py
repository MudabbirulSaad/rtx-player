"""Download the optional NCNN RIFE provider beside the Windows desktop executable."""
import argparse
import json
from pathlib import Path
import re
import shutil
import stat
import tempfile
import urllib.request
import zipfile

from scripts.fetch_runtime import install_member, verified_copy


def extract_zip(archive, destination):
    total = 0
    with zipfile.ZipFile(archive) as z:
        for entry in z.infolist():
            parts = entry.filename.rstrip("/").split("/")
            if any(not re.fullmatch(r"[A-Za-z0-9_+. -]+", p) or p in (".", "..")
                   or p.endswith((".", " ")) or re.fullmatch(r"(?i)(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?", p)
                   for p in parts):
                raise ValueError(f"Invalid archive member: {entry.filename}")
            kind = stat.S_IFMT(entry.external_attr >> 16)
            if kind not in (0, stat.S_IFREG, stat.S_IFDIR):
                raise ValueError("Archive contains a link or special file")
            total += entry.file_size
            if total > 256 * 1024 * 1024:
                raise ValueError("Provider archive exceeds its extraction budget")
            target = destination.joinpath(*parts)
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with z.open(entry) as src, target.open("xb") as out:
                    shutil.copyfileobj(src, out)


def install(executable):
    manifest = json.loads((Path(__file__).resolve().parents[1] / "build/rife.json").read_text())
    parent = executable.resolve(strict=True).parent / "providers"
    parent.mkdir(exist_ok=True)
    installed = parent / "rife"
    if installed.exists():
        raise FileExistsError("The RIFE provider directory already exists")
    with tempfile.TemporaryDirectory(dir=parent) as tmp:
        root = Path(tmp) / "rife"
        root.mkdir()
        for entry in manifest:
            name = entry["name"]
            print(f"Downloading {name}", flush=True)
            binary = Path(tmp) / "download"
            with urllib.request.urlopen(entry["url"], timeout=60) as stream:
                verified_copy(stream, binary, entry["size"], entry["sha256"])
            if name in ("python", "vapoursynth"):
                folder = root / "python"
                if name == "vapoursynth":
                    folder = folder / "Lib/site-packages"
                extract_zip(binary, folder)
                binary.unlink()
            else:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                binary.rename(target)
        (root / "python/python314._pth").write_text("python314.zip\n.\nLib/site-packages\n", encoding="ascii")
        root.rename(installed)
    return installed


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--export-reader", action="store_true", help="add the finite video reader for RIFE export")
    args = parser.parse_args()
    if args.export_reader:
        app = args.executable.resolve(strict=True).parent
        if not (app / "providers/rife/librife.dll").is_file():
            parser.error("Install the RIFE playback provider first")
        folder = app / "providers/rife-export"
        folder.mkdir(exist_ok=True)
        manifest = json.loads((Path(__file__).resolve().parents[1] / "build/rife-reader.json").read_text())
        print(install_member(manifest, folder / "bestsource.dll"))
    else:
        print(install(args.executable))
