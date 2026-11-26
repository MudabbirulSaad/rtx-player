use rtx_player::export::{Encoder, Profile, transcode};
use std::path::Path;
use std::process::Command;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[test]
#[ignore = "requires the pinned libmpv DLL and a local SDR test clip with audio"]
fn transcode_and_cancel_real_media() {
    let runtime =
        PathBuf::from(std::env::var_os("RTX_TEST_RUNTIME").expect("Set RTX_TEST_RUNTIME"));
    let source = PathBuf::from(std::env::var_os("RTX_TEST_MEDIA").expect("Set RTX_TEST_MEDIA"));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("export.mp4");
    let profile = Profile {
        encoder: Encoder::Software,
        height: Some(540),
        quality: 22,
        denoise: true,
        anime: false,
        hdr: false,
        bitrate_kbps: None,
        target_fps: None,
    };
    let cancel = AtomicBool::new(false);
    let mut last = 0.;
    transcode(&runtime, &source, &dest, &profile, &cancel, |v| last = v).unwrap();
    assert_eq!(last, 1.);
    assert!(std::fs::metadata(&dest).unwrap().len() > 1024);
    let ffmpeg = runtime.with_file_name("tools").join("export/ffmpeg.exe");
    let thumbnail = rtx_player::export::thumbnail(runtime.parent().unwrap(), &source).unwrap();
    assert!(thumbnail.starts_with(&[0xff, 0xd8]) && thumbnail.ends_with(&[0xff, 0xd9]));
    assert!(thumbnail.len() > 1024 && thumbnail.len() <= 128 * 1024);
    assert_eq!(
        decoded_frames(&ffmpeg, &dest),
        decoded_frames(&ffmpeg, &source)
    );
    let other = dir.path().join("cancelled.mp4");
    let err = transcode(&runtime, &source, &other, &profile, &cancel, |_| {
        cancel.store(true, Ordering::Relaxed)
    })
    .unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
    assert!(!other.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn decoded_frames(ffmpeg: &Path, media: &Path) -> usize {
    let result = Command::new(ffmpeg)
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-nostats",
            "-progress",
            "pipe:1",
            "-i",
        ])
        .arg(media)
        .args([
            "-map",
            "0:v:0",
            "-fps_mode",
            "passthrough",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            line.strip_prefix("frame=")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .next_back()
        .filter(|count| *count > 0)
        .expect("decoded frame count")
}

#[test]
#[ignore = "requires a Vulkan GPU, NVENC, Anime4K and a finite SDR test clip"]
fn encode_neural_upscale_and_hdr_with_nvenc() {
    let runtime = PathBuf::from(std::env::var_os("RTX_TEST_RUNTIME").unwrap());
    let source = PathBuf::from(std::env::var_os("RTX_TEST_MEDIA").unwrap());
    let folder = tempfile::tempdir().unwrap();
    let dest = folder.path().join("enhanced.mp4");
    let profile = Profile {
        encoder: Encoder::Nvenc,
        height: None,
        quality: 23,
        denoise: false,
        anime: true,
        hdr: true,
        bitrate_kbps: None,
        target_fps: None,
    };
    let codec = transcode(
        &runtime,
        &source,
        &dest,
        &profile,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(codec, "hevc_nvenc");
    let ffmpeg = runtime.with_file_name("tools").join("export/ffmpeg.exe");
    assert_eq!(
        decoded_frames(&ffmpeg, &source),
        decoded_frames(&ffmpeg, &dest)
    );
}

#[test]
#[ignore = "requires 24 fps CFR fixture, RIFE export reader, Vulkan and NVENC"]
fn interpolate_every_frame_into_a_60_fps_hdr_export() {
    let runtime = PathBuf::from(std::env::var_os("RTX_TEST_RUNTIME").unwrap());
    let source = PathBuf::from(std::env::var_os("RTX_TEST_MEDIA").unwrap());
    let folder = tempfile::tempdir().unwrap();
    let dest = folder.path().join("combined.mp4");
    let profile = Profile {
        encoder: Encoder::Nvenc,
        height: None,
        quality: 24,
        denoise: false,
        anime: true,
        hdr: true,
        bitrate_kbps: None,
        target_fps: Some(60),
    };
    assert_eq!(
        transcode(
            &runtime,
            &source,
            &dest,
            &profile,
            &AtomicBool::new(false),
            |_| {}
        )
        .unwrap(),
        "hevc_nvenc"
    );
    let ffmpeg = runtime.with_file_name("tools").join("export/ffmpeg.exe");
    assert_eq!(
        decoded_frames(&ffmpeg, &dest),
        decoded_frames(&ffmpeg, &source) * 5 / 2
    );
    let flag = AtomicBool::new(false);
    let cancelled = folder.path().join("cancelled.mkv");
    let result = transcode(&runtime, &source, &cancelled, &profile, &flag, |_| {
        flag.store(true, Ordering::Relaxed)
    });
    assert!(result.is_err());
    assert!(!cancelled.exists());
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
}
