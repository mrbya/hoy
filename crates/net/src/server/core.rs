use std::collections::HashMap;
use std::net::SocketAddr;

use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::error::NetError;
use crate::server::client_id::ClientId;
use crate::server::command::ServerCommand;
use crate::server::connection::handle_connection;

/// Default room name.
const DEFAULT_ROOM: &str = "#general";
/// Size of the server command channel buffer.
const SERVER_COMMAND_CHANNEL_SIZE: usize = 128;

/// Server client handle.
#[derive(Debug)]
struct ClientHandle {
    /// Client username.
    username: Option<String>,

    /// Outgoing packet channel for the client.
    tx: mpsc::Sender<ServerPacket>,
}

/// Server state.
#[derive(Debug, Default)]
struct ServerState {
    /// Map of clients connected/known to server
    clients: HashMap<ClientId, ClientHandle>,
}

impl ServerState {
    /**
     * Broadcasts a packet to all connected clients.
     *
     * # Arguments
     * - `packet`: Packet to send to each client.
     */
    async fn broadcast(&self, packet: &ServerPacket) {
        for client in self.clients.values() {
            let send_result = client.tx.send(packet.clone()).await;
            if let Err(e) = send_result {
                eprintln!("Client channel send failed: {e}");
            }
        }
    }

    /**
     * Sends a packet to a specific client.
     *
     * # Arguments
     * - `client_id`: Target client identifier.
     * - `packet`: Packet to send.
     *
     * # Returns
     * `Ok(())` if the client channel accepts the packet.
     *
     * # Errors
     * Returns `NetError::ClientChannelClosed` if the client is missing or closed.
     */
    async fn send_to_client(
        &self,
        client_id: ClientId,
        packet: ServerPacket,
    ) -> Result<(), NetError> {
        let Some(client) = self.clients.get(&client_id) else {
            return Err(NetError::ClientChannelClosed);
        };

        client.tx.send(packet).await.map_err(|e| {
            let _ = e;
            eprint!("Client channel send failed: {e}");
            NetError::ClientChannelClosed
        })
    }

    /**
     * Returns the username for a client if it is initialized.
     *
     * # Arguments
     * - `client_id`: Client identifier to look up.
     *
     * # Returns
     * Username if present.
     */
    fn username_of(&self, client_id: &ClientId) -> Option<&str> {
        self.clients
            .get(client_id)
            .and_then(|client| client.username.as_deref())
    }

    /**
     * Checks whether a username is already in use.
     *
     * # Arguments
     * - `username`: Candidate username to check.
     *
     * # Returns
     * `true` if any client has the same username.
     */
    fn username_exists(&self, username: &str) -> bool {
        self.clients
            .values()
            .filter_map(|client| client.username.as_deref())
            .any(|existing| existing == username)
    }
}

/**
 * Spawns the TCP accept loop and forwards connections into the server channel.
 *
 * # Arguments
 * - `listener`: Bound TCP listener.
 * - `server_tx`: Channel to send server commands.
 */
fn spawn_accept_loop(listener: TcpListener, server_tx: mpsc::Sender<ServerCommand>) {
    tokio::spawn(async move {
        let mut next_client_id: u64 = 1;

        loop {
            let accepted = listener.accept().await;

            let Ok((stream, _peer_addr)) = accepted else {
                eprintln!("Accept error.");
                break;
            };

            let client_id = ClientId::new(next_client_id);
            next_client_id = match next_client_id.checked_add(1) {
                Some(id) => id,
                None => break,
            };

            let connection_server_tx = server_tx.clone();

            tokio::spawn(async move {
                let connection_result =
                    handle_connection(stream, client_id, connection_server_tx).await;

                if let Err(e) = connection_result {
                    eprintln!("Connection error: {e:?}");
                }
            });
        }
    });
}

/**
 * Handles a hello handshake from a client.
 *
 * # Arguments
 * - `state`: Mutable server state.
 * - `client_id`: Client that sent the hello.
 * - `client_name`: Requested username.
 */
