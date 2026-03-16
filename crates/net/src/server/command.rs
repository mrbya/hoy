use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::sync::mpsc;

use crate::server::client_id::ClientId;

/// Commands sent to the central server state taks.
#[derive(Debug, Clone)]
pub enum ServerCommand {
    /// A new client connection was established.
    Connected {
        /// Assigned client id.
        client_id: ClientId,

        /// Outgoing packet channel for this client.
        tx: mpsc::Sender<ServerPacket>,
    },

    /// A client disconnected.
    Disconnected {
        /// Client id of the disconnected client.
        client_id: ClientId,
    },

    /// A decoded client packet was received from a connection task.
    Packet {
        /// Sender client id.
        client_id: ClientId,

        /// Decoded packet payload.
        packet: ClientPacket,
    },
}
