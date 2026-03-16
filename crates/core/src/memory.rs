use std::collections::HashMap;

use crate::error::StoreError;
use crate::store::{RoomName, RoomRecord, ServerStore, StoredMessage};

/**
 * In memory `ServerStore`
 *
 * # Examples
 * ```
 * use hoy_core::store::{RoomName, ServerStore};
 * use hoy_core::memory::InMemoryStore;
 *
 * async fn foo() {
 *     let mut store = InMemoryStore::new();
 *     let general = RoomName::new("general").unwrap();
 *     store.ensure_room(&general).await.unwrap();
 *     let rooms = store.load_rooms().await.unwrap();
 *     assert_eq!(rooms.len(), 1);
 * }
 * ```
 */
#[derive(Debug, Default)]
pub struct InMemoryStore {
    /// Maps room name -> ordered message history.
    rooms: HashMap<RoomName, Vec<StoredMessage>>,
}

#[allow(dead_code)]
impl InMemoryStore {
    /// Constructs a new, empty `InMemoryStore`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ServerStore for InMemoryStore {
    async fn ensure_room(&mut self, name: &RoomName) -> Result<(), crate::error::StoreError> {
        // `entry` is idempotent
        self.rooms.entry(name.clone()).or_default();
        Ok(())
    }

    async fn load_rooms(&self) -> Result<Vec<crate::store::RoomRecord>, crate::error::StoreError> {
        let mut records: Vec<RoomRecord> = self
            .rooms
            .keys()
            .cloned()
            .map(|name| RoomRecord { name })
            .collect();

        // stable ordering so callers get a predictable list
        records.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(records)
    }

    async fn append_message(&mut self, msg: StoredMessage) -> Result<(), crate::error::StoreError> {
        let history = self
            .rooms
            .get_mut(&msg.room)
            .ok_or_else(|| StoreError::RoomNotFound(msg.room.clone()))?;
        history.push(msg);
        Ok(())
    }

    async fn load_recent_messages(
        &self,
        room: &RoomName,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, StoreError> {
        let history = self
            .rooms
            .get(room)
            .ok_or_else(|| StoreError::RoomNotFound(room.clone()))?;
        let start = history.len().saturating_sub(limit);
        Ok(history.get(start..).unwrap_or_default().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use hoy_test::assert_matches;

    use crate::error::StoreError;
    use crate::memory::InMemoryStore;
    use crate::store::{RoomName, ServerStore, StoredMessage};

    fn general() -> RoomName {
        RoomName::new("general").expect("hardcoded name is valid")
    }

    fn username1() -> String {
        String::from("bruce_lee")
    }

    fn message(text: String) -> StoredMessage {
        StoredMessage {
            room: general(),
            from: username1(),
            text,
        }
    }

    #[tokio::test]
    async fn ensure_room_is_idempotent() {
        let mut store = InMemoryStore::new();
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
            .expect("Room storeage failed unexpectedly");
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
        let mut store = InMemoryStore::new();
        assert_matches!(
            store.append_message(message("hello".into())).await,
            Err(StoreError::RoomNotFound(_))
        );
    }

    #[tokio::test]
    async fn load_recent_respects_limit() {
        let mut store = InMemoryStore::new();
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
            recent.last().expect("Failed to retrieve message 2").text,
            "msg 9"
        );
    }

    #[tokio::test]
    async fn load_rooms_sorted() {
        let mut store = InMemoryStore::new();
        store
            .ensure_room(
                &RoomName::new("zebra").expect("RoomName construction failed unexpectedly"),
            )
            .await
            .expect("Room storage failed unexpectedly");
        store
            .ensure_room(
                &RoomName::new("alpha").expect("RoomName construction failed unexpectedly"),
            )
            .await
            .expect("Room storage failed unexpectedly");
        store
            .ensure_room(
                &RoomName::new("general").expect("RoomName construction failed unexpectedly"),
            )
            .await
            .expect("Room storage failed unexpectedly");

        let rooms = store
            .load_rooms()
            .await
            .expect("Rooms load failed unexpectedly");
        let names: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["alpha", "general", "zebra"]);
    }
}
