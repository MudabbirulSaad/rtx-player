use super::{Catalog, Library};
use crate::engine::Engine;
use rusqlite::params;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

const MAX_ITEMS: usize = 5000;
use rtx_player::media::EXTENSIONS;

#[derive(Serialize)]
pub struct Collection {
    id: i64,
    name: String,
    is_folder: bool,
    count: i64,
}
#[derive(Serialize)]
pub struct Item {
    ordinal: i64,
    title: String,
    bytes: i64,
    duration: f64,
}
struct MediaFile {
    path: PathBuf,
    title: String,
    bytes: i64,
}

fn media_file(path: &Path) -> Result<Option<MediaFile>, String> {
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
    {
        return Ok(None);
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.file_type().is_file() {
        return Ok(None);
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let title = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Some(MediaFile {
        path,
        title,
        bytes: i64::try_from(meta.len()).map_err(|_| "Unsupported file size")?,
    }))
}

fn folder_files(path: &Path) -> Result<Vec<MediaFile>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(file) = media_file(&entry.path())? {
            files.push(file)
        }
        if files.len() > MAX_ITEMS {
            return Err("Choose a folder with at most 5000 videos.".into());
        }
    }
    files.sort_by_cached_key(|f| f.title.to_lowercase());
    Ok(files)
}

