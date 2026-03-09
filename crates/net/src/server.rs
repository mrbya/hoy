use std::collections::HashMap;
use std::net::SocketAddr;

use hoy_protocol::packet::{ClientPacket, ServerPacket};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::client_id::ClientId;
use crate::command::ServerCommand;
use crate::connection::handle_connection;
use crate::error::NetError;

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
                let _ = e;
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
                    handle_connection(stream, &client_id, connection_server_tx).await;

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
async fn handle_hello(state: &mut ServerState, client_id: ClientId, client_name: String) {
    let username = client_name;
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
    let Some(username) = state.username_of(&client_id) else {
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
        .broadcast(&ServerPacket::ChatMesage {
            from: String::from(username),
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
