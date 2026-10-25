use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{path::Path, sync::Mutex};

#[derive(Serialize)]
pub struct SavedProgress {
    pub(crate) payload: String,
    pub(crate) revision: i64,
}

pub struct ProgressStore(Mutex<Connection>);

impl ProgressStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection =
            Connection::open(path).map_err(|_| "Could not open local progress storage.")?;
        connection.execute_batch(
            "PRAGMA journal_mode=DELETE;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS mission_progress (
               mission_id TEXT PRIMARY KEY, payload TEXT NOT NULL, revision INTEGER NOT NULL
             );"
        ).map_err(|_| "Could not prepare local progress storage. Your existing save has not been removed.")?;
        Ok(Self(Mutex::new(connection)))
    }

    pub fn load(&self, mission_id: &str) -> Result<Option<SavedProgress>, String> {
        let connection = self
            .0
            .lock()
            .map_err(|_| "Progress storage is busy; restart the app.")?;
        connection
            .query_row(
                "SELECT payload, revision FROM mission_progress WHERE mission_id = ?1",
                [mission_id],
                |row| {
                    Ok(SavedProgress {
                        payload: row.get(0)?,
                        revision: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|_| {
                "Could not read saved progress. Do not delete the save; retry after restarting."
                    .into()
            })
    }

    pub fn save(
        &self,
        mission_id: &str,
        payload: &str,
        expected_revision: i64,
    ) -> Result<i64, String> {
        if mission_id.is_empty()
            || mission_id.len() > 64
            || !mission_id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("Invalid Mission identity.".into());
        }
        if payload.len() > 128 * 1024 || serde_json::from_str::<serde_json::Value>(payload).is_err()
        {
            return Err("Invalid progress data; save was not changed.".into());
        }
        let connection = self
            .0
            .lock()
            .map_err(|_| "Progress storage is busy; restart the app.")?;
        let next = expected_revision
            .checked_add(1)
            .filter(|_| expected_revision >= 0)
            .ok_or("Invalid save revision.")?;
        let changed = connection.execute(
            "INSERT INTO mission_progress (mission_id, payload, revision) SELECT ?1, ?2, ?4 WHERE ?3 = 0
             ON CONFLICT(mission_id) DO NOTHING",
            params![mission_id, payload, expected_revision, next],
        ).map_err(|_| "Could not save progress. Check disk space and retry.")?;
        let changed = if changed == 0 {
            connection.execute("UPDATE mission_progress SET payload = ?2, revision = ?4 WHERE mission_id = ?1 AND revision = ?3",
                params![mission_id, payload, expected_revision, next])
                .map_err(|_| "Could not save progress. Check disk space and retry.")?
        } else {
            changed
        };
        if changed == 0 {
            return Err(
                "Progress changed in another window. Reopen this Mission before continuing.".into(),
            );
        }
        Ok(next)
    }
}

#[tauri::command]
pub fn load_progress(
    store: tauri::State<'_, ProgressStore>,
    mission_id: String,
) -> Result<Option<SavedProgress>, String> {
    store.load(&mission_id)
}

#[tauri::command]
pub fn save_progress(
    store: tauri::State<'_, ProgressStore>,
    mission_id: String,
    payload: String,
    expected_revision: i64,
) -> Result<i64, String> {
    store.save(&mission_id, &payload, expected_revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_survives_reopening_and_rejects_stale_writers() {
        let path =
            std::env::temp_dir().join(format!("tinkercrew-progress-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let store = ProgressStore::open(&path).unwrap();
        assert!(store.load("treasure-guardian").unwrap().is_none());
        assert_eq!(
            store
                .save("treasure-guardian", "{\"stageIndex\":2}", 0)
                .unwrap(),
            1
        );
        drop(store);
        let store = ProgressStore::open(&path).unwrap();
        let saved = store.load("treasure-guardian").unwrap().unwrap();
        assert_eq!(saved.payload, "{\"stageIndex\":2}");
        assert_eq!(saved.revision, 1);
        assert!(
            store
                .save("treasure-guardian", "{}", 0)
                .unwrap_err()
                .contains("another window")
        );
        assert_eq!(
            store
                .save("treasure-guardian", "{\"stageIndex\":3}", 1)
                .unwrap(),
            2
        );
        assert!(store.save("other", "not json", 0).is_err());
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}