async fn handle_hello(state: &mut ServerState, client_id: ClientId, username: String) {
    if state.username_of(&client_id).is_some() {
        let _send_result = state
            .send_to_client(
                client_id,
                ServerPacket::Error {
                    message: String::from("client is already initialized"),
                },
            )
            .await;
        return;
    }

    if state.username_exists(&username) {
        let _send_result = state
            .send_to_client(
                client_id,
                ServerPacket::Error {
                    message: String::from("Username is already in use"),
                },
            )
            .await;
        return;
    }

    if let Some(client) = state.clients.get_mut(&client_id) {
        client.username = Some(username.clone());
    }

    let _send_result = state
        .send_to_client(
            client_id,
            ServerPacket::Welcome {
                username: username.clone(),
                room: String::from(DEFAULT_ROOM),
            },
        )
        .await;

    state
        .broadcast(&ServerPacket::SystemMessage {
            text: format!("{username} joined {DEFAULT_ROOM}"),
        })
        .await;
}

/**
 * Handles a chat message from a client.
 *
 * # Arguments
 * - `state`: Server state.
 * - `client_id`: Client that sent the message.
 * - `text`: Message body.
 */
async fn handle_send_message(state: &ServerState, client_id: ClientId, text: String) {
    let Some(username) = state.username_of(&client_id).map(String::from) else {
        let _send_result = state
            .send_to_client(
                client_id,
                ServerPacket::Error {
                    message: String::from("Client must say hello first"),
                },
            )
            .await;
        return;
    };

    state
        .broadcast(&ServerPacket::ChatMessage {
            from: username,
            room: String::from(DEFAULT_ROOM),
            text,
        })
        .await;
}

/**
 * Handles a single server command and updates the state.
 *
 * # Arguments
 * - `state`: Mutable server state.
 * - `command`: Command to process.
 */
async fn handle_server_command(state: &mut ServerState, command: ServerCommand) {
    match command {
        ServerCommand::Connected { client_id, tx } => {
            let _ = state
                .clients
                .insert(client_id, ClientHandle { username: None, tx });
        }

        ServerCommand::Disconnected { client_id } => {
            let removed = state.clients.remove(&client_id);

            if let Some(client) = removed {
                if let Some(username) = client.username {
                    state
                        .broadcast(&ServerPacket::SystemMessage {
                            text: format!("{username} left {DEFAULT_ROOM}"),
                        })
                        .await;
                }
            }
        }

        ServerCommand::Packet { client_id, packet } => match packet {
            ClientPacket::Hello { username } => {
                handle_hello(state, client_id, username).await;
            }
            ClientPacket::SendMessage { text } => {
                handle_send_message(state, client_id, text).await;
            }
            ClientPacket::Ping => {
                let _send_result = state.send_to_client(client_id, ServerPacket::Pong).await;
            }
        },
    }
}

/**
 * Runs hoy server client accept loop and central state loop.
 *
 * # Errors
 * Returns `NetError` if:
 * - listening socket cannot be created,
 * - fails to accept new client connection.
 */
