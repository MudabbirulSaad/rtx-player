//! Local SDR transcodes. The encoder never receives the final output filename.
use crate::playback::{PlaybackEvent, Player};
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Encoder {
    Software,
    Nvenc,
}

pub struct Profile {
    pub encoder: Encoder,
    pub height: Option<u32>,
    pub quality: u8,
    pub denoise: bool,
}

impl Profile {
    fn filter(&self) -> Result<String, String> {
        if self.quality > 51 {
            return Err("H.264 quality must be between 0 and 51.".into());
        }
        let mut filters = Vec::new();
        if self.denoise {
            filters.push("hqdn3d".to_owned());
        }
        if let Some(h) = self.height {
            if !(144..=4320).contains(&h) || h % 2 != 0 {
                return Err("Output height must be even and between 144 and 4320.".into());
            }
            filters.push(format!("scale=-2:{h}:flags=lanczos"));
        }
        filters.push("format=yuv420p".into());
        Ok(filters.join(","))
    }
}

struct Media {
    duration: f64,
    height: u32,
    audio: bool,
    gamma: String,
}

fn inspect(runtime: &Path, path: &Path, cancel: &AtomicBool) -> Result<Media, String> {
    let mut probe = Player::offline(
        runtime,
        &[
            (c"vo", "null"),
            (c"ao", "null"),
            (c"pause", "yes"),
            (c"hwdec", "no"),
            (c"audio-display", "no"),
        ],
    )?;
    probe.load(path)?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        check_cancel(cancel)?;
        if matches!(
            probe.event(0.05)?,
            PlaybackEvent::Ended | PlaybackEvent::Shutdown
        ) {
            break;
        }
        let Some(height) = probe
            .property(c"video-params/h")
            .and_then(|v| v.parse().ok())
        else {
            continue;
        };
        let Some(duration) = probe
            .property(c"duration")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.)
        else {
            continue;
        };
        let audio = probe.property(c"audio-params/samplerate").is_some();
        if !audio && probe.property(c"current-tracks/audio/id").is_some() {
            continue;
        }
        let gamma = probe
            .property(c"video-params/gamma")
            .ok_or("Unknown video transfer function")?;
        return Ok(Media {
            duration,
            height,
            audio,
            gamma,
        });
    }
    Err("Could not inspect a finite local video stream.".into())
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Export cancelled.".into())
    } else {
        Ok(())
    }
}

fn run_encoder(
    mut command: Command,
    cancel: &AtomicBool,
    mut report: impl FnMut(u64),
) -> Result<(), String> {
    let mut child = command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start the bundled FFmpeg export tool: {e}"))?;
    let output = child.stdout.take().expect("encoder stdout is piped");
    let errors = child.stderr.take().expect("encoder stderr is piped");
    let micros = AtomicU64::new(0);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Some(t) = line
                    .strip_prefix("out_time_us=")
                    .and_then(|s| s.parse().ok())
                {
                    micros.store(t, Ordering::Relaxed);
                }
            }
        });
        let log = scope.spawn(move || {
            let mut last = Vec::new();
            let mut errors = errors;
            let mut block = [0u8; 2048];
            while let Ok(n) = errors.read(&mut block) {
                if n == 0 {
                    break;
                }
                last.extend_from_slice(&block[..n]);
                if last.len() > 4096 {
                    last.drain(..last.len() - 4096);
                }
            }
            String::from_utf8_lossy(&last).trim().to_owned()
        });
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                child.wait().map_err(|e| e.to_string())?;
                return Err("Export cancelled.".into());
            }
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => {
                    let detail = log.join().unwrap_or_default();
                    return Err(format!("Encoding failed. {detail}"));
                }
                Ok(None) => report(micros.load(Ordering::Relaxed)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    })
}

pub fn transcode(
    runtime: &Path,
    source: &Path,
    destination: &Path,
    profile: &Profile,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> Result<(), String> {
    check_cancel(cancel)?;
    let filter = profile.filter()?;
    if destination.try_exists().map_err(|e| e.to_string())? {
        return Err("The output file already exists. Choose a new filename.".into());
    }
    let extension = destination
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = match extension.as_str() {
        "mp4" => "mp4",
        "mkv" => "matroska",
        _ => return Err("Choose an MP4 or MKV output.".into()),
    };
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let input = inspect(runtime, &source, cancel)?;
    if !matches!(
        input.gamma.as_str(),
        "bt.1886" | "srgb" | "gamma2.2" | "gamma2.4"
    ) {
        return Err("This H.264 profile requires an SDR source.".into());
    }
    let parent = destination
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix(".rtx-export-")
        .suffix(".partial")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?
        .into_temp_path();
    let encoder = runtime.with_file_name("tools").join("ffmpeg.exe");
    let mut command = Command::new(encoder);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-xerror",
            "-nostats",
            "-progress",
            "pipe:1",
            "-protocol_whitelist",
            "file",
            "-i",
        ])
        .arg(&source)
        .args([
            "-map",
            "0:V:0",
            "-map",
            "0:a?",
            "-map",
            "0:s?",
            "-map_chapters",
            "0",
            "-map_metadata",
            "0",
            "-vf",
            &filter,
            "-fps_mode",
            "passthrough",
            "-c:a",
            "aac",
        ]);
    if format == "matroska" {
        command.args(["-c:s", "copy", "-map", "0:t?", "-c:t", "copy"]);
    } else {
        command.args(["-c:s", "mov_text"]);
    }
    let quality = profile.quality.to_string();
    match profile.encoder {
        Encoder::Software => command.args(["-c:v", "libx264", "-crf", &quality]),
        Encoder::Nvenc => command.args([
            "-c:v",
            "h264_nvenc",
            "-rc",
            "vbr",
            "-cq",
            &quality,
            "-b:v",
            "0",
        ]),
    };
    command
        .args(["-f", format, "-y"])
        .arg(temporary.as_os_str());
    run_encoder(command, cancel, |micros| {
        progress((micros as f64 / 1_000_000.0 / input.duration).clamp(0., 0.99))
    })?;
    let output = inspect(runtime, &temporary, cancel)?;
    if (output.duration - input.duration).abs() > 0.25
        || output.height != profile.height.unwrap_or(input.height)
        || (input.audio && !output.audio)
    {
        return Err("The encoded file failed duration, resolution or audio verification.".into());
    }
    check_cancel(cancel)?;
    temporary
        .persist_noclobber(destination)
        .map_err(|e| e.error.to_string())?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_odd_heights_and_out_of_range_quality() {
        let mut profile = Profile {
            encoder: Encoder::Software,
            height: Some(721),
            quality: 20,
            denoise: false,
        };
        assert!(profile.filter().is_err());
        profile.height = Some(720);
        profile.quality = 52;
        assert!(profile.filter().is_err());
    }

    #[test]
    fn rejects_existing_destination_before_loading_the_engine() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.mp4");
        std::fs::write(&path, b"original media").unwrap();
        let profile = Profile {
            encoder: Encoder::Software,
            height: None,
            quality: 20,
            denoise: false,
        };
        let result = transcode(
            Path::new("missing.dll"),
            &path,
            &path,
            &profile,
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(result.unwrap_err().contains("already exists"));
        assert_eq!(std::fs::read(&path).unwrap(), b"original media");
    }
}
