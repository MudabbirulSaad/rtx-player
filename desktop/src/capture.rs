use crate::engine::Engine;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{State, ipc::Response};

#[tauri::command]
pub async fn save_frame(engine: State<'_, Engine>) -> Result<bool, String> {
    let source = engine
        .state()?
        .source
        .ok_or("Open a video to capture a frame.")?;
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Save video frame")
        .set_file_name("video-frame.png")
        .add_filter("PNG", &["png"]);
    let dialog = if let Some(folder) = source.parent() {
        dialog.set_directory(folder)
    } else {
        dialog
    };
    let Some(file) = dialog.save_file().await else {
        return Ok(false);
    };
    let dest = file.path().to_owned();
    if dest.exists()
        || !dest
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err("Choose a new PNG filename.".into());
    }
    let temp = tempfile::Builder::new()
        .prefix(".rtx-frame-")
        .suffix(".png")
        .tempfile_in(dest.parent().ok_or("Missing output folder")?)
        .map_err(|e| e.to_string())?
        .into_temp_path();
    let rx = engine.screenshot(source, temp.to_path_buf())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv()
            .map_err(|_| "Playback stopped during frame capture".to_owned())??;
        let mut header = [0u8; 8];
        use std::io::Read;
        std::fs::File::open(&temp)
            .and_then(|mut f| f.read_exact(&mut header))
            .map_err(|e| e.to_string())?;
        if header != *b"\x89PNG\r\n\x1a\n" {
            return Err("Renderer did not produce a PNG.".into());
        }
        temp.persist_noclobber(dest)
            .map_err(|e| e.error.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Default)]
pub struct Preview(Arc<AtomicBool>);
struct Pending(Arc<AtomicBool>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn seek_preview(
    engine: State<'_, Engine>,
    preview: State<'_, Preview>,
    seconds: f64,
) -> Result<Response, String> {
    let state = engine.state()?;
    let path = state.source.ok_or("No media")?;
    let duration = state.duration.ok_or("Duration unavailable")?;
    if !seconds.is_finite() || !(0.0..duration).contains(&seconds) {
        return Err("Preview time is outside the video.".into());
    }
    if preview.0.swap(true, Ordering::AcqRel) {
        return Err("A preview is in progress.".into());
    }
    let pending = Pending(Arc::clone(&preview.0));
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        rtx_player::export::thumbnail_at(
            exe.parent().ok_or("Missing application directory")?,
            &path,
            Some(seconds),
        )
        .map(Response::new)
    })
    .await
    .map_err(|e| e.to_string())?
}
