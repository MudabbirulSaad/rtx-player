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
    log_level: unsafe extern "C" fn(Handle, *const c_char) -> c_int,
    observe: unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int,
    get: unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char,
    free: unsafe extern "C" fn(*mut c_void),
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
                log_level: symbol!("mpv_request_log_messages"),
                observe: symbol!("mpv_observe_property"),
                get: symbol!("mpv_get_property_string"),
                free: symbol!("mpv_free"),
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
        Self::create(runtime, None, None, None)
    }

    /// Attach to a live, application-owned child HWND on Windows.
    pub fn embedded(runtime: &Path, window: usize) -> Result<Self, String> {
        Self::create(runtime, Some(window), None, None)
    }

    pub fn embedded_on(
        runtime: &Path,
        window: usize,
        adapter: Option<&str>,
    ) -> Result<Self, String> {
        Self::create(runtime, Some(window), None, adapter)
    }

    pub(crate) fn offline(runtime: &Path, options: &[(&CStr, &str)]) -> Result<Self, String> {
        Self::create(runtime, None, Some(options), None)
    }

    fn create(
        runtime: &Path,
        window: Option<usize>,
        offline: Option<&[(&CStr, &str)]>,
        adapter: Option<&str>,
    ) -> Result<Self, String> {
        let api = Api::load(runtime)?;
        // The owned handle is destroyed before unloading the library, including on init failure.
        let handle =
            NonNull::new(unsafe { (api.create)() }).ok_or("Could not create media engine.")?;
        let player = Self { api, handle };
        for (key, value) in [
            (c"config", c"no"),
            (c"load-scripts", c"no"),
            (c"ytdl", c"no"),
        ] {
            // The handle is live and mpv copies each option before returning.
            let code =
                unsafe { (player.api.option)(handle.as_ptr(), key.as_ptr(), value.as_ptr()) };
            player.api.check(code, key.to_string_lossy().as_ref())?;
        }
        if let Some(options) = offline {
            for &(key, value) in options {
                let value = CString::new(value).map_err(|_| "Invalid encoder setting")?;
                let result =
                    unsafe { (player.api.option)(handle.as_ptr(), key.as_ptr(), value.as_ptr()) };
                player.api.check(result, key.to_string_lossy().as_ref())?;
            }
        } else {
            for (key, value) in [
                (c"vo", c"gpu-next"),
                (c"gpu-api", c"d3d11"),
                (c"hwdec", c"auto-safe"),
                (c"input-default-bindings", c"yes"),
                (c"input-vo-keyboard", c"yes"),
                (c"title", c"RTX Player"),
            ] {
                let result =
                    unsafe { (player.api.option)(handle.as_ptr(), key.as_ptr(), value.as_ptr()) };
                player.api.check(result, key.to_string_lossy().as_ref())?;
            }
        }
        if let Some(window) = window {
            if let Some(adapter) = adapter {
                let value = CString::new(adapter).map_err(|_| "Invalid Direct3D adapter name")?;
                let result = unsafe {
                    (player.api.option)(handle.as_ptr(), c"d3d11-adapter".as_ptr(), value.as_ptr())
                };
                player.api.check(result, "Select Direct3D adapter")?;
            }
            let wid = CString::new(window.to_string()).map_err(|e| e.to_string())?;
            for (key, value) in [
                (c"wid", wid.as_c_str()),
                (c"idle", c"yes"),
                (c"keep-open", c"yes"),
                (c"osc", c"no"),
                (c"input-default-bindings", c"no"),
            ] {
                let result =
                    unsafe { (player.api.option)(handle.as_ptr(), key.as_ptr(), value.as_ptr()) };
                player.api.check(result, "Configure embedded surface")?;
            }
        }
        player.api.check(
            unsafe { (player.api.initialize)(handle.as_ptr()) },
            "Initialize playback",
        )?;
        Ok(player)
    }

    pub fn watch_video_processor(&mut self) -> Result<(), String> {
        for (tag, name) in [(1, c"track-list"), (2, c"chapter-list")] {
            self.api.check(
                unsafe { (self.api.observe)(self.handle.as_ptr(), tag, name.as_ptr(), 0) },
                "Observe media streams",
            )?;
        }
        self.api.check(
            unsafe { (self.api.log_level)(self.handle.as_ptr(), c"v".as_ptr()) },
            "Subscribe to media events",
        )
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        self.command(&[
            "loadfile",
            path.to_str().ok_or("Unsupported filename encoding")?,
        ])
    }

    pub fn command(&mut self, args: &[&str]) -> Result<(), String> {
        let strings = args
            .iter()
            .map(|arg| CString::new(*arg))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid command argument")?;
        let pointers: Vec<_> = strings
            .iter()
            .map(|s| s.as_ptr())
            .chain([ptr::null()])
            .collect();
        // mpv copies these strings during the synchronous call.
        self.api.check(
            unsafe { (self.api.command)(self.handle.as_ptr(), pointers.as_ptr()) },
            args.first().copied().unwrap_or("Media command"),
        )
    }

    pub fn property(&self, name: &CStr) -> Option<String> {
        // Client API returns an allocated string or NULL for an unavailable property.
        unsafe {
            let value = NonNull::new((self.api.get)(self.handle.as_ptr(), name.as_ptr()))?;
            let text = CStr::from_ptr(value.as_ptr())
                .to_string_lossy()
                .into_owned();
            (self.api.free)(value.as_ptr().cast());
            Some(text)
        }
    }

    pub fn named_property(&self, name: &str) -> Option<String> {
        self.property(&CString::new(name).ok()?)
    }

    pub fn event(&mut self, timeout: f64) -> Result<PlaybackEvent, String> {
        // mpv owns both event records until the next wait on this handle.
        unsafe {
            let event = (self.api.wait)(self.handle.as_ptr(), timeout)
                .as_ref()
                .ok_or("Missing playback event")?;
            match event.id {
                0 => Ok(PlaybackEvent::Idle),
                1 => Ok(PlaybackEvent::Shutdown),
                8 => Ok(PlaybackEvent::Loaded),
                22 if event.reply_userdata == 1 || event.reply_userdata == 2 => {
                    Ok(PlaybackEvent::StreamsChanged)
                }
                2 => {
                    #[repr(C)]
                    struct Message {
                        prefix: *const c_char,
                        level: *const c_char,
                        text: *const c_char,
                    }
                    let Some(msg) = event.data.cast::<Message>().as_ref() else {
                        return Ok(PlaybackEvent::Other);
                    };
                    // The runtime owns both strings until the next wait.
                    let prefix = CStr::from_ptr(msg.prefix).to_bytes();
                    let text = CStr::from_ptr(msg.text).to_string_lossy();
                    if prefix == b"vapoursynth"
                        && (text.contains("could not init VS")
                            || text.starts_with("Filter error at frame "))
                    {
                        return Ok(PlaybackEvent::InterpolationFailed);
                    }
                    if prefix == b"vo/gpu-next/libplacebo"
                        && text.contains("swap chain configuration")
                    {
                        return Ok(PlaybackEvent::SwapChain {
                            hdr10: text.contains("R10G10B10A2_UNORM")
                                && text.contains("RGB_FULL_G2084_NONE_P2020"),
                        });
                    }
                    if prefix == b"d3d11vpp" {
                        return Ok(match text.trim() {
                            "NVIDIA RTX Super Resolution enabled." => {
                                PlaybackEvent::SuperResolution
                            }
                            "NVIDIA RTX Video HDR enabled." => PlaybackEvent::VideoHdr,
                            _ if text.contains("Failed to enable NVIDIA")
                                || text.contains("NVIDIA RTX Video HDR not supported") =>
                            {
                                PlaybackEvent::ExtensionFailed
                            }
                            _ => PlaybackEvent::Other,
                        });
                    }
                    Ok(PlaybackEvent::Other)
                }
                7 => {
                    let end = event
                        .data
                        .cast::<EndFile>()
                        .as_ref()
                        .ok_or("Missing end-file information")?;
                    if end.reason == 4 {
                        self.api.check(end.error, "Media playback failed")?;
                    }
                    Ok(if end.reason == 5 {
                        PlaybackEvent::Other
                    } else {
                        PlaybackEvent::Ended
                    })
                }
                _ => Ok(PlaybackEvent::Other),
            }
        }
    }

    pub fn play(&mut self, path: &Path) -> Result<(), String> {
        self.load(path)?;
        loop {
            if matches!(
                self.event(-1.0)?,
                PlaybackEvent::Ended | PlaybackEvent::Shutdown
            ) {
                return Ok(());
            }
        }
    }
}

pub enum PlaybackEvent {
    InterpolationFailed,
    StreamsChanged,
    SwapChain { hdr10: bool },
    SuperResolution,
    VideoHdr,
    ExtensionFailed,
    Idle,
    Loaded,
    Ended,
    Shutdown,
    Other,
}
impl Drop for Player {
    fn drop(&mut self) {
        // No other owner can access this handle and the library is still loaded.
        unsafe { (self.api.destroy)(self.handle.as_ptr()) }
    }
}
