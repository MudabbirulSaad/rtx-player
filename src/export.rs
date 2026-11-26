//! Verified local video exports. The encoder never receives the final output filename.
mod frames;
use crate::playback::{PlaybackEvent, Player};
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    Software,
    Nvenc,
    Auto,
}

pub struct Profile {
    pub encoder: Encoder,
    pub height: Option<u32>,
    pub quality: u8,
    pub denoise: bool,
    pub anime: bool,
    pub hdr: bool,
    pub bitrate_kbps: Option<u32>,
    pub target_fps: Option<u32>,
}

impl Profile {
    fn filter(&self) -> Result<String, String> {
        if self.quality > 51 {
            return Err("Encoding quality must be between 0 and 51.".into());
        }
        if self
            .target_fps
            .is_some_and(|rate| ![48, 60, 72, 96, 120, 144].contains(&rate))
        {
            return Err("Choose a supported export frame rate.".into());
        }
        if self
            .bitrate_kbps
            .is_some_and(|n| !(128..=200_000).contains(&n))
        {
            return Err("Choose a bitrate between 128 and 200000 kb/s.".into());
        }
        let mut filters = Vec::new();
        if self.denoise {
            filters.push("hqdn3d".to_owned());
        }
        if let Some(h) = self.height {
            if !(144..=4320).contains(&h) || h % 2 != 0 {
                return Err("Output height must be even and between 144 and 4320.".into());
            }
            if !self.anime && !self.hdr {
                filters.push(format!("scale=-2:{h}:flags=lanczos"));
            }
        }
        if self.anime || self.hdr {
            let height = self.height.map(|h| h.to_string()).unwrap_or_else(|| {
                if self.anime {
                    "ih*2".into()
                } else {
                    "ih".into()
                }
            });
            let mut gpu = format!("libplacebo=w=2*round(oh*iw/ih/2):h={height}");
            if self.anime {
                gpu.push_str(":custom_shader_path=shaders/Anime4K/upscale-x2-s.glsl");
            }
            if self.hdr {
                gpu.push_str(":format=yuv420p10le:colorspace=bt2020nc:color_primaries=bt2020:color_trc=smpte2084:range=tv:inverse_tonemapping=1");
            } else {
                gpu.push_str(":format=yuv420p");
            }
            filters.push(gpu);
        } else {
            filters.push("format=yuv420p".into());
        }
        Ok(filters.join(","))
    }
}

