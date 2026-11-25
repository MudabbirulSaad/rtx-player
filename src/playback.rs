use libloading::os::windows::Library;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::ptr::{self, NonNull};

type Handle = *mut c_void;

#[repr(C)]
struct Event {
    id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *const c_void,
}

#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}

// Function signatures and the prefix of mpv_event_end_file follow mpv/client.h (client ABI 2).
struct Api {
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    option: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(Handle, *const *const c_char) -> c_int,
    wait: unsafe extern "C" fn(Handle, f64) -> *const Event,
    error: unsafe extern "C" fn(c_int) -> *const c_char,
    destroy: unsafe extern "C" fn(Handle),
    _library: Library,
}

impl Api {
    fn load(path: &Path) -> Result<Self, String> {
        // The absolute application-owned path and system32 are the only DLL search locations.
        unsafe {
            let library = Library::load_with_flags(path, 0x100 | 0x800)
                .map_err(|e| format!("Cannot load the bundled libmpv runtime: {e}"))?;
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|_| format!("The runtime is missing {}.", $name))?
                };
            }
            let version: unsafe extern "C" fn() -> std::ffi::c_ulong =
                symbol!("mpv_client_api_version");
            if version() >> 16 != 2 {
                return Err("This build requires libmpv client ABI 2.".into());
            }
            Ok(Self {
                create: symbol!("mpv_create"),
                initialize: symbol!("mpv_initialize"),
                option: symbol!("mpv_set_option_string"),
                command: symbol!("mpv_command"),
                wait: symbol!("mpv_wait_event"),
                error: symbol!("mpv_error_string"),
                destroy: symbol!("mpv_terminate_destroy"),
                _library: library,
            })
        }
    }

    fn check(&self, code: c_int, operation: &str) -> Result<(), String> {
        if code < 0 {
            // mpv_error_string returns a static NUL-terminated message for every code.
            let error = unsafe { CStr::from_ptr((self.error)(code)) };
            Err(format!("{operation}: {}", error.to_string_lossy()))
        } else {
            Ok(())
        }
    }
}

pub struct Player {
    api: Api,
    handle: NonNull<c_void>,
}

impl Player {
    pub fn open(runtime: &Path) -> Result<Self, String> {
        let api = Api::load(runtime)?;
        // The owned handle is destroyed before unloading the library, including on init failure.
        let handle =
            NonNull::new(unsafe { (api.create)() }).ok_or("Could not create media engine.")?;
        let player = Self { api, handle };
        for (key, value) in [
            (c"config", c"no"),
            (c"load-scripts", c"no"),
            (c"ytdl", c"no"),
            (c"vo", c"gpu-next"),
            (c"gpu-api", c"d3d11"),
            (c"hwdec", c"auto-safe"),
            (c"input-default-bindings", c"yes"),
            (c"input-vo-keyboard", c"yes"),
            (c"title", c"RTX Player"),
        ] {
            // The handle is live and mpv copies each option before returning.
            let code =
                unsafe { (player.api.option)(handle.as_ptr(), key.as_ptr(), value.as_ptr()) };
            player.api.check(code, key.to_string_lossy().as_ref())?;
        }
        player.api.check(
            unsafe { (player.api.initialize)(handle.as_ptr()) },
            "Initialize playback",
        )?;
        Ok(player)
    }

    pub fn play(&mut self, path: &Path) -> Result<(), String> {
        let path = CString::new(path.to_str().ok_or("Unsupported file name encoding.")?)
            .map_err(|_| "Invalid file name")?;
        let args = [c"loadfile".as_ptr(), path.as_ptr(), ptr::null()];
        // All arguments stay live for the synchronous command. Events stay valid until the next wait.
        unsafe {
            self.api.check(
                (self.api.command)(self.handle.as_ptr(), args.as_ptr()),
                "Open media",
            )?;
            loop {
                let event = (self.api.wait)(self.handle.as_ptr(), -1.0)
                    .as_ref()
                    .ok_or("The media engine returned an empty event.")?;
                match event.id {
                    1 => return Ok(()), // MPV_EVENT_SHUTDOWN
                    7 => {
                        let end = event
                            .data
                            .cast::<EndFile>()
                            .as_ref()
                            .ok_or("Missing end-file information.")?;
                        if end.reason == 4 {
                            return self.api.check(end.error, "Media playback failed");
                        }
                        if end.reason != 5 {
                            return Ok(());
                        } // Continue through playlist redirects.
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // No other owner can access this handle and the library is still loaded.
        unsafe { (self.api.destroy)(self.handle.as_ptr()) }
    }
}
