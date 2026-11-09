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

    pub fn update_resume_position(&self, video_id: &str, resume_pos: f64) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE history SET resume_position_seconds = ?1 WHERE video_id = ?2;",
            params![resume_pos, video_id],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: u32) -> Result<Vec<HistoryItem>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT video_id, title, channel_title, thumbnail_url, duration_seconds, watched_at, resume_position_seconds
             FROM history
             ORDER BY watched_at DESC
             LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let dur: Option<i64> = row.get(4)?;
            let watched: i64 = row.get(5)?;
            Ok(HistoryItem {
                video_id: row.get(0)?,
                title: row.get(1)?,
                channel_title: row.get(2)?,
                thumbnail_url: row.get(3)?,
                duration_seconds: dur.map(|d| d as u64),
                watched_at: watched as u64,
                resume_position_seconds: row.get(6)?,
            })
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn add_subscription(&self, channel_id: &str, title: &str, avatar_url: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        conn.execute(
            "INSERT INTO subscriptions (channel_id, title, avatar_url, subscribed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(channel_id) DO UPDATE SET title = excluded.title, avatar_url = excluded.avatar_url;",
            params![channel_id, title, avatar_url, now as i64],
        )?;
        Ok(())
    }

    pub fn remove_subscription(&self, channel_id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM subscriptions WHERE channel_id = ?1;", params![channel_id])?;
        Ok(())
    }

    pub fn get_subscriptions(&self) -> Result<Vec<SubscriptionItem>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT channel_id, title, avatar_url, subscribed_at FROM subscriptions ORDER BY subscribed_at DESC;")?;
        let rows = stmt.query_map([], |row| {
            let sub_at: i64 = row.get(3)?;
            Ok(SubscriptionItem {
                channel_id: row.get(0)?,
                title: row.get(1)?,
                avatar_url: row.get(2)?,
                subscribed_at: sub_at as u64,
            })
        })?;
        let mut items = Vec::new();
        for r in rows { items.push(r?); }
        Ok(items)
    }

    pub fn is_subscribed(&self, channel_id: &str) -> Result<bool, StoreError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE channel_id = ?1;",
            params![channel_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionItem {
    pub channel_id: String,
    pub title: String,
    pub avatar_url: String,
    pub subscribed_at: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryItem {
    pub video_id: String,
    pub title: String,
    pub channel_title: String,
    pub thumbnail_url: String,
    pub duration_seconds: Option<u64>,
    pub watched_at: u64,
    pub resume_position_seconds: f64,
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

        let history = store.get_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].video_id, "v123");
        assert_eq!(history[0].resume_position_seconds, 45.5);

        store.update_resume_position("v123", 60.0).unwrap();
        let updated = store.get_history(10).unwrap();
        assert_eq!(updated[0].resume_position_seconds, 60.0);
    }
}