struct Media {
    duration: f64,
    width: u32,
    height: u32,
    audio: bool,
    gamma: String,
    primaries: Option<String>,
    pixel_format: Option<String>,
    fps: Option<f64>,
    matrix: Option<String>,
    levels: Option<String>,
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
            width: probe
                .property(c"video-params/w")
                .and_then(|v| v.parse().ok())
                .ok_or("Unknown source width")?,
            height,
            audio,
            gamma,
            primaries: probe.property(c"video-params/primaries"),
            pixel_format: probe.property(c"video-params/pixelformat"),
            fps: probe
                .property(c"container-fps")
                .and_then(|s| s.parse().ok()),
            matrix: probe.property(c"video-params/colormatrix"),
            levels: probe.property(c"video-params/colorlevels"),
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
    input: Stdio,
    cancel: &AtomicBool,
    mut report: impl FnMut(u64),
) -> Result<(), String> {
    let mut child = command
        .creation_flags(0x08000000)
        .stdin(input)
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
) -> Result<String, String> {
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
        return Err("These export profiles require an SDR source.".into());
    }
    let height = profile.height.unwrap_or(if profile.anime {
        input.height.saturating_mul(2)
    } else {
        input.height
    });
    if height > 4320
        || f64::from(height) * f64::from(input.width) / f64::from(input.height) > 8192.0
    {
        return Err("Choose an export resolution within 8192 × 4320.".into());
    }
    if profile.anime && f64::from(height) <= f64::from(input.height) * 1.2 {
        return Err("Choose an output height above 1.2× the source for Anime4K.".into());
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
    let application = runtime.parent().ok_or("Missing application directory")?;
    if profile.target_fps.is_some() {
        crate::provider::verify_rife_export(application)?;
    }
    let encoder = application.join("tools/export/ffmpeg.exe");
    let command_for = |hardware: bool| {
        let mut command = Command::new(&encoder);
        command.current_dir(application);
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
            .arg(&source);
        let video = if profile.target_fps.is_some() {
            // Y4M carries the chroma layout but has limited color metadata.
            let matrix = match input.matrix.as_deref() {
                Some("bt.601") => "smpte170m",
                Some("bt.2020-ncl") => "bt2020nc",
                _ => "bt709",
            };
            let primaries = match input.primaries.as_deref() {
                Some("bt.601-525") => "smpte170m",
                Some("bt.601-625") => "bt470bg",
                Some("bt.2020") => "bt2020",
                _ => "bt709",
            };
            let trc = if input.gamma == "srgb" {
                "iec61966-2-1"
            } else {
                "bt709"
            };
            command.args([
                "-f",
                "yuv4mpegpipe",
                "-protocol_whitelist",
                "pipe",
                "-colorspace",
                matrix,
                "-color_primaries",
                primaries,
                "-color_trc",
                trc,
                "-color_range",
                if input.levels.as_deref() == Some("full") {
                    "pc"
                } else {
                    "tv"
                },
                "-i",
                "pipe:0",
            ]);
            "1:v:0"
        } else {
            "0:V:0"
        };
        command.args([
            "-map",
            video,
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
        let codec = match (profile.hdr, hardware) {
            (true, true) => "hevc_nvenc",
            (true, false) => "libx265",
            (false, true) => "h264_nvenc",
            (false, false) => "libx264",
        };
        command.args(["-c:v", codec]);
        if profile.hdr {
            command.args([
                "-profile:v",
                "main10",
                "-color_trc",
                "smpte2084",
                "-color_primaries",
                "bt2020",
                "-colorspace",
                "bt2020nc",
            ]);
            if format == "mp4" {
                command.args(["-tag:v", "hvc1"]);
            }
        }
        if let Some(rate) = profile.bitrate_kbps {
            command.args(["-b:v", &format!("{rate}k")]);
        } else if hardware {
            command.args([
                "-rc",
                "vbr",
                "-cq",
                &profile.quality.to_string(),
                "-b:v",
                "0",
            ]);
        } else {
            command.args(["-crf", &profile.quality.to_string()]);
        }
        command
            .args(["-f", format, "-y"])
            .arg(temporary.as_os_str());
        (command, codec)
    };
    let mut report =
        |micros| progress((micros as f64 / 1_000_000.0 / input.duration).clamp(0., 0.99));
    let mut encode = |hardware| -> Result<&'static str, String> {
        check_cancel(cancel)?;
        let mut frames = profile
            .target_fps
            .map(|fps| frames::Frames::start(application, &source, fps))
            .transpose()?;
        let stdin = frames
            .as_mut()
            .map(frames::Frames::output)
            .unwrap_or_else(Stdio::null);
        let (command, codec) = command_for(hardware);
        let result = run_encoder(command, stdin, cancel, &mut report);
        if let Some(frames) = &mut frames {
            frames.finish(result)?
        } else {
            result?
        }
        Ok(codec)
    };
    let codec = match encode(profile.encoder != Encoder::Software) {
        Ok(codec) => codec,
        Err(e) => {
            let nvenc_error = e.contains("[h264_nvenc") || e.contains("[hevc_nvenc");
            if profile.encoder != Encoder::Auto || !nvenc_error {
                return Err(e);
            }
            check_cancel(cancel)?;
            encode(false)?
        }
    };
    let output = inspect(runtime, &temporary, cancel)?;
    if (output.duration - input.duration).abs() > 0.25
        || output.height != height
        || (input.audio && !output.audio)
        || profile.target_fps.is_some_and(|rate| {
            !output
                .fps
                .is_some_and(|actual| (actual - f64::from(rate)).abs() < 0.1)
        })
    {
        return Err(
            "The encoded file failed duration, resolution, frame rate or audio verification."
                .into(),
        );
    }
    if profile.hdr
        && (output.gamma != "pq"
            || output.primaries.as_deref() != Some("bt.2020")
            || !output
                .pixel_format
                .as_deref()
                .is_some_and(|v| v.contains("10")))
    {
        return Err("The encoded file failed 10-bit BT.2020/PQ verification.".into());
    }
    check_cancel(cancel)?;
    temporary
        .persist_noclobber(destination)
        .map_err(|e| e.error.to_string())?;
    progress(1.0);
    Ok(codec.to_owned())
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
            anime: false,
            hdr: false,
            bitrate_kbps: None,
            target_fps: None,
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
            anime: false,
            hdr: false,
            bitrate_kbps: None,
            target_fps: None,
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
