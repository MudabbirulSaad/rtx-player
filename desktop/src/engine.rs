use rtx_player::playback::{PlaybackEvent, Player};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub title: Option<String>,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    pub volume: f64,
    pub speed: f64,
    pub decoder: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Control {
    Toggle,
    Seek(f64),
    Volume(f64),
    Speed(f64),
    Mute,
}

impl Control {
    fn apply(self, p: &mut Player) -> Result<(), String> {
        match self {
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
    Open(PathBuf),
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
    pub fn start(
        runtime: PathBuf,
        surface: usize,
        initial: Option<PathBuf>,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel(16);
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = snapshot.clone();
        let thread = std::thread::Builder::new()
            .name("playback".into())
            .spawn(move || {
                let result = run(runtime, surface, rx, &shared);
                if let Err(error) = result
                    && let Ok(mut s) = shared.lock()
                {
                    s.error = Some(error);
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
        self.send(Request::Open(
            path.canonicalize().map_err(|e| e.to_string())?,
        ))
    }

    pub fn control(&self, command: Control) -> Result<(), String> {
        self.send(Request::Control(command))
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
    rx: mpsc::Receiver<Request>,
    shared: &Mutex<Snapshot>,
) -> Result<(), String> {
    let mut player = Player::embedded(&runtime, surface)?;
    let mut last = Instant::now();
    let mut error = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Request::Open(path)) => {
                error = player.load(&path).err();
                if error.is_none() {
                    error = player.command(&["set", "pause", "no"]).err();
                }
            }
            Ok(Request::Control(command)) => {
                if let Err(message) = command.apply(&mut player) {
                    error = Some(message);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for _ in 0..32 {
            match player.event(0.0) {
                Ok(PlaybackEvent::Idle) => break,
                Ok(PlaybackEvent::Shutdown) => return Ok(()),
                Ok(_) => (),
                Err(message) => {
                    error = Some(message);
                    break;
                }
            }
        }
        if last.elapsed() >= Duration::from_millis(200) {
            let number = |key| {
                player
                    .property(key)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
            };
            let state = Snapshot {
                title: player.property(c"media-title"),
                position: number(c"time-pos"),
                duration: number(c"duration"),
                paused: player.property(c"pause").as_deref() == Some("yes"),
                volume: number(c"volume").unwrap_or(100.0),
                speed: number(c"speed").unwrap_or(1.0),
                decoder: player.property(c"hwdec-current"),
                error: error.clone(),
            };
            *shared.lock().map_err(|_| "Playback state unavailable")? = state;
            last = Instant::now();
        }
    }
}
