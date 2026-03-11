use std::net::SocketAddr;

use hoy_net::server::core::run_server;
use hoy_protocol::codec::encode_frame;
use hoy_protocol::frame_buffer::FrameBuffer;
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use pretty_assertions::assert_eq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};

const CONNECT_TIMEOUT_MS: u64 = 500;
const IO_TIMEOUT_MS: u64 = 500;
const BROADCAST_TIMEOUT_MS: u64 = 1500;

async fn spawn_server() -> Option<(SocketAddr, JoinHandle<()>)> {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return None;
        }
        Err(error) => panic!("failed to bind test listener: {error}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(error) => panic!("failed to query local addr: {error}"),
    };
    drop(listener);

    let join = tokio::spawn(async move {
        let _ = run_server(addr).await;
    });

    Some((addr, join))
}

async fn connect_within(addr: SocketAddr, timeout_ms: u64) -> Result<TcpStream, ()> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        match TcpStream::connect(addr).await {
            Ok(stream) => return Ok(stream),
            Err(_) => {
                if Instant::now() >= deadline {
                    return Err(());
                }
                tokio::task::yield_now().await;
            }
        }
    }
}

async fn send_client_packet(stream: &mut TcpStream, packet: &ClientPacket) -> Result<(), ()> {
    let frame = encode_frame(packet).map_err(|_err| ())?;
    match tokio::time::timeout(
        Duration::from_millis(IO_TIMEOUT_MS),
        stream.write_all(&frame),
    )
    .await
    {
        Ok(Ok(())) => {}
        _ => return Err(()),
    }

    match tokio::time::timeout(Duration::from_millis(IO_TIMEOUT_MS), stream.flush()).await {
        Ok(Ok(())) => {}
        _ => return Err(()),
    }
    Ok(())
}

async fn recv_server_packet_with_timeout(
    stream: &mut TcpStream,
    timeout_ms: u64,
) -> Result<ServerPacket, ()> {
    let mut buffer = [0_u8; 1024];
    let mut frame_buffer = FrameBuffer::with_capacity(2048);

    loop {
        let bytes_read = match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            stream.read(&mut buffer),
        )
        .await
        {
            Ok(Ok(bytes)) => bytes,
            _ => return Err(()),
        };
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

async fn recv_server_packet(stream: &mut TcpStream) -> Result<ServerPacket, ()> {
    recv_server_packet_with_timeout(stream, IO_TIMEOUT_MS).await
}

async fn hello_client(stream: &mut TcpStream, username: &str) -> Result<ServerPacket, ()> {
    let hello = ClientPacket::Hello {
        username: String::from(username),
    };
    send_client_packet(stream, &hello).await?;
    recv_until(stream, IO_TIMEOUT_MS, |packet| {
        matches!(packet, ServerPacket::Welcome { .. })
    })
    .await
}

async fn recv_until<F>(
    stream: &mut TcpStream,
    timeout_ms: u64,
    mut predicate: F,
) -> Result<ServerPacket, ()>
where
    F: FnMut(&ServerPacket) -> bool,
{
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let remaining_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u128::from(timeout_ms)) as u64;
        if remaining_ms == 0 {
            return Err(());
        }

        let packet = recv_server_packet_with_timeout(stream, remaining_ms).await?;
        if predicate(&packet) {
            return Ok(packet);
        }
    }
}

#[tokio::test]
async fn client_can_connect_and_receive_welcome() -> Result<(), ()> {
    let Some((addr, server_task)) = spawn_server().await else {
        return Ok(());
    };
    let mut stream = connect_within(addr, CONNECT_TIMEOUT_MS).await?;

    let packet = hello_client(&mut stream, "viktor").await?;
    match packet {
        ServerPacket::Welcome { username, room } => {
            assert_eq!(username, "viktor");
            assert_eq!(room, "#general");
        }
        _ => return Err(()),
    }

    server_task.abort();
    Ok(())
}

#[tokio::test]
async fn message_broadcast_reaches_all_clients() -> Result<(), ()> {
    let Some((addr, server_task)) = spawn_server().await else {
        return Ok(());
    };

    let mut first = connect_within(addr, CONNECT_TIMEOUT_MS).await?;
    let mut second = connect_within(addr, CONNECT_TIMEOUT_MS).await?;

    let first_welcome = hello_client(&mut first, "alice").await?;
    assert_eq!(
        first_welcome,
        ServerPacket::Welcome {
            username: String::from("alice"),
            room: String::from("#general"),
        }
    );

    let second_welcome = hello_client(&mut second, "bob").await?;
    assert_eq!(
        second_welcome,
        ServerPacket::Welcome {
            username: String::from("bob"),
            room: String::from("#general"),
        }
    );

    let join_notice = recv_until(&mut first, BROADCAST_TIMEOUT_MS, |packet| {
        matches!(
            packet,
            ServerPacket::SystemMessage { text } if text == "bob joined #general"
        )
    })
    .await?;
    assert_eq!(
        join_notice,
        ServerPacket::SystemMessage {
            text: String::from("bob joined #general"),
        }
    );

    send_client_packet(
        &mut first,
        &ClientPacket::SendMessage {
            text: String::from("hello all"),
        },
    )
    .await?;

    let first_msg = recv_until(&mut first, BROADCAST_TIMEOUT_MS, |packet| {
        matches!(packet, ServerPacket::ChatMessage { .. })
    })
    .await?;
    let second_msg = recv_until(&mut second, BROADCAST_TIMEOUT_MS, |packet| {
        matches!(packet, ServerPacket::ChatMessage { .. })
    })
    .await?;

    assert_eq!(
        first_msg,
        ServerPacket::ChatMessage {
            from: String::from("alice"),
            room: String::from("#general"),
            text: String::from("hello all"),
        }
    );
    assert_eq!(
        second_msg,
        ServerPacket::ChatMessage {
            from: String::from("alice"),
            room: String::from("#general"),
            text: String::from("hello all"),
        }
    );

    server_task.abort();
    Ok(())
}

#[tokio::test]
async fn ping_receives_pong() -> Result<(), ()> {
    let Some((addr, server_task)) = spawn_server().await else {
        return Ok(());
    };
    let mut stream = connect_within(addr, CONNECT_TIMEOUT_MS).await?;

    let welcome = hello_client(&mut stream, "viktor").await?;
    assert_eq!(
        welcome,
        ServerPacket::Welcome {
            username: String::from("viktor"),
            room: String::from("#general"),
        }
    );

    send_client_packet(&mut stream, &ClientPacket::Ping).await?;

    let packet = recv_server_packet(&mut stream).await?;
    assert_eq!(packet, ServerPacket::Pong);

    server_task.abort();
    Ok(())
}
