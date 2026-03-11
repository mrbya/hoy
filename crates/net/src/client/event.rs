use std::net::SocketAddr;

use hoy_protocol::packet::ServerPacket;

/**
 * Events emmited by the client core towards the frontend.
 *
 * These represent client or session-level events that are intentionally
 * decoupled from the wire protocol. Frontend should consume these events
 * rather than raw protocol.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    /// Client started connecting to server.
    Connecting {
        /// Server address.
        server_addr: SocketAddr,
        /// Requested username.
        username: String,
    },

    /// Client successfully connected to server.
    Connected {
        /// Connected server address.
        server_addr: SocketAddr,
        /// Accepted username.
        username: String,
        /// Active room after connection.
        room: String,
    },

    /// Client session has been disconnected.
    Disconnected,

    /// Client received a message from the server.
    MessageReceived {
        /// sender username.
        from: String,
        /// Server room the message belongs to.
        room: String,
        /// Message text.
        text: String,
    },

    /// System message received.
    SystemMessage {
        /// Message text.
        text: String,
    },

    /// Client erred out or received a server error.
    Error {
        /// Error message.
        message: String,
    },

    /// Pong response from the connected server.
    Pong,
}

/// Internal client events
pub enum InternalEvent {
    /// Received packet.
    PacketReceived(ServerPacket),

    /// Connection to the server closed.
    ConnectionClosed,

    /// Connection erred out.
    ConnectionError,

    /// Writer task stopped.
    WriterStopped,
}
