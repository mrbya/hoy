use hoy_protocol::codec::encode_frame;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::client_id::ClientId;
use crate::command::ServerCommand;
use crate::error::NetError;

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
    client_id: &ClientId,
    server_tx: mpsc::Sender<ServerCommand>,
) -> Result<(), NetError> {
    let (mut reader, mut writer) = stream.into_split();

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
