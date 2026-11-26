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
    CreateWindowExW, GetCursorPos, MoveWindow, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_VISIBLE,
};
use windows_sys::Win32::{Foundation::POINT, Graphics::Gdi::ScreenToClient};

#[tauri::command]
async fn open_media(engine: State<'_, Engine>) -> Result<(), String> {
    if let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Open video")
        .add_filter("Video files", rtx_player::media::EXTENSIONS)
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
fn playback_state(window: tauri::Window, engine: State<'_, Engine>) -> Result<Snapshot, String> {
    let mut snapshot = engine.state()?;
    let view = window.state::<Surfaces>();
    if view.fullscreen.load(Ordering::Relaxed) {
        snapshot.fullscreen = true;
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let mut pos = POINT::default();
        let (_, strip) = layout(size, scale);
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;
        if unsafe { GetCursorPos(&mut pos) != 0 && ScreenToClient(hwnd, &mut pos) != 0 } {
            let bottom = pos.x >= 0
                && pos.x < size.width as i32
                && pos.y >= size.height.saturating_sub(strip) as i32
                && pos.y < size.height as i32;
            if view.hover.swap(bottom, Ordering::Relaxed) != bottom {
                view.arrange(size, scale).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(snapshot)
}

#[tauri::command]
fn fullscreen(window: tauri::Window) -> Result<(), String> {
    let next = !window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(next).map_err(|e| e.to_string())?;
    let view = window.state::<Surfaces>();
    view.fullscreen.store(next, Ordering::Relaxed);
    view.keyboard.store(false, Ordering::Relaxed);
    view.arrange(
        window.inner_size().map_err(|e| e.to_string())?,
        window.scale_factor().map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal_controls(window: tauri::Window) -> Result<(), String> {
    let view = window.state::<Surfaces>();
    view.keyboard.store(true, Ordering::Relaxed);
    view.arrange(
        window.inner_size().map_err(|e| e.to_string())?,
        window.scale_factor().map_err(|e| e.to_string())?,
    )
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
    fullscreen: AtomicBool,
    hover: AtomicBool,
    keyboard: AtomicBool,
}
impl Surfaces {
    fn arrange(&self, size: PhysicalSize<u32>, scale: f64) -> tauri::Result<()> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let (top, strip) = layout(size, scale);
        let (video, bar_top, controls) = if self.expanded.load(Ordering::Relaxed) {
            (0, 0, size.height)
        } else if self.fullscreen.load(Ordering::Relaxed) {
            let height =
                if self.hover.load(Ordering::Relaxed) || self.keyboard.load(Ordering::Relaxed) {
                    strip
                } else {
                    1
                };
            (size.height, size.height - height, height)
        } else {
            (top, top, strip)
        };
        unsafe {
            MoveWindow(self.video as _, 0, 0, size.width as i32, video as i32, 1);
        }
        self.controls.set_bounds(tauri::Rect {
            position: PhysicalPosition::new(0, bar_top as i32).into(),
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
        .manage(crate::setup::Setup::default())
        .setup(|app| {
            app.manage(crate::gpu::Gpus::start()?);
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
                fullscreen: AtomicBool::new(false),
                hover: AtomicBool::new(false),
                keyboard: AtomicBool::new(false),
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
            let safe = std::env::args_os().skip(1).any(|arg| arg == "--safe-mode");
            let journal = crate::diagnostics::Journal::open(profile.as_deref().ok(), safe);
            let crash = Arc::clone(&journal);
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                crash.event("panic", serde_json::Value::Null);
                previous(info)
            }));
            app.manage(Arc::clone(&journal));
            let library = profile
                .and_then(|p| crate::library::Library::open_profile(&p.join("library.sqlite3")))
                .map(Arc::new);
            app.manage(crate::library::Catalog(library.clone()));
            app.manage(Engine::start(
                runtime,
                surface,
                app.path().resolve(
                    "shaders/Anime4K/upscale-x2-s.glsl",
                    tauri::path::BaseDirectory::Resource,
                )?,
                std::env::args_os()
                    .skip(1)
                    .find(|arg| arg != "--safe-mode")
                    .map(PathBuf::from),
                library.ok(),
                crate::engine::Services {
                    journal,
                    gpus: Arc::clone(&app.state::<crate::gpu::Gpus>().latest),
                },
            )?);
            let host = window.clone();
            window.on_window_event(move |event| {
                let size = match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(worker) = host.state::<Engine>().shutdown() {
                            let app = host.app_handle().clone();
                            let export = host.state::<Exports>().shutdown();
                            let setup = host.state::<crate::setup::Setup>().shutdown();
                            // Pump window messages until libmpv releases the child surface.
                            std::thread::spawn(move || {
                                if worker.join().is_err() {
                                    eprintln!("Playback worker failed during shutdown");
                                }
                                if let Some(export) = export {
                                    let _ = export.join();
                                }
                                if let Some(setup) = setup {
                                    let _ = setup.join();
                                }
                                app.state::<Arc<crate::diagnostics::Journal>>().clean();
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
            reveal_controls,
            crate::library::library_entries,
            crate::library::library_preferences,
            crate::library::set_library_preferences,
            crate::library::open_recent,
            crate::library::clear_history,
            crate::library::collections::list_collections,
            crate::library::collections::collection_items,
            crate::library::collections::create_playlist,
            crate::library::collections::import_folder,
            crate::library::collections::refresh_folder,
            crate::library::collections::add_playlist_files,
            crate::library::collections::play_collection,
            crate::library::collections::remove_collection,
            crate::library::collections::remove_playlist_item,
            crate::library::previews::collection_preview,
            crate::library::previews::clear_previews,
            crate::export::start_export,
            crate::tracks::open_subtitle,
            crate::gpu::gpu_stats,
            crate::diagnostics::export_diagnostics,
            crate::diagnostics::diagnostic_policy,
            crate::diagnostics::configure_diagnostics,
            crate::settings::set_player_options,
            crate::setup::setup_provider,
            crate::setup::provider_status,
            crate::setup::cancel_provider,
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
