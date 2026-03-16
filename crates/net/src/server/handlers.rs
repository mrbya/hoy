//! Packet handlers and broadcast helpers for the central server loop.
//!
//! Each `handle_*` function corresponds to one
//! [`ClientPacket`](hoy_protocol::packet::ClientPacket) variant.
//! Helpers `send_packet`, `send_error`, and `broadcast_to_room` are shared
//! utilities used across handlers.

use std::collections::HashMap;

use hoy_core::store::{RoomName, ServerStore, StoredMessage};
use hoy_protocol::packet::{MessageRecord, ServerPacket};
use tokio::sync::mpsc;

use crate::error::StateError;
use crate::server::client_id::ClientId;
use crate::server::state::ServerState;

/// Clients that have connected but have not yet completed the `Hello` handshake.
///
/// Keyed by [`ClientId`]; holds the outbound writer channel until the client
/// identifies itself and can be registered into [`ServerState`].
pub(crate) type PendingClients = HashMap<ClientId, mpsc::Sender<ServerPacket>>;

// ── Handlers ──────────────────────────────────────────────────────────────────

/**
 * Handles a `Hello` handshake from a client.
 *
 * Pulls the sender channel from `pending`, validates the username, registers
 * the client in `state`, and sends `Welcome` followed by a join broadcast to
 * the rest of the default room.
 *
 * # Arguments
 * - `state`: mutable server state,
 * - `pending`: pre-hello client map,
 * - `client_id`: client that sent the hello,
 * - `username`: requested username,
 * - `default_room`: the room to place the client in.
 */
pub(crate) async fn handle_hello(
    state: &mut ServerState,
    store: &mut impl ServerStore,
    pending: &mut PendingClients,
    client_id: ClientId,
    username: String,
    default_room: &RoomName,
) {
    if state.username_of(client_id).is_some() {
        if let Some(tx) = state.sender_of(client_id) {
            send_error(tx, "Client is already initialized").await;
        }
        return;
    }

    let Some(tx) = pending.remove(&client_id) else {
        return;
    };

    match state.add_client(
        client_id,
        username.clone(),
        default_room.clone(),
        tx.clone(),
    ) {
        Ok(()) => {
            send_packet(
                &tx,
                ServerPacket::Welcome {
                    username: username.clone(),
                    room: default_room.to_string(),
                },
            )
            .await;

            broadcast_to_room(
                state,
                default_room,
                &ServerPacket::SystemMessage {
                    text: format!("{username} joined #{default_room}"),
                },
                Some(client_id),
            )
            .await;

            handle_join_room(state, store, client_id, default_room.to_string()).await;
        }

        Err(StateError::UsernameTaken(_)) => {
            send_error(&tx, &format!("Username {username:?} is already in use")).await;
        }

        Err(e) => {
            eprintln!("Error adding client: {e}");
            send_error(&tx, "Internal server error during client handshake").await;
        }
    }
}

/**
 * Handles a `SendMessage` packet from an identified client.
 *
 * Appends the message to the store (non-fatal on failure) and broadcasts it
 * to all members of the sender's current room.
 *
 * # Arguments
 * - `state`: server state,
 * - `store`: persistent store,
 * - `client_id`: client that sent the message,
 * - `text`: message body.
 */
pub(crate) async fn handle_send_message(
    state: &ServerState,
    store: &mut impl ServerStore,
    client_id: ClientId,
    text: String,
) {
    let Some(username) = state.username_of(client_id).map(String::from) else {
        if let Some(tx) = state.sender_of(client_id) {
            send_error(tx, "Client must say hello first").await;
        }
        return;
    };

    let Some(room) = state.current_room_of(client_id).cloned() else {
        return; // should be unreachable (username implies client had joined a room)
    };

    if let Err(e) = store.append_message(StoredMessage {
        room: room.clone(),
        from: username.clone(),
        text: text.clone(),
    }) {
        eprintln!("Message store error: {e}");
        // Non-fatal: broadcast even if storage erred out
    }

    broadcast_to_room(
        state,
        &room,
        &ServerPacket::ChatMessage {
            from: username,
            room: room.to_string(),
            text,
        },
        None,
    )
    .await;
}

/**
 * Handles a `JoinRoom` request from an identified client.
 *
 * Validates the room name, persists it via the store, ensures a runtime entry
 * exists, moves the client, and broadcasts leave/join system messages.
 *
 * # Arguments
 * - `state`: mutable server state,
 * - `store`: persistent store,
 * - `client_id`: client requesting the join,
 * - `room_str`: raw room name string from the packet.
 */
