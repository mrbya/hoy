use std::net::SocketAddr;

/**
 * Client command API (frontend -> core).
 *
 * These commands represent user or UI-level intent. They are intentionally
 * frontend-facing rather than wire-protocol commands, so the caller does not
 * need to concern itself with transport or protocol details.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCommand {
    /// Connect to a server and begin session handshake.
    Connect {
        /// Server address to connect to.
        server_addr: SocketAddr,
        /// Requested username.
        username: String,
    },

    /// Disconnect current session (if connected).
    Disconnect,

    /// Send a chat message.
    SendMessage {
        /// Message text.
        text: String,
    },

    /// Heartbeat ping.
    Ping,

    /// Request to join or create a room.
    JoinRoom {
        /// Name of the room to join/create.
        room: String,
    },

    /// Request to list available rooms.
    ListRooms,

    /// Shut down client core. Unlike `Disconnect`, this terminates the client
    /// core task instead of only closing the current network session.
    Shutdown,
}
