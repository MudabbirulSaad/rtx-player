"""Place the pinned Windows playback runtime beside an existing player executable."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request


def verified_copy(source, destination, size, digest):
    count = 0
    checksum = hashlib.sha256()
    with destination.open("xb") as output:
        while block := source.read(1024 * 1024):
            count += len(block)
            if count > size:
                raise ValueError("Runtime data exceeds the pinned size")
            checksum.update(block)
            output.write(block)
    if count != size or checksum.hexdigest() != digest:
        raise ValueError("Runtime size or SHA-256 differs from the manifest")


def install(executable):
    manifest = json.loads((Path(__file__).resolve().parents[1] / "build/runtime.json").read_text())
    destination = executable.resolve(strict=True).parent / "libmpv-2.dll"
    return install_member(manifest, destination)


def install_member(manifest, destination):
    if destination.exists():
        with destination.open("rb") as existing:
            if hashlib.file_digest(existing, "sha256").hexdigest() == manifest["member_sha256"]:
                return destination
        raise FileExistsError(f"Another runtime already exists at {destination}")
    with tempfile.TemporaryDirectory(dir=destination.parent) as scratch:
        archive = Path(scratch) / "download.7z"
        member = Path(scratch) / "verified-member"
        with urllib.request.urlopen(manifest["source"], timeout=60) as response:
            verified_copy(response, archive, manifest["archive_bytes"], manifest["archive_sha256"])
        # Extract one member to stdout. Archive-controlled names never become file paths.
        tar = Path(os.environ["SystemRoot"]) / "System32/tar.exe"
        with subprocess.Popen([tar, "-xOf", archive, manifest["member"]], stdout=subprocess.PIPE) as proc:
            try:
                verified_copy(proc.stdout, member, manifest["member_bytes"], manifest["member_sha256"])
            finally:
                proc.stdout.close()
            if proc.wait() != 0:
                raise RuntimeError("Windows tar could not read the runtime archive")
        member.rename(destination)
    return destination


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    print(install(parser.parse_args().executable))
