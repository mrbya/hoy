use std::net::SocketAddr;

use hoy_protocol::codec::encode_frame;
use hoy_protocol::error::ProtocolError;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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

/// Internal events emitted by session-side tasks toward the client core.
#[derive(Debug)]
pub(crate) enum InternalEvent {
    /// A protocol packet was received from the server.
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

/**
 * Spawns the background packet reader task for a TCP connection.
 *
 * # Arguments
 * - `reader`: owned read half of the TCP stream,
 * - `internal_tx`: internal event channel sender.
 *
 * # Returns
 * Join handle for the spawned reader task.
 */
fn spawn_reader_task(
    reader: OwnedReadHalf,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>> {
    spawn_reader_task_io(reader, internal_tx)
}

/**
 * Spawns background packet reader task for generic async readers.
 *
 * # Arguments
 * - `reader`: inbound stream used for packet frames,
 * - `internal_tx`: internal event channel sender.
 *
 * # Returns
 * Join handle for the spawned reader task.
 */
fn spawn_reader_task_io<R>(
    mut reader: R,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
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

/**
 * Spawns the background packet writer task for a TCP connection.
 *
 * # Arguments
 * - `writer`: owned write half of the TCP stream,
 * - `packet_rx`: outgoing packet receiver,
 * - `internal_tx`: internal event channel sender.
 *
 * # Returns
 * Join handle for the spawned writer task.
 */
fn spawn_writer_task(
    writer: OwnedWriteHalf,
    packet_rx: mpsc::Receiver<ClientPacket>,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>> {
    spawn_writer_task_io(writer, packet_rx, internal_tx)
}

/**
 * Spawns background packet writer task for generic async writers.
 *
 * # Arguments
 * - `writer`: outbound stream used for packet frames,
 * - `packet_rx`: outgoing packet receiver,
 * - `internal_tx`: internal event channel sender.
 *
 * # Returns
 * Join handle for the spawned writer task.
 */
fn spawn_writer_task_io<W>(
    mut writer: W,
    mut packet_rx: mpsc::Receiver<ClientPacket>,
    internal_tx: mpsc::Sender<InternalEvent>,
) -> JoinHandle<Result<(), NetError>>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
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

#[cfg(test)]
mod tests {
    use hoy_protocol::codec::encode_frame;
    use hoy_protocol::frame_buffer::FrameBuffer;
    use hoy_protocol::packet::{ClientPacket, ServerPacket};
    use hoy_test::async_ok;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream};
    use tokio::sync::mpsc;

    use super::{InternalEvent, spawn_reader_task_io, spawn_writer_task_io};

    struct IoHarness {
        client: Option<DuplexStream>,
        server: DuplexStream,
    }

    impl IoHarness {
        fn connect() -> Self {
            let (client, server) = tokio::io::duplex(4096);
            Self {
                client: Some(client),
                server,
            }
        }

        fn split_client(
            &mut self,
        ) -> (
            tokio::io::ReadHalf<DuplexStream>,
            tokio::io::WriteHalf<DuplexStream>,
        ) {
            tokio::io::split(self.client.take().expect("Client stream missing."))
        }

        fn server_mut(&mut self) -> &mut DuplexStream {
            &mut self.server
        }
    }

    async fn read_client_packet<R>(server: &mut R) -> Result<ClientPacket, ()>
    where
        R: AsyncRead + Unpin,
    {
        let mut buffer = [0_u8; 1024];
        let mut frame_buffer = FrameBuffer::with_capacity(2048);

        loop {
            let bytes_read = async_ok!(200, server.read(&mut buffer)).map_err(|_err| ())?;
            if bytes_read == 0 {
                return Err(());
            }

            let chunk = buffer.get(..bytes_read).ok_or(())?;
            frame_buffer.append(chunk).map_err(|_err| ())?;

            if let Some(packet) = frame_buffer
                .try_decode::<ClientPacket>()
                .map_err(|_err| ())?
            {
                return Ok(packet);
            }
        }
    }

    #[tokio::test]
    async fn reader_task_emits_packet_received() -> Result<(), ()> {
        let mut harness = IoHarness::connect();
        let (reader, _writer) = harness.split_client();
        let (internal_tx, mut internal_rx) = mpsc::channel(8);

        let join = spawn_reader_task_io(reader, internal_tx);

        let packet = ServerPacket::Pong;
        let frame = encode_frame(&packet).map_err(|_err| ())?;
        async_ok!(200, harness.server_mut().write_all(&frame)).map_err(|_err| ())?;
        async_ok!(200, harness.server_mut().flush()).map_err(|_err| ())?;

        let event = async_ok!(200, internal_rx.recv()).ok_or(())?;
        match event {
            InternalEvent::PacketReceived(decoded) => {
                assert_eq!(decoded, ServerPacket::Pong);
            }
            _ => return Err(()),
        }

        drop(harness);
        match async_ok!(200, join) {
            Ok(Ok(())) => {}
            _ => return Err(()),
        }
        Ok(())
    }

    #[tokio::test]
    async fn reader_task_emits_connection_closed_on_eof() -> Result<(), ()> {
        let mut harness = IoHarness::connect();
        let (reader, _writer) = harness.split_client();
        let (internal_tx, mut internal_rx) = mpsc::channel(8);

        let join = spawn_reader_task_io(reader, internal_tx);

        drop(harness);

        let event = async_ok!(200, internal_rx.recv()).ok_or(())?;
        match event {
            InternalEvent::ConnectionClosed => {}
            _ => return Err(()),
        }

        match async_ok!(200, join) {
            Ok(Ok(())) => {}
            _ => return Err(()),
        }
        Ok(())
    }

    #[tokio::test]
    async fn writer_task_encodes_and_writes_packets() -> Result<(), ()> {
        let mut harness = IoHarness::connect();
        let (_reader, writer) = harness.split_client();
        let (packet_tx, packet_rx) = mpsc::channel(8);
        let (internal_tx, mut internal_rx) = mpsc::channel(8);

        let join = spawn_writer_task_io(writer, packet_rx, internal_tx);

        async_ok!(200, packet_tx.send(ClientPacket::Ping)).map_err(|_err| ())?;

        let packet = read_client_packet(harness.server_mut()).await?;
        assert_eq!(packet, ClientPacket::Ping);

        drop(packet_tx);
        match async_ok!(200, join) {
            Ok(Ok(())) => {}
            _ => return Err(()),
        }

        let unexpected = internal_rx.try_recv().ok();
        assert!(unexpected.is_none());
        Ok(())
    }
}