pub(crate) async fn handle_join_room(
    state: &mut ServerState,
    store: &mut impl ServerStore,
    client_id: ClientId,
    room_str: String,
) {
    let Some(tx) = state.sender_of(client_id).cloned() else {
        return;
    };

    if state.username_of(client_id).is_none() {
        send_error(&tx, "Client must say hello first").await;
        return;
    }

    let target = match RoomName::new(room_str) {
        Ok(r) => r,
        Err(e) => {
            send_error(&tx, &e.to_string()).await;
            return;
        }
    };

    if let Err(e) = store.ensure_room(&target) {
        eprintln!("Failed to create room entry for {target}: {e}");
        send_error(&tx, "Failed to create room entry").await;
        return;
    }
    state.ensure_room(target.clone());

    match state.move_client_to_room(client_id, target.clone()) {
        Ok(old_room) => {
            let username = state.username_of(client_id).unwrap_or("").to_owned();

            if old_room != target {
                broadcast_to_room(
                    state,
                    &old_room,
                    &ServerPacket::SystemMessage {
                        text: format!("{username} left #{old_room}"),
                    },
                    None,
                )
                .await;
            }

            let load_result = store.load_recent_messages(&target, 50);
            let Ok(history) = load_result else {
                eprintln!("Failed to load message history for {target}");
                send_error(&tx, "Failed to load message history").await;
                return;
            };

            let messages: Vec<MessageRecord> = if history.is_empty() {
                Vec::new()
            } else {
                history
                    .iter()
                    .map(|m| MessageRecord {
                        from: m.from.clone(),
                        text: m.text.clone(),
                    })
                    .collect()
            };

            send_packet(
                &tx,
                ServerPacket::RoomJoined {
                    room: target.to_string(),
                    messages,
                },
            )
            .await;

            broadcast_to_room(
                state,
                &target,
                &ServerPacket::SystemMessage {
                    text: format!("{username} joined #{target}"),
                },
                Some(client_id),
            )
            .await;
        }

        Err(e) => {
            eprintln!("Failed to move client to room: {e}");
            send_error(&tx, "Internal server error while joining room").await;
        }
    }
}

/**
 * Handles a `ListRooms` request from an identified client.
 *
 * Responds with a sorted list of all rooms currently tracked in runtime state.
 *
 * # Arguments
 * - `state`: server state,
 * - `client_id`: requesting client.
 */
pub(crate) async fn handle_list_rooms(state: &ServerState, client_id: ClientId) {
    let Some(tx) = state.sender_of(client_id).cloned() else {
        return;
    };

    if state.username_of(client_id).is_none() {
        send_error(&tx, "Client must say hello first").await;
        return;
    }

    let mut rooms: Vec<String> = state.room_names().map(RoomName::to_string).collect();
    rooms.sort();

    send_packet(&tx, ServerPacket::RoomList { rooms }).await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Sends a single packet on `tx`, logging any channel send failure.
 *
 * # Arguments
 * - `tx`: outbound client channel,
 * - `packet`: packet to send.
 */
pub(crate) async fn send_packet(tx: &mpsc::Sender<ServerPacket>, packet: ServerPacket) {
    if let Err(e) = tx.send(packet).await {
        eprint!("Client channel send failed: {e}");
    }
}

/**
 * Sends a [`ServerPacket::Error`] on `tx`.
 *
 * # Arguments
 * - `tx`: outbound client channel,
 * - `message`: human-readable error description.
 */
pub(crate) async fn send_error(tx: &mpsc::Sender<ServerPacket>, message: &str) {
    send_packet(
        tx,
        ServerPacket::Error {
            message: message.to_owned(),
        },
    )
    .await;
}

/**
 * Broadcasts `packet` to every member of `room`, optionally skipping one client.
 *
 * # Arguments
 * - `state`: server state,
 * - `room`: target room,
 * - `packet`: packet to broadcast,
 * - `exclude`: optional client id to skip (e.g. the sender).
 */
pub(crate) async fn broadcast_to_room(
    state: &ServerState,
    room: &RoomName,
    packet: &ServerPacket,
    exclude: Option<ClientId>,
) {
    let Some(members) = state.room_members(room) else {
        return;
    };

    let members: Vec<ClientId> = members.iter().copied().collect();
    for member_id in members {
        if exclude == Some(member_id) {
            continue;
        }
        if let Some(tx) = state.sender_of(member_id) {
            send_packet(tx, packet.clone()).await;
        }
    }
}
