use rtx_player::provider::{matches_digest, verify_files};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::State;

#[derive(Deserialize)]
struct Asset {
    name: String,
    url: String,
    size: u64,
    sha256: String,
}

#[derive(Default, Clone, Serialize)]
pub struct Progress {
    running: bool,
    message: String,
}

#[derive(Default)]
pub struct Setup {
    worker: Mutex<Option<JoinHandle<()>>>,
    state: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
}
impl Setup {
    pub fn shutdown(&self) -> Option<JoinHandle<()>> {
        self.cancel.store(true, Ordering::Relaxed);
        self.worker.lock().ok()?.take()
    }
}
fn system_tool(name: &str) -> Result<Command, String> {
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 512];
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return Err("Cannot locate Windows system tools.".into());
    }
    let mut command = Command::new(PathBuf::from(String::from_utf16_lossy(&buf[..len])).join(name));
    command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}
fn run(cmd: &mut Command, cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Setup cancelled.".into());
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start a Windows setup tool: {e}"))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Setup cancelled.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "Setup tool returned {status}. Check the connection and available storage."
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
}
fn download(asset: &Asset, to: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut curl = system_tool("curl.exe")?;
    curl.args([
        "--disable",
        "--fail",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-redirs",
        "5",
        "--connect-timeout",
        "15",
        "--max-time",
        "180",
        "--max-filesize",
        &asset.size.to_string(),
        "--output",
    ])
    .arg(to)
    .arg(&asset.url);
    run(&mut curl, cancel)?;
    if fs::metadata(to).map_err(|e| e.to_string())?.len() != asset.size
        || !matches_digest(to, &asset.sha256)
    {
        return Err(format!(
            "{} failed its download integrity check.",
            asset.name
        ));
    }
    Ok(())
}
fn install(
    app: &Path,
    reader: bool,
    cancel: &AtomicBool,
    update: impl Fn(&str),
) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    let root = app.join("providers");
    fs::create_dir_all(&root).map_err(io)?;
    let target = root.join(if reader { "rife-export" } else { "rife" });
    if target.try_exists().map_err(io)? {
        return Err("The provider directory already exists. Setup keeps its contents.".into());
    }
    let scratch = tempfile::tempdir_in(&root).map_err(io)?;
    let stage = scratch.path().join("payload");
    fs::create_dir(&stage).map_err(io)?;
    let archive = scratch.path().join("download");
    if reader {
        rtx_player::provider::verify_rife(app)?;
        let m: serde_json::Value =
            serde_json::from_str(include_str!("../../build/rife-reader.json"))
                .expect("pinned reader");
        let string = |key: &str| m[key].as_str().expect("pinned string field");
        let asset = Asset {
            name: "BestSource".into(),
            url: string("source").into(),
            size: m["archive_bytes"].as_u64().expect("archive size"),
            sha256: string("archive_sha256").into(),
        };
        update("Downloading BestSource");
        download(&asset, &archive, cancel)?;
        let file = stage.join("bestsource.dll");
        run(
            system_tool("tar.exe")?
                .arg("-xOf")
                .arg(&archive)
                .arg(string("member"))
                .stdout(fs::File::create_new(&file).map_err(io)?),
            cancel,
        )?;
        if !matches_digest(&file, string("member_sha256")) {
            return Err("The extracted reader failed its integrity check.".into());
        }
    } else {
        let assets: Vec<Asset> = serde_json::from_str(include_str!("../../build/rife.json"))
            .expect("pinned RIFE assets");
        for asset in assets {
            update(&format!("Downloading {}", asset.name));
            download(&asset, &archive, cancel)?;
            if asset.name == "python" || asset.name == "vapoursynth" {
                let folder = if asset.name == "python" {
                    stage.join("python")
                } else {
                    stage.join("python/Lib/site-packages")
                };
                fs::create_dir_all(&folder).map_err(io)?;
                // Only extract archives that matched a digest compiled into this executable.
                run(
                    system_tool("tar.exe")?
                        .arg("-xf")
                        .arg(&archive)
                        .arg("-C")
                        .arg(&folder),
                    cancel,
                )?;
                fs::remove_file(&archive).map_err(io)?;
            } else {
                let file = stage.join(&asset.name);
                fs::create_dir_all(file.parent().expect("nested asset")).map_err(io)?;
                fs::rename(&archive, file).map_err(io)?;
            }
        }
        fs::write(
            stage.join("python/python314._pth"),
            b"python314.zip\r\n.\r\nLib/site-packages\r\n",
        )
        .map_err(io)?;
        update("Checking the installed runtime and model");
        verify_files(&stage)?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Setup cancelled.".into());
    }
    fs::rename(stage, target).map_err(io)
}

#[tauri::command]
pub fn setup_provider(state: State<'_, Setup>, reader: bool) -> Result<(), String> {
    let app = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Missing application folder")?
        .to_owned();
    let mut worker = state.worker.lock().map_err(|_| "Setup unavailable")?;
    if worker.as_ref().is_some_and(|w| !w.is_finished()) {
        return Err("Setup is already running.".into());
    }
    if let Some(w) = worker.take() {
        w.join()
            .map_err(|_| "Previous setup stopped unexpectedly")?;
    }
    let status = Arc::clone(&state.state);
    let cancel = Arc::clone(&state.cancel);
    cancel.store(false, Ordering::Relaxed);
    *status.lock().map_err(|_| "Setup unavailable")? = Progress {
        running: true,
        message: "Starting setup".into(),
    };
    *worker = Some(thread::spawn(move || {
        let result = install(&app, reader, &cancel, |message| {
            if let Ok(mut s) = status.lock() {
                s.message = message.into();
            }
        });
        if let Ok(mut s) = status.lock() {
            s.running = false;
            s.message = result.err().unwrap_or_else(|| {
                "Installed and verified. Restart the player to use the provider.".into()
            });
        }
    }));
    Ok(())
}
#[tauri::command]
pub fn provider_status(state: State<'_, Setup>) -> Result<Progress, String> {
    state
        .state
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Setup unavailable".into())
}
#[tauri::command]
pub fn cancel_provider(state: State<'_, Setup>) {
    state.cancel.store(true, Ordering::Relaxed);
}
