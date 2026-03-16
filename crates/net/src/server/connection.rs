//! TCP connection handling and accept loop.

use hoy_protocol::codec::encode_frame;
use hoy_protocol::error::ProtocolError;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::error::NetError;
use crate::server::client_id::ClientId;
use crate::server::command::ServerCommand;

/// Incoming channel read buffer size.
const READ_BUFFER_SIZE: usize = 1024;
/// Outgoing channel size.
const CLIENT_CHANNEL_SIZE: usize = 32;
/// Frame buffer size.
const FRAME_BUFFER_SIZE: usize = 4096;

/**
 * Spawns the TCP accept loop and forwards new connections into the server channel.
 *
 * Each accepted connection gets a unique [`ClientId`] and is handed off to
 * [`handle_connection`] in its own task.
 *
 * # Arguments
 * - `listener`: bound TCP listener,
 * - `server_tx`: channel to send server commands.
 */
pub(crate) fn spawn_accept_loop(listener: TcpListener, server_tx: mpsc::Sender<ServerCommand>) {
    tokio::spawn(async move {
        loop {
            let accepted = listener.accept().await;

            let Ok((stream, _peer_addr)) = accepted else {
                eprintln!("Failed to accept TCP connection.");
                break;
            };

            let client_id = ClientId::new();
            let connection_server_tx = server_tx.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, client_id, connection_server_tx).await {
                    eprintln!("Connection error: {e:?}");
                }
            });
        }
    });
}

/**
 * Handles a newly accepted TCP client connection.
 *
 * 1. Creates an outbound writer task.
 * 2. Registers the client with the server state task via [`ServerCommand::Connected`].
 * 3. Decodes incoming client packets and forwards them as [`ServerCommand::Packet`].
 * 4. Sends [`ServerCommand::Disconnected`] on EOF or error.
 *
 * # Arguments
 * - `stream`: incoming TCP stream,
 * - `client_id`: client id of the connected client,
 * - `server_tx`: server loop command channel.
 *
 * # Returns
 * `Ok(())` on successful client disconnect and loop termination.
 *
 * # Errors
 * Returns `NetError` if:
 * - socket I/O fails,
 * - protocol decoding fails,
 * - command channel communication fails.
 */
pub(crate) async fn handle_connection(
    stream: TcpStream,
    client_id: ClientId,
    server_tx: mpsc::Sender<ServerCommand>,
) -> Result<(), NetError> {
    let (reader, writer) = stream.into_split();
    handle_connection_io(reader, writer, client_id, server_tx).await
}

/**
 * Handles a client connection using generic async I/O.
 *
 * Extracted from [`handle_connection`] to allow in-memory duplex streams in tests.
 *
 * # Arguments
 * - `reader`: inbound stream for client frames,
 * - `writer`: outbound stream for server frames,
 * - `client_id`: client id of the connected client,
 * - `server_tx`: server loop command channel.
 *
 * # Returns
 * `Ok(())` on successful client disconnect and loop termination.
 *
 * # Errors
 * Returns `NetError` if:
 * - socket I/O fails,
 * - protocol decoding fails,
 * - command channel communication fails.
 */
