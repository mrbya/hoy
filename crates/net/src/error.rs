use hoy_core::{error::StoreError, store::RoomName};
use hoy_protocol::error::ProtocolError;
use thiserror::Error;
use tokio::task::JoinError;

use crate::server::client_id::ClientId;

/// Errors produced by hoy networking layer.
#[derive(Debug, Error)]
pub enum NetError {
    /// I/O error while operating on a TCP connection or listener.
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),

    /// Protocol-level error.
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Failed to send a command to the central server task.
    #[error("Server command channel closed")]
    CommandChannelClosed,

    /// Failed to send an outgoing packet to a connection writer task.
    #[error("Client channel closed")]
    ClientChannelClosed,

    /// Client writer task failed to join.
    #[error("Client task failed to join: {0}")]
    ClientTaskJoin(#[from] JoinError),

    /// Internal server error.
    #[error("Internal server error: {0}")]
    InternalServerError(#[from] StateError),
}

/// `ServerState` mutation errors
#[derive(Debug, Error)]
pub enum StateError {
    /// Another connected client already holds this username.
    #[error("Username already taken: {0}")]
    UsernameTaken(String),

    /// The target room has no entry.
    #[error("Room not found: {0}")]
    RoomNotFound(RoomName),

    /// No connected client exists with this id.
    #[error("Client not found: {0:?}")]
    ClientNotFound(ClientId),

    /// Store operation error.
    #[error("Persistent storage error: {0:?}")]
    StoreError(#[from] StoreError),
}
