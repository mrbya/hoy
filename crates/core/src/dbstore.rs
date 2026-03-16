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

    /// Path to db storage file.
    db_path: PathBuf,
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
            let Some(filepath) = alt_path.parent() else {
                return Err(StoreError::NoDataDirectory);
            };
            std::fs::create_dir_all(filepath)?;
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

        Ok(Self { pool, db_path })
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
     * Returns [`StoreError`]if the db query fails.
     */
    pub async fn fetch_room_id(&self, room: &RoomName) -> Result<Option<i64>, StoreError> {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM rooms WHERE name = ?")
            .bind(room.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Internal(e.to_string()))?;

        Ok(id)
    }

    /**
     * Closes a currently open `SQLite` database.
     */
    pub async fn close(&self) {
        SqlitePool::close(&self.pool).await;
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

    fn storage_slug(&self) -> String {
        format!("SQLite DB @ {}", &self.db_path.display())
    }
}

#[cfg(test)]
mod tests {
    use hoy_test::assert_matches;
    use tempfile::TempDir;

    use crate::dbstore::DbStore;
    use crate::error::StoreError;
    use crate::store::{RoomName, ServerStore, StoredMessage};

    async fn setup() -> (DbStore, TempDir) {
        let dir = TempDir::new().expect("temp dir creation failed unexpectedly");
        let store = DbStore::new(Some(dir.path().join("hoy.db")))
            .await
            .expect("Db opening failed unexpectedly");
        (store, dir)
    }

    fn general() -> RoomName {
        RoomName::general()
    }

    fn username() -> String {
        String::from("bruce_lee")
    }

    fn room_name(name: &str) -> RoomName {
        RoomName::new(String::from(name)).expect("Hardcoded room name is valid")
    }

    fn message(text: String) -> StoredMessage {
        StoredMessage {
            room: general(),
            from: username(),
            text,
        }
    }

    #[tokio::test]
    async fn ensure_room_is_idempotent() {
        let (mut store, _dir) = setup().await;
        store
            .ensure_room(&general())
            .await
            .expect("Room storage failed unexpectedly");

        assert_eq!(
            store
                .load_rooms()
                .await
                .expect("Room load failed unexpectedly")
                .len(),
            1
        );

        store
            .ensure_room(&general())
            .await
            .expect("Room storage failed unexpectedly");

        assert_eq!(
            store
                .load_rooms()
                .await
                .expect("Room load failed unexpectedly")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn append_requires_room_to_exist() {
        let (mut store, _dir) = setup().await;
        let room = RoomName::new("nonexistent").expect("Hardcoded room name is valid");
        let res = store
            .append_message(StoredMessage {
                room,
                from: username(),
                text: String::from("hello"),
            })
            .await;
        assert_matches!(res, Err(StoreError::RoomNotFound(_)));
    }

    #[tokio::test]
    async fn load_requires_room_to_exist() {
        let (store, _dir) = setup().await;
        let room = room_name("nonexistent");
        let res = store.load_recent_messages(&room, 5).await;
        assert_matches!(res, Err(StoreError::RoomNotFound(_)));
    }

    #[tokio::test]
    async fn load_recent_respects_limit() {
        let (mut store, _dir) = setup().await;
        let room = general();

        store
            .ensure_room(&room)
            .await
            .expect("Room storage failed unexpectedly");

        for i in 0..10_u32 {
            store
                .append_message(message(format!("msg {i}")))
                .await
                .expect("Message append failed unexpectedly");
        }

        let recent = store
            .load_recent_messages(&room, 3)
            .await
            .expect("Loading messages failed unexpectedly");

        assert_eq!(recent.len(), 3);
        assert_eq!(
            recent.first().expect("Failed to retrieve message 0").text,
            "msg 7"
        );
        assert_eq!(
            recent.last().expect("Failed to retrieve message 3").text,
            "msg 9"
        );
    }

    #[tokio::test]
    async fn load_rooms_sorted() {
        let (mut store, _dir) = setup().await;
        store
            .ensure_room(&room_name("zebra"))
            .await
            .expect("Room storage failed unexpectedly");
        store
            .ensure_room(&room_name("alpha"))
            .await
            .expect("Room storage failed unexpectedly");
        store
            .ensure_room(&room_name("general"))
            .await
            .expect("Room storage failed unexpectedly");

        let rooms = store
            .load_rooms()
            .await
            .expect("Load rooms failed unexpectedly");
        let names: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["alpha", "general", "zebra"]);
    }

    #[tokio::test]
    async fn storage_persistence() {
        let (mut store, dir) = setup().await;
        let room = general();
        store
            .ensure_room(&room)
            .await
            .expect("Room storage failed unexpectedly");
        store
            .ensure_room(&room_name("room2"))
            .await
            .expect("Room storage failed unexpectedly");

        for i in 0..10_u32 {
            store
                .append_message(message(format!("msg {i}")))
                .await
                .expect("Message append failed unexpectedly");
        }

        store.close().await;

        let store2 = DbStore::new(Some(dir.path().join("hoy.db")))
            .await
            .expect("Paralel failed");

        let rooms = store2
            .load_rooms()
            .await
            .expect("Failed to load rooms from a new connection");
        let messages = store2
            .load_recent_messages(&room, 20)
            .await
            .expect("Failed to load messages form a new connection");
        assert_eq!(rooms.len(), 2);
        assert_eq!(
            rooms
                .first()
                .expect("Failed to retrieve first room")
                .name
                .as_str(),
            room.as_str()
        );
        assert_eq!(messages.len(), 10);
        assert_eq!(
            messages
                .first()
                .expect("Failed to retrieve first message")
                .text,
            "msg 0"
        );
    }
}
