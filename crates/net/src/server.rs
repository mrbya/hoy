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
    async fn broadcast(&self, packet: &ServerPacket) {
        for client in self.clients.values() {
            let send_result = client.tx.send(packet.clone()).await;
            if let Err(e) = send_result {
                let _ = e;
            }
        }
    }

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

    fn username_of(&self, client_id: &ClientId) -> Option<&str> {
        self.clients
            .get(client_id)
            .and_then(|client| client.username.as_deref())
    }

    fn username_exists(&self, username: &str) -> bool {
        self.clients
            .values()
            .filter_map(|client| client.username.as_deref())
            .any(|existing| existing == username)
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

    let accept_tx = server_tx.clone();

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

            let connection_server_tx = accept_tx.clone();

            tokio::spawn(async move {
                let connection_result =
                    handle_connection(stream, &client_id, connection_server_tx).await;

                if let Err(e) = connection_result {
                    eprintln!("Connection error: {e:?}");
                }
            });
        }
    });

    let mut state = ServerState::default();

    while let Some(command) = server_rx.recv().await {
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
                ClientPacket::Hello {
                    username: client_name,
                } => {
                    let username = client_name.clone();
                    if state.username_of(&client_id).is_some() {
                        let _ = state
                            .send_to_client(
                                client_id,
                                ServerPacket::Error {
                                    message: String::from("client is already initialized"),
                                },
                            )
                            .await;
                        continue;
                    }

                    if state.username_exists(&username) {
                        let _ = state
                            .send_to_client(
                                client_id,
                                ServerPacket::Error {
                                    message: String::from("Username is already in use"),
                                },
                            )
                            .await;
                        continue;
                    }

                    if let Some(client) = state.clients.get_mut(&client_id) {
                        client.username = Some(username.clone());
                    }

                    let _ = state
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
                ClientPacket::SendMessage { text } => {
                    let Some(username) = state.username_of(&client_id) else {
                        let _ = state
                            .send_to_client(
                                client_id,
                                ServerPacket::Error {
                                    message: String::from("Client must say hello first"),
                                },
                            )
                            .await;
                        continue;
                    };

                    state
                        .broadcast(&ServerPacket::ChatMesage {
                            from: String::from(username),
                            room: String::from(DEFAULT_ROOM),
                            text,
                        })
                        .await;
                }
                ClientPacket::Ping => {
                    let _ = state.send_to_client(client_id, ServerPacket::Pong).await;
                }
            },
        }
    }

    Ok(())
}
