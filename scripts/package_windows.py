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
import urllib.request
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


def dependency_notices(metadata, output):
    packages=[(p["name"],p["version"],p.get("license"),Path(p["manifest_path"]).parent,
               p.get("license_file")) for p in metadata["packages"] if p.get("source")]
    for name,p in json.loads((ROOT/"package-lock.json").read_text())["packages"].items():
        if name and not p.get("dev") and (ROOT/name).is_dir():
            packages.append((name.rsplit("node_modules/",1)[-1],p.get("version",""),p.get("license"),ROOT/name,None))
    with output.open("w",encoding="utf-8",newline="\n") as out:
        for name,version,license_id,folder,declared_file in sorted(packages,key=lambda row:row[:2]):
            out.write(f"\n{name} {version}\nDeclared licence: {license_id or 'see upstream distribution'}\n")
            texts={p for p in folder.iterdir() if p.is_file() and
                   p.name.upper().startswith(("LICENSE","LICENCE","COPYING","NOTICE","COPYRIGHT"))}
            if declared_file and (folder/declared_file).is_file():
                texts.add(folder/declared_file)
            for file in sorted(texts,key=lambda f:f.name):
                out.write(f"\n{file.name}\n{file.read_text(encoding='utf-8',errors='replace').rstrip()}\n")


def installer_template(scratch, files):
    # Tauri 2.12.6 enumerates resource directories from a hash set. Fix their order
    # before NSIS assigns string-table offsets to the install and uninstall steps.
    version=json.loads((ROOT/"node_modules/@tauri-apps/cli/package.json").read_text())["version"]
    if version!="2.12.6": raise RuntimeError("Review the installer template after updating Tauri CLI")
    url=f"https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v{version}/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi"
    template=scratch/"installer.nsi"
    with urllib.request.urlopen(url,timeout=30) as source:
        fetch_runtime.verified_copy(source,template,34027,"d00089d3b1a2c4a30d7b6e098079100cb8f25e980f84d4515aca4597b95e967d")
    content=template.read_text(encoding="utf-8")
    directories={parent.as_posix() for name in files for parent in Path(name).parents if parent!=Path(".")}
    for key,command,reverse in [("resources_dirs","CreateDirectory",False),("resources_ancestors","RMDir /REBOOTOK",True)]:
        start=content.index("{{#each "+key+"}}")
        end=content.index("{{/each}}",start)+len("{{/each}}")
        order=sorted(directories,key=lambda d:(d.count("/"),d),reverse=reverse)
        lines=[command+' "$INSTDIR\\'+name.replace("/","\\")+'"' for name in order]
        content=content[:start]+"\n  ".join(lines)+content[end:]
    template.write_text(content,encoding="utf-8")
    return template


