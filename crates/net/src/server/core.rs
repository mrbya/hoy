//! Central server event loop.
//!
//! [`run_server`] binds the listener and owns both the live [`ServerState`]
//! and the [`ServerStore`]. All packet-level logic is delegated to
//! [`super::handlers`].

use std::net::SocketAddr;

use hoy_core::store::{RoomName, ServerStore};
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::{net::TcpListener, sync::mpsc};

use crate::{
    error::{NetError, StateError},
    server::{
        command::ServerCommand,
        connection::spawn_accept_loop,
        handlers::{
            PendingClients, broadcast_to_room, handle_hello, handle_join_room, handle_list_rooms,
            handle_send_message, send_packet,
        },
        state::ServerState,
    },
};

/// Default room every client is placed in after Hello handshake.
const DEFAULT_ROOM: &str = RoomName::GENERAL;
/// Size of the server command channel.
const SERVER_COMMAND_CHANNEL_SIZE: usize = 128;

/**
 * Dispatches one [`ServerCommand`] to the appropriate handler.
 *
 * # Arguments
 * - `state`: mutable live server state,
 * - `store`: persistent store,
 * - `pending`: pre-hello client map,
 * - `default_room`: cached default room name,
 * - `command`: command to process.
 */
async fn handle_server_command(
    state: &mut ServerState,
    store: &mut impl ServerStore,
    pending: &mut PendingClients,
    default_room: &RoomName,
    command: ServerCommand,
) {
    match command {
        ServerCommand::Connected { client_id, tx } => {
            let _ = pending.insert(client_id, tx);
        }

        ServerCommand::Disconnected { client_id } => {
            pending.remove(&client_id);
            if let Some(handle) = state.remove_client(client_id) {
                broadcast_to_room(
                    state,
                    &handle.current_room,
                    &ServerPacket::SystemMessage {
                        text: format!("{} left #{}", handle.username, handle.current_room),
                    },
                    None,
                )
                .await;
            }
        }

        ServerCommand::Packet { client_id, packet } => {
            match packet {
                ClientPacket::Hello { username } => {
                    handle_hello(state, pending, client_id, username, default_room).await;
                }
                ClientPacket::SendMessage { text } => {
                    handle_send_message(state, store, client_id, text).await;
                }
                ClientPacket::JoinRoom { room } => {
                    handle_join_room(state, store, client_id, room).await;
                }
                ClientPacket::ListRooms => {
                    handle_list_rooms(state, client_id).await;
                }
                ClientPacket::Ping => {
                    // Respond to ping even before hello handshake is established
                    let tx = state
                        .sender_of(client_id)
                        .or_else(|| pending.get(&client_id))
                        .cloned();
                    if let Some(client_tx) = tx {
                        send_packet(&client_tx, ServerPacket::Pong).await;
                    }
                }
            }
        }
    }
}

/**
 * Runs the hoy server: accept loop and central state loop.
 *
 * Hydrates runtime rooms from the store, guarantees the default room exists,
 * then processes [`ServerCommand`]s until the channel closes.
 *
 * # Arguments
 * - `bind_addr`: address and port to listen on,
 * - `store`: persistent storage implementation.
 *
 * # Returns
 * `Ok(())` when the server shuts down cleanly.
 *
 * # Errors
 * Returns `NetError` if:
 * - the listening socket cannot be created,
 * - persisted rooms cannot be loaded from the store.
 *
 * # Panics
 * No panic expected as the default, hard-coded room name is valid.
 */
pub async fn run_server(
    bind_addr: SocketAddr,
    mut store: impl ServerStore,
) -> Result<(), NetError> {
    let listener = TcpListener::bind(bind_addr).await?;
    let (server_tx, mut server_rx) = mpsc::channel::<ServerCommand>(SERVER_COMMAND_CHANNEL_SIZE);

    spawn_accept_loop(listener, server_tx.clone());

    let mut state = ServerState::default();
    let mut pending = PendingClients::default();

    match store.load_rooms() {
        Ok(rooms) => {
            for record in rooms {
                state.ensure_room(record.name.clone());
            }
        }
        Err(e) => return Err(NetError::InternalServerError(StateError::StoreError(e))),
    }

    let default_room = RoomName::new(DEFAULT_ROOM).expect("Hardcoded default room name is valid");
    if let Err(e) = store.ensure_room(&default_room) {
        return Err(NetError::InternalServerError(StateError::StoreError(e)));
    }
    state.ensure_room(default_room.clone());

    while let Some(command) = server_rx.recv().await {
        handle_server_command(&mut state, &mut store, &mut pending, &default_room, command).await;
    }

    Ok(())
}

