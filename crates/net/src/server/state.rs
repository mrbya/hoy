//! Live server runtime state.
//!
//! [`ServerState`] is the *only* owner of mutable runtime data.  It lives
//! exclusively inside the central server task; no external task holds
//! references to it.

use std::collections::{HashMap, HashSet};

use hoy_core::store::RoomName;
use hoy_protocol::packet::ServerPacket;
use tokio::sync::mpsc;

use crate::error::StateError;
use crate::server::client_id::ClientId;

/// All live data te server holds about one connected, identified client.
#[derive(Debug)]
pub struct ClientHandle {
    /// Chosen display name set during `Hello` handshake.
    pub username: String,
    /// The room the client is currently in.
    pub current_room: RoomName,
    /// Outbound client writer task channel.
    pub tx: mpsc::Sender<ServerPacket>,
}

/// Live membership state of a room.
#[derive(Debug, Default)]
pub struct RoomRuntimeState {
    /// Clients currently in this room.
    pub members: HashSet<ClientId>,
}

/// Complete live runtime state of the server.
///
/// All fields are private; mutations go through explicit methods so
/// invariants (room membership mirrors `current_room`, username uniqueness) are
/// always maintained in one place.
#[derive(Debug, Default)]
pub struct ServerState {
    /// Map of clients connected/known to server.
    clients: HashMap<ClientId, ClientHandle>,

    /// Map of rooms available on the server.
    rooms: HashMap<RoomName, RoomRuntimeState>,
}

impl ServerState {
    /// Ensures a room exists in the runtime state, creating an empty room if absent.
    ///
    /// Called on startup and when a client creates a new room via `JoinRoom`.
    pub fn ensure_room(&mut self, name: RoomName) {
        self.rooms.entry(name).or_default();
    }

    /// Returns the names of all rooms currently tracked in the runtime state.
    pub fn room_names(&self) -> impl Iterator<Item = &RoomName> {
        self.rooms.keys()
    }

    /// Returns the current member set of `room` or `None` if unknown.
    #[must_use]
    pub fn room_members(&self, room: &RoomName) -> Option<&HashSet<ClientId>> {
        self.rooms.get(room).map(|r| &r.members)
    }

    /// Registers a newly identified client and places them in `room`.
    ///
    /// # Errors
    /// - [`StateError::UsernameTaken`] if another connected client uses this name.
    /// - [`StateError::RoomNotFound`] if `room` has no runtime entry yet.
    pub fn add_client(
        &mut self,
        id: ClientId,
        username: String,
        room: RoomName,
        tx: mpsc::Sender<ServerPacket>,
    ) -> Result<(), StateError> {
        let name_taken = self.clients.values().any(|c| c.username == username);
        if name_taken {
            return Err(StateError::UsernameTaken(username));
        }

        let room_state = self
            .rooms
            .get_mut(&room)
            .ok_or_else(|| StateError::RoomNotFound(room.clone()))?;
        room_state.members.insert(id);

        self.clients.insert(
            id,
            ClientHandle {
                username,
                current_room: room,
                tx,
            },
        );

        Ok(())
    }

    /// Removes a disconnected client and strips then from their `current_room`.
    ///
    /// Returns the removed [`ClientHandle`], or `None` if `id` was unknown.
    pub fn remove_client(&mut self, id: ClientId) -> Option<ClientHandle> {
        let handle = self.clients.remove(&id)?;
        if let Some(room) = self.rooms.get_mut(&handle.current_room) {
            room.members.remove(&id);
        }
        Some(handle)
    }

    /// Moves `id` from its current room to `new_room`.
    ///
    /// Returns the *old* room name so the caller can broadcast leave/join
    /// system messages.
    ///
    /// If the client is already in `new_room` this is a no-op and the current
    /// room name is returned.
    ///
    /// # Errors
    /// - [`StateError::ClientNotFound`] if `id` is unknown.
    /// - [`StateError::RoomNotFound`] if `new_room` has no runtime entry.
    pub fn move_client_to_room(
        &mut self,
        id: ClientId,
        new_room: RoomName,
    ) -> Result<RoomName, StateError> {
        if !self.rooms.contains_key(&new_room) {
            return Err(StateError::RoomNotFound(new_room));
        }

        let handle = self
            .clients
            .get_mut(&id)
            .ok_or(StateError::ClientNotFound(id))?;

        let old_room = handle.current_room.clone();
        if old_room == new_room {
            return Ok(old_room);
        }

        if let Some(r) = self.rooms.get_mut(&old_room) {
            r.members.remove(&id);
        }
        if let Some(r) = self.rooms.get_mut(&new_room) {
            r.members.insert(id);
        }

        handle.current_room = new_room;

        Ok(old_room)
    }

    /// Returns the current room of `id`
    #[must_use]
    pub fn current_room_of(&self, id: ClientId) -> Option<&RoomName> {
        self.clients.get(&id).map(|c| &c.current_room)
    }

    /// Returns the display name of `id`
    #[must_use]
    pub fn username_of(&self, id: ClientId) -> Option<&str> {
        self.clients.get(&id).map(|c| c.username.as_str())
    }

    /// Returns the outbound packet channel for `id`
    #[must_use]
    pub fn sender_of(&self, id: ClientId) -> Option<&mpsc::Sender<ServerPacket>> {
        self.clients.get(&id).map(|c| &c.tx)
    }

    /// Returns `true` if `username` is already in use.
    #[must_use]
    pub fn is_username_taken(&self, username: &str) -> bool {
        self.clients.values().any(|c| c.username == username)
    }
}
