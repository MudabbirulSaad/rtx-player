use rtx_player::export::{Encoder, Profile, transcode};
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
    let dest = dir.path().join("export.mkv");
    let profile = Profile {
        encoder: Encoder::Software,
        height: Some(540),
        quality: 22,
        denoise: true,
    };
    let cancel = AtomicBool::new(false);
    let mut last = 0.;
    transcode(&runtime, &source, &dest, &profile, &cancel, |v| last = v).unwrap();
    assert_eq!(last, 1.);
    assert!(std::fs::metadata(&dest).unwrap().len() > 1024);
    let other = dir.path().join("cancelled.mp4");
    let err = transcode(&runtime, &source, &other, &profile, &cancel, |_| {
        cancel.store(true, Ordering::Relaxed)
    })
    .unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
    assert!(!other.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
