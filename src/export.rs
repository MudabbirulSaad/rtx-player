//! Local SDR transcodes. The encoder never receives the final output filename.
use crate::playback::{PlaybackEvent, Player};
use std::path::Path;
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
        Ok(format!("lavfi=[{}]", filters.join(",")))
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
    let path = temporary.to_str().ok_or("Unsupported output path")?;
    let (codec, quality) = match profile.encoder {
        Encoder::Software => ("libx264", format!("crf={},preset=medium", profile.quality)),
        Encoder::Nvenc => ("h264_nvenc", format!("rc=vbr,cq={},b=0", profile.quality)),
    };
    {
        let mut writer = Player::offline(
            runtime,
            &[
                (c"o", path),
                (c"of", format),
                (c"ovc", codec),
                (c"ovcopts", &quality),
                (c"oac", "aac"),
                (c"vf", &filter),
                (c"hwdec", "no"),
                (c"sid", "no"),
                (c"audio-display", "no"),
            ],
        )?;
        writer.load(&source)?;
        let mut last = Instant::now();
        loop {
            check_cancel(cancel)?;
            match writer.event(0.1)? {
                PlaybackEvent::Ended => break,
                PlaybackEvent::Shutdown => {
                    return Err("The encoder stopped before completion.".into());
                }
                _ => (),
            }
            if last.elapsed() >= Duration::from_millis(250) {
                if let Some(pos) = writer
                    .property(c"time-pos")
                    .and_then(|s| s.parse::<f64>().ok())
                {
                    progress((pos / input.duration).clamp(0., 0.99));
                }
                last = Instant::now();
            }
        }
    }
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
