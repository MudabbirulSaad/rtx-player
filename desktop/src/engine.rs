use rtx_player::playback::{PlaybackEvent, Player};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub input: crate::input::ProcessingInput,
    pub display_fps: Option<f64>,
    pub options: crate::settings::Options,
    pub audio_devices: Vec<crate::settings::Device>,
    pub phase: &'static str,
    pub safe_mode: bool,
    pub source_width: Option<f64>,
    pub source_height: Option<f64>,
    pub cache_seconds: Option<f64>,
    pub streams: crate::tracks::Streams,
    pub audio_id: Option<i64>,
    pub subtitle_id: Option<i64>,
    pub chapter: Option<i64>,
    pub queue_size: u32,
    pub queue_index: u32,
    pub audio_delay: f64,
    pub subtitle_delay: f64,
    pub fullscreen: bool,
    #[serde(skip)]
    pub source: Option<PathBuf>,
    pub title: Option<String>,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    pub volume: f64,
    pub speed: f64,
    pub display_hdr: Option<bool>,
    pub display_bits: Option<u32>,
    pub hdr10_swapchain: Option<bool>,
    pub source_transfer: Option<String>,
    pub output_transfer: Option<String>,
    pub target_transfer: Option<String>,
    pub target_primaries: Option<String>,
    pub target_peak: Option<f64>,
    pub processed_width: Option<f64>,
    pub processed_height: Option<f64>,
    pub video_area_width: Option<f64>,
    pub video_area_height: Option<f64>,
    pub processing_scale: f64,
    pub requested_rtx: crate::rtx::Effects,
    pub active_rtx: crate::rtx::Effects,
    pub upscale: bool,
    pub rife_available: bool,
    pub rife_problem: Option<String>,
    pub rife_export_available: bool,
    pub factor: f64,
    pub requested_factor: f64,
    pub adaptive: bool,
    pub adaptation: Option<String>,
    pub source_fps: Option<f64>,
    pub avsync: Option<f64>,
    pub dropped: Option<f64>,
    pub filtered_fps: Option<f64>,
    pub decoder: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Control {
    Next,
    Previous,
    Adaptive(bool),
    AudioTrack(i64),
    SubtitleTrack(Option<i64>),
    Chapter(u32),
    AudioDelay(f64),
    SubtitleDelay(f64),
    Toggle,
    Seek(f64),
    Volume(f64),
    Speed(f64),
    Mute,
    Upscale(bool),
    Interpolate(f64),
    Input(crate::input::ProcessingInput),
    Rtx(crate::rtx::Effects),
    ProcessingScale(f64),
}

