//! Protocol packet definitions.

use serde::{Deserialize, Serialize};

/// Client-to-server protocol packets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientPacket {
    /// Initial handshake packet sent by client after connecting.
    Hello {
        /// Client username.
        username: String,
    },

    /// Chat message sent by a client.
    SendMessage {
        /// Message text
        text: String,
    },

    /// Heartbeat ping.
    Ping,
}

/// Server-to-client protocol packets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerPacket {
    /// Successful handshake response.
    Welcome {
        /// Accepted client username.
        username: String,
        /// Joined room.
        room: String,
    },

    /// Chat message broadcast by the server.
    ChatMesage {
        /// Message sender username.
        from: String,

        /// Target room name.
        room: String,

        /// Message text.
        text: String,
    },

    /// System generated message.
    SystemMessage {
        /// System message text.
        text: String,
    },

    /// Error reported by the server.
    Error {
        /// Error description.
        message: String,
    },

    /// Heartbeat ping response.
    Pong,
}
