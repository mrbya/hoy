use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::error::NetError;

/// Outgoing packet channel size.
const CLIENT_PACKET_CHANNEL_SIZE: usize = 32;
/// Temporary socket read buffer size.
const READ_BUFFER_SIZE: usize = 1024;
/// Initial frame buffer capacity.
const FRAME_BUFFER_CAPACITY: usize = 4096;

/// Internal events emmited by session-side tasks toward the client core.
#[derive(Debug)]
pub(crate) enum InternalEvent {
    /// Ap protocol packet was received from the server.
    PacketReceived(ServerPacket),

    /// Server closed TCP connection.
    ConnectionClosed,

    /// Connection-level failure.
    ConnectionError {
        /// Error message.
        message: String,
    },
}

/**
 * Active client network session.
 *
 * This handle owns the network packet sender and spawned reader/writer
 * tasks associated with a TCP client session.
 */
#[derive(Debug)]
pub(crate) struct SessionHandle {
    /// Outgoing packet channel used by client core.
    packet_tx: mpsc::Sender<ClientPacket>,

    /// Background socket reader task.
    reader_task: JoinHandle<Result<(), NetError>>,

    /// Background socket writer task.
    writer_task: JoinHandle<Result<(), NetError>>,
}

impl SessionHandle {
    /// Constructs a new session handle
    #[must_use]
    const fn new(
        packet_tx: mpsc::Sender<ClientPacket>,
        reader_task: JoinHandle<Result<(), NetError>>,
        writer_task: JoinHandle<Result<(), NetError>>,
    ) -> Self {
        Self {
            packet_tx,
            reader_task,
            writer_task,
        }
    }

    /// Returns the outgoing packet channel for this session.
    #[must_use]
    pub(crate) const fn packet_tx(&self) -> &mpsc::Sender<ClientPacket> {
        &self.packet_tx
    }

    /**
     * Send a client packet through the active session.
     *
     * # Arguments
     * - `packet`: Client packet to send.
     *
     * # Returns
     * `Ok(())` on success
     *
     * # Errors
     * Returns `NetError` if session writer channel is no longer available.
     */
    pub(crate) async fn send(&self, packet: ClientPacket) -> Result<(), NetError> {
        self.packet_tx.send(packet).await.map_err(|e| {
            let _ = e;
            NetError::ClientChannelClosed
        })
    }

    //pub(crate) async fn shutdown(&mut self) -> Result<(), NetError> {}
}