impl Control {
    fn apply(
        self,
        p: &mut Player,
        shader: &std::path::Path,
        rife: &mut crate::rife::Rife,
        rtx: &mut crate::rtx::VideoProcessor,
        surface: usize,
    ) -> Result<(), String> {
        match self {
            Self::Next => p.command(&["playlist-next", "weak"]),
            Self::Previous => p.command(&["playlist-prev", "weak"]),
            Self::Adaptive(_) => Ok(()),
            Self::AudioTrack(id) if id > 0 => p.command(&["set", "aid", &id.to_string()]),
            Self::SubtitleTrack(None) => p.command(&["set", "sid", "no"]),
            Self::SubtitleTrack(Some(id)) if id > 0 => p.command(&["set", "sid", &id.to_string()]),
            Self::Chapter(index) => p.command(&["set", "chapter", &index.to_string()]),
            Self::AudioDelay(seconds) if (-10.0..=10.0).contains(&seconds) => {
                p.command(&["set", "audio-delay", &seconds.to_string()])
            }
            Self::SubtitleDelay(seconds) if (-60.0..=60.0).contains(&seconds) => {
                p.command(&["set", "sub-delay", &seconds.to_string()])
            }
            Self::Input(next) => {
                let previous = rtx.input;
                let effects = rtx.requested;
                rtx.decoder(p, rife.factor, next)?;
                if let Err(e) = rtx.input.change(p, next) {
                    rtx.decoder(p, rife.factor, rtx.input)?;
                    return Err(e);
                }
                if effects.any()
                    && let Err(e) = rtx.set(p, effects, surface)
                {
                    rtx.input.change(p, previous)?;
                    rtx.decoder(p, rife.factor, previous)?;
                    rtx.set(p, effects, surface)?;
                    return Err(e);
                }
                Ok(())
            }
            Self::ProcessingScale(scale) => rtx.set_scale(p, scale, surface),
            Self::Rtx(settings) => rtx.set(p, settings, surface),
            Self::Interpolate(factor) => {
                let previous = rtx.requested;
                rtx.clear(p)?;
                rtx.decoder(p, factor, rtx.input)?;
                let changed = rife.set(p, factor);
                rtx.decoder(p, rife.factor, rtx.input)?;
                let restored = rtx.set(p, previous, surface);
                changed.and(restored)
            }
            Self::Upscale(true) => {
                if rtx.requested.super_resolution {
                    rtx.set(
                        p,
                        crate::rtx::Effects {
                            super_resolution: false,
                            hdr: rtx.requested.hdr,
                        },
                        surface,
                    )?;
                }
                if !shader.is_file() {
                    return Err(
                        "The Anime4K shader is missing. Reinstall the application resources."
                            .into(),
                    );
                }
                p.command(&[
                    "change-list",
                    "glsl-shaders",
                    "set",
                    shader.to_str().ok_or("Unsupported shader path")?,
                ])
            }
            Self::Upscale(false) => p.command(&["change-list", "glsl-shaders", "clr", ""]),
            Self::Toggle => p.command(&["cycle", "pause"]),
            Self::Mute => p.command(&["cycle", "mute"]),
            Self::Seek(n) if n.is_finite() && n >= 0.0 => {
                p.command(&["seek", &n.to_string(), "absolute+exact"])
            }
            Self::Volume(n) if (0.0..=100.0).contains(&n) => {
                p.command(&["set", "volume", &n.to_string()])
            }
            Self::Speed(n) if (0.25..=4.0).contains(&n) => {
                p.command(&["set", "speed", &n.to_string()])
            }
            _ => Err("Playback value is outside the supported range.".into()),
        }
    }
}

enum Request {
    Options(crate::settings::Options, mpsc::Sender<Result<(), String>>),
    Subtitle(PathBuf),
    Open(Vec<PathBuf>),
    Control(Control),
}

struct Worker {
    tx: mpsc::SyncSender<Request>,
    thread: std::thread::JoinHandle<()>,
}

pub struct Engine {
    worker: Mutex<Option<Worker>>,
    snapshot: Arc<Mutex<Snapshot>>,
}

impl Engine {
    pub fn options(
        &self,
        options: crate::settings::Options,
    ) -> Result<mpsc::Receiver<Result<(), String>>, String> {
        options.validate()?;
        let (tx, rx) = mpsc::channel();
        self.send(Request::Options(options, tx))?;
        Ok(rx)
    }
    pub fn start(
        runtime: PathBuf,
        surface: usize,
        shader: PathBuf,
        initial: Option<PathBuf>,
        library: Option<Arc<crate::library::Library>>,
        journal: Arc<crate::diagnostics::Journal>,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel(16);
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = snapshot.clone();
        let thread = std::thread::Builder::new()
            .name("playback".into())
            .spawn(move || {
                let result = run(
                    runtime,
                    surface,
                    shader,
                    rx,
                    &shared,
                    library.as_deref(),
                    &journal,
                );
                if let Err(error) = result
                    && let Ok(mut s) = shared.lock()
                {
                    s.phase = "error";
                    s.error = Some(error);
                    journal.event("engine_stopped", serde_json::Value::Null);
                }
            })
            .map_err(|e| e.to_string())?;
        let engine = Self {
            worker: Mutex::new(Some(Worker { tx, thread })),
            snapshot,
        };
        if let Some(file) = initial {
            engine.open(file)?;
        }
        Ok(engine)
    }

    pub fn open(&self, path: PathBuf) -> Result<(), String> {
        if !path.is_file() {
            return Err("Select a local media file.".into());
        }
        self.send(Request::Open(vec![
            path.canonicalize().map_err(|e| e.to_string())?,
        ]))
    }

