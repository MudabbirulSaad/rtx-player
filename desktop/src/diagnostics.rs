use crate::engine::{Engine, Snapshot};
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::State;

struct Startup {
    file: File,
    prior: u32,
}
impl Startup {
    fn begin(root: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(root)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(root.join("startup.state"))?;
        let mut old = String::new();
        (&mut file).take(16).read_to_string(&mut old)?;
        let prior = old
            .trim()
            .parse::<u32>()
            .unwrap_or(if old.is_empty() { 0 } else { 2 });
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        writeln!(file, "{}", prior.saturating_add(1).min(3))?;
        file.sync_data()?;
        Ok(Self { file, prior })
    }
    fn clean(mut self) -> std::io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file.write_all(b"0\n")?;
        self.file.sync_data()
    }
}
#[derive(Clone, Serialize)]
pub struct Policy {
    kib: u32,
    days: u32,
}
struct Store {
    db: Connection,
    policy: Policy,
}

impl Store {
    fn open(root: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(root.join("diagnostics.sqlite3"))?;
        db.busy_timeout(Duration::from_millis(80))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY,stamp INTEGER NOT NULL,payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),kib INTEGER NOT NULL,days INTEGER NOT NULL);
            INSERT OR IGNORE INTO settings VALUES(1,256,7);")?;
        let policy = db.query_row("SELECT kib,days FROM settings WHERE id=1", [], |r| {
            Ok(Policy {
                kib: r.get(0)?,
                days: r.get(1)?,
            })
        })?;
        let policy = if [64, 256, 1024].contains(&policy.kib) && [1, 7, 30].contains(&policy.days) {
            policy
        } else {
            Policy { kib: 256, days: 7 }
        };
        Ok(Self { db, policy })
    }
    fn trim(&self) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM events WHERE stamp<unixepoch()-?1*86400",
            [self.policy.days],
        )?;
        self.db.execute("DELETE FROM events WHERE id IN (SELECT id FROM (SELECT id,sum(length(payload)) OVER(ORDER BY id DESC) AS bytes FROM events) WHERE bytes>?1)",[self.policy.kib*1024])?;
        Ok(())
    }
    fn put(&self, entry: &Value) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT INTO events(stamp,payload) VALUES(unixepoch(),?1)",
            [entry.to_string()],
        )?;
        self.trim()
    }
    fn recent(&self) -> rusqlite::Result<Vec<Value>> {
        let mut q=self.db.prepare("SELECT payload FROM (SELECT id,payload FROM events ORDER BY id DESC LIMIT 240) ORDER BY id")?;
        q.query_map([], |r| r.get::<_, String>(0))?
            .map(|row| row.map(|s| serde_json::from_str(&s).unwrap_or(Value::Null)))
            .collect()
    }
}

pub struct Journal {
    startup: Mutex<Option<Startup>>,
    store: Mutex<Option<Store>>,
    start: Instant,
    session: u128,
    pub safe: bool,
}

impl Journal {
    pub fn open(profile: Option<&Path>, safe: bool) -> Arc<Self> {
        let startup = profile.and_then(|p| Startup::begin(p).ok());
        let previous = startup.as_ref().map_or(0, |s| s.prior);
        let store = profile.and_then(|p| Store::open(p).ok());
        let journal = Arc::new(Self {
            startup: Mutex::new(startup),
            store: Mutex::new(store),
            start: Instant::now(),
            session: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros(),
            safe: safe || previous >= 2,
        });
        journal.event("startup",json!({"previous_unclean_exits":previous,"safe_mode":journal.safe,"build":env!("RTX_BUILD")}));
        journal
    }
    pub fn event(&self, kind: &str, data: Value) {
        if let Ok(store) = self.store.try_lock()
            && let Some(store) = store.as_ref()
        {
            let _=store.put(&json!({"session":self.session.to_string(),"elapsed_ms":self.start.elapsed().as_millis(),"event":kind,"data":data}));
        }
    }
    pub fn clean(&self) {
        self.event("shutdown", Value::Null);
        if let Ok(mut slot) = self.startup.lock()
            && let Some(startup) = slot.take()
        {
            let _ = startup.clean();
        }
    }
    fn recent(&self) -> Result<Vec<Value>, String> {
        self.store
            .lock()
            .map_err(|_| "Diagnostics unavailable")?
            .as_ref()
            .ok_or("Local diagnostic log unavailable")?
            .recent()
            .map_err(|e| e.to_string())
    }
}

