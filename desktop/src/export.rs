use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

use rtx_player::export::{Encoder, Profile, transcode};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Codec {
    Software,
    Nvenc,
    Auto,
}

#[derive(Deserialize)]
pub struct Options {
    encoder: Codec,
    height: Option<u32>,
    quality: u8,
    denoise: bool,
    #[serde(default)]
    anime: bool,
    #[serde(default)]
    hdr: bool,
    bitrate_kbps: Option<u32>,
    target_fps: Option<u32>,
}

#[derive(Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Running {
        fraction: f64,
    },
    Complete {
        encoder: String,
    },
    Cancelled,
    Error {
        message: String,
    },
}

struct Job {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct Exports {
    job: Mutex<Option<Job>>,
    status: Arc<Mutex<Status>>,
}

impl Exports {
    fn start(&self, source: PathBuf, dest: PathBuf, options: Options) -> Result<(), String> {
        let mut job = self.job.lock().map_err(|_| "Export worker unavailable")?;
        if job.as_ref().is_some_and(|j| !j.thread.is_finished()) {
            return Err("An export is already running.".into());
        }
        if let Some(previous) = job.take() {
            previous
                .thread
                .join()
                .map_err(|_| "Previous export worker stopped unexpectedly")?;
        }
        let runtime = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("libmpv-2.dll");
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let status = self.status.clone();
        let profile = Profile {
            encoder: match options.encoder {
                Codec::Software => Encoder::Software,
                Codec::Nvenc => Encoder::Nvenc,
                Codec::Auto => Encoder::Auto,
            },
            height: options.height,
            quality: options.quality,
            denoise: options.denoise,
            anime: options.anime,
            hdr: options.hdr,
            bitrate_kbps: options.bitrate_kbps,
            target_fps: options.target_fps,
        };
        let mut initial = self
            .status
            .lock()
            .map_err(|_| "Export status unavailable")?;
        let thread = std::thread::Builder::new()
            .name("export".into())
            .spawn(move || {
                let result = transcode(&runtime, &source, &dest, &profile, &flag, |fraction| {
                    if let Ok(mut state) = status.lock() {
                        *state = Status::Running { fraction }
                    }
                });
                if let Ok(mut state) = status.lock() {
                    *state = match result {
                        Ok(encoder) => Status::Complete { encoder },
                        Err(_) if flag.load(Ordering::Relaxed) => Status::Cancelled,
                        Err(message) => Status::Error { message },
                    };
                }
            })
            .map_err(|e| e.to_string())?;
        *initial = Status::Running { fraction: 0. };
        *job = Some(Job { cancel, thread });
        Ok(())
    }

    pub fn shutdown(&self) -> Option<JoinHandle<()>> {
        self.job.lock().ok()?.take().map(|job| {
            job.cancel.store(true, Ordering::Relaxed);
            job.thread
        })
    }
}

#[tauri::command]
pub async fn start_export(
    engine: State<'_, crate::engine::Engine>,
    exports: State<'_, Exports>,
    options: Options,
) -> Result<(), String> {
    let source = engine
        .state()?
        .source
        .ok_or("Open a local video before exporting.")?;
    let file = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let dest = rfd::AsyncFileDialog::new()
        .set_title("Export video")
        .set_directory(source.parent().ok_or("Source folder unavailable")?)
        .set_file_name(format!("{file}-export.mp4"))
        .add_filter("Video", &["mp4", "mkv"])
        .save_file()
        .await;
    if let Some(dest) = dest {
        exports.start(source, dest.path().to_owned(), options)?;
    }
    Ok(())
}

#[tauri::command]
pub fn export_status(exports: State<'_, Exports>) -> Result<Status, String> {
    exports
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Export status unavailable".into())
}

#[tauri::command]
pub fn cancel_export(exports: State<'_, Exports>) -> Result<(), String> {
    let job = exports
        .job
        .lock()
        .map_err(|_| "Export worker unavailable")?;
    if let Some(job) = job.as_ref() {
        job.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}