//#[cfg(test)]
//mod tests {
//    use std::collections::HashMap;
//
//    use hoy_protocol::packet::{ClientPacket, ServerPacket};
//    use hoy_test::async_ok;
//    use tokio::sync::mpsc;
//
//    use crate::server::client_id::ClientId;
//    use crate::server::command::ServerCommand;
//    use crate::server::core::{
//        ClientHandle, ServerState, handle_hello, handle_send_message, handle_server_command,
//    };
//
//    struct TestHarness {
//        state: ServerState,
//        clients: HashMap<ClientId, mpsc::Receiver<ServerPacket>>,
//    }
//
//    impl TestHarness {
//        fn new() -> Self {
//            Self {
//                state: ServerState::default(),
//                clients: HashMap::new(),
//            }
//        }
//
//        fn add_client(&mut self, id: u64, username: Option<&str>) -> ClientId {
//            let (tx, rx) = mpsc::channel(8);
//            let client_id = ClientId::new(id);
//            let _previous = self.state.clients.insert(
//                client_id.clone(),
//                ClientHandle {
//                    username: username.map(String::from),
//                    tx,
//                },
//            );
//            let _previous_rx = self.clients.insert(client_id.clone(), rx);
//            client_id
//        }
//
//        async fn recv(&mut self, client_id: &ClientId) -> Option<ServerPacket> {
//            let rx = self
//                .clients
//                .get_mut(client_id)
//                .expect("Client receiver missing.");
//            async_ok!(200, rx.recv())
//        }
//
//        fn try_recv_now(&mut self, client_id: &ClientId) -> Option<ServerPacket> {
//            let rx = self
//                .clients
//                .get_mut(client_id)
//                .expect("Client receiver missing.");
//            rx.try_recv().ok()
//        }
//    }
//
//    #[tokio::test]
//    async fn harness_can_receive_broadcasts() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let first = harness.add_client(1, Some("bruce_lee"));
//        let second = harness.add_client(2, Some("ip_man"));
//
//        harness.state.broadcast(&ServerPacket::Pong).await;
//
//        let first_packet = harness.recv(&first).await.ok_or(())?;
//        let second_packet = harness.recv(&second).await.ok_or(())?;
//
//        match (first_packet, second_packet) {
//            (ServerPacket::Pong, ServerPacket::Pong) => Ok(()),
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn handle_hello_registers_client_and_sends_welcome() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let client_id = harness.add_client(1, None);
//        let observer_id = harness.add_client(2, Some("observer"));
//
//        async_ok!(
//            200,
//            handle_hello(
//                &mut harness.state,
//                client_id.clone(),
//                String::from("bruce_lee")
//            )
//        );
//
//        let username = harness.state.username_of(&client_id);
//        assert_eq!(username, Some("bruce_lee"));
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly");
//
//        match packet {
//            ServerPacket::Welcome {
//                username: name,
//                room,
//            } => {
//                assert_eq!(name, "bruce_lee");
//                assert_eq!(room, "#general");
//                let observer_packet = harness
//                    .recv(&observer_id)
//                    .await
//                    .expect("Observer reception failed unexpectedly.");
//
//                match observer_packet {
//                    ServerPacket::SystemMessage { text } => {
//                        assert_eq!(text, "bruce_lee joined #general");
//                        Ok(())
//                    }
//                    _ => Err(()),
//                }
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn handle_send_message_broadcasts_message() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let client_id = harness.add_client(1, Some("bruce_lee"));
//
//        async_ok!(
//            200,
//            handle_send_message(&harness.state, client_id.clone(), String::from("abcd"))
//        );
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly.");
//
//        match packet {
//            ServerPacket::ChatMessage { from, room, text } => {
//                assert_eq!(from, "bruce_lee");
//                assert_eq!(room, "#general");
//                assert_eq!(text, "abcd");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn send_message_before_hello_returns_error() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let client_id = harness.add_client(1, None);
//
//        async_ok!(
//            200,
//            handle_send_message(&harness.state, client_id.clone(), String::from("hi"))
//        );
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly.");
//
//        match packet {
//            ServerPacket::Error { message } => {
//                assert_eq!(message, "Client must say hello first");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn send_message_after_hello_broadcasts_to_all_clients() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let sender_id = harness.add_client(1, Some("bruce_lee"));
//        let listener_id = harness.add_client(2, Some("ip_man"));
//
//        async_ok!(
//            200,
//            handle_send_message(&harness.state, sender_id.clone(), String::from("hello all"))
//        );
//
//        let sender_packet = harness
//            .recv(&sender_id)
//            .await
//            .expect("Sender reception failed unexpectedly.");
//        let listener_packet = harness
//            .recv(&listener_id)
//            .await
//            .expect("Listener reception failed unexpectedly.");
//
//        match (sender_packet, listener_packet) {
//            (
//                ServerPacket::ChatMessage { from, room, text },
//                ServerPacket::ChatMessage {
//                    from: other_from,
//                    room: other_room,
//                    text: other_text,
//                },
//            ) => {
//                assert_eq!(from, "bruce_lee");
//                assert_eq!(room, "#general");
//                assert_eq!(text, "hello all");
//                assert_eq!(other_from, "bruce_lee");
//                assert_eq!(other_room, "#general");
//                assert_eq!(other_text, "hello all");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn ping_sends_pong_to_requesting_client() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let client_id = harness.add_client(1, Some("bruce_lee"));
//
//        let command = ServerCommand::Packet {
//            client_id: client_id.clone(),
//            packet: ClientPacket::Ping,
//        };
//
//        async_ok!(200, handle_server_command(&mut harness.state, command));
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly.");
//
//        match packet {
//            ServerPacket::Pong => Ok(()),
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn disconnect_with_username_broadcasts_left_message() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let leaver_id = harness.add_client(1, Some("bruce_lee"));
//        let observer_id = harness.add_client(2, Some("ip_man"));
//
//        let command = ServerCommand::Disconnected {
//            client_id: leaver_id.clone(),
//        };
//
//        async_ok!(200, handle_server_command(&mut harness.state, command));
//
//        let observer_packet = harness
//            .recv(&observer_id)
//            .await
//            .expect("Observer reception failed unexpectedly.");
//
//        match observer_packet {
//            ServerPacket::SystemMessage { text } => {
//                assert_eq!(text, "bruce_lee left #general");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn disconnect_without_username_has_no_broadcast() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let leaver_id = harness.add_client(1, None);
//        let observer_id = harness.add_client(2, Some("ip_man"));
//
//        let command = ServerCommand::Disconnected {
//            client_id: leaver_id.clone(),
//        };
//
//        async_ok!(200, handle_server_command(&mut harness.state, command));
//
//        let observer_packet = harness.try_recv_now(&observer_id);
//        assert!(observer_packet.is_none());
//
//        Ok(())
//    }
//
//    #[tokio::test]
//    async fn username_helpers_match_state() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let named_id = harness.add_client(1, Some("bruce_lee"));
//        let unnamed_id = harness.add_client(2, None);
//
//        assert_eq!(harness.state.username_of(&named_id), Some("bruce_lee"));
//        assert_eq!(harness.state.username_of(&unnamed_id), None);
//        assert!(harness.state.username_exists("bruce_lee"));
//        assert!(!harness.state.username_exists("ip_man"));
//
//        Ok(())
//    }
//
//    #[tokio::test]
//    async fn handle_hello_rejects_duplicate_for_same_client() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let client_id = harness.add_client(1, Some("bruce_lee"));
//
//        async_ok!(
//            200,
//            handle_hello(
//                &mut harness.state,
//                client_id.clone(),
//                String::from("bruce_lee")
//            )
//        );
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly.");
//
//        match packet {
//            ServerPacket::Error { message } => {
//                assert_eq!(message, "client is already initialized");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn handle_hello_rejects_username_collision() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let _existing_id = harness.add_client(1, Some("bruce_lee"));
//        let client_id = harness.add_client(2, None);
//
//        async_ok!(
//            200,
//            handle_hello(
//                &mut harness.state,
//                client_id.clone(),
//                String::from("bruce_lee")
//            )
//        );
//
//        let packet = harness
//            .recv(&client_id)
//            .await
//            .expect("Packet reception failed unexpectedly.");
//
//        match packet {
//            ServerPacket::Error { message } => {
//                assert_eq!(message, "Username is already in use");
//                Ok(())
//            }
//            _ => Err(()),
//        }
//    }
//
//    #[tokio::test]
//    async fn handle_server_command_handles_connected_and_disconnect() -> Result<(), ()> {
//        let mut harness = TestHarness::new();
//        let (tx, _rx) = mpsc::channel(8);
//        let client_id = ClientId::new(2);
//        let command_connect = ServerCommand::Connected {
//            client_id: client_id.clone(),
//            tx,
//        };
//
//        async_ok!(
//            200,
//            handle_server_command(&mut harness.state, command_connect)
//        );
//
//        let handle = harness
//            .state
//            .clients
//            .get(&client_id)
//            .expect("Handle retrieval failed unexpectedly.");
//        assert_eq!(handle.username, None);
//
//        let command_disconnect = ServerCommand::Disconnected {
//            client_id: client_id.clone(),
//        };
//
//        async_ok!(
//            200,
//            handle_server_command(&mut harness.state, command_disconnect)
//        );
//
//        let handle_none = harness.state.clients.get(&client_id);
//        match handle_none {
//            None => Ok(()),
//            Some(_) => Err(()),
//        }
//    }
//}