    pub fn playlist(&self, files: Vec<PathBuf>) -> Result<(), String> {
        if files.is_empty() {
            return Err("This playlist is empty.".into());
        }
        if let Some(missing) = files.iter().find(|p| !p.is_absolute() || !p.is_file()) {
            return Err(format!(
                "Playlist file unavailable: {}",
                missing.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        self.send(Request::Open(files))
    }

    pub fn control(&self, command: Control) -> Result<(), String> {
        self.send(Request::Control(command))
    }

    pub fn subtitle(&self, path: PathBuf) -> Result<(), String> {
        self.send(Request::Subtitle(path))
    }

    fn send(&self, command: Request) -> Result<(), String> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| "Playback worker unavailable")?;
        worker
            .as_ref()
            .ok_or("Player is closing")?
            .tx
            .try_send(command)
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => "Playback controls are busy. Try again.".into(),
                mpsc::TrySendError::Disconnected(_) => "The playback engine has stopped.".into(),
            })
    }

    pub fn shutdown(&self) -> Option<std::thread::JoinHandle<()>> {
        self.worker.lock().ok()?.take().map(|worker| worker.thread)
    }

    pub fn state(&self) -> Result<Snapshot, String> {
        self.snapshot
            .lock()
            .map(|s| s.clone())
            .map_err(|_| "Playback state is unavailable.".into())
    }
}

