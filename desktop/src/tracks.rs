use rtx_player::playback::Player;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Serialize)]
pub struct Track {
    id: i64,
    kind: String,
    title: Option<String>,
    language: Option<String>,
    codec: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct Chapter {
    index: usize,
    title: Option<String>,
    time: f64,
}
#[derive(Clone, Default, Serialize)]
pub struct Streams {
    tracks: Vec<Track>,
    chapters: Vec<Chapter>,
}

impl Streams {
    pub fn read(player: &Player) -> Self {
        let count = |name, limit| {
            player
                .property(name)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
                .min(limit)
        };
        let tracks = (0..count(c"track-list/count", 512))
            .filter_map(|i| {
                let get = |key: &str| player.named_property(&format!("track-list/{i}/{key}"));
                let kind = get("type")?;
                if kind != "audio" && kind != "sub" {
                    return None;
                }
                Some(Track {
                    id: get("id")?.parse().ok()?,
                    kind,
                    title: get("title"),
                    language: get("lang"),
                    codec: get("codec"),
                })
            })
            .collect();
        let chapters = (0..count(c"chapter-list/count", 4096))
            .filter_map(|index| {
                let get = |key: &str| player.named_property(&format!("chapter-list/{index}/{key}"));
                Some(Chapter {
                    index,
                    title: get("title"),
                    time: get("time")?.parse().ok()?,
                })
            })
            .collect();
        Self { tracks, chapters }
    }
}

#[tauri::command]
pub async fn open_subtitle(engine: tauri::State<'_, crate::engine::Engine>) -> Result<(), String> {
    if let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Add subtitles")
        .add_filter(
            "Subtitle files",
            &["srt", "ass", "ssa", "vtt", "sub", "idx", "sup"],
        )
        .pick_file()
        .await
    {
        let path: PathBuf = file.path().canonicalize().map_err(|e| e.to_string())?;
        if !path.is_file() {
            return Err("Choose a local subtitle file.".into());
        }
        engine.subtitle(path)?;
    }
    Ok(())
}
