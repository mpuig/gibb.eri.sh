use gibberish_transcript::{Transcript, TranscriptRepository};
use rusqlite::Connection;
use std::path::PathBuf;
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
    pub fn open(path: &PathBuf) -> Result<Self> {
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
        let conn = self.conn.lock().unwrap();
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

            CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
            "#,
        )?;
        Ok(())
    }
}

impl TranscriptRepository for Database {
    type Error = StorageError;

    fn save(&self, transcript: &Transcript) -> Result<()> {
        let json = serde_json::to_string(transcript)?;
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let json: String = conn
            .query_row(
                "SELECT transcript_json FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StorageError::NotFound(format!("transcript {}", id))
                }
                other => StorageError::DatabaseError(other),
            })?;
        Ok(serde_json::from_str(&json)?)
    }

    fn list(&self) -> Result<Vec<Transcript>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM sessions WHERE id = ?1", [id.to_string()])?;
        if affected == 0 {
            return Err(StorageError::NotFound(format!("transcript {}", id)));
        }
        Ok(())
    }
}