fn run(
    runtime: PathBuf,
    surface: usize,
    shader: PathBuf,
    rx: mpsc::Receiver<Request>,
    shared: &Mutex<Snapshot>,
    library: Option<&crate::library::Library>,
    journal: &crate::diagnostics::Journal,
) -> Result<(), String> {
    let saved_options = library.map(|db| db.options()).transpose();
    let mut options = saved_options
        .as_ref()
        .ok()
        .and_then(Clone::clone)
        .unwrap_or_default();
    let adapter = if journal.safe {
        None
    } else {
        options.adapter.as_deref()
    };
    let mut player = Player::embedded_on(&runtime, surface, adapter).or_else(|e| {
        if adapter.is_some() {
            Player::embedded(&runtime, surface)
        } else {
            Err(e)
        }
    })?;
    let mut option_error = saved_options.err();
    if let Err(e) = options.apply(&mut player, journal.safe, false) {
        option_error = Some(e);
        options = Default::default();
        options.apply(&mut player, journal.safe, false)?;
    }
    if journal.safe {
        player.command(&["set", "hwdec", "no"])?;
    }
    let app = runtime.parent().ok_or("Missing application directory")?;
    let mut rife = if journal.safe {
        crate::rife::Rife::disabled(app)
    } else {
        crate::rife::Rife::new(app)
    };
    player.watch_video_processor()?;
    let mut rtx = crate::rtx::VideoProcessor::default();
    let mut display = rtx_player::display::for_window(surface).ok();
    let mut display_check = Instant::now();
    let available = rife.available();
    let export_available = available
        && runtime
            .parent()
            .is_some_and(|p| rtx_player::provider::verify_export_reader(p).is_ok());
    let mut last = Instant::now();
    let mut error = option_error;
    let mut devices = crate::settings::audio_devices(&player);
    let mut device_check = Instant::now();
    let mut loading = false;
    let mut media_failed = false;
    let mut previous_phase = "";
    let mut sampled = Instant::now();
    let mut streams = crate::tracks::Streams::default();
    let mut streams_dirty = true;
    let mut adaptive = crate::adaptive::Adaptive::default();
    adaptive.enabled = options.adaptive && !journal.safe;
    rtx.scale = options.scale;
    rtx.software_decode = journal.safe || options.software_decode;
    adaptive.reset(1.0);
    let mut hdr10_swapchain = None;
    if let Some(db) = library {
        match db.preferences() {
            Ok(prefs) => {
                player.command(&["set", "volume", &prefs.volume.to_string()])?;
                player.command(&["set", "speed", &prefs.speed.to_string()])?;
            }
            Err(e) => error = Some(e),
        }
    }
    let mut saved = Instant::now();
    let save_progress = || -> Result<(), String> {
        let snapshot = shared
            .lock()
            .map_err(|_| "Playback state unavailable")?
            .clone();
        if let (Some(db), Some(path), Some(pos), Some(duration)) = (
            library,
            snapshot.source,
            snapshot.position,
            snapshot.duration,
        ) {
            db.position(&path, pos, duration)?;
        }
        Ok(())
    };
    loop {
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Request::Options(next, reply)) => {
                let result = (|| {
                    if next.audio_device != "auto"
                        && !crate::settings::audio_devices(&player)
                            .iter()
                            .any(|d| d.name == next.audio_device)
                    {
                        return Err("The selected audio output is no longer available.".into());
                    }
                    let db = library.ok_or("Local settings storage unavailable")?;
                    next.apply(
                        &mut player,
                        journal.safe,
                        rife.factor > 1. || rtx.input != Default::default(),
                    )?;
                    rtx.set_scale(&mut player, next.scale, surface)?;
                    db.save_options(&next)?;
                    adaptive.enabled = next.adaptive && !journal.safe;
                    rtx.software_decode = journal.safe || next.software_decode;
                    options = next;
                    Ok(())
                })();
                if result.is_err() {
                    let _ = options.apply(
                        &mut player,
                        journal.safe,
                        rife.factor > 1. || rtx.input != Default::default(),
                    );
                    let _ = rtx.set_scale(&mut player, options.scale, surface);
                }
                let _ = reply.send(result);
            }
            Ok(Request::Subtitle(path)) => {
                error = path
                    .to_str()
                    .ok_or_else(|| "Unsupported subtitle path".to_owned())
                    .and_then(|s| player.command(&["sub-add", s, "select"]))
                    .err();
            }
            Ok(Request::Open(files)) => {
                loading = true;
                media_failed = false;
                adaptive.reset(1.0);
                if let Err(e) = save_progress() {
                    eprintln!("Save playback position: {e}")
                }
                if rtx.requested.any() {
                    rtx.clear(&mut player)?;
                }
                if rife.factor > 1.0 {
                    rife.set(&mut player, 1.0)?;
                }
                rtx.input.change(&mut player, Default::default())?;
                rtx.decoder(&mut player, 1., Default::default())?;
                error = (|| {
                    let (first, rest) = files.split_first().ok_or("Empty playlist")?;
                    player.load(first)?;
                    for path in rest {
                        player.command(&[
                            "loadfile",
                            path.to_str().ok_or("Unsupported playlist path")?,
                            "append",
                        ])?;
                    }
                    Ok::<_, String>(())
                })()
                .err();
                if error.is_none() {
                    error = player.command(&["set", "pause", "no"]).err();
                }
            }
            Ok(Request::Control(command)) => {
                if journal.safe
                    && matches!(
                        &command,
                        Control::Upscale(true)
                            | Control::Interpolate(_)
                            | Control::Rtx(_)
                            | Control::Input(_)
                    )
                {
                    error = Some("Safe mode disables video enhancement providers.".into());
                    continue;
                }
                let new_rate = match &command {
                    Control::Adaptive(enabled) => {
                        adaptive.enabled = *enabled;
                        None
                    }
                    Control::Interpolate(v) => Some(*v),
                    _ => None,
                };
                let preference = matches!(command, Control::Volume(_) | Control::Speed(_));
                error = command
                    .apply(&mut player, &shader, &mut rife, &mut rtx, surface)
                    .err();
                if let Some(rate) = new_rate
                    && error.is_none()
                {
                    adaptive.reset(rate);
                    if options.software_decode {
                        error = player.command(&["set", "hwdec", "no"]).err()
                    }
                }
                if preference
                    && error.is_none()
                    && let Some(db) = library
                {
                    let volume = player.property(c"volume").and_then(|s| s.parse().ok());
                    let speed = player.property(c"speed").and_then(|s| s.parse().ok());
                    if let (Some(volume), Some(speed)) = (volume, speed) {
                        error = db.transport(volume, speed).err();
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return save_progress(),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for _ in 0..32 {
            match player.event(0.0) {
                Ok(PlaybackEvent::Idle) => break,
                Ok(PlaybackEvent::Shutdown) => return Ok(()),
                Ok(PlaybackEvent::Loaded) => {
                    if let Err(e) = rtx.input.change(&mut player, Default::default()) {
                        error = Some(e);
                    }
                    loading = false;
                    media_failed = false;
                    adaptive.reset(1.0);
                    if rtx.requested.any()
                        && let Err(e) = rtx.clear(&mut player)
                    {
                        error = Some(e)
                    }
                    if rife.factor > 1.0
                        && let Err(e) = rife.set(&mut player, 1.0)
                    {
                        error = Some(e)
                    }
                    if let Err(e) = rtx.decoder(&mut player, rife.factor, rtx.input) {
                        error = Some(e);
                    }
                    streams_dirty = true;
                    if let (Some(db), Some(path)) =
                        (library, player.property(c"path").map(PathBuf::from))
                        && path.is_absolute()
                    {
                        let result = (|| {
                            let resume = db.resume(&path)?;
                            let title = player.property(c"media-title").unwrap_or_else(|| {
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned()
                            });
                            db.visit(&path, &title)?;
                            if let Some(pos) = resume {
                                player.command(&["seek", &pos.to_string(), "absolute+exact"])?;
                            }
                            Ok::<_, String>(())
                        })();
                        if let Err(e) = result {
                            error = Some(e)
                        }
                    }
                }
                Ok(PlaybackEvent::StreamsChanged) => streams_dirty = true,
                Ok(PlaybackEvent::SwapChain { hdr10 }) => hdr10_swapchain = Some(hdr10),
                Ok(PlaybackEvent::SuperResolution) => {
                    rtx.accepted.super_resolution = rtx.requested.super_resolution
                }
                Ok(PlaybackEvent::VideoHdr) => rtx.accepted.hdr = rtx.requested.hdr,
                Ok(PlaybackEvent::InterpolationFailed) if rife.factor > 1.0 => {
                    journal.event(
                        "interpolation_failed",
                        serde_json::json!({"factor":rife.factor}),
                    );
                    let recovery = Control::Interpolate(1.0).apply(
                        &mut player,
                        &shader,
                        &mut rife,
                        &mut rtx,
                        surface,
                    );
                    error = Some(match recovery {
                        Ok(()) => {
                            "RIFE stopped processing frames. Interpolation was disabled.".into()
                        }
                        Err(e) => format!("RIFE stopped and playback recovery failed: {e}"),
                    });
                }
                Ok(PlaybackEvent::ExtensionFailed) => {
                    if rtx.requested.any() {
                        journal.event(
                            "driver_extension_failed",
                            serde_json::json!({"requested":rtx.requested}),
                        );
                        let restore = rtx.clear(&mut player);
                        error = Some(restore.err().unwrap_or_else(|| {
                            "The driver rejected RTX video processing. Original video restored."
                                .into()
                        }));
                    }
                }
                Ok(_) => (),
                Err(message) => {
                    loading = false;
                    media_failed = true;
                    error = Some(message);
                    break;
                }
            }
        }
        if last.elapsed() >= Duration::from_millis(200) {
            if streams_dirty {
                streams = crate::tracks::Streams::read(&player);
                streams_dirty = false;
            }
            if display_check.elapsed() > Duration::from_secs(2) {
                display = rtx_player::display::for_window(surface).ok();
                display_check = Instant::now();
                if rtx.requested.hdr && !display.as_ref().is_some_and(|s| s.hdr_active) {
                    rtx.set(
                        &mut player,
                        crate::rtx::Effects {
                            hdr: false,
                            ..rtx.requested
                        },
                        surface,
                    )?;
                    error = Some("HDR output became unavailable. SDR playback restored.".into());
                }
            }
            let number = |key| {
                player
                    .property(key)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
            };
            let paused = player.property(c"pause").as_deref() == Some("yes");
            let position = number(c"time-pos");
            let phase = if media_failed {
                "error"
            } else if loading {
                "loading"
            } else if player.property(c"eof-reached").as_deref() == Some("yes") {
                "ended"
            } else if player.property(c"seeking").as_deref() == Some("yes") {
                "seeking"
            } else if player.property(c"paused-for-cache").as_deref() == Some("yes") {
                "buffering"
            } else if position.is_none() {
                "idle"
            } else if paused {
                "paused"
            } else {
                "playing"
            };
            if device_check.elapsed() > Duration::from_secs(8) {
                devices = crate::settings::audio_devices(&player);
                device_check = Instant::now();
            }
            let state = Snapshot {
                options: options.clone(),
                audio_devices: devices.clone(),
                phase,
                safe_mode: journal.safe,
                source_width: number(c"video-params/w"),
                source_height: number(c"video-params/h"),
                cache_seconds: number(c"demuxer-cache-duration"),
                streams: streams.clone(),
                audio_id: player
                    .property(c"current-tracks/audio/id")
                    .and_then(|s| s.parse().ok()),
                subtitle_id: player
                    .property(c"current-tracks/sub/id")
                    .and_then(|s| s.parse().ok()),
                chapter: player.property(c"chapter").and_then(|s| s.parse().ok()),
                queue_size: player
                    .property(c"playlist-count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                queue_index: player
                    .property(c"playlist-pos")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                audio_delay: number(c"audio-delay").unwrap_or(0.0),
                subtitle_delay: number(c"sub-delay").unwrap_or(0.0),
                fullscreen: false,
                source: player
                    .property(c"path")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute()),
                title: player.property(c"media-title"),
                position,
                duration: number(c"duration"),
                paused,
                volume: number(c"volume").unwrap_or(100.0),
                speed: number(c"speed").unwrap_or(1.0),
                display_hdr: display.as_ref().map(|s| s.hdr_active),
                display_bits: display.as_ref().map(|s| s.bits_per_channel),
                hdr10_swapchain,
                requested_rtx: rtx.requested,
                processing_scale: rtx.scale,
                active_rtx: rtx.accepted,
                source_transfer: player.property(c"video-params/gamma"),
                output_transfer: player.property(c"video-out-params/gamma"),
                target_transfer: player.property(c"video-target-params/gamma"),
                target_primaries: player.property(c"video-target-params/primaries"),
                target_peak: number(c"video-target-params/max-luma"),
                processed_width: number(c"video-out-params/w"),
                processed_height: number(c"video-out-params/h"),
                video_area_width: number(c"osd-dimensions/w")
                    .zip(number(c"osd-dimensions/ml"))
                    .zip(number(c"osd-dimensions/mr"))
                    .map(|((w, l), r)| w - l - r),
                video_area_height: number(c"osd-dimensions/h")
                    .zip(number(c"osd-dimensions/mt"))
                    .zip(number(c"osd-dimensions/mb"))
                    .map(|((h, t), b)| h - t - b),
                rife_available: available,
                rife_export_available: export_available,
                rife_problem: rife.problem.clone(),
                factor: rife.factor,
                requested_factor: adaptive.requested,
                adaptive: adaptive.enabled,
                adaptation: adaptive.reason.clone(),
                source_fps: number(c"container-fps"),
                input: rtx.input,
                display_fps: number(c"display-fps"),
                avsync: number(c"avsync"),
                dropped: number(c"frame-drop-count"),
                filtered_fps: number(c"estimated-vf-fps"),
                upscale: player
                    .property(c"glsl-shaders")
                    .is_some_and(|v| !v.is_empty()),
                decoder: player.property(c"hwdec-current"),
                error: error.clone(),
            };
            let change = adaptive.observe(
                Instant::now(),
                !state.paused,
                state.position,
                state.dropped,
                state.avsync,
                rife.factor,
            );
            if phase != previous_phase
                || (phase == "playing" && sampled.elapsed() > Duration::from_secs(10))
            {
                journal.event("playback", crate::diagnostics::metrics(&state));
                sampled = Instant::now();
                previous_phase = phase;
            }
            *shared.lock().map_err(|_| "Playback state unavailable")? = state;
            if let Some(rate) = change
                && let Err(e) = Control::Interpolate(rate).apply(
                    &mut player,
                    &shader,
                    &mut rife,
                    &mut rtx,
                    surface,
                )
            {
                error = Some(e);
                adaptive.enabled = false;
            }
            if saved.elapsed() >= Duration::from_secs(5) {
                if let Err(e) = save_progress() {
                    error = Some(e);
                }
                saved = Instant::now();
            }
            last = Instant::now();
        }
    }
}
