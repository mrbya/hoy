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

struct TestClient {
    stream: TcpStream,
    frame_buffer: FrameBuffer,
}

impl TestClient {
    async fn connect(addr: SocketAddr) -> Result<Self, ()> {
        let stream = connect_within(addr, CONNECT_TIMEOUT_MS).await?;
        Ok(Self {
            stream,
            frame_buffer: FrameBuffer::with_capacity(2048),
        })
    }

    async fn send(&mut self, packet: &ClientPacket) -> Result<(), ()> {
        send_client_packet(&mut self.stream, packet).await
    }

    async fn recv_with_timeout(&mut self, timeout_ms: u64) -> Result<ServerPacket, ()> {
        let mut buffer = [0_u8; 1024];

        // First check if we already have a complete frame buffered.
        if let Some(packet) = self
            .frame_buffer
            .try_decode::<ServerPacket>()
            .map_err(|_| ())?
        {
            return Ok(packet);
        }

        loop {
            let bytes_read = match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                self.stream.read(&mut buffer),
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
            self.frame_buffer.append(chunk).map_err(|_| ())?;

            if let Some(packet) = self
                .frame_buffer
                .try_decode::<ServerPacket>()
                .map_err(|_| ())?
            {
                return Ok(packet);
            }
        }
    }

    async fn recv_until<F>(&mut self, timeout_ms: u64, mut predicate: F) -> Result<ServerPacket, ()>
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

            let packet = self.recv_with_timeout(remaining_ms).await?;
            if predicate(&packet) {
                return Ok(packet);
            }
        }
    }

    async fn hello(&mut self, username: &str) -> Result<ServerPacket, ()> {
        let hello = ClientPacket::Hello {
            username: String::from(username),
        };
        self.send(&hello).await?;
        self.recv_until(IO_TIMEOUT_MS, |packet| {
            matches!(packet, ServerPacket::Welcome { .. })
        })
        .await
    }
}

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

#[tokio::test]
async fn client_can_connect_and_receive_welcome() -> Result<(), ()> {
    let Some((addr, server_task)) = spawn_server().await else {
        return Ok(());
    };
    let mut client = TestClient::connect(addr).await?;

    let packet = client.hello("viktor").await?;
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

    let mut first = TestClient::connect(addr).await?;
    let mut second = TestClient::connect(addr).await?;

    let first_welcome = first.hello("alice").await?;
    assert_eq!(
        first_welcome,
        ServerPacket::Welcome {
            username: String::from("alice"),
            room: String::from("#general"),
        }
    );

    let second_welcome = second.hello("bob").await?;
    assert_eq!(
        second_welcome,
        ServerPacket::Welcome {
            username: String::from("bob"),
            room: String::from("#general"),
        }
    );

    let join_notice = first
        .recv_until(BROADCAST_TIMEOUT_MS, |packet| {
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

    first
        .send(&ClientPacket::SendMessage {
            text: String::from("hello all"),
        })
        .await?;

    let first_msg = first
        .recv_until(BROADCAST_TIMEOUT_MS, |packet| {
            matches!(packet, ServerPacket::ChatMessage { .. })
        })
        .await?;
    let second_msg = second
        .recv_until(BROADCAST_TIMEOUT_MS, |packet| {
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
    let mut client = TestClient::connect(addr).await?;

    let welcome = client.hello("viktor").await?;
    assert_eq!(
        welcome,
        ServerPacket::Welcome {
            username: String::from("viktor"),
            room: String::from("#general"),
        }
    );

    client.send(&ClientPacket::Ping).await?;

    let packet = client
        .recv_until(IO_TIMEOUT_MS, |p| matches!(p, ServerPacket::Pong))
        .await?;
    assert_eq!(packet, ServerPacket::Pong);

    server_task.abort();
    Ok(())
}
