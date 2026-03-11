use std::net::SocketAddr;

use hoy_protocol::codec::encode_frame;
use hoy_protocol::error::ProtocolError;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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
    pub(crate) const fn new(
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

    /**
     * Shuts down this session.
     *
     * This drops the outgoing packet sender, waits for the writer task
     * to finish, and aborts the reader taks if still running.
     *
     * # Returns
     * `Ok(())` on success.
     *
     * # Errors
     * Returns `NetError` if:
     * - writer task reports an error,
     * - reader task fails to join,
     * - writer task fails to abort/join.
     */
    pub(crate) async fn shutdown(self) -> Result<(), NetError> {
        let Self {
            packet_tx,
            reader_task,
            writer_task,
        } = self;

        drop(packet_tx);

        match writer_task.await {
            Ok(result) => result?,
            Err(je) => {
                return Err(NetError::ClientTaskJoin(je));
            }
        }

        reader_task.abort();
        match reader_task.await {
            Ok(result) => result?,
            Err(je) => {
                if !je.is_cancelled() {
                    return Err(NetError::ClientTaskJoin(je));
                }
            }
        }

        Ok(())
    }
}

/**
 * Spawns a new TCP client session.
 *
 * Establishes a TCP connection, splits the sockets into owned read/write halves,
 * and spawns the background reader and writer tasks associated with this session.
 *
 * # Returns
 * Newly constructed `SessionHandle` on success.
 *
 * # Errors
 * Returns `NetError` if a TCP connection cannot be established.
 */
pub(crate) async fn spawn_session(
    server_addr: SocketAddr,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> Result<SessionHandle, NetError> {
    let stream = TcpStream::connect(server_addr).await?;
    let (reader, writer) = stream.into_split();

    let (packet_tx, packet_rx) = mpsc::channel::<ClientPacket>(CLIENT_PACKET_CHANNEL_SIZE);

    let reader_task = spawn_reader_task(reader, internal_tx.clone());
    let writer_task = spawn_writer_task(writer, packet_rx, internal_tx);

    Ok(SessionHandle::new(packet_tx, reader_task, writer_task))
}

/// Spawns background packet reader task.
fn spawn_reader_task(
    mut reader: OwnedReadHalf,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>> {
    tokio::spawn(async move {
        let mut frame_buffer = FrameBuffer::with_capacity(FRAME_BUFFER_CAPACITY);
        let mut read_buffer: [u8; READ_BUFFER_SIZE] = [0; READ_BUFFER_SIZE];

        loop {
            let bytes_read: usize = match reader.read(&mut read_buffer).await {
                Ok(bytes_read) => bytes_read,
                Err(e) => {
                    let message = e.to_string();
                    emit_internal_event(&internal_tx, InternalEvent::ConnectionError { message })
                        .await;

                    return Err(NetError::Io(e));
                }
            };

            if bytes_read == 0 {
                emit_internal_event(&internal_tx, InternalEvent::ConnectionClosed).await;
                break;
            }

            let Some(chunk) = read_buffer.get(..bytes_read) else {
                let protocol_error = ProtocolError::TruncatedFrame;
                let message = protocol_error.to_string();

                emit_internal_event(&internal_tx, InternalEvent::ConnectionError { message }).await;

                return Err(NetError::Protocol(protocol_error));
            };

            if let Err(e) = frame_buffer.append(chunk) {
                let message = e.to_string();

                emit_internal_event(&internal_tx, InternalEvent::ConnectionError { message }).await;

                return Err(NetError::Protocol(e));
            }

            loop {
                let packet = match frame_buffer.try_decode() {
                    Ok(p) => p,
                    Err(e) => {
                        let message = e.to_string();

                        emit_internal_event(
                            &internal_tx,
                            InternalEvent::ConnectionError { message },
                        )
                        .await;

                        return Err(NetError::Protocol(e));
                    }
                };

                let Some(pkt) = packet else {
                    break;
                };

                emit_internal_event(&internal_tx, InternalEvent::PacketReceived(pkt)).await;
            }
        }
        Ok(())
    })
}

/// Spawns background packet writer task
fn spawn_writer_task(
    mut writer: OwnedWriteHalf,
    mut packet_rx: mpsc::Receiver<ClientPacket>,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>> {
    tokio::spawn(async move {
        while let Some(packet) = packet_rx.recv().await {
            let frame: Vec<u8> = match encode_frame(&packet) {
                Ok(f) => f,
                Err(e) => {
                    let message = e.to_string();

                    emit_internal_event(&internal_tx, InternalEvent::ConnectionError { message })
                        .await;

                    return Err(NetError::Protocol(e));
                }
            };

            if let Err(e) = writer.write_all(&frame).await {
                let message = e.to_string();

                emit_internal_event(&internal_tx, InternalEvent::ConnectionError { message }).await;

                return Err(NetError::Io(e));
            }
        }

        Ok(())
    })
}

/**
 * Emit an internal event to the client core event loop.
 *
 * # Arguments
 * - `internal_tx`: internal event sender.
 * - `event`: event to forward.
 */
async fn emit_internal_event(internal_tx: &mpsc::Sender<InternalEvent>, event: InternalEvent) {
    let send_result = internal_tx.send(event).await;
    if let Err(e) = send_result {
        eprintln!("Internal event processing error: {e:?}");
    }
}
