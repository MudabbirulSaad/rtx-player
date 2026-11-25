use crate::engine::{Control, Engine, Snapshot};
use crate::export::Exports;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WindowEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, MoveWindow, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};

#[tauri::command]
async fn open_media(engine: State<'_, Engine>) -> Result<(), String> {
    if let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Open video")
        .pick_file()
        .await
    {
        engine.open(file.path().to_owned())?;
    }
    Ok(())
}

#[tauri::command]
fn transport(engine: State<'_, Engine>, command: Control) -> Result<(), String> {
    engine.control(command)
}

#[tauri::command]
fn playback_state(engine: State<'_, Engine>) -> Result<Snapshot, String> {
    engine.state()
}

#[tauri::command]
fn fullscreen(window: tauri::Window) -> Result<(), String> {
    window
        .set_fullscreen(!window.is_fullscreen().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn layout(size: PhysicalSize<u32>, scale: f64) -> (u32, u32) {
    let controls = ((150.0 * scale).round() as u32).clamp(1, size.height.max(1));
    (size.height.saturating_sub(controls), controls)
}

struct Surfaces {
    video: usize,
    controls: tauri::Webview,
    expanded: AtomicBool,
}
impl Surfaces {
    fn arrange(&self, size: PhysicalSize<u32>, scale: f64) -> tauri::Result<()> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let (video, controls) = if self.expanded.load(Ordering::Relaxed) {
            (0, size.height)
        } else {
            layout(size, scale)
        };
        unsafe {
            MoveWindow(self.video as _, 0, 0, size.width as i32, video as i32, 1);
        }
        self.controls.set_bounds(tauri::Rect {
            position: PhysicalPosition::new(0, video as i32).into(),
            size: PhysicalSize::new(size.width, controls).into(),
        })
    }
}

#[tauri::command]
fn expand_controls(window: tauri::Window, expanded: bool) -> Result<(), String> {
    let surfaces = window.state::<Surfaces>();
    surfaces.expanded.store(expanded, Ordering::Relaxed);
    surfaces
        .arrange(
            window.inner_size().map_err(|e| e.to_string())?,
            window.scale_factor().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
}

// SS_BLACKRECT paints the idle STATIC control black before a stream opens.
const SS_BLACKRECT: u32 = 4;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .manage(Exports::default())
        .setup(|app| {
            let window = WindowBuilder::new(app, "player")
                .title("RTX Player")
                .inner_size(1000.0, 680.0)
                .min_inner_size(580.0, 320.0)
                .build()?;
            let size = window.inner_size()?;
            let (video, controls) = layout(size, window.scale_factor()?);
            // The parent owns this child HWND. Keep the WebView and video in separate client rectangles.
            let surface = unsafe {
                CreateWindowExW(
                    0,
                    windows_sys::w!("STATIC"),
                    std::ptr::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | SS_BLACKRECT,
                    0,
                    0,
                    size.width as i32,
                    video as i32,
                    window.hwnd()?.0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            if surface.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let surface = surface as usize;
            let runtime = std::env::current_exe()?.with_file_name("libmpv-2.dll");
            let bar = window.add_child(
                WebviewBuilder::new("controls", WebviewUrl::App("index.html".into())),
                PhysicalPosition::new(0, video as i32),
                PhysicalSize::new(size.width, controls),
            )?;
            app.manage(Surfaces {
                video: surface,
                controls: bar,
                expanded: AtomicBool::new(false),
            });
            let executable = std::env::current_exe()?;
            let dir = executable
                .parent()
                .ok_or_else(|| std::io::Error::other("Missing application directory"))?;
            let profile = if dir.join("portable.mode").is_file() {
                Ok(dir.join("data"))
            } else {
                app.path().app_local_data_dir().map_err(|e| e.to_string())
            };
            let library = profile
                .and_then(|p| crate::library::Library::open(&p.join("library.sqlite3")))
                .map(Arc::new);
            app.manage(crate::library::Catalog(library.clone()));
            app.manage(Engine::start(
                runtime,
                surface,
                app.path().resolve(
                    "shaders/Anime4K/upscale-x2-s.glsl",
                    tauri::path::BaseDirectory::Resource,
                )?,
                std::env::args_os().nth(1).map(PathBuf::from),
                library.ok(),
            )?);
            let host = window.clone();
            window.on_window_event(move |event| {
                let size = match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(worker) = host.state::<Engine>().shutdown() {
                            let app = host.app_handle().clone();
                            let export = host.state::<Exports>().shutdown();
                            // Pump window messages until libmpv releases the child surface.
                            std::thread::spawn(move || {
                                if worker.join().is_err() {
                                    eprintln!("Playback worker failed during shutdown");
                                }
                                if let Some(export) = export {
                                    let _ = export.join();
                                }
                                app.exit(0);
                            });
                        }
                        return;
                    }
                    WindowEvent::Resized(s) => *s,
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => *new_inner_size,
                    WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                        if let Some(path) = paths.first()
                            && let Err(err) = host.state::<Engine>().open(path.clone())
                        {
                            eprintln!("{err}");
                        }
                        return;
                    }
                    _ => return,
                };
                if let Err(e) = host
                    .state::<Surfaces>()
                    .arrange(size, host.scale_factor().unwrap_or(1.0))
                {
                    eprintln!("Resize controls: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            expand_controls,
            crate::library::library_entries,
            crate::library::library_preferences,
            crate::library::set_library_preferences,
            crate::library::open_recent,
            crate::library::clear_history,
            crate::export::start_export,
            crate::export::export_status,
            crate::export::cancel_export,
            open_media,
            playback_state,
            transport,
            fullscreen
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_surface_stays_inside_client_at_high_dpi() {
        assert_eq!(layout(PhysicalSize::new(800, 700), 1.5), (475, 225));
        assert_eq!(layout(PhysicalSize::new(800, 100), 2.0), (0, 100));
    }
}
