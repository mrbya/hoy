use hoy_net::server::core::run_server;
use hoy_protocol::packet::ClientPacket;
use pretty_assertions::assert_eq;

/// TODO: Example integration test, feel free to replace it with something meaningful.
#[test]
fn integrate() {
    let x = 42;
    assert_eq!(x, 42);
}

#[tokio::test]
async fn client_can_connect_and_receive_welcome() {
    use std::time::Duration;

    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::timeout;

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(error) => panic!("failed to bind test listener: {error}"),
    };

    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(error) => panic!("failed to query local addr: {error}"),
    };

    let server_task = tokio::spawn(async move {
        // run a test-only server entry point here
        let _ = run_server(addr).await;
    });

    let mut stream = match TcpStream::connect(addr).await {
        Ok(stream) => stream,
        Err(error) => panic!("failed to connect test client: {error}"),
    };

    let hello = ClientPacket::Hello {
        username: String::from("viktor"),
    };

    let frame = match hoy_protocol::codec::encode_frame(&hello) {
        Ok(frame) => frame,
        Err(error) => panic!("failed to encode hello: {error}"),
    };

    match timeout(
        Duration::from_secs(1),
        tokio::io::AsyncWriteExt::write_all(&mut stream, &frame),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => panic!("failed to write hello: {error}"),
        Err(error) => panic!("timed out writing hello: {error}"),
    }

    // then read bytes, append to FrameBuffer, decode ServerPacket, assert Welcome

    server_task.abort();
}
