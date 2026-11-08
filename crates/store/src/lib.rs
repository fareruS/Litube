use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Migration failed from version {0} to {1}: {2}")]
    Migration(u32, u32, String),
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    /// Open SQLite database at `path` (or in-memory if ":memory:") with WAL mode enabled.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let conn = if path.as_ref().to_str() == Some(":memory:") {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };

        // 1. Enforce performance pragma settings per project spec
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;",
        )?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        store.migrate()?;
        Ok(store)
    }

    /// Run versioned migrations using PRAGMA user_version
    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();

        let current_version: u32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if current_version < 1 {
            conn.execute_batch(
                "BEGIN TRANSACTION;

                 CREATE TABLE IF NOT EXISTS settings (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );

                 CREATE TABLE IF NOT EXISTS history (
                     video_id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     channel_title TEXT NOT NULL,
                     thumbnail_url TEXT NOT NULL,
                     duration_seconds INTEGER,
                     watched_at INTEGER NOT NULL,
                     resume_position_seconds REAL DEFAULT 0.0
                 );

                 CREATE TABLE IF NOT EXISTS subscriptions (
                     channel_id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     avatar_url TEXT NOT NULL,
                     subscribed_at INTEGER NOT NULL
                 );

                 CREATE TABLE IF NOT EXISTS watch_later (
                     video_id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     channel_title TEXT NOT NULL,
                     thumbnail_url TEXT NOT NULL,
                     added_at INTEGER NOT NULL
                 );

                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }

        Ok(())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1;")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn record_history(
        &self,
        video_id: &str,
        title: &str,
        channel_title: &str,
        thumbnail_url: &str,
        duration: Option<u64>,
        resume_pos: f64,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let dur_i64 = duration.map(|d| d as i64);
        let now_i64 = now as i64;

        conn.execute(
            "INSERT INTO history (video_id, title, channel_title, thumbnail_url, duration_seconds, watched_at, resume_position_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(video_id) DO UPDATE SET
                watched_at = excluded.watched_at,
                resume_position_seconds = excluded.resume_position_seconds;",
            params![video_id, title, channel_title, thumbnail_url, dur_i64, now_i64, resume_pos],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_migrations_and_settings() {
        let store = Store::open(":memory:").expect("open in-memory db");

        store.set_setting("quality", "1080p").unwrap();
        assert_eq!(store.get_setting("quality").unwrap().as_deref(), Some("1080p"));
        assert_eq!(store.get_setting("non_existent").unwrap(), None);

        store
            .record_history("v123", "Test Title", "Test Channel", "https://thumb", Some(120), 45.5)
            .unwrap();
    }
}
