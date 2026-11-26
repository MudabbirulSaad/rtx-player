use std::path::Path;

pub const EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "3gp", "3g2", "mkv", "webm", "avi", "mpg", "mpeg", "vob", "ts", "mts",
    "m2ts", "wmv", "asf", "flv", "ogv", "mxf", "ivf",
];

pub fn demuxer(path: &Path) -> Result<&'static str, String> {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    Ok(match extension.as_str() {
        "mp4" | "m4v" | "mov" | "3gp" | "3g2" => "mov",
        "mkv" | "webm" => "matroska",
        "mpg" | "mpeg" | "vob" => "mpeg",
        "ts" | "mts" | "m2ts" => "mpegts",
        "avi" => "avi",
        "asf" | "wmv" => "asf",
        "flv" => "flv",
        "ogv" => "ogg",
        "mxf" => "mxf",
        "ivf" => "ivf",
        _ => {
            return Err(
                "Select a supported local video container, such as MP4, MKV, WebM or AVI.".into(),
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_registered_containers_reach_a_media_demuxer() {
        for ext in EXTENSIONS {
            assert!(demuxer(&Path::new("video").with_extension(ext)).is_ok());
        }
        assert_eq!(demuxer(Path::new("Video.WeBm")).unwrap(), "matroska");
        for name in [
            "filter.vpy",
            "playlist.m3u",
            "clip.avs",
            "script.lua",
            "file",
        ] {
            assert!(demuxer(Path::new(name)).is_err());
        }
    }
}