pub async fn run_server(bind_addr: SocketAddr) -> Result<(), NetError> {
    let listener = TcpListener::bind(bind_addr).await?;
    let (server_tx, mut server_rx) = mpsc::channel::<ServerCommand>(SERVER_COMMAND_CHANNEL_SIZE);

    spawn_accept_loop(listener, server_tx.clone());

    let mut state = ServerState::default();

    while let Some(command) = server_rx.recv().await {
        handle_server_command(&mut state, command).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use hoy_protocol::packet::{ClientPacket, ServerPacket};
    use hoy_test::async_ok;
    use tokio::sync::mpsc;

    use crate::server::client_id::ClientId;
    use crate::server::command::ServerCommand;
    use crate::server::core::{
        ClientHandle, ServerState, handle_hello, handle_send_message, handle_server_command,
    };

    struct TestHarness {
        state: ServerState,
        clients: HashMap<ClientId, mpsc::Receiver<ServerPacket>>,
    }

    impl TestHarness {
        fn new() -> Self {
            Self {
                state: ServerState::default(),
                clients: HashMap::new(),
            }
        }

        fn add_client(&mut self, id: u64, username: Option<&str>) -> ClientId {
            let (tx, rx) = mpsc::channel(8);
            let client_id = ClientId::new(id);
            let _previous = self.state.clients.insert(
                client_id.clone(),
                ClientHandle {
                    username: username.map(String::from),
                    tx,
                },
            );
            let _previous_rx = self.clients.insert(client_id.clone(), rx);
            client_id
        }

        async fn recv(&mut self, client_id: &ClientId) -> Option<ServerPacket> {
            let rx = self
                .clients
                .get_mut(client_id)
                .expect("Client receiver missing.");
            async_ok!(200, rx.recv())
        }

        fn try_recv_now(&mut self, client_id: &ClientId) -> Option<ServerPacket> {
            let rx = self
                .clients
                .get_mut(client_id)
                .expect("Client receiver missing.");
            rx.try_recv().ok()
        }
    }

    #[tokio::test]
    async fn harness_can_receive_broadcasts() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let first = harness.add_client(1, Some("bruce_lee"));
        let second = harness.add_client(2, Some("ip_man"));

        harness.state.broadcast(&ServerPacket::Pong).await;

        let first_packet = harness.recv(&first).await.ok_or(())?;
        let second_packet = harness.recv(&second).await.ok_or(())?;

        match (first_packet, second_packet) {
            (ServerPacket::Pong, ServerPacket::Pong) => Ok(()),
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn handle_hello_registers_client_and_sends_welcome() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let client_id = harness.add_client(1, None);
        let observer_id = harness.add_client(2, Some("observer"));

        async_ok!(
            200,
            handle_hello(
                &mut harness.state,
                client_id.clone(),
                String::from("bruce_lee")
            )
        );

        let username = harness.state.username_of(&client_id);
        assert_eq!(username, Some("bruce_lee"));

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly");

        match packet {
            ServerPacket::Welcome {
                username: name,
                room,
            } => {
                assert_eq!(name, "bruce_lee");
                assert_eq!(room, "#general");
                let observer_packet = harness
                    .recv(&observer_id)
                    .await
                    .expect("Observer reception failed unexpectedly.");

                match observer_packet {
                    ServerPacket::SystemMessage { text } => {
                        assert_eq!(text, "bruce_lee joined #general");
                        Ok(())
                    }
                    _ => Err(()),
                }
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn handle_send_message_broadcasts_message() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let client_id = harness.add_client(1, Some("bruce_lee"));

        async_ok!(
            200,
            handle_send_message(&harness.state, client_id.clone(), String::from("abcd"))
        );

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly.");

        match packet {
            ServerPacket::ChatMessage { from, room, text } => {
                assert_eq!(from, "bruce_lee");
                assert_eq!(room, "#general");
                assert_eq!(text, "abcd");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn send_message_before_hello_returns_error() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let client_id = harness.add_client(1, None);

        async_ok!(
            200,
            handle_send_message(&harness.state, client_id.clone(), String::from("hi"))
        );

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly.");

        match packet {
            ServerPacket::Error { message } => {
                assert_eq!(message, "Client must say hello first");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn send_message_after_hello_broadcasts_to_all_clients() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let sender_id = harness.add_client(1, Some("bruce_lee"));
        let listener_id = harness.add_client(2, Some("ip_man"));

        async_ok!(
            200,
            handle_send_message(&harness.state, sender_id.clone(), String::from("hello all"))
        );

        let sender_packet = harness
            .recv(&sender_id)
            .await
            .expect("Sender reception failed unexpectedly.");
        let listener_packet = harness
            .recv(&listener_id)
            .await
            .expect("Listener reception failed unexpectedly.");

        match (sender_packet, listener_packet) {
            (
                ServerPacket::ChatMessage { from, room, text },
                ServerPacket::ChatMessage {
                    from: other_from,
                    room: other_room,
                    text: other_text,
                },
            ) => {
                assert_eq!(from, "bruce_lee");
                assert_eq!(room, "#general");
                assert_eq!(text, "hello all");
                assert_eq!(other_from, "bruce_lee");
                assert_eq!(other_room, "#general");
                assert_eq!(other_text, "hello all");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn ping_sends_pong_to_requesting_client() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let client_id = harness.add_client(1, Some("bruce_lee"));

        let command = ServerCommand::Packet {
            client_id: client_id.clone(),
            packet: ClientPacket::Ping,
        };

        async_ok!(200, handle_server_command(&mut harness.state, command));

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly.");

        match packet {
            ServerPacket::Pong => Ok(()),
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn disconnect_with_username_broadcasts_left_message() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let leaver_id = harness.add_client(1, Some("bruce_lee"));
        let observer_id = harness.add_client(2, Some("ip_man"));

        let command = ServerCommand::Disconnected {
            client_id: leaver_id.clone(),
        };

        async_ok!(200, handle_server_command(&mut harness.state, command));

        let observer_packet = harness
            .recv(&observer_id)
            .await
            .expect("Observer reception failed unexpectedly.");

        match observer_packet {
            ServerPacket::SystemMessage { text } => {
                assert_eq!(text, "bruce_lee left #general");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn disconnect_without_username_has_no_broadcast() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let leaver_id = harness.add_client(1, None);
        let observer_id = harness.add_client(2, Some("ip_man"));

        let command = ServerCommand::Disconnected {
            client_id: leaver_id.clone(),
        };

        async_ok!(200, handle_server_command(&mut harness.state, command));

        let observer_packet = harness.try_recv_now(&observer_id);
        assert!(observer_packet.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn username_helpers_match_state() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let named_id = harness.add_client(1, Some("bruce_lee"));
        let unnamed_id = harness.add_client(2, None);

        assert_eq!(harness.state.username_of(&named_id), Some("bruce_lee"));
        assert_eq!(harness.state.username_of(&unnamed_id), None);
        assert!(harness.state.username_exists("bruce_lee"));
        assert!(!harness.state.username_exists("ip_man"));

        Ok(())
    }

    #[tokio::test]
    async fn handle_hello_rejects_duplicate_for_same_client() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let client_id = harness.add_client(1, Some("bruce_lee"));

        async_ok!(
            200,
            handle_hello(
                &mut harness.state,
                client_id.clone(),
                String::from("bruce_lee")
            )
        );

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly.");

        match packet {
            ServerPacket::Error { message } => {
                assert_eq!(message, "client is already initialized");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn handle_hello_rejects_username_collision() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let _existing_id = harness.add_client(1, Some("bruce_lee"));
        let client_id = harness.add_client(2, None);

        async_ok!(
            200,
            handle_hello(
                &mut harness.state,
                client_id.clone(),
                String::from("bruce_lee")
            )
        );

        let packet = harness
            .recv(&client_id)
            .await
            .expect("Packet reception failed unexpectedly.");

        match packet {
            ServerPacket::Error { message } => {
                assert_eq!(message, "Username is already in use");
                Ok(())
            }
            _ => Err(()),
        }
    }

    #[tokio::test]
    async fn handle_server_command_handles_connected_and_disconnect() -> Result<(), ()> {
        let mut harness = TestHarness::new();
        let (tx, _rx) = mpsc::channel(8);
        let client_id = ClientId::new(2);
        let command_connect = ServerCommand::Connected {
            client_id: client_id.clone(),
            tx,
        };

        async_ok!(
            200,
            handle_server_command(&mut harness.state, command_connect)
        );

        let handle = harness
            .state
            .clients
            .get(&client_id)
            .expect("Handle retrieval failed unexpectedly.");
        assert_eq!(handle.username, None);

        let command_disconnect = ServerCommand::Disconnected {
            client_id: client_id.clone(),
        };

        async_ok!(
            200,
            handle_server_command(&mut harness.state, command_disconnect)
        );

        let handle_none = harness.state.clients.get(&client_id);
        match handle_none {
            None => Ok(()),
            Some(_) => Err(()),
        }
    }
}
