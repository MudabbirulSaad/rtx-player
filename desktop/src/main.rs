#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod adaptive;
mod desktop;
mod diagnostics;
mod engine;
mod export;
mod gpu;
mod input;
mod library;
mod rife;
mod rtx;
mod settings;
mod tracks;
fn main() -> tauri::Result<()> {
    // Set the optional script runtime before Tauri or the playback worker starts.
    let provider = std::env::current_exe()?
        .with_file_name("providers")
        .join("rife/python/Lib/site-packages/vapoursynth/vsscript.dll");
    unsafe { std::env::set_var("VSSCRIPT_PATH", provider) };
    desktop::run()
}
