use std::fmt;

use crate::error::StoreError;

/**
 * A valid room name.
 *
 * Allowed characters: lowercase ASCII letters, Ascii digits, `_`, `-`
 * Length: 1-64 characters.
 */
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomName(String);

impl RoomName {
    /// Default room name every client joins on connect
    pub const GENERAL: &'static str = "general";
    /// Maximum length of a room name.
    const ROOM_NAME_MAX_LEN: usize = 64;

    /**
     * Constructs and validates room name.
     *
     * # Arguments
     * - `s`: room name.
     *
     * # Returns
     * `Ok(RoomName)` on success.
     *
     * # Errors
     * Returns `StoreError` if:
     * - provided room name is empty,
     * - name exceeds 64 characters,
     * - contains characters outside [a-z0-9_-]
     */
    pub fn new(s: impl Into<String>) -> Result<Self, StoreError> {
        let s = s.into();

        if s.is_empty() {
            return Err(StoreError::InvalidRoomName(
                "room name cannot be empty".into(),
            ));
        }

        if s.len() > Self::ROOM_NAME_MAX_LEN {
            return Err(StoreError::InvalidRoomName(format!(
                "room name canot exceed 64 characters, got {}",
                s.len()
            )));
        }

        let invalid = s
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_'));
        if let Some(bad) = invalid {
            return Err(StoreError::InvalidRoomName(format!(
                "room name contains invalid character {bad:?}; only lowercase letters, digits, \
                 '-', and '_' are allowed"
            )));
        }

        Ok(Self(s))
    }

    /// Returns the room nae as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts into the inner `String`.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RoomName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A stored room record.
#[derive(Debug, Clone)]
pub struct RoomRecord {
    /// Canonical name of this room.
    pub name: RoomName,
}

/// A single stored chat message
#[derive(Debug, Clone)]
pub struct StoredMessage {
    /// The room the message was sent in.
    pub room: RoomName,
    /// Sender username.
    pub from: String,
    /// Message text.
    pub text: String,
}

/**
 * Presistent storage for durable server data.
 *
 * The store owns `persistent` state only - room definitions and message history.
 * Live connection state (connected clients, active room membership, writer channels) is
 * managed by the network layer.
 *
 * [`Send`] + `'static` so it can be moved into a central server task.
 */
pub trait ServerStore: Send + 'static {
    /**
     * Ensures `name` exists as a resisted room, creating it, if absent.
     *
     * Idempotent: calling this for an existing room is a no-op.
     *
     * # Arguments
     * - `name`: Room name.
     *
     * # Returns
     * `Ok(())` on success.
     *
     * # Errors
     * Returns `StoreError` if creation fails.
     */
    fn ensure_room(&mut self, name: &RoomName) -> Result<(), StoreError>;

    /**
     * Returns all persisted rooms.
     *
     * # Returns
     * `Vec<RoomRecord>` vector of room records on success.
     *
     * # Errors
     * Returns `StoreError` if the underlying store fails to read.
     */
    fn load_rooms(&self) -> Result<Vec<RoomRecord>, StoreError>;

    /**
     * Appends `msg` to the room's history.
     *
     * # Arguments
     * - `msg`: message record to store.
     *
     * # Errors
     * Returns `StoreError` if:
     * - the room does not exist,
     * - write fails.
     */
    fn append_message(&mut self, msg: StoredMessage) -> Result<(), StoreError>;

    /**
     * Loads up to `limit` of the most recent messages from `room`, oldest first.
     *
     * # Arguments
     * - `room`: room name,
     * - `limit`: upper limit as to how much messages to load.
     *
     * # Returns
     * `Vec<StoredMessage>` vector of stored messages on success.
     *
     * # Errors
     * Returns `StoreError` if:
     * - the room does not exist,
     * - read fails.
     */
    fn load_recent_messages(
        &self,
        room: &RoomName,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, StoreError>;
}

#[cfg(test)]
mod tests {
    use super::RoomName;
    use crate::error::StoreError;

    #[test]
    fn room_name_empty_is_error() {
        let err = RoomName::new("").expect_err("empty name should fail");
        assert!(matches!(err, StoreError::InvalidRoomName(_)));
    }

    #[test]
    fn room_name_too_long_is_error() {
        let long = "a".repeat(65);
        let err = RoomName::new(long).expect_err("65-char name should fail");
        assert!(matches!(err, StoreError::InvalidRoomName(_)));
    }

    #[test]
    fn room_name_invalid_char_uppercase_is_error() {
        let err = RoomName::new("BadRoom").expect_err("uppercase should fail");
        assert!(matches!(err, StoreError::InvalidRoomName(_)));
    }

    #[test]
    fn room_name_invalid_char_space_is_error() {
        let err = RoomName::new("bad room").expect_err("space should fail");
        assert!(matches!(err, StoreError::InvalidRoomName(_)));
    }

    #[test]
    fn room_name_invalid_char_exclamation_is_error() {
        let err = RoomName::new("bad!").expect_err("exclamation should fail");
        assert!(matches!(err, StoreError::InvalidRoomName(_)));
    }

    #[test]
    fn room_name_valid_succeeds() {
        let name = RoomName::new("my-room_01").expect("valid name should succeed");
        assert_eq!(name.as_str(), "my-room_01");
    }

    #[test]
    fn room_name_max_length_succeeds() {
        let exactly_64 = "a".repeat(64);
        let name = RoomName::new(exactly_64.clone()).expect("64-char name should succeed");
        assert_eq!(name.as_str(), exactly_64);
    }

    #[test]
    fn room_name_display() {
        let name = RoomName::new("general").expect("valid name should succeed");
        assert_eq!(format!("{name}"), "general");
    }
}
