use std::future::Future;
use std::mem;
use std::net::SocketAddr;

use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::sync::mpsc;

use crate::client::command::ClientCommand;
use crate::client::event::ClientEvent;
use crate::client::session::{InternalEvent, SessionHandle, spawn_session};
use crate::client::state::ClientState;
use crate::error::NetError;

/// Public client command channel size.
const CLIENT_COMMAND_CHANNEL_SIZE: usize = 32;
/// Public client event channel size.
const CLIENT_EVENT_CHANNEL_SIZE: usize = 64;
/// Internal session event channel size.
const INTERNAL_EVENT_CHANNEL_SIZE: usize = 64;

/// Frontend-facing handle used to control client core.
#[derive(Debug, Clone)]
pub struct ClientHandle {
    /// Command sender into the client core event loop.
    command_tx: mpsc::Sender<ClientCommand>,
}

impl ClientHandle {
    /// Constructs a new client handle.
    #[must_use]
    const fn new(command_tx: mpsc::Sender<ClientCommand>) -> Self {
        Self { command_tx }
    }

    /**
     * Sends a command to client core.
     *
     * # Arguments
     * - `command`: command to send to client core.
     *
     * # Returns
     * `Ok(())` on success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn send(&self, command: ClientCommand) -> Result<(), NetError> {
        self.command_tx.send(command).await.map_err(|e| {
            let _ = e;
            NetError::ClientChannelClosed
        })
    }

    /**
     * Requests a new connection.
     *
     * # Arguments
     * - `server_addr`: remote server address,
     * - `username`: requested session username.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn connect(&self, server_addr: SocketAddr, username: String) -> Result<(), NetError> {
        self.send(ClientCommand::Connect {
            server_addr,
            username,
        })
        .await
    }

    /**
     * Requests a disconnect of the current session.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn disconnect(&self) -> Result<(), NetError> {
        self.send(ClientCommand::Disconnect).await
    }

    /**
     * Requests sending a chat message.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn send_message(&self, text: String) -> Result<(), NetError> {
        self.send(ClientCommand::SendMessage { text }).await
    }

    /**
     * Requests a ping packet to be sent.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn ping(&self) -> Result<(), NetError> {
        self.send(ClientCommand::Ping).await
    }

    /**
     * Requests a [`ClientCommand::JoinRoom`] packet to be sent.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if client core is no longer accepting commands.
     */
    pub async fn join_room(&self, room: String) -> Result<(), NetError> {
        self.send(ClientCommand::JoinRoom { room }).await
    }

    /**
     * Requests a [`ClientCommand::ListRooms`] packet to be sent.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if client core is no longer accepting commands.
     */
    pub async fn list_rooms(&self) -> Result<(), NetError> {
        self.send(ClientCommand::ListRooms).await
    }

    /**
     * Requests a shutdown of the client core.
     *
     * # Returns
     * `Ok(())` on request sending success.
     *
     * # Errors
     * Returns `NetError` if the client core is no longer accepting commands.
     */
    pub async fn shutdown(&self) -> Result<(), NetError> {
        self.send(ClientCommand::Shutdown).await
    }
}

/// Frontend-facing event stream emitted by the client core.
#[derive(Debug)]
pub struct ClientEventStream {
    /// Client event receiver stream.
    event_rx: mpsc::Receiver<ClientEvent>,
}

impl ClientEventStream {
    /// Constructs a new client event stream.
    #[must_use]
    const fn new(event_rx: mpsc::Receiver<ClientEvent>) -> Self {
        Self { event_rx }
    }

    /**
     * Receives the next client event.
     *
     * # Returns
     * - `Some(ClientEvent)` while the client core is active and events available,
     * - `None` once the event stream is closed or no events emitted.
     */
    pub async fn recv(&mut self) -> Option<ClientEvent> {
        self.event_rx.recv().await
    }
}

/**
 * Spawns the client core event loop and returns the public command/event
 * handles.
 *
 * # Returns
 * Tuple of:
 * - `ClientHandle`: command sender used by the frontend,
 * - `ClientEventStream`: event receiver consumed by the frontend.
 */
#[must_use]
pub fn spawn_client() -> (ClientHandle, ClientEventStream) {
    let (command_tx, command_rx) = mpsc::channel::<ClientCommand>(CLIENT_COMMAND_CHANNEL_SIZE);
    let (event_tx, event_rx) = mpsc::channel::<ClientEvent>(CLIENT_EVENT_CHANNEL_SIZE);
    let (internal_tx, internal_rx) = mpsc::channel::<InternalEvent>(INTERNAL_EVENT_CHANNEL_SIZE);

    tokio::spawn(async move {
        client_loop(command_rx, event_tx, internal_tx, internal_rx).await;
    });

    (
        ClientHandle::new(command_tx),
        ClientEventStream::new(event_rx),
    )
}