pub(crate) async fn handle_connection_io<R, W>(
    mut reader: R,
    writer: W,
    client_id: ClientId,
    server_tx: mpsc::Sender<ServerCommand>,
) -> Result<(), NetError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (client_tx, mut client_rx) = mpsc::channel::<ServerPacket>(CLIENT_CHANNEL_SIZE);

    server_tx
        .send(ServerCommand::Connected {
            client_id,
            tx: client_tx,
        })
        .await
        .map_err(|e| {
            let _ = e;
            NetError::CommandChannelClosed
        })?;

    let writer_task = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(packet) = client_rx.recv().await {
            let frame = encode_frame(&packet)?;
            writer.write_all(&frame).await?;
        }

        Ok::<(), NetError>(())
    });

    let mut frame_buffer = FrameBuffer::with_capacity(FRAME_BUFFER_SIZE);
    let mut read_buffer: [u8; READ_BUFFER_SIZE] = [0; READ_BUFFER_SIZE];

    loop {
        let bytes_read: usize = reader.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            break;
        }

        frame_buffer.append(
            read_buffer
                .get(..bytes_read)
                .ok_or(NetError::Protocol(ProtocolError::TruncatedFrame))?,
        )?;

        loop {
            let Some(packet) = frame_buffer.try_decode::<ClientPacket>()? else {
                break;
            };

            server_tx
                .send(ServerCommand::Packet { client_id, packet })
                .await
                .map_err(|e| {
                    let _ = e;
                    NetError::CommandChannelClosed
                })?;
        }
    }

    server_tx
        .send(ServerCommand::Disconnected { client_id })
        .await
        .map_err(|e| {
            let _ = e;
            NetError::CommandChannelClosed
        })?;

    writer_task.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use hoy_protocol::codec::encode_frame;
    use hoy_protocol::packet::{ClientPacket, ServerPacket};
    use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
    use tokio::sync::mpsc;

    use super::handle_connection_io;
    use crate::error::NetError;
    use crate::server::client_id::ClientId;
    use crate::server::command::ServerCommand;

    // ── Stubs ─────────────────────────────────────────────────────────────────

    struct FailReader;

    impl AsyncRead for FailReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::other("stub read fail")))
        }
    }

    struct FailWriter;

    impl AsyncWrite for FailWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Err(io::Error::other("stub write fail")))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn connection_io_command_channel_closed_on_connect() {
        let (server_tx, server_rx) = mpsc::channel::<ServerCommand>(4);
        drop(server_rx); // no receiver → Connected send must fail

        let (_, server_side) = tokio::io::duplex(1024);
        let (server_reader, server_writer) = tokio::io::split(server_side);
        let client_id = ClientId::new();

        let result = handle_connection_io(server_reader, server_writer, client_id, server_tx).await;
        assert!(matches!(result, Err(NetError::CommandChannelClosed)));
    }

    #[tokio::test]
    async fn connection_io_command_channel_closed_on_packet() {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let (server_reader, server_writer) = tokio::io::split(server_side);
        let (_, mut client_writer) = tokio::io::split(client_side);

        let (server_tx, mut server_rx) = mpsc::channel::<ServerCommand>(4);
        let client_id = ClientId::new();

        let task = tokio::spawn(async move {
            handle_connection_io(server_reader, server_writer, client_id, server_tx).await
        });

        let _ = server_rx.recv().await; // consume Connected
        drop(server_rx); // close the receiver so the next Packet send fails

        // Write a valid packet — the task will decode it and fail to forward it
        let frame = encode_frame(&ClientPacket::Ping).expect("encode Ping");
        client_writer.write_all(&frame).await.expect("write frame");

        let result = task.await.expect("task panicked");
        assert!(matches!(result, Err(NetError::CommandChannelClosed)));
    }

    #[tokio::test]
    async fn connection_io_read_error_propagates() {
        let (server_tx, _server_rx) = mpsc::channel::<ServerCommand>(4);
        let client_id = ClientId::new();

        // FailReader returns an io::Error on the first read; sink() discards all writes
        let result =
            handle_connection_io(FailReader, tokio::io::sink(), client_id, server_tx).await;
        assert!(matches!(result, Err(NetError::Io(_))));
    }

    #[tokio::test]
    async fn connection_io_writer_error_propagates() -> Result<(), &'static str> {
        let (client_side, server_side) = tokio::io::duplex(1024);
        let (server_reader, _server_writer) = tokio::io::split(server_side);
        let (_client_reader, _client_writer) = tokio::io::split(client_side);
        // _client_writer is kept alive so server_reader does not hit EOF

        let (server_tx, mut server_rx) = mpsc::channel::<ServerCommand>(4);
        let client_id = ClientId::new();

        tokio::spawn(async move {
            drop(handle_connection_io(server_reader, FailWriter, client_id, server_tx).await);
        });

        let Some(ServerCommand::Connected { tx: client_tx, .. }) = server_rx.recv().await else {
            return Err("expected Connected command");
        };

        // Trigger the writer task — FailWriter fails on write_all, the task exits and drops
        // client_rx, making further sends fail.
        drop(client_tx.send(ServerPacket::Pong).await);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(
            client_tx.send(ServerPacket::Pong).await.is_err(),
            "writer task should have exited and dropped client_rx"
        );
        Ok(())
    }
}
