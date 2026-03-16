use thiserror::Error;

use crate::store::RoomName;

/// Server store errors.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The given string failed [`RoomName`] validation
    #[error("Invalid room name: {0}")]
    InvalidRoomName(String),

    /// The operation references a room that does not exist.
    #[error("Room {0} does not exist.")]
    RoomNotFound(RoomName),

    /// A generic internal store failure.
    #[error("Internal store error: {0}")]
    Internal(String),
}