/**
 * Runs the central client event loop.
 *
 * The client core owns the session state and reacts to:
 * - public frontend commands,
 * - internal events coming from session reader/writer tasks.
 *
 * Once the loop exits, any active session is shut down.
 */
async fn client_loop(
    mut command_rx: mpsc::Receiver<ClientCommand>,
    event_tx: mpsc::Sender<ClientEvent>,
    internal_tx: mpsc::Sender<InternalEvent>,
    mut internal_rx: mpsc::Receiver<InternalEvent>,
) {
    let mut state = ClientState::default();

    loop {
        tokio::select! {
            maybe_command = command_rx.recv() => {
                let Some(command) = maybe_command else {
                    break;
                };

                let should_continue =
                    handle_command(&mut state, command, &event_tx, &internal_tx).await;

                if !should_continue {
                    break;
                }
            }

            maybe_internal = internal_rx.recv() => {
                let Some(internal_event) = maybe_internal else {
                    break;
                };

                let should_continue =
                    handle_internal_event(&mut state, internal_event, &event_tx).await;

                if !should_continue {
                    break;
                }
            }
        }
    }

    shutdown_state(&mut state).await;
}

/**
 * Handles a client command and translates it into
 * client state changes and frontend-facing client events.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `command`: command to handle,
 * - `event_tx`: ui-facing event channel stream,
 * - `internal_tx`: internal event channel stream.
 *
 * # Returns
 * - `true` if client loop should continue running,
 * - `false` if it should terminate.
 */
async fn handle_command(
    state: &mut ClientState,
    command: ClientCommand,
    event_tx: &mpsc::Sender<ClientEvent>,
    internal_tx: &mpsc::Sender<InternalEvent>,
) -> bool {
    handle_command_with_spawner(state, command, event_tx, internal_tx, spawn_session).await
}

/**
 * Handles a client command using a custom session spawner.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `command`: command to handle,
 * - `event_tx`: ui-facing event channel stream,
 * - `internal_tx`: internal event channel stream,
 * - `spawn_session_fn`: session spawner used for Connect.
 *
 * # Returns
 * - `true` if client loop should continue running,
 * - `false` if it should terminate.
 */
async fn handle_command_with_spawner<F, Fut>(
    state: &mut ClientState,
    command: ClientCommand,
    event_tx: &mpsc::Sender<ClientEvent>,
    internal_tx: &mpsc::Sender<InternalEvent>,
    spawn_session_fn: F,
) -> bool
where
    F: FnOnce(SocketAddr, mpsc::Sender<InternalEvent>) -> Fut,
    Fut: Future<Output = Result<SessionHandle, NetError>>,
{
    match command {
        ClientCommand::Connect {
            server_addr,
            username,
        } => {
            if !state.is_disconnected() {
                return emit_error(event_tx, "Client is already disconnected or connecting").await;
            }

            if !emit_event(
                event_tx,
                ClientEvent::Connecting {
                    server_addr,
                    username: username.clone(),
                },
            )
            .await
            {
                return false;
            }

            let session = match spawn_session_fn(server_addr, internal_tx.clone()).await {
                Ok(s) => s,
                Err(e) => {
                    return emit_error(event_tx, &e.to_string()).await;
                }
            };

            let hello_result = session
                .send(ClientPacket::Hello {
                    username: username.clone(),
                })
                .await;

            if let Err(e) = hello_result {
                let _shutdown_result = session.shutdown().await;
                return emit_error(event_tx, &e.to_string()).await;
            }

            *state = ClientState::AwaitingWelcome {
                server_addr,
                username,
                session,
            };

            true
        }

        ClientCommand::Disconnect => {
            let was_connected = !state.is_disconnected();
            shutdown_state(state).await;

            if was_connected {
                emit_event(event_tx, ClientEvent::Disconnected).await
            } else {
                true
            }
        }

        ClientCommand::SendMessage { text } => {
            handle_generic_command(
                state,
                event_tx,
                ClientPacket::SendMessage { text },
                "Failed to send message to active session",
            )
            .await
        }

        ClientCommand::Ping => {
            handle_generic_command(
                state,
                event_tx,
                ClientPacket::Ping,
                "Failed to send ping to active session",
            )
            .await
        }

        ClientCommand::JoinRoom { room } => {
            handle_generic_command(
                state,
                event_tx,
                ClientPacket::JoinRoom { room },
                "Failed to send a request to join a room",
            )
            .await
        }

        ClientCommand::ListRooms => {
            handle_generic_command(
                state,
                event_tx,
                ClientPacket::ListRooms,
                "Failed to request a list of available rooms",
            )
            .await
        }

        ClientCommand::Shutdown => {
            let was_connected = !state.is_disconnected();
            shutdown_state(state).await;

            if was_connected && !emit_event(event_tx, ClientEvent::Disconnected).await {
                return false;
            }

            false
        }
    }
}

