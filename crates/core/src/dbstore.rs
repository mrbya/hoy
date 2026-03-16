//! SQLite-backed [`ServerStore`](crate::store::ServerStore) implementation.

use std::path::PathBuf;

use directories::ProjectDirs;
use sqlx::{Row, SqlitePool};

use crate::error::StoreError;
use crate::store::{RoomName, RoomRecord, ServerStore, StoredMessage};

/**
 * SQLite-backed [`ServerStore`].
 *
 * Persists room definitions and message history to a `SQLite` database file.
 * Runs schema migrations automatically on [`DbStore::new`].
 *
 * # Example
 *
 * ```no_run
 * use hoy_core::dbstore::DbStore;
 * use hoy_core::store::{RoomName, ServerStore};
 *
 * # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
 *     let store = DbStore::new(None).await?;
 *     let general = RoomName::new("general").expect("Hardcoded room name is valid");
 *     let messages = store.load_recent_messages(&general, 50).await?;
 *     println!("{messages:?}");
 *     Ok(())
 * }
 */
#[derive(Debug, Clone)]
pub struct DbStore {
    /// Underlying connection pool.
    pool: SqlitePool,
}

impl DbStore {
    /**
     * Opens (or creates) a `SQLite` database at `url` and runs pending migrations.
     *
     * # Arguments
     * - `url`: `SQLite` connection URL, e.g. `sqlite://hoy.db?mode=rwc`.
     *
     * # Returns
     * `Ok(DbStore)` on success.
     *
     * # Errors
     * Returns [`StoreError`] if:
     * - the database cannot be opened,
     * - migrations fail to apply.
     */
    pub async fn new(path: Option<PathBuf>) -> Result<Self, StoreError> {
        let db_path: PathBuf = if let Some(alt_path) = path {
            std::fs::create_dir_all(&alt_path)?;
            alt_path
        } else {
            let dirs = ProjectDirs::from("com", "hoy", "hoy").ok_or(StoreError::NoDataDirectory)?;
            let data_dir = dirs.data_dir();
            std::fs::create_dir_all(data_dir)?;
            data_dir.join("hoy.db")
        };

        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        Ok(Self { pool })
    }

    /**
     * Fetches the room index for a given [`RoomName`]
     *
     * # Arguments
     * - `room`: room name.
     *
     * # Returns
     * - `Ok(Some(id))` if room name indexed in db,
     * - `Ok(None)` if room not found in db.
     *
     * # Errors
     * Returns [`StoreError`] if the db query fails.
     */
    pub async fn fetch_room_id(&self, room: &RoomName) -> Result<Option<i64>, StoreError> {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM rooms WHERE name = ?")
            .bind(room.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        Ok(id)
    }
}

impl ServerStore for DbStore {
    async fn ensure_room(&mut self, name: &RoomName) -> Result<(), StoreError> {
        sqlx::query("INSERT OR IGNORE INTO rooms (name) VALUES (?)")
            .bind(name.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn load_rooms(&self) -> Result<Vec<RoomRecord>, StoreError> {
        let rows = sqlx::query("SELECT name FROM rooms ORDER BY name ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        rows.iter()
            .map(|row| {
                let name: &str = row
                    .try_get("name")
                    .map_err(|e| StoreError::Internal(e.to_string()))?;
                Ok(RoomRecord {
                    name: RoomName::new(name)?,
                })
            })
            .collect()
    }

    async fn append_message(&mut self, msg: StoredMessage) -> Result<(), StoreError> {
        let room_id = &self.fetch_room_id(&msg.room).await?;
        let id = room_id.ok_or_else(|| StoreError::RoomNotFound(msg.room.clone()))?;

        sqlx::query("INSERT INTO messages (room_id, from_user, text) VALUES (?, ?, ?)")
            .bind(id)
            .bind(&msg.from)
            .bind(&msg.text)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn load_recent_messages(
        &self,
        room: &RoomName,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, StoreError> {
        let room_id = &self.fetch_room_id(room).await?;
        let id = room_id.ok_or_else(|| StoreError::RoomNotFound(room.clone()))?;
        let limit_safe = i64::try_from(limit).map_err(|e| StoreError::Internal(e.to_string()))?;

        let rows = sqlx::query(
            "SELECT from_user, text
            FROM messages
            WHERE room_id = ?
            ORDER BY id DESC
            LIMIT ?",
        )
        .bind(id)
        .bind(limit_safe)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Internal(e.to_string()))?;

        let mut messages: Vec<StoredMessage> = rows
            .iter()
            .map(|row| -> Result<StoredMessage, StoreError> {
                Ok(StoredMessage {
                    room: room.clone(),
                    from: row
                        .try_get("from_user")
                        .map_err(|e| StoreError::Internal(e.to_string()))?,
                    text: row
                        .try_get("text")
                        .map_err(|e| StoreError::Internal(e.to_string()))?,
                })
            })
            .collect::<Result<_, _>>()?;

        messages.reverse();
        Ok(messages)
    }
}
