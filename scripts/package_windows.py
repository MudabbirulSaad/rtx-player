"""Build the Windows desktop player, portable archive and per-user NSIS installer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

from scripts import fetch_export, fetch_runtime

ROOT = Path(__file__).resolve().parents[1]


def run(args, cwd=ROOT, env=None):
    return subprocess.run(args, cwd=cwd, env=env, check=True, text=True)


def digest(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file,"sha256").hexdigest()


def sbom(metadata):
    components = [{"name": p["name"], "version": p["version"], "license": p["license"],
                   "ecosystem": "cargo"} for p in metadata["packages"]]
    lock = json.loads((ROOT/"package-lock.json").read_text())
    for name,p in lock["packages"].items():
        if name and not p.get("dev") and "version" in p:
            components.append({"name": name.rsplit("node_modules/",1)[-1], "version": p["version"],
                               "license": p.get("license"), "ecosystem": "npm"})
    return {"scope": "Resolved Rust dependencies and production JavaScript packages",
            "components": sorted(components,key=lambda c:(c["ecosystem"],c["name"],c["version"])),
            "native_distributions": {p.stem:json.loads(p.read_text()) for p in
                [ROOT/"build/runtime.json",ROOT/"build/export-runtime.json"]}}


def portable(path, files):
    with zipfile.ZipFile(path, "x") as archive:
        for name,source in sorted(files.items()):
            info = zipfile.ZipInfo("RTX Player/"+name, date_time=(1980,1,1,0,0,0))
            info.compress_type = zipfile.ZIP_DEFLATED
            with archive.open(info,"w") as out, source.open("rb") as src:
                shutil.copyfileobj(src,out)


def build(output):
    if os.name!="nt": raise OSError("Packaging requires Windows")
    if output.exists(): raise FileExistsError("Select a new artifact directory")
    def revision():
        status=subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True)
        if status.strip(): raise RuntimeError("Package a clean checkout so the revision identifies its sources")
        return subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()
    commit=revision()
    run(["npm.cmd","run","build"])
    run(["cargo","build","--release","--locked","-p","rtx-player-desktop"])
    toolchain=subprocess.check_output(["rustc","-vV"],text=True)
    host=next(line.removeprefix("host: ") for line in toolchain.splitlines() if line.startswith("host: "))
    metadata=json.loads(subprocess.check_output(["cargo","metadata","--format-version","1","--locked","--filter-platform",host],cwd=ROOT))
    binary=Path(metadata["target_directory"])/"release/rtx-player-desktop.exe"
    runtime=fetch_runtime.install(binary)
    encoder=fetch_export.install(binary)
    version=tomllib.loads((ROOT/"desktop/Cargo.toml").read_text())["package"]["version"]
    tag=f"RTX-Player-{version}-{commit[:12]}-windows-x64"
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        scratch=Path(temp)
        payload=scratch/"payload"
        payload.mkdir()
        files={"rtx-player-desktop.exe":binary,"libmpv-2.dll":runtime,"tools/ffmpeg.exe":encoder,
               "LICENSE":ROOT/"LICENSE","THIRD_PARTY_NOTICES.md":ROOT/"THIRD_PARTY_NOTICES.md",
               "README.md":ROOT/"README.md","docs/performance.md":ROOT/"docs/performance.md",
               "WebView2Loader.dll":binary.with_name("WebView2Loader.dll")}
        if host=="x86_64-pc-windows-gnullvm":
            sysroot=Path(subprocess.check_output(["rustc","--print","sysroot"],text=True).strip())
            files["libunwind.dll"]=sysroot/"bin/libunwind.dll"
        for folder,resource in [(ROOT/"assets/Anime4K","shaders/Anime4K"),(ROOT/"assets/filters","filters")]:
            for file in folder.glob("*"):
                if file.is_file(): files[f"{resource}/{file.name}"]=file
        (payload/"components.json").write_text(json.dumps(sbom(metadata),indent=2)+"\n",encoding="utf-8")
        files["components.json"]=payload/"components.json"
        for name,file in files.items():
            if not file.is_file(): raise FileNotFoundError(file)
        # Use a separate staging tree so the bundler never copies over a source DLL.
        resources={}
        for name,file in files.items():
            if name=="rtx-player-desktop.exe" or name.startswith(("shaders/","filters/")): continue
            dest=payload/name
            dest.parent.mkdir(parents=True,exist_ok=True)
            if dest!=file: shutil.copyfile(file,dest)
            resources[str(dest)]=name
        config=scratch/"bundle.json"
        config.write_text(json.dumps({"bundle":{"resources":resources}}),encoding="utf-8")
        cli=ROOT/"node_modules/@tauri-apps/cli/tauri.js"
        # Tauri stamps the input EXE. Give it its own copy and keep Cargo's output pristine.
        bundle_target=scratch/"bundle-target"
        bundle_release=bundle_target/"release"
        bundle_release.mkdir(parents=True)
        shutil.copy2(binary,bundle_release/binary.name)
        run(["node",str(cli),"bundle","--bundles","nsis","--config",str(config)],ROOT/"desktop",
            env={**os.environ,"CARGO_TARGET_DIR":str(bundle_target)})
        built=bundle_release/"bundle/nsis"/f"RTX Player_{version}_x64-setup.exe"
        if not built.is_file(): raise FileNotFoundError(built)
        final=scratch/"artifacts"
        final.mkdir()
        marker=payload/"portable.mode"
        marker.touch()
        portable(final/(tag+".zip"),dict(files,**{"portable.mode":marker}))
        shutil.copyfile(built,final/(tag+"-setup.exe"))
        checksum="".join(f"{digest(p)}  {p.name}\n" for p in sorted(final.iterdir()))
        (final/"SHA256SUMS").write_text(checksum,encoding="ascii")
        if revision()!=commit: raise RuntimeError("The checkout changed during packaging")
        final.rename(output)
    return output


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    print(build(parser.parse_args().output.resolve()))