/**
 * Generic command handler that:
 * 1. checks if client connected,
 * 2. if client session available sends a packet
 * 3. shutsdown state and disconnects client if fails.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `event_tx`: ui-facing event channel stream,
 * - `packet`: packet to send in response to command,
 * - `message`: error message if execution fails.
 *
 * # Returns
 * - `true` if client loop should continue running,
 * - `false` if it should terminate.
 */
async fn handle_generic_command(
    state: &mut ClientState,
    event_tx: &mpsc::Sender<ClientEvent>,
    packet: ClientPacket,
    message: &str,
) -> bool {
    if !state.is_connected() {
        return emit_error(event_tx, "Client is not connected").await;
    }

    let Some(packet_tx) = state.packet_tx().cloned() else {
        return emit_error(event_tx, "Client session is unavailable").await;
    };

    let send_result = packet_tx.send(packet).await;
    if let Err(e) = send_result {
        let _ = e;
        shutdown_state(state).await;

        if !emit_error(event_tx, message).await {
            return false;
        }

        emit_event(event_tx, ClientEvent::Disconnected).await
    } else {
        true
    }
}

/**
 * Handles an internal session event and translates it into
 * client state changes and frontend-facing client events.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `internal_event`: `internal_event` to handle,
 * - `event_tx`: ui-facing event channel stream.
 *
 * # Returns
 * - `true` if client loop should continue running,
 * - `false` if it should terminate.
 */
async fn handle_internal_event(
    state: &mut ClientState,
    internal_event: InternalEvent,
    event_tx: &mpsc::Sender<ClientEvent>,
) -> bool {
    match internal_event {
        InternalEvent::PacketReceived(packet) => {
            handle_server_packet(state, packet, event_tx).await
        }

        InternalEvent::ConnectionClosed => {
            if state.is_disconnected() {
                return true;
            }

            shutdown_state(state).await;
            emit_event(event_tx, ClientEvent::Disconnected).await
        }

        InternalEvent::ConnectionError { message } => {
            if state.is_disconnected() {
                return true;
            }

            shutdown_state(state).await;

            if !emit_error(event_tx, &message).await {
                return false;
            }

            emit_event(event_tx, ClientEvent::Disconnected).await
        }
    }
}

/**
 * Handles a decoded server packet and translates it into
 * client state changes and frontend-facing client events.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `packet`: decoded server packet,
 * - `event_tx`: ui-facing event channel stream.
 *
 * # Returns
 * - `true` if client loop should continue running,
 * - `false` if it should terminate.
 */
async fn handle_server_packet(
    state: &mut ClientState,
    packet: ServerPacket,
    event_tx: &mpsc::Sender<ClientEvent>,
) -> bool {
    match packet {
        ServerPacket::Welcome { username, room } => {
            let previous_state = mem::take(state);

            match previous_state {
                ClientState::AwaitingWelcome {
                    server_addr,
                    username: _requested_username,
                    session,
                } => {
                    *state = ClientState::Connected {
                        server_addr,
                        username: username.clone(),
                        room: room.clone(),
                        session,
                    };
                    emit_event(
                        event_tx,
                        ClientEvent::Connected {
                            server_addr,
                            username,
                            room,
                        },
                    )
                    .await
                }

                other_state => {
                    *state = other_state;
                    emit_error(event_tx, "Received unexpected welcome packet").await
                }
            }
        }

        ServerPacket::ChatMessage { from, room, text } => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::MessageReceived { from, room, text }).await
        }

        ServerPacket::SystemMessage { text } => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::SystemMessage { text }).await
        }

        ServerPacket::Error { message } => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::Error { message }).await
        }

        ServerPacket::Pong => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::Pong).await
        }

        ServerPacket::RoomJoined { room } => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::RoomJoined { room }).await
        }

        ServerPacket::RoomList { rooms } => {
            if state.is_disconnected() {
                return true;
            }

            emit_event(event_tx, ClientEvent::RoomList { rooms }).await
        }
    }
}

