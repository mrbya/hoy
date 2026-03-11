use std::mem;
use std::net::SocketAddr;

use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::sync::mpsc;

use crate::client::command::ClientCommand;
use crate::client::event::ClientEvent;
use crate::client::session::{InternalEvent, spawn_session};
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
    command_tx: mpsc::Sender<ClientCommand>,
}

impl ClientHandle {
    /// Constructs a new client handle.
    #[must_use]
    fn new(command_tx: mpsc::Sender<ClientCommand>) -> Self {
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

/// Frontend-facing event stream emmited by the client core.
#[derive(Debug)]
pub struct ClientEventStream {
    /// Client event receiver stream.
    event_rx: mpsc::Receiver<ClientEvent>,
}

impl ClientEventStream {
    /// Constructs a new client event stream.
    #[must_use]
    fn new(event_rx: mpsc::Receiver<ClientEvent>) -> Self {
        Self { event_rx }
    }

    /**
     * Receives the next client event.
     *
     * # Returns
     * - `Some(ClientEvent)` while the client core is active and events available,
     * - `None` once the event stream is closed or no events emmited.
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

            let session = match spawn_session(server_addr, internal_tx.clone()).await {
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
            if !state.is_connected() {
                return emit_error(event_tx, "Client is not connected").await;
            }

            let Some(packet_tx) = state.packet_tx().cloned() else {
                return emit_error(event_tx, "Client session is unavailable").await;
            };

            let send_result = packet_tx.send(ClientPacket::SendMessage { text }).await;
            if let Err(e) = send_result {
                let _ = e;
                shutdown_state(state).await;

                if !emit_error(event_tx, "Failed to send message to active session").await {
                    return false;
                }

                emit_event(event_tx, ClientEvent::Disconnected).await
            } else {
                true
            }
        }

        ClientCommand::Ping => {
            if !state.is_connected() {
                return emit_error(event_tx, "Client is not connected").await;
            }

            let Some(packet_tx) = state.packet_tx().cloned() else {
                return emit_error(event_tx, "Client session is unavailable").await;
            };

            let send_result = packet_tx.send(ClientPacket::Ping).await;
            if let Err(e) = send_result {
                let _ = e;
                shutdown_state(state).await;

                if !emit_error(event_tx, "Failed to send ping to active session").await {
                    return false;
                }

                emit_event(event_tx, ClientEvent::Disconnected).await
            } else {
                true
            }
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
 * Handles an internal session event and translates it into
 * client state changes and frontend-facing client events.
 *
 * # Arguments
 * - `state`: client state to mutate,
 * - `internal_event`: internal_event to handle,
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
    }
}

/**
 * Emits a single client event toward the frontend.
 *
 * # Arguments
 * - `event_tx`: Frontend event sender.
 * - `message`: Error text to emit.
 *
 * # Returns
 * - `true` if the event was delivered successfully
 * - `false` otherwise.
 */
async fn emit_event(event_tx: &mpsc::Sender<ClientEvent>, event: ClientEvent) -> bool {
    event_tx.send(event).await.is_ok()
}

/**
 * Emmits a client-visible error event.
 *
 * # Arguments
 * - `event_tx`: Frontend event sender.
 * - `message`: Error text to emit.
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
 */
async fn shutdown_state(state: &mut ClientState) {
    let previous_state = mem::take(state);
    *state = ClientState::Disconnected;

    let Some(session) = previous_state.take_session() else {
        return;
    };

    let _shutdown_result = session.shutdown().await;
}
