use super::{Catalog, Library};
use rusqlite::{OptionalExtension, params};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tauri::{State, ipc::Response};

impl Library {
    fn preview(&self, app: &Path, id: i64, ordinal: i64) -> Result<Vec<u8>, String> {
        let path: String = self.with(|db| {
            db.query_row(
                "SELECT path FROM members WHERE collection_id=?1 AND ordinal=?2",
                params![id, ordinal],
                |r| r.get(0),
            )
        })?;
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        let modified = meta
            .modified()
            .map_err(|e| e.to_string())?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis() as i64;
        let bytes = i64::try_from(meta.len()).map_err(|_| "Unsupported video size")?;
        let cached=self.with(|db|db.query_row("UPDATE previews SET accessed=unixepoch() WHERE collection_id=?1 AND path=?2 AND bytes=?3 AND modified=?4 RETURNING jpeg",
            params![id,path,bytes,modified],|r|r.get::<_,Vec<u8>>(0)).optional())?;
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let jpeg = rtx_player::export::thumbnail(app, Path::new(&path))?;
        self.with(|db|{
            let tx=db.unchecked_transaction()?;
            tx.execute("INSERT OR REPLACE INTO previews(collection_id,path,bytes,modified,jpeg,accessed)
                SELECT collection_id,path,?3,?4,?5,unixepoch() FROM members WHERE collection_id=?1 AND path=?2",params![id,path,bytes,modified,&jpeg])?;
            tx.execute("DELETE FROM previews WHERE rowid IN (SELECT id FROM
                (SELECT rowid AS id,sum(length(jpeg)) OVER (ORDER BY accessed DESC,rowid DESC) AS total FROM previews) WHERE total>33554432)",[])?;
            tx.commit()
        })?;
        Ok(jpeg)
    }
}

#[tauri::command]
pub async fn collection_preview(
    catalog: State<'_, Catalog>,
    id: i64,
    ordinal: i64,
) -> Result<Response, String> {
    let db = catalog.0.clone()?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        db.preview(exe.parent().ok_or("Missing app directory")?, id, ordinal)
            .map(Response::new)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn clear_previews(catalog: State<'_, Catalog>) -> Result<(), String> {
    catalog
        .get()?
        .with(|db| db.execute("DELETE FROM previews", []).map(|_| ()))
}
