//! Local history and transport preferences. Playback can run without a catalog.
pub mod collections;
pub mod previews;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;

pub struct Library(Mutex<Connection>);

#[derive(Serialize)]
pub struct Preferences {
    pub remember: bool,
    pub resume: bool,
    pub volume: f64,
    pub speed: f64,
}
#[derive(Serialize)]
pub struct Entry {
    id: i64,
    title: String,
    position: f64,
    duration: f64,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, String> {
        let parent = path.parent().ok_or("Missing settings directory")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(Duration::from_millis(300))
            .map_err(|e| e.to_string())?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let version: i64 = tx
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        match version{
            0=>tx.execute_batch("CREATE TABLE preferences(id INTEGER PRIMARY KEY CHECK(id=1),remember INTEGER NOT NULL DEFAULT 0,resume INTEGER NOT NULL DEFAULT 1,volume REAL NOT NULL DEFAULT 100,speed REAL NOT NULL DEFAULT 1);
                INSERT INTO preferences(id) VALUES(1);
                CREATE TABLE media(id INTEGER PRIMARY KEY,path TEXT UNIQUE NOT NULL,title TEXT NOT NULL,position REAL NOT NULL DEFAULT 0,duration REAL NOT NULL DEFAULT 0,visited INTEGER NOT NULL);
                PRAGMA user_version=1;").map_err(|e|e.to_string())?,
            1..=3=>(),
            _=>return Err("The local library was created by a newer version of RTX Player.".into())
        }
        if version < 2 {
            tx.execute_batch("CREATE TABLE collections(id INTEGER PRIMARY KEY, name TEXT NOT NULL, folder TEXT UNIQUE);
                CREATE TABLE members(collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                    ordinal INTEGER NOT NULL, path TEXT NOT NULL, title TEXT NOT NULL, bytes INTEGER NOT NULL,
                    PRIMARY KEY(collection_id, path), UNIQUE(collection_id, ordinal));
                PRAGMA user_version=2;").map_err(|e|e.to_string())?;
        }
        if version < 3 {
            tx.execute_batch("CREATE TABLE previews(collection_id INTEGER NOT NULL,path TEXT NOT NULL,
                bytes INTEGER NOT NULL,modified INTEGER NOT NULL,jpeg BLOB NOT NULL,accessed INTEGER NOT NULL,
                PRIMARY KEY(collection_id,path),FOREIGN KEY(collection_id,path) REFERENCES members(collection_id,path) ON DELETE CASCADE);
                PRAGMA user_version=3;").map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        db.pragma_update(None, "foreign_keys", true)
            .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(db)))
    }

    fn with<T>(&self, read: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let db = self.0.lock().map_err(|_| "Library unavailable")?;
        read(&db).map_err(|e| e.to_string())
    }

    pub fn preferences(&self) -> Result<Preferences, String> {
        self.with(|db| {
            db.query_row(
                "SELECT remember,resume,volume,speed FROM preferences WHERE id=1",
                [],
                |r| {
                    Ok(Preferences {
                        remember: r.get(0)?,
                        resume: r.get(1)?,
                        volume: r.get(2)?,
                        speed: r.get(3)?,
                    })
                },
            )
        })
    }
    pub fn preference(&self, remember: bool, resume: bool) -> Result<(), String> {
        self.with(|db| {
            db.execute(
                "UPDATE preferences SET remember=?1,resume=?2 WHERE id=1",
                params![remember, resume],
            )
            .map(|_| ())
        })
    }
    pub fn transport(&self, volume: f64, speed: f64) -> Result<(), String> {
        if !(0.0..=100.).contains(&volume) || !(0.25..=4.).contains(&speed) {
            return Err("Invalid transport preference".into());
        }
        self.with(|db| {
            db.execute(
                "UPDATE preferences SET volume=?1,speed=?2 WHERE id=1",
                params![volume, speed],
            )
            .map(|_| ())
        })
    }
    pub fn visit(&self, path: &Path, title: &str) -> Result<(), String> {
        self.with(|db|db.execute("INSERT INTO media(path,title,visited) SELECT ?1,?2,COALESCE((SELECT MAX(visited)+1 FROM media),1) FROM preferences WHERE id=1 AND remember=1
            ON CONFLICT(path) DO UPDATE SET title=excluded.title,visited=excluded.visited",params![path.to_string_lossy(),title]).map(|_|()))
    }
    pub fn position(&self, path: &Path, pos: f64, duration: f64) -> Result<(), String> {
        if !pos.is_finite() || !duration.is_finite() || duration <= 0. {
            return Ok(());
        }
        let pos = if pos >= duration - 3. {
            0.
        } else {
            pos.clamp(0., duration)
        };
        self.with(|db|db.execute("UPDATE media SET position=?1,duration=?2 WHERE path=?3 AND (SELECT remember FROM preferences WHERE id=1)",params![pos,duration,path.to_string_lossy()]).map(|_|()))
    }
    pub fn resume(&self, path: &Path) -> Result<Option<f64>, String> {
        self.with(|db|db.query_row("SELECT position FROM media,preferences WHERE preferences.id=1 AND remember=1 AND resume=1 AND path=?1 AND position>=3 AND position<duration-3",
            [path.to_string_lossy()], |r|r.get(0)).optional())
    }
    pub fn entries(&self, search: &str) -> Result<Vec<Entry>, String> {
        self.with(|db|{
            let mut query=db.prepare("SELECT id,title,position,duration FROM media WHERE instr(lower(title),lower(?1))>0 ORDER BY visited DESC LIMIT 200")?;
            query.query_map([search],|r|Ok(Entry{id:r.get(0)?,title:r.get(1)?,position:r.get(2)?,duration:r.get(3)?}))?.collect()
        })
    }
    fn file(&self, id: i64) -> Result<PathBuf, String> {
        self.with(|db| {
            db.query_row("SELECT path FROM media WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
        })
        .map(PathBuf::from)
    }
    pub fn clear(&self) -> Result<(), String> {
        self.with(|db| db.execute("DELETE FROM media", []).map(|_| ()))
    }
}

pub struct Catalog(pub Result<Arc<Library>, String>);
impl Catalog {
    fn get(&self) -> Result<&Library, String> {
        self.0.as_deref().map_err(Clone::clone)
    }
}

#[tauri::command]
pub fn library_entries(catalog: State<'_, Catalog>, search: String) -> Result<Vec<Entry>, String> {
    catalog.get()?.entries(&search)
}
#[tauri::command]
pub fn library_preferences(catalog: State<'_, Catalog>) -> Result<Preferences, String> {
    catalog.get()?.preferences()
}
#[tauri::command]
pub fn set_library_preferences(
    catalog: State<'_, Catalog>,
    engine: State<'_, crate::engine::Engine>,
    remember: bool,
    resume: bool,
) -> Result<(), String> {
    let db = catalog.get()?;
    db.preference(remember, resume)?;
    if remember {
        let state = engine.state()?;
        if let (Some(path), Some(title)) = (state.source, state.title) {
            db.visit(&path, &title)?;
        }
    }
    Ok(())
}
#[tauri::command]
pub fn clear_history(catalog: State<'_, Catalog>) -> Result<(), String> {
    catalog.get()?.clear()
}
#[tauri::command]
pub fn open_recent(
    catalog: State<'_, Catalog>,
    engine: State<'_, crate::engine::Engine>,
    id: i64,
) -> Result<(), String> {
    engine.open(catalog.get()?.file(id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_opt_in_and_resume_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("library.sqlite3");
        let lib = Library::open(&file).unwrap();
        let media = Path::new("film.mp4");
        lib.visit(media, "Film").unwrap();
        assert!(lib.entries("Film").unwrap().is_empty());
        lib.preference(true, true).unwrap();
        lib.visit(media, "Film").unwrap();
        lib.position(media, 42.0, 180.0).unwrap();
        drop(lib);
        let next = Library::open(&file).unwrap();
        let entries = next.entries("film").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(next.resume(media).unwrap(), Some(42.0));
        next.position(media, 179.0, 180.0).unwrap();
        assert_eq!(next.resume(media).unwrap(), None);
        next.clear().unwrap();
        assert!(next.entries("").unwrap().is_empty());
    }
}
