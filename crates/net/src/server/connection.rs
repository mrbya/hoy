use hoy_protocol::codec::encode_frame;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::error::NetError;
use crate::server::client_id::ClientId;
use crate::server::command::ServerCommand;

/// Incomming channel read buffer size.
const READ_BUFFER_SIZE: usize = 1024;
/// Outgoing channel size.
const CLIENT_CHANNEL_SIZE: usize = 32;
/// Frame buffer size.
const FRAME_BUFFER_SIZE: usize = 4096;

/**
 * Handles a newly accepted TCP client connection.
 *
 * 1. Creates a writer task.
 * 2. Registers the client with the server state task.
 * 3. Decodes incomming client packets.
 * 4. Notifies server on disconnects.
 *
 * # Arguments
 * - `stream`: incomming TCP stream,
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
 * - command channel communication fails
 */
pub async fn handle_connection(
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
 * - command channel communication fails
 */
async fn handle_connection_io<R, W>(
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
            client_id: client_id.clone(),
            tx: client_tx,
        })
        .await
        .map_err(|send_error| {
            let _ = send_error;
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

        frame_buffer.append(read_buffer.get(..bytes_read).ok_or(NetError::Protocol(
            hoy_protocol::error::ProtocolError::TruncatedFrame,
        ))?)?;

        loop {
            let Some(packet) = frame_buffer.try_decode::<ClientPacket>()? else {
                break;
            };

            server_tx
                .send(ServerCommand::Packet {
                    client_id: client_id.clone(),
                    packet,
                })
                .await
                .map_err(|e| {
                    let _ = e;
                    NetError::CommandChannelClosed
                })?;
        }
    }

    server_tx
        .send(ServerCommand::Disconnected {
            client_id: client_id.clone(),
        })
        .await
        .map_err(|e| {
            let _ = e;
            NetError::CommandChannelClosed
        })?;

    match writer_task.await {
        Ok(result) => result?,
        Err(error) => {
            let _ = error;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use hoy_protocol::codec::encode_frame;
    use hoy_protocol::frame_buffer::FrameBuffer;
    use hoy_protocol::packet::{ClientPacket, ServerPacket};
    use hoy_test::async_ok;
    use tokio::io::{self, AsyncReadExt, AsyncWriteExt, DuplexStream};
    use tokio::sync::mpsc;

    use crate::server::client_id::ClientId;
    use crate::server::command::ServerCommand;
    use crate::server::connection::handle_connection_io;

    struct ConnectionHarness {
        client: Option<DuplexStream>,
        server_rx: mpsc::Receiver<ServerCommand>,
        join: tokio::task::JoinHandle<Result<(), crate::error::NetError>>,
        client_id: ClientId,
    }

    impl ConnectionHarness {
        fn spawn(client_id_value: u64) -> Self {
            let (client, server) = io::duplex(4096);
            let (server_tx, server_rx) = mpsc::channel(8);
            let client_id = ClientId::new(client_id_value);
            let (reader, writer) = tokio::io::split(server);
            let join_id = client_id.clone();
            let join = tokio::spawn(async move {
                handle_connection_io(reader, writer, join_id, server_tx).await
            });

            Self {
                client: Some(client),
                server_rx,
                join,
                client_id,
            }
        }

        fn client_mut(&mut self) -> &mut DuplexStream {
            self.client.as_mut().expect("Client stream missing.")
        }

        fn close_client(&mut self) {
            self.client.take();
        }

        async fn recv_command(&mut self) -> Option<ServerCommand> {
            async_ok!(200, self.server_rx.recv())
        }

        fn try_recv_command(&mut self) -> Option<ServerCommand> {
            self.server_rx.try_recv().ok()
        }
    }

    async fn read_server_packet(client: &mut DuplexStream) -> Result<ServerPacket, ()> {
        let mut buffer = [0_u8; 1024];
        let mut frame_buffer = FrameBuffer::with_capacity(2048);

        loop {
            let bytes_read = async_ok!(200, client.read(&mut buffer)).map_err(|_| ())?;
            if bytes_read == 0 {
                return Err(());
            }

            let chunk = buffer.get(..bytes_read).ok_or(())?;
            frame_buffer.append(chunk).map_err(|_err| ())?;

            if let Some(packet) = frame_buffer
                .try_decode::<ServerPacket>()
                .map_err(|_err| ())?
            {
                return Ok(packet);
            }
        }
    }

    #[tokio::test]
    async fn harness_emits_connected_and_disconnect() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(1);
        let _ = harness.client_mut();

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");

        match connected {
            ServerCommand::Connected { client_id, tx } => {
                assert_eq!(client_id, harness.client_id);
                drop(tx);
            }
            _ => return Err(()),
        }

        harness.close_client();

        let disconnected = harness
            .recv_command()
            .await
            .expect("Disconnected command missing.");

        match disconnected {
            ServerCommand::Disconnected { client_id } => {
                assert_eq!(client_id, harness.client_id);
            }
            _ => return Err(()),
        }

        let join_result = async_ok!(200, harness.join);
        match join_result {
            Ok(Ok(())) => Ok(()),
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn read_loop_emits_packet_for_complete_frame() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(42);
        let client = harness.client_mut();

        let packet = ClientPacket::Ping;
        let frame = encode_frame(&packet).map_err(|_err| ())?;

        async_ok!(200, client.write_all(&frame)).map_err(|_| ())?;
        async_ok!(200, client.flush()).map_err(|_| ())?;

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        match connected {
            ServerCommand::Connected { client_id, .. } => {
                assert_eq!(client_id, harness.client_id);
            }
            _ => return Err(()),
        }

        let packet_cmd = harness
            .recv_command()
            .await
            .expect("Packet command missing.");
        match packet_cmd {
            ServerCommand::Packet {
                client_id,
                packet: decoded_packet,
            } => {
                assert_eq!(client_id, harness.client_id);
                assert_eq!(decoded_packet, ClientPacket::Ping);
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn read_loop_waits_for_complete_frame() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(7);

        let packet = ClientPacket::Ping;
        let frame = encode_frame(&packet).map_err(|_err| ())?;
        let split = frame.len().saturating_div(2);
        let (first, second) = frame.split_at(split);

        {
            let client = harness.client_mut();
            async_ok!(200, client.write_all(first)).map_err(|_| ())?;
            async_ok!(200, client.flush()).map_err(|_| ())?;
        }

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        match connected {
            ServerCommand::Connected { client_id, .. } => {
                assert_eq!(client_id, harness.client_id);
            }
            _ => return Err(()),
        }

        let early = harness.try_recv_command();
        assert!(early.is_none());

        {
            let client = harness.client_mut();
            async_ok!(200, client.write_all(second)).map_err(|_| ())?;
            async_ok!(200, client.flush()).map_err(|_| ())?;
        }

        let packet_cmd = harness
            .recv_command()
            .await
            .expect("Packet command missing.");
        match packet_cmd {
            ServerCommand::Packet {
                client_id,
                packet: decoded_packet,
            } => {
                assert_eq!(client_id, harness.client_id);
                assert_eq!(decoded_packet, ClientPacket::Ping);
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn read_loop_sends_disconnected_on_eof() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(99);
        let _ = harness.client_mut();

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        match connected {
            ServerCommand::Connected { client_id, .. } => {
                assert_eq!(client_id, harness.client_id);
            }
            _ => return Err(()),
        }

        harness.close_client();

        let disconnected = harness
            .recv_command()
            .await
            .expect("Disconnected command missing.");

        match disconnected {
            ServerCommand::Disconnected { client_id } => {
                assert_eq!(client_id, harness.client_id);
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn write_loop_encodes_packets_to_stream() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(11);

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        let client_tx = match connected {
            ServerCommand::Connected { client_id, tx } => {
                assert_eq!(client_id, harness.client_id);
                tx
            }
            _ => return Err(()),
        };

        async_ok!(200, client_tx.send(ServerPacket::Pong)).map_err(|_| ())?;

        let packet = {
            let client = harness.client_mut();
            read_server_packet(client).await?
        };
        match packet {
            ServerPacket::Pong => Ok(()),
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn writer_task_exits_when_channel_closes() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(12);
        let _ = harness.client_mut();

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        match connected {
            ServerCommand::Connected { client_id, tx } => {
                assert_eq!(client_id, harness.client_id);
                drop(tx);
            }
            _ => return Err(()),
        }

        harness.close_client();

        let _ = harness
            .recv_command()
            .await
            .expect("Disconnected command missing.");

        let join_result = async_ok!(200, harness.join);
        match join_result {
            Ok(Ok(())) => Ok(()),
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn end_to_end_duplex_flow_handles_read_and_write() -> Result<(), ()> {
        let mut harness = ConnectionHarness::spawn(77);

        let packet = ClientPacket::Ping;
        let frame = encode_frame(&packet).map_err(|_err| ())?;

        {
            let client = harness.client_mut();
            async_ok!(200, client.write_all(&frame)).map_err(|_| ())?;
            async_ok!(200, client.flush()).map_err(|_| ())?;
        }

        let connected = harness
            .recv_command()
            .await
            .expect("Connected command missing.");
        let client_tx = match connected {
            ServerCommand::Connected { client_id, tx } => {
                assert_eq!(client_id, harness.client_id);
                tx
            }
            _ => return Err(()),
        };

        let packet_cmd = harness
            .recv_command()
            .await
            .expect("Packet command missing.");
        match packet_cmd {
            ServerCommand::Packet {
                client_id,
                packet: decoded_packet,
            } => {
                assert_eq!(client_id, harness.client_id);
                assert_eq!(decoded_packet, ClientPacket::Ping);
            }
            _ => return Err(()),
        }

        async_ok!(200, client_tx.send(ServerPacket::Pong)).map_err(|_| ())?;

        let written_packet = {
            let client = harness.client_mut();
            read_server_packet(client).await?
        };

        match written_packet {
            ServerPacket::Pong => Ok(()),
            _ => Err(()),
        }
    }
}