/**
 * Emits a single client event toward the frontend.
 *
 * # Arguments
 * - `event_tx`: frontend event sender,
 * - `event`: client event to emit.
 *
 * # Returns
 * - `true` if the event was delivered successfully,
 * - `false` otherwise.
 */
async fn emit_event(event_tx: &mpsc::Sender<ClientEvent>, event: ClientEvent) -> bool {
    event_tx.send(event).await.is_ok()
}

/**
 * Emits a client-visible error event.
 *
 * # Arguments
 * - `event_tx`: frontend event sender,
 * - `message`: error text to emit.
 *
 * # Returns
 * - `true` if the event was delivered successfully
 * - `false` otherwise.
 */
async fn emit_error(event_tx: &mpsc::Sender<ClientEvent>, message: &str) -> bool {
    emit_event(
        event_tx,
        ClientEvent::Error {
            message: String::from(message),
        },
    )
    .await
}

/**
 * Shuts down the currently active client session, if any, and resets the state
 * to `Disconnected`.
 *
 * # Arguments
 * - `state`: client state to reset.
 */
async fn shutdown_state(state: &mut ClientState) {
    let previous_state = mem::take(state);
    *state = ClientState::Disconnected;

    let Some(session) = previous_state.take_session() else {
        return;
    };

    let _shutdown_result = session.shutdown().await;
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use hoy_protocol::codec::encode_frame;
    use hoy_protocol::frame_buffer::FrameBuffer;
    use hoy_protocol::packet::{ClientPacket, ServerPacket};
    use hoy_test::async_ok;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream};
    use tokio::sync::mpsc;

    use super::{
        ClientState, handle_command, handle_command_with_spawner, handle_internal_event,
        shutdown_state,
    };
    use crate::client::command::ClientCommand;
    use crate::client::event::ClientEvent;
    use crate::client::session::{InternalEvent, SessionHandle};
    use crate::error::NetError;

    async fn read_client_packet<R>(stream: &mut R) -> Result<ClientPacket, ()>
    where
        R: AsyncRead + Unpin,
    {
        let mut buffer = [0_u8; 1024];
        let mut frame_buffer = FrameBuffer::with_capacity(2048);

        loop {
            let bytes_read = async_ok!(200, stream.read(&mut buffer)).map_err(|_err| ())?;
            if bytes_read == 0 {
                return Err(());
            }

            let chunk = buffer.get(..bytes_read).ok_or(())?;
            frame_buffer.append(chunk).map_err(|_err| ())?;

            if let Some(packet) = frame_buffer
                .try_decode::<ClientPacket>()
                .map_err(|_err| ())?
            {
                return Ok(packet);
            }
        }
    }

    fn dummy_session() -> SessionHandle {
        let (packet_tx, _packet_rx) = mpsc::channel(1);
        let reader_task = tokio::spawn(async { Ok(()) });
        let writer_task = tokio::spawn(async { Ok(()) });
        SessionHandle::new(packet_tx, reader_task, writer_task)
    }

    fn session_with_stream() -> (SessionHandle, DuplexStream) {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let (_reader, mut writer) = tokio::io::split(client_side);
        let (packet_tx, mut packet_rx) = mpsc::channel(8);

        let reader_task = tokio::spawn(async { Ok::<(), NetError>(()) });
        let writer_task = tokio::spawn(async move {
            while let Some(packet) = packet_rx.recv().await {
                let frame = encode_frame(&packet).map_err(NetError::Protocol)?;
                writer.write_all(&frame).await.map_err(NetError::Io)?;
            }
            Ok::<(), NetError>(())
        });

        (
            SessionHandle::new(packet_tx, reader_task, writer_task),
            server_side,
        )
    }

    async fn recv_event(rx: &mut mpsc::Receiver<ClientEvent>) -> Option<ClientEvent> {
        async_ok!(200, rx.recv())
    }

    #[tokio::test]
    async fn connect_emits_connecting_sends_hello_and_sets_awaiting_welcome() -> Result<(), ()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 5555));

        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (internal_tx, _internal_rx) = mpsc::channel(8);
        let mut state = ClientState::default();

        let (session, mut server_stream) = session_with_stream();
        let mut session_opt = Some(session);

        let command = ClientCommand::Connect {
            server_addr: addr,
            username: String::from("bruce_lee"),
        };

        let spawner = move |_addr, _internal| {
            let spawned_session = session_opt.take().expect("session already taken");
            async move { Ok(spawned_session) }
        };

        let should_continue = async_ok!(
            200,
            handle_command_with_spawner(&mut state, command, &event_tx, &internal_tx, spawner)
        );
        assert!(should_continue);

        let event = recv_event(&mut event_rx).await.ok_or(())?;
        match event {
            ClientEvent::Connecting {
                server_addr,
                username,
            } => {
                assert_eq!(server_addr, addr);
                assert_eq!(username, "bruce_lee");
            }
            _ => return Err(()),
        }

        let packet = read_client_packet(&mut server_stream).await?;
        match packet {
            ClientPacket::Hello { username } => {
                assert_eq!(username, "bruce_lee");
            }
            _ => return Err(()),
        }

        assert!(state.is_awaiting_welcome());
        shutdown_state(&mut state).await;
        Ok(())
    }

    #[tokio::test]
    async fn welcome_packet_transitions_to_connected_and_emits_connected() -> Result<(), ()> {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let mut state = ClientState::AwaitingWelcome {
            server_addr: SocketAddr::from(([127, 0, 0, 1], 1234)),
            username: String::from("requester"),
            session: dummy_session(),
        };

        let internal_event = InternalEvent::PacketReceived(ServerPacket::Welcome {
            username: String::from("bruce_lee"),
            room: String::from("#general"),
        });

        let should_continue = async_ok!(
            200,
            handle_internal_event(&mut state, internal_event, &event_tx)
        );
        assert!(should_continue);

        let event = recv_event(&mut event_rx).await.ok_or(())?;
        match event {
            ClientEvent::Connected {
                server_addr,
                username,
                room,
            } => {
                assert_eq!(server_addr, SocketAddr::from(([127, 0, 0, 1], 1234)));
                assert_eq!(username, "bruce_lee");
                assert_eq!(room, "#general");
            }
            _ => return Err(()),
        }

        assert!(state.is_connected());
        shutdown_state(&mut state).await;
        Ok(())
    }

    #[tokio::test]
    async fn connection_error_emits_error_then_disconnected_and_resets_state() -> Result<(), ()> {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let mut state = ClientState::Connected {
            server_addr: SocketAddr::from(([127, 0, 0, 1], 1234)),
            username: String::from("bruce_lee"),
            room: String::from("#general"),
            session: dummy_session(),
        };

        let internal_event = InternalEvent::ConnectionError {
            message: String::from("bad connection"),
        };

        let should_continue = async_ok!(
            200,
            handle_internal_event(&mut state, internal_event, &event_tx)
        );
        assert!(should_continue);

        let first = recv_event(&mut event_rx).await.ok_or(())?;
        let second = recv_event(&mut event_rx).await.ok_or(())?;

        match first {
            ClientEvent::Error { message } => {
                assert_eq!(message, "bad connection");
            }
            _ => return Err(()),
        }

        match second {
            ClientEvent::Disconnected => {}
            _ => return Err(()),
        }

        assert!(state.is_disconnected());
        Ok(())
    }

    #[tokio::test]
    async fn connection_closed_emits_disconnected_and_resets_state() -> Result<(), ()> {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let mut state = ClientState::Connected {
            server_addr: SocketAddr::from(([127, 0, 0, 1], 1234)),
            username: String::from("bruce_lee"),
            room: String::from("#general"),
            session: dummy_session(),
        };

        let should_continue = async_ok!(
            200,
            handle_internal_event(&mut state, InternalEvent::ConnectionClosed, &event_tx)
        );
        assert!(should_continue);

        let event = recv_event(&mut event_rx).await.ok_or(())?;
        match event {
            ClientEvent::Disconnected => {}
            _ => return Err(()),
        }

        assert!(state.is_disconnected());
        Ok(())
    }

    #[tokio::test]
    async fn send_message_and_ping_while_disconnected_emit_errors() -> Result<(), ()> {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (internal_tx, _internal_rx) = mpsc::channel(8);
        let mut state = ClientState::default();

        let send_command = ClientCommand::SendMessage {
            text: String::from("hi"),
        };
        let send_should_continue = async_ok!(
            200,
            handle_command(&mut state, send_command, &event_tx, &internal_tx)
        );
        assert!(send_should_continue);

        let ping_command = ClientCommand::Ping;
        let ping_should_continue = async_ok!(
            200,
            handle_command(&mut state, ping_command, &event_tx, &internal_tx)
        );
        assert!(ping_should_continue);

        let first = recv_event(&mut event_rx).await.ok_or(())?;
        let second = recv_event(&mut event_rx).await.ok_or(())?;

        match first {
            ClientEvent::Error { message } => {
                assert_eq!(message, "Client is not connected");
            }
            _ => return Err(()),
        }

        match second {
            ClientEvent::Error { message } => {
                assert_eq!(message, "Client is not connected");
            }
            _ => return Err(()),
        }

        assert!(state.is_disconnected());
        Ok(())
    }
}
