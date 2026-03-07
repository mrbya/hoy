use thiserror::Error;

/// Errors produced by the Hoy protocol codec.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// The serialized payload length does not fit into a `u32` length prefix.
    #[error("Frame too large: {size} bytes")]
    FrameTooLarge {
        /// Payload size in bytes.
        size: usize,
    },

    /// The provided frame contains less bytes than declared in its header.
    #[error("Truncated frame")]
    TruncatedFrame,

    /// The provided frame header is malformed.
    #[error("Invalid frame length prefix")]
    InvalidLengthPrefix,

    /// The provided frame length cannot be represented by the current platform.
    #[error("Frame length cannot be represented on this platform: {length}")]
    FrameLengthOutOfRange {
        /// Raw frame length decoded from the wire.
        length: u32,
    },

    /// Failed to compute a valid buffer capacity for a frame.
    #[error("Frame capacity overflow")]
    CapacityOverflow,

    /// Serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
