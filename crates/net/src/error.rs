use hoy_protocol::error::ProtocolError;
use thiserror::Error;

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
}