impl Library {
    fn collections(&self) -> Result<Vec<Collection>, String> {
        self.with(|db| {
            let mut q=db.prepare("SELECT id,name,folder IS NOT NULL,count(m.path) FROM collections c LEFT JOIN members m ON c.id=m.collection_id GROUP BY c.id ORDER BY c.name COLLATE NOCASE,c.id")?;
            q.query_map([],|r|Ok(Collection{id:r.get(0)?,name:r.get(1)?,is_folder:r.get(2)?,count:r.get(3)?}))?.collect()
        })
    }
    fn insert_collection(
        &self,
        name: &str,
        folder: Option<&Path>,
        files: &[MediaFile],
    ) -> Result<i64, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err("Choose a collection name between 1 and 120 characters.".into());
        }
        self.with(|db|{
            let tx=db.unchecked_transaction()?;
            let id:i64=tx.query_row("INSERT INTO collections(name,folder) VALUES(?1,?2) ON CONFLICT(folder) DO UPDATE SET name=excluded.name RETURNING id",
                params![name,folder.map(|p|p.to_string_lossy())],|r|r.get(0))?;
            tx.execute("DELETE FROM members WHERE collection_id=?1",[id])?;
            append(&tx,id,files)?;
            tx.commit()?;
            Ok(id)
        })
    }
    fn playlist_add(&self, id: i64, files: &[MediaFile]) -> Result<(), String> {
        self.with(|db| {
            let tx = db.unchecked_transaction()?;
            tx.query_row(
                "SELECT id FROM collections WHERE id=?1 AND folder IS NULL",
                [id],
                |_| Ok(()),
            )?;
            append(&tx, id, files)?;
            tx.commit()
        })
    }
    fn members(&self, id: i64, search: &str) -> Result<Vec<Item>, String> {
        self.with(|db|{
            let mut q=db.prepare("SELECT m.ordinal,m.title,m.bytes,COALESCE(media.duration,0) FROM members m LEFT JOIN media ON media.path=m.path WHERE collection_id=?1 AND instr(lower(m.title),lower(?2))>0 ORDER BY m.ordinal")?;
            q.query_map(params![id,search],|r|Ok(Item{ordinal:r.get(0)?,title:r.get(1)?,bytes:r.get(2)?,duration:r.get(3)?}))?.collect()
        })
    }
    fn queue(&self, id: i64, start: i64) -> Result<Vec<PathBuf>, String> {
        self.with(|db| {
            let mut q = db.prepare(
                "SELECT path FROM members WHERE collection_id=?1 AND ordinal>=?2 ORDER BY ordinal",
            )?;
            q.query_map(params![id, start], |r| {
                r.get::<_, String>(0).map(PathBuf::from)
            })?
            .collect()
        })
    }
}
fn append(db: &rusqlite::Connection, id: i64, files: &[MediaFile]) -> rusqlite::Result<()> {
    let (mut ordinal, mut count): (i64, i64) = db.query_row(
        "SELECT COALESCE(MAX(ordinal)+1,0),count(*) FROM members WHERE collection_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut add=db.prepare("INSERT INTO members(collection_id,ordinal,path,title,bytes) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(collection_id,path) DO NOTHING")?;
    for file in files {
        let inserted = add.execute(params![
            id,
            ordinal,
            file.path.to_string_lossy(),
            file.title,
            file.bytes
        ])?;
        ordinal += inserted as i64;
        count += inserted as i64;
        if count > MAX_ITEMS as i64 {
            return Err(rusqlite::Error::InvalidParameterName(
                "A playlist holds at most 5000 videos".into(),
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_collections(catalog: State<'_, Catalog>) -> Result<Vec<Collection>, String> {
    catalog.get()?.collections()
}
#[tauri::command]
pub fn collection_items(
    catalog: State<'_, Catalog>,
    id: i64,
    search: String,
) -> Result<Vec<Item>, String> {
    catalog.get()?.members(id, &search)
}
#[tauri::command]
pub fn create_playlist(catalog: State<'_, Catalog>, name: String) -> Result<i64, String> {
    catalog.get()?.insert_collection(&name, None, &[])
}
#[tauri::command]
pub async fn import_folder(
    catalog: State<'_, Catalog>,
    engine: State<'_, Engine>,
) -> Result<Option<i64>, String> {
    let db = catalog.0.clone()?;
    let mut picker = rfd::AsyncFileDialog::new().set_title("Add video folder");
    if let Some(source) = engine.state()?.source
        && let Some(parent) = source.parent()
    {
        picker = picker.set_directory(parent);
    }
    let Some(folder) = picker.pick_folder().await else {
        return Ok(None);
    };
    let path = folder.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let folder = path.canonicalize().map_err(|e| e.to_string())?;
        let files = folder_files(&folder)?;
        let title = folder
            .file_name()
            .unwrap_or(folder.as_os_str())
            .to_string_lossy();
        db.insert_collection(&title, Some(&folder), &files)
            .map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn refresh_folder(catalog: State<'_, Catalog>, id: i64) -> Result<(), String> {
    let db = catalog.0.clone()?;
    tauri::async_runtime::spawn_blocking(move || {
        let (name, path): (String, String) = db.with(|conn| {
            conn.query_row(
                "SELECT name,folder FROM collections WHERE id=?1 AND folder IS NOT NULL",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })?;
        let files = folder_files(Path::new(&path))?;
        // A concurrent removal should stay removed.
        db.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.query_row(
                "SELECT id FROM collections WHERE id=?1 AND name=?2 AND folder=?3",
                params![id, name, path],
                |_| Ok(()),
            )?;
            tx.execute("DELETE FROM members WHERE collection_id=?1", [id])?;
            append(&tx, id, &files)?;
            tx.commit()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn add_playlist_files(catalog: State<'_, Catalog>, id: i64) -> Result<(), String> {
    let db = catalog.0.clone()?;
    let Some(files) = rfd::AsyncFileDialog::new()
        .add_filter("Videos", EXTENSIONS)
        .pick_files()
        .await
    else {
        return Ok(());
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut selected = Vec::new();
        for file in files {
            if let Some(item) = media_file(file.path())? {
                selected.push(item)
            }
        }
        db.playlist_add(id, &selected)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn play_collection(
    catalog: State<'_, Catalog>,
    engine: State<'_, Engine>,
    id: i64,
    start: i64,
) -> Result<(), String> {
    engine.playlist(catalog.get()?.queue(id, start)?)
}
#[tauri::command]
pub fn remove_collection(catalog: State<'_, Catalog>, id: i64) -> Result<(), String> {
    catalog.get()?.with(|db| {
        db.execute("DELETE FROM collections WHERE id=?1", [id])
            .map(|_| ())
    })
}
#[tauri::command]
pub fn remove_playlist_item(
    catalog: State<'_, Catalog>,
    id: i64,
    ordinal: i64,
) -> Result<(), String> {
    catalog.get()?.with(|db|db.execute("DELETE FROM members WHERE collection_id=?1 AND ordinal=?2 AND EXISTS(SELECT 1 FROM collections WHERE id=?1 AND folder IS NULL)",params![id,ordinal]).map(|_|()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collections_survive_reopen_and_history_cleanup() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("B.mp4"), b"b").unwrap();
        std::fs::write(folder.path().join("A.mkv"), b"a").unwrap();
        std::fs::write(folder.path().join("personal.txt"), b"x").unwrap();
        std::fs::create_dir(folder.path().join("sub")).unwrap();
        std::fs::write(folder.path().join("sub/C.mp4"), b"c").unwrap();
        let files = folder_files(folder.path()).unwrap();
        assert_eq!(files.len(), 2);
        let path = folder.path().join("library.sqlite3");
        let db = Library::open(&path).unwrap();
        let id = db.insert_collection("Evening", None, &[]).unwrap();
        db.playlist_add(id, &files).unwrap();
        db.playlist_add(id, &files).unwrap();
        db.clear().unwrap();
        drop(db);
        let reopened = Library::open(&path).unwrap();
        assert_eq!(reopened.collections().unwrap()[0].count, 2);
        assert_eq!(reopened.members(id, ".MKV").unwrap()[0].title, "A.mkv");
        assert!(reopened.queue(id, 1).unwrap()[0].ends_with("B.mp4"));
        assert!(reopened.entries("").unwrap().is_empty());
    }
}
