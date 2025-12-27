use gibberish_events::Activity;
use gibberish_transcript::{Transcript, TranscriptRepository};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                duration_ms INTEGER,
                audio_path TEXT,
                transcript_json TEXT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS activities (
                id TEXT PRIMARY KEY,
                activity_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL,
                parent_id TEXT,
                content_json TEXT NOT NULL,
                FOREIGN KEY (parent_id) REFERENCES activities(id)
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
            CREATE INDEX IF NOT EXISTS idx_activities_timestamp ON activities(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_activities_parent ON activities(parent_id);
            "#,
        )?;
        Ok(())
    }
}

impl TranscriptRepository for Database {
    type Error = StorageError;

    fn save(&self, transcript: &Transcript) -> Result<()> {
        let json = serde_json::to_string(transcript)?;
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, title, created_at, updated_at, duration_ms, transcript_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                transcript.id.to_string(),
                &transcript.title,
                transcript.created_at.timestamp(),
                transcript.updated_at.timestamp(),
                transcript.duration_ms as i64,
                json,
            ),
        )?;
        Ok(())
    }

    fn get(&self, id: &Uuid) -> Result<Transcript> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let json: String = conn
            .query_row(
                "SELECT transcript_json FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StorageError::NotFound(format!("transcript {id}"))
                }
                other => StorageError::DatabaseError(other),
            })?;
        Ok(serde_json::from_str(&json)?)
    }

    fn list(&self) -> Result<Vec<Transcript>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT transcript_json FROM sessions ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        })?;

        let mut transcripts = Vec::new();
        for row in rows {
            let json = row?;
            if let Ok(t) = serde_json::from_str(&json) {
                transcripts.push(t);
            }
        }
        Ok(transcripts)
    }

    fn delete(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let affected = conn.execute("DELETE FROM sessions WHERE id = ?1", [id.to_string()])?;
        if affected == 0 {
            return Err(StorageError::NotFound(format!("transcript {id}")));
        }
        Ok(())
    }
}

/// Repository for activity persistence.
pub trait ActivityRepository {
    type Error;
    fn save_activity(&self, activity: &Activity) -> std::result::Result<(), Self::Error>;
    fn get_activities(&self, limit: usize) -> std::result::Result<Vec<Activity>, Self::Error>;
    fn delete_activity(&self, id: &str) -> std::result::Result<(), Self::Error>;
    fn clear_activities(&self) -> std::result::Result<(), Self::Error>;
}

impl ActivityRepository for Database {
    type Error = StorageError;

    fn save_activity(&self, activity: &Activity) -> Result<()> {
        let content_json = serde_json::to_string(&activity.content)?;
        let activity_type = serde_json::to_string(&activity.activity_type)?;
        let status = serde_json::to_string(&activity.status)?;

        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO activities (id, activity_type, timestamp, status, parent_id, content_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                &activity.id,
                activity_type.trim_matches('"'),
                activity.timestamp,
                status.trim_matches('"'),
                &activity.parent_id,
                content_json,
            ),
        )?;
        Ok(())
    }

    fn get_activities(&self, limit: usize) -> Result<Vec<Activity>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, activity_type, timestamp, status, parent_id, content_json FROM activities ORDER BY timestamp DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit as i64], |row| {
            let id: String = row.get(0)?;
            let activity_type_str: String = row.get(1)?;
            let timestamp: i64 = row.get(2)?;
            let status_str: String = row.get(3)?;
            let parent_id: Option<String> = row.get(4)?;
            let content_json: String = row.get(5)?;

            Ok((id, activity_type_str, timestamp, status_str, parent_id, content_json))
        })?;

        let mut activities = Vec::new();
        for row in rows {
            let (id, activity_type_str, timestamp, status_str, parent_id, content_json) = row?;

            // Parse activity_type
            let activity_type = serde_json::from_str(&format!("\"{}\"", activity_type_str))
                .unwrap_or(gibberish_events::ActivityType::Transcript);

            // Parse status
            let status = serde_json::from_str(&format!("\"{}\"", status_str))
                .unwrap_or(gibberish_events::ActivityStatus::Completed);

            // Parse content
            let content = serde_json::from_str(&content_json)
                .unwrap_or_default();

            activities.push(Activity {
                id,
                activity_type,
                timestamp,
                status,
                parent_id,
                content,
                expanded: None,
            });
        }
        Ok(activities)
    }

    fn delete_activity(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let affected = conn.execute("DELETE FROM activities WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(StorageError::NotFound(format!("activity {id}")));
        }
        Ok(())
    }

    fn clear_activities(&self) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute("DELETE FROM activities", [])?;
        Ok(())
    }
}
