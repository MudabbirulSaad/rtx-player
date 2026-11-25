use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rtx_player::playback;

fn media_argument(args: impl IntoIterator<Item = PathBuf>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    let path = args.next().ok_or("Usage: rtx-player <local video file>")?;
    if args.next().is_some() {
        return Err("Open one video file at a time.".into());
    }
    if !path.is_file() {
        return Err("The selected media file could not be found.".into());
    }
    path.canonicalize().map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let media = media_argument(std::env::args_os().skip(1).map(PathBuf::from))?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = executable.parent().ok_or("Missing application directory")?;
    let mut player = playback::Player::open(&dir.join("libmpv-2.dll"))?;
    player.play(Path::new(&media))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_media_and_ambiguous_arguments() {
        assert!(media_argument([]).is_err());
        assert!(media_argument([PathBuf::from("https://example.invalid/video")]).is_err());
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(media_argument([file.clone(), file.clone()]).is_err());
        assert_eq!(
            media_argument([file.clone()]),
            Ok(file.canonicalize().unwrap())
        );
    }
}