def build(output):
    if os.name!="nt": raise OSError("Packaging requires Windows")
    if output.exists(): raise FileExistsError("Select a new artifact directory")
    def revision():
        status=subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True)
        if status.strip(): raise RuntimeError("Package a clean checkout so the revision identifies its sources")
        return subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()
    commit=revision()
    run(["npm.cmd","run","build"])
    toolchain=subprocess.check_output(["rustc","-vV"],text=True)
    host=next(line.removeprefix("host: ") for line in toolchain.splitlines() if line.startswith("host: "))
    epoch=subprocess.check_output(["git","show","-s","--format=%ct",commit],cwd=ROOT,text=True).strip()
    build_env={**os.environ,"SOURCE_DATE_EPOCH":epoch}
    metadata=json.loads(subprocess.check_output(["cargo","metadata","--format-version","1","--locked","--filter-platform",host],cwd=ROOT))
    cargo_home=Path(os.environ.get("CARGO_HOME",Path.home()/".cargo")).resolve()
    roots=[(ROOT,"/src"),(cargo_home,"/cargo"),(Path(metadata["target_directory"]),"/build")]
    encoded=os.environ.get("CARGO_ENCODED_RUSTFLAGS")
    flags=(encoded.split("\x1f") if encoded else os.environ.get("RUSTFLAGS","").split())
    for original,stable in sorted(roots,key=lambda pair:len(str(pair[0]))):
        flags.append(f"--remap-path-prefix={original}={stable}")
    build_env["CARGO_ENCODED_RUSTFLAGS"]="\x1f".join(flags)
    linker_flag="/Brepro" if host.endswith("-msvc") else "-Wl,--no-insert-timestamp"
    run(["cargo","rustc","--release","--locked","-p","rtx-player-desktop","--bin","rtx-player-desktop",
         "--","-C","link-arg="+linker_flag],env=build_env)
    binary=Path(metadata["target_directory"])/"release/rtx-player-desktop.exe"
    executable_bytes=binary.read_bytes().lower()
    for path,_ in roots:
        for name in {str(path),path.as_posix()}:
            if any(name.lower().encode(encoding) in executable_bytes for encoding in ("utf-8","utf-16-le")):
                raise RuntimeError("Release executable still contains a local build path")
    runtime=fetch_runtime.install(binary)
    encoder=fetch_export.install(binary)
    version=tomllib.loads((ROOT/"desktop/Cargo.toml").read_text())["package"]["version"]
    tag=f"RTX-Player-{version}-{commit[:12]}-windows-x64"
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        scratch=Path(temp)
        payload=scratch/"payload"
        payload.mkdir()
        files={"rtx-player-desktop.exe":binary,"libmpv-2.dll":runtime,"tools/export/ffmpeg.exe":encoder,
               "LICENSE":ROOT/"LICENSE","THIRD_PARTY_NOTICES.md":ROOT/"THIRD_PARTY_NOTICES.md",
               "README.md":ROOT/"README.md",
               "WebView2Loader.dll":binary.with_name("WebView2Loader.dll")}
        if host=="x86_64-pc-windows-gnullvm":
            sysroot=Path(subprocess.check_output(["rustc","--print","sysroot"],text=True).strip())
            files["libunwind.dll"]=sysroot/"bin/libunwind.dll"
        for doc in sorted((ROOT/"docs").rglob("*")):
            if doc.is_file() and doc.suffix in {".md",".png"}:
                files[doc.relative_to(ROOT).as_posix()]=doc
        for folder,resource in [(ROOT/"assets/Anime4K","shaders/Anime4K"),(ROOT/"assets/filters","filters")]:
            for file in folder.glob("*"):
                if file.is_file(): files[f"{resource}/{file.name}"]=file
        (payload/"components.json").write_text(json.dumps(sbom(metadata),indent=2)+"\n",encoding="utf-8")
        files["components.json"]=payload/"components.json"
        dependency_notices(metadata,payload/"DEPENDENCY-LICENSES.txt")
        files["DEPENDENCY-LICENSES.txt"]=payload/"DEPENDENCY-LICENSES.txt"
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
        # Discard filesystem timestamps from generated installer entries.
        hooks=scratch/"timestamps.nsh"
        hooks.write_text("SetDateSave off\n",encoding="ascii")
        template=installer_template(scratch,files)
        config.write_text(json.dumps({"bundle":{"resources":resources,"windows":{"nsis":{"installerHooks":str(hooks),"template":str(template)}}}}),encoding="utf-8")
        cli=ROOT/"node_modules/@tauri-apps/cli/tauri.js"
        # Tauri stamps the input EXE. Give it its own copy and keep Cargo's output pristine.
        bundle_target=scratch/"bundle-target"
        bundle_release=bundle_target/"release"
        bundle_release.mkdir(parents=True)
        shutil.copy2(binary,bundle_release/binary.name)
        run(["node",str(cli),"bundle","--bundles","nsis","--config",str(config)],ROOT/"desktop",
            env={**build_env,"CARGO_TARGET_DIR":str(bundle_target)})
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
