use std::{
    io::{self, BufRead},
    net::SocketAddr,
    thread,
};

use hoy_protocol::{
    codec::encode_frame,
    error::ProtocolError,
    frame_buffer::FrameBuffer,
    packet::{ClientPacket, ServerPacket},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

use crate::error::NetError;

/// Outgoing packet channel size.
const CLIENT_PACKET_CHANNEL_SIZE: usize = 32;
/// Read buffer size.
const READ_BUFFER_SIZE: usize = 1024;

/**
 * Runs a minimal temporary chat client.
 *
 * This client:
 * - connects to the server,
 * - sends initial hello packet,
 * - prints incomming server packets,
 * - reads user input from stdin,
 * - sends user input lines as chat messages.
 *
 * Enter `/quit` or `/exit` to exit.
 *
 * # Errors
 * Returns `NetError` if:
 * - connection fails,
 * - packet encoding/decoding fails,
 * - socket I/O fails.
 */
pub async fn run_temp_client(server_addr: SocketAddr, username: String) -> Result<(), NetError> {
    let stream = TcpStream::connect(server_addr).await?;
    let (mut reader, mut writer) = stream.into_split();

    let (packet_tx, mut packet_rx) = mpsc::channel::<ClientPacket>(CLIENT_PACKET_CHANNEL_SIZE);
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();

    let input_thread = thread::spawn(move || {
        let stdin = io::stdin();

        loop {
            let mut line = String::new();

            let read_result = stdin.lock().read_line(&mut line);
            let bytes_read: usize = match read_result {
                Ok(bytes_read) => bytes_read,
                Err(e) => {
                    eprint!("Stdin read error: {e}");
                    break;
                }
            };

            if bytes_read == 0 {
                break;
            }

            let trimmed: String = line.trim_end().to_owned();
            let should_exit: bool = trimmed == "/quit" || trimmed == "/exit";

            if line_tx.send(trimmed).is_err() {
                break;
            }

            if should_exit {
                break;
            }
        }
    });

    let writer_task = tokio::spawn(async move {
        while let Some(packet) = packet_rx.recv().await {
            let frame: Vec<u8> = encode_frame(&packet)?;
            writer.write_all(&frame).await?;
        }

        Ok::<(), NetError>(())
    });

    let reader_task = tokio::spawn(async move {
        let mut frame_buffer = FrameBuffer::with_capacity(4096);
        let mut read_buffer: [u8; READ_BUFFER_SIZE] = [0; READ_BUFFER_SIZE];

        loop {
            let bytes_read: usize = reader.read(&mut read_buffer).await?;

            if bytes_read == 0 {
                println!("Server closed connection");
                break;
            }

            let chunk: &[u8] = match read_buffer.get(..bytes_read) {
                Some(ch) => ch,
                None => return Err(NetError::Protocol(ProtocolError::TruncatedFrame)),
            };

            frame_buffer.append(chunk)?;

            loop {
                let Some(packet) = frame_buffer.try_decode::<ServerPacket>()? else {
                    break;
                };

                print_server_packet(&packet);
            }
        }

        Ok::<(), NetError>(())
    });

    packet_tx
        .send(ClientPacket::Hello { username })
        .await
        .map_err(|e| {
            let _ = e;
            NetError::ClientChannelClosed
        })?;

    while let Some(line) = line_rx.recv().await {
        if line.is_empty() {
            continue;
        }

        if line == "/quit" || line == "/exit" {
            break;
        }

        packet_tx
            .send(ClientPacket::SendMessage { text: line })
            .await
            .map_err(|e| {
                let _ = e;
                NetError::ClientChannelClosed
            })?;
    }

    drop(packet_tx);

    let writer_result = writer_task.await;
    match writer_result {
        Ok(result) => result?,
        Err(je) => {
            eprintln!("Writer task error: {je:?}");
        }
    }

    let reader_result = reader_task.await;
    match reader_result {
        Ok(result) => result?,
        Err(je) => {
            eprintln!("Reader task error: {je:?}");
        }
    }

    let input_result = input_thread.join();
    if let Err(je) = input_result {
        eprintln!("Input thread error: {je:?}");
    }

    Ok(())
}

/**
 * Prints received packet to stdout.
 */
fn print_server_packet(packet: &ServerPacket) {
    match *packet {
        ServerPacket::Welcome {
            ref username,
            ref room,
        } => {
            println!("Connected as {username} in {room}");
        }

        ServerPacket::ChatMessage {
            ref from,
            ref room,
            ref text,
        } => {
            println!("[{room}] {from}: {text}");
        }

        ServerPacket::SystemMessage { ref text } => {
            println!("* {text}");
        }

        ServerPacket::Error { ref message } => {
            println!("Server error: {message}");
        }

        ServerPacket::Pong => {
            println!("pong");
        }
    }
}