pub fn metrics(s: &Snapshot) -> Value {
    json!({"phase":s.phase,"decoder":s.decoder,"source_fps":s.source_fps,"filter_fps":s.filtered_fps,
        "source_size":[s.source_width,s.source_height],"filter_size":[s.processed_width,s.processed_height],
        "video_area":[s.video_area_width,s.video_area_height],"drop_count":s.dropped,"av_offset_s":s.avsync,
        "cache_s":s.cache_seconds,"rife_factor":s.factor,"anime4k_requested":s.upscale,
        "rtx_accepted":s.active_rtx,"hdr_display":s.display_hdr,"link_bits":s.display_bits,
        "hdr10_swapchain":s.hdr10_swapchain,"safe_mode":s.safe_mode})
}

#[tauri::command]
pub fn diagnostic_policy(journal: State<'_, Arc<Journal>>) -> Result<Policy, String> {
    journal
        .store
        .lock()
        .map_err(|_| "Diagnostics unavailable")?
        .as_ref()
        .map(|s| s.policy.clone())
        .ok_or_else(|| "Local diagnostic log unavailable".into())
}
#[tauri::command]
pub fn configure_diagnostics(
    journal: State<'_, Arc<Journal>>,
    kib: u32,
    days: u32,
) -> Result<(), String> {
    if ![64, 256, 1024].contains(&kib) || ![1, 7, 30].contains(&days) {
        return Err("Unsupported diagnostic retention.".into());
    }
    let mut slot = journal
        .store
        .lock()
        .map_err(|_| "Diagnostics unavailable")?;
    let s = slot.as_mut().ok_or("Local diagnostic log unavailable")?;
    s.db.execute(
        "UPDATE settings SET kib=?1,days=?2 WHERE id=1",
        params![kib, days],
    )
    .map_err(|e| e.to_string())?;
    s.policy = Policy { kib, days };
    s.trim().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_diagnostics(
    journal: State<'_, Arc<Journal>>,
    engine: State<'_, Engine>,
    gpus: State<'_, crate::gpu::Gpus>,
) -> Result<bool, String> {
    let mut picker = rfd::AsyncFileDialog::new()
        .set_title("Save playback diagnostics")
        .set_file_name("rtx-player-diagnostics.json")
        .add_filter("JSON", &["json"]);
    if let Some(source) = engine.state()?.source
        && let Some(folder) = source.parent()
    {
        picker = picker.set_directory(folder);
    }
    let Some(destination) = picker.save_file().await else {
        return Ok(false);
    };
    let hardware = gpus
        .latest
        .lock()
        .map_err(|_| "GPU data unavailable")?
        .clone();
    let doc = json!({"format":1,"version":env!("CARGO_PKG_VERSION"),"build":env!("RTX_BUILD"),
        "playback":metrics(&engine.state()?),"gpu":hardware.devices,"driver":hardware.driver,
        "recent_events":journal.recent()?});
    let mut data = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
    data.push(b'\n');
    let path = destination.path();
    let folder = path.parent().ok_or("Missing destination directory")?;
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.persist_noclobber(path)
        .map_err(|e| format!("Choose a new destination filename: {}", e.error))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_unclean_exclusive_sessions_enable_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let one = Journal::open(Some(dir.path()), false);
        assert!(!one.safe);
        let concurrent = Journal::open(Some(dir.path()), false);
        assert!(!concurrent.safe);
        concurrent.clean();
        drop(one);
        let two = Journal::open(Some(dir.path()), false);
        assert!(!two.safe);
        drop(two);
        let three = Journal::open(Some(dir.path()), false);
        assert!(three.safe);
        three.clean();
        assert!(!Journal::open(Some(dir.path()), false).safe);
    }
    #[test]
    fn summary_excludes_paths_titles_and_raw_errors() {
        let s = Snapshot {
            title: Some("private video".into()),
            source: Some("secret.mkv".into()),
            error: Some("private failure".into()),
            ..Default::default()
        };
        let output = metrics(&s).to_string();
        assert!(!output.contains("private"));
        assert!(!output.contains("secret"));
    }
}
