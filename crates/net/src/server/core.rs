//! Central server event loop.
//!
//! [`run_server`](crate::server::core::run_server) binds the listener and owns both the live
//! [`ServerState`](crate::server::state::ServerState) and the
//! [`ServerStore`](hoy_core::store::ServerStore). All packet-level logic is
//! delegated to [`handlers`].

use std::net::SocketAddr;

use hoy_core::store::{RoomName, ServerStore};
use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::error::{NetError, StateError};
use crate::server::command::ServerCommand;
use crate::server::connection::spawn_accept_loop;
use crate::server::handlers::{
    PendingClients, broadcast_to_room, handle_hello, handle_join_room, handle_list_rooms,
    handle_send_message, send_packet,
};
use crate::server::state::ServerState;

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
                    handle_hello(state, store, pending, client_id, username, default_room).await;
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

    match store.load_rooms().await {
        Ok(rooms) => {
            for record in rooms {
                state.ensure_room(record.name.clone());
            }
        }
        Err(e) => return Err(NetError::InternalServerError(StateError::StoreError(e))),
    }

    let default_room = RoomName::general();
    if let Err(e) = store.ensure_room(&default_room).await {
        return Err(NetError::InternalServerError(StateError::StoreError(e)));
    }
    state.ensure_room(default_room.clone());

    while let Some(command) = server_rx.recv().await {
        handle_server_command(&mut state, &mut store, &mut pending, &default_room, command).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use hoy_core::error::StoreError;
    use hoy_core::memory::InMemoryStore;
    use hoy_core::store::{RoomName, RoomRecord, ServerStore, StoredMessage};
    use hoy_protocol::packet::{ClientPacket, MessageRecord, ServerPacket};
    use hoy_test::async_ok;
    use tokio::sync::mpsc;

    use crate::server::client_id::ClientId;
    use crate::server::command::ServerCommand;
    use crate::server::core::handle_server_command;
    use crate::server::handlers::{
        PendingClients, broadcast_to_room, handle_hello, handle_join_room, handle_list_rooms,
        handle_send_message,
    };
    use crate::server::state::ServerState;

    /// Default room every client is placed in after Hello handshake.
    const DEFAULT_ROOM: &str = RoomName::GENERAL;

    // ── Harness ───────────────────────────────────────────────────────────────

    struct TestHarness {
        state: ServerState,
        pending: PendingClients,
        store: InMemoryStore,
        receivers: HashMap<ClientId, mpsc::Receiver<ServerPacket>>,
    }

    impl TestHarness {
        async fn new() -> Self {
            let mut store = InMemoryStore::new();
            let general = default_room();
            store
                .ensure_room(&general)
                .await
                .expect("default room setup failed");
            let mut state = ServerState::default();
            state.ensure_room(general);
            Self {
                state,
                pending: PendingClients::default(),
                store,
                receivers: HashMap::new(),
            }
        }

        /// Adds a fully identified client directly into `state` (already past `Hello`).
        fn add_identified_client(&mut self, username: &str) -> ClientId {
            let (tx, rx) = mpsc::channel(8);
            let id = ClientId::new();
            self.state
                .add_client(id, username.to_owned(), default_room(), tx)
                .expect("test add_client failed");
            let _ = self.receivers.insert(id, rx);
            id
        }

        /// Adds a pending client (connected but not yet `Hello`'d).
        fn add_pending_client(&mut self) -> ClientId {
            let (tx, rx) = mpsc::channel(8);
            let id = ClientId::new();
            let _ = self.pending.insert(id, tx);
            let _ = self.receivers.insert(id, rx);
            id
        }

        /// Receives the next packet for `id`, timing out after 200 ms.
        async fn recv(&mut self, id: ClientId) -> Option<ServerPacket> {
            let rx = self.receivers.get_mut(&id).expect("receiver missing");
            async_ok!(200, rx.recv())
        }

        /// Asserts no packet is immediately available for `id`.
        fn assert_no_packet(&mut self, id: ClientId) {
            let rx = self.receivers.get_mut(&id).expect("receiver missing");
            assert!(
                rx.try_recv().is_err(),
                "expected no packet but one was pending"
            );
        }

        /// Dispatches one command through the full [`handle_server_command`] path.
        async fn dispatch(&mut self, command: ServerCommand) {
            handle_server_command(
                &mut self.state,
                &mut self.store,
                &mut self.pending,
                &default_room(),
                command,
            )
            .await;
        }
    }

    fn default_room() -> RoomName {
        RoomName::new(DEFAULT_ROOM).expect("hardcoded room name is valid")
    }

    // ── StoreStub ─────────────────────────────────────────────────────────────

    /// Thin wrapper around [`InMemoryStore`] that can be configured to fail on
    /// specific operations, used to test error-handling paths in handlers.
    struct StoreStub {
        fail_ensure_room: bool,
        fail_load_messages: bool,
        inner: InMemoryStore,
    }

    impl StoreStub {
        async fn failing_ensure_room() -> Self {
            let mut inner = InMemoryStore::new();
            inner
                .ensure_room(&default_room())
                .await
                .expect("default room");
            Self {
                fail_ensure_room: true,
                fail_load_messages: false,
                inner,
            }
        }

        async fn failing_load_messages() -> Self {
            let mut inner = InMemoryStore::new();
            inner
                .ensure_room(&default_room())
                .await
                .expect("default room");
            Self {
                fail_ensure_room: false,
                fail_load_messages: true,
                inner,
            }
        }
    }

    impl ServerStore for StoreStub {
        async fn ensure_room(&mut self, name: &RoomName) -> Result<(), StoreError> {
            if self.fail_ensure_room {
                return Err(StoreError::Internal("stub: ensure_room failure".into()));
            }
            self.inner.ensure_room(name).await
        }

        async fn load_rooms(&self) -> Result<Vec<RoomRecord>, StoreError> {
            self.inner.load_rooms().await
        }

        async fn append_message(&mut self, msg: StoredMessage) -> Result<(), StoreError> {
            self.inner.append_message(msg).await
        }

        async fn load_recent_messages(
            &self,
            room: &RoomName,
            limit: usize,
        ) -> Result<Vec<StoredMessage>, StoreError> {
            if self.fail_load_messages {
                return Err(StoreError::Internal("stub: load_messages failure".into()));
            }
            self.inner.load_recent_messages(room, limit).await
        }
    }

    // ── Hello ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn hello_sends_welcome_and_broadcasts_join() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let joiner = h.add_pending_client();
        let observer = h.add_identified_client("observer");

        async_ok!(
            200,
            handle_hello(
                &mut h.state,
                &mut h.store,
                &mut h.pending,
                joiner,
                "alice".into(),
                &default_room()
            )
        );

        //let _ = h.recv(joiner).await.ok_or(())?;
        let ServerPacket::Welcome { username, room } = h.recv(joiner).await.ok_or(())? else {
            return Err(());
        };
        assert_eq!(username, "alice");
        assert_eq!(room, DEFAULT_ROOM);

        // handle_hello calls handle_join_room internally, which broadcasts the join
        // once to all room members except the joiner.
        let ServerPacket::SystemMessage { text } = h.recv(observer).await.ok_or(())? else {
            return Err(());
        };
        assert!(text.contains("alice"), "join message should mention alice");

        // handle_join_room sends RoomJoined to the joiner (no prior history).
        let ServerPacket::RoomJoined {
            room: joined_room,
            messages,
        } = h.recv(joiner).await.ok_or(())?
        else {
            return Err(());
        };
        assert_eq!(joined_room, DEFAULT_ROOM);
        assert!(
            messages.is_empty(),
            "no message history expected on first join"
        );

        // The joiner must not receive the join broadcast about themselves.
        h.assert_no_packet(joiner);
        h.assert_no_packet(observer);
        Ok(())
    }

    #[tokio::test]
    async fn hello_rejects_duplicate_for_same_client() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_identified_client("bruce_lee");

        async_ok!(
            200,
            handle_hello(
                &mut h.state,
                &mut h.store,
                &mut h.pending,
                id,
                "bruce_lee".into(),
                &default_room()
            )
        );

        let ServerPacket::Error { .. } = h.recv(id).await.ok_or(())? else {
            return Err(());
        };
        Ok(())
    }

    #[tokio::test]
    async fn hello_rejects_username_collision() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let _existing = h.add_identified_client("bruce_lee");
        let newcomer = h.add_pending_client();

        async_ok!(
            200,
            handle_hello(
                &mut h.state,
                &mut h.store,
                &mut h.pending,
                newcomer,
                "bruce_lee".into(),
                &default_room()
            )
        );

        let ServerPacket::Error { message } = h.recv(newcomer).await.ok_or(())? else {
            return Err(());
        };
        assert!(message.contains("already in use"));
        Ok(())
    }

    // ── SendMessage ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn send_message_before_hello_produces_no_broadcast() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_pending_client();

        h.dispatch(ServerCommand::Packet {
            client_id: id,
            packet: ClientPacket::SendMessage { text: "hi".into() },
        })
        .await;

        // Pending clients have no entry in state, so no packet is reachable.
        h.assert_no_packet(id);
        Ok(())
    }

    #[tokio::test]
    async fn send_message_broadcasts_to_room_members() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let sender = h.add_identified_client("alice");
        let receiver = h.add_identified_client("bob");

        async_ok!(
            200,
            handle_send_message(&h.state, &mut h.store, sender, "hello".into())
        );

        let ServerPacket::ChatMessage {
            from: sender_from,
            text: sender_text,
            ..
        } = h.recv(sender).await.ok_or(())?
        else {
            return Err(());
        };
        assert_eq!(sender_from, "alice");
        assert_eq!(sender_text, "hello");

        let ServerPacket::ChatMessage {
            from: receiver_from,
            text: receiver_text,
            ..
        } = h.recv(receiver).await.ok_or(())?
        else {
            return Err(());
        };
        assert_eq!(receiver_from, "alice");
        assert_eq!(receiver_text, "hello");
        Ok(())
    }

    // ── Ping ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn ping_returns_pong_for_identified_client() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_identified_client("alice");

        h.dispatch(ServerCommand::Packet {
            client_id: id,
            packet: ClientPacket::Ping,
        })
        .await;

        let ServerPacket::Pong = h.recv(id).await.ok_or(())? else {
            return Err(());
        };
        Ok(())
    }

    #[tokio::test]
    async fn ping_returns_pong_for_pending_client() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_pending_client();

        h.dispatch(ServerCommand::Packet {
            client_id: id,
            packet: ClientPacket::Ping,
        })
        .await;

        let ServerPacket::Pong = h.recv(id).await.ok_or(())? else {
            return Err(());
        };
        Ok(())
    }

    // ── Disconnect ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_named_client_broadcasts_leave() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let leaver = h.add_identified_client("alice");
        let observer = h.add_identified_client("bob");

        h.dispatch(ServerCommand::Disconnected { client_id: leaver })
            .await;

        let ServerPacket::SystemMessage { text } = h.recv(observer).await.ok_or(())? else {
            return Err(());
        };
        assert!(text.contains("alice"));
        Ok(())
    }

    #[tokio::test]
    async fn disconnect_pending_client_does_not_broadcast() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let observer = h.add_identified_client("bob");
        let pending = h.add_pending_client();

        h.dispatch(ServerCommand::Disconnected { client_id: pending })
            .await;

        h.assert_no_packet(observer);
        Ok(())
    }

    // ── JoinRoom ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn join_room_sends_room_joined_and_broadcasts_leave() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let joiner = h.add_identified_client("alice");
        let observer = h.add_identified_client("bob");

        async_ok!(
            200,
            handle_join_room(&mut h.state, &mut h.store, joiner, "rust".into())
        );

        let ServerPacket::RoomJoined { room, messages } = h.recv(joiner).await.ok_or(())? else {
            return Err(());
        };
        assert_eq!(room, "rust");
        assert!(
            messages.is_empty(),
            "no message history expected for a new room"
        );

        let ServerPacket::SystemMessage { text } = h.recv(observer).await.ok_or(())? else {
            return Err(());
        };
        assert!(text.contains("alice") && text.contains("left"));
        Ok(())
    }

    #[tokio::test]
    async fn join_room_rejects_invalid_room_name() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_identified_client("alice");

        async_ok!(
            200,
            handle_join_room(&mut h.state, &mut h.store, id, "INVALID NAME!!".into())
        );

        let ServerPacket::Error { .. } = h.recv(id).await.ok_or(())? else {
            return Err(());
        };
        Ok(())
    }

    #[tokio::test]
    async fn join_room_includes_message_history() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let alice = h.add_identified_client("alice");
        let bob = h.add_identified_client("bob");

        // Alice sends a message; both alice and bob receive the ChatMessage broadcast.
        async_ok!(
            200,
            handle_send_message(&h.state, &mut h.store, alice, "hello world".into())
        );
        let _ = h.recv(alice).await; // drain alice's own ChatMessage
        let _ = h.recv(bob).await; // drain bob's ChatMessage

        // Bob re-joins #general. Because old_room == target the leave broadcast is
        // suppressed, but RoomJoined is still sent with the stored history.
        async_ok!(
            200,
            handle_join_room(&mut h.state, &mut h.store, bob, DEFAULT_ROOM.into())
        );

        let ServerPacket::RoomJoined { room, messages } = h.recv(bob).await.ok_or(())? else {
            return Err(());
        };
        assert_eq!(room, DEFAULT_ROOM);
        assert_eq!(
            messages,
            vec![MessageRecord {
                from: "alice".to_owned(),
                text: "hello world".to_owned(),
            }]
        );

        Ok(())
    }

    // ── ListRooms ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_rooms_returns_sorted_room_names() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_identified_client("alice");

        async_ok!(
            200,
            handle_join_room(&mut h.state, &mut h.store, id, "zebra".into())
        );
        let _ = h.recv(id).await; // consume RoomJoined

        async_ok!(200, handle_list_rooms(&h.state, id));

        let ServerPacket::RoomList { rooms } = h.recv(id).await.ok_or(())? else {
            return Err(());
        };
        assert!(rooms.contains(&"general".to_owned()));
        assert!(rooms.contains(&"zebra".to_owned()));
        let mut sorted = rooms.clone();
        sorted.sort();
        assert_eq!(rooms, sorted, "rooms should be sorted");
        Ok(())
    }

    // ── broadcast_to_room ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn broadcast_skips_excluded_client() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let sender = h.add_identified_client("alice");
        let receiver = h.add_identified_client("bob");

        async_ok!(
            200,
            broadcast_to_room(
                &h.state,
                &default_room(),
                &ServerPacket::SystemMessage {
                    text: "test".into()
                },
                Some(sender),
            )
        );

        h.assert_no_packet(sender);
        assert!(h.recv(receiver).await.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn broadcast_to_room_unknown_room_is_no_op() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let id = h.add_identified_client("alice");
        let unknown_room = RoomName::new("no-such-room").expect("valid name");

        async_ok!(
            200,
            broadcast_to_room(
                &h.state,
                &unknown_room,
                &ServerPacket::SystemMessage {
                    text: "ghost".into()
                },
                None,
            )
        );

        h.assert_no_packet(id);
        Ok(())
    }

    #[tokio::test]
    async fn hello_missing_pending_entry_is_no_op() -> Result<(), ()> {
        let mut h = TestHarness::new().await;
        let observer = h.add_identified_client("observer");
        // A fresh ClientId that was never added to pending or state.
        let unknown = ClientId::new();

        async_ok!(
            200,
            handle_hello(
                &mut h.state,
                &mut h.store,
                &mut h.pending,
                unknown,
                "ghost".into(),
                &default_room()
            )
        );

        h.assert_no_packet(observer);
        Ok(())
    }

    #[tokio::test]
    async fn handle_join_room_store_ensure_fails_sends_error() -> Result<(), ()> {
        let mut state = ServerState::default();
        let general = default_room();
        state.ensure_room(general.clone());

        let (tx, mut rx) = mpsc::channel(8);
        let id = ClientId::new();
        state
            .add_client(id, "alice".to_owned(), general, tx)
            .expect("add_client failed");

        let mut stub = StoreStub::failing_ensure_room().await;
        async_ok!(
            200,
            handle_join_room(&mut state, &mut stub, id, "new-room".into())
        );

        let packet = async_ok!(200, rx.recv()).ok_or(())?;
        assert!(matches!(packet, ServerPacket::Error { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn handle_join_room_store_load_fails_sends_error() -> Result<(), ()> {
        let mut state = ServerState::default();
        let general = default_room();
        state.ensure_room(general.clone());

        let (tx, mut rx) = mpsc::channel(8);
        let id = ClientId::new();
        state
            .add_client(id, "alice".to_owned(), general, tx)
            .expect("add_client failed");

        let mut stub = StoreStub::failing_load_messages().await;
        async_ok!(
            200,
            handle_join_room(&mut state, &mut stub, id, "new-room".into())
        );

        let packet = async_ok!(200, rx.recv()).ok_or(())?;
        assert!(matches!(packet, ServerPacket::Error { .. }));
        Ok(())
    }
}
