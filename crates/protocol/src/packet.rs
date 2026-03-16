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

    /// Request to join or create a room.
    JoinRoom {
        /// Name of the room client is requesting to join.
        room: String,
    },

    /// Request the list of all known rooms.
    ListRooms,
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
    ChatMessage {
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

    /// Confirms the client had successfully joined a room.
    RoomJoined {
        /// Name of the room the client had joined.
        room: String,
        /// Message history..
        messages: Vec<MessageRecord>,
    },

    /// Response to `ListRooms`,
    RoomList {
        /// List of available rooms.
        rooms: Vec<String>,
    },
}

/// Message record for serialized packets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageRecord {
    /// Sender display name.
    pub from: String,
    /// Message text.
    pub text: String,
}
