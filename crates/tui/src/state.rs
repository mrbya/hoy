//! TUI display state.
//!
//! [`TuiState`](crate::state::TuiState) is the single source of truth for everything the renderer needs.
//! It is owned exclusively by the app loop and mutated by translating
//! [`ClientEvent`](hoy_net::client::event::ClientEvent) — no async concerns here.

use std::collections::HashMap;
use std::net::SocketAddr;

/// A single entry in a room's messge history
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// A message sent by a user.
    User {
        /// Sender's display name.
        from: String,
        /// Message text.
        text: String,
    },

    /// A server-generated system notification.
    System {
        /// Notification text.
        text: String,
    },
}

/// Connection lifecycle state shown in the status bar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    /// No active connection.
    #[default]
    Disconnected,

    /// TCP connected; waiting for server handshake.
    Connecting,

    /// Handshake complete; session active.
    Connected,
}

/**
 * Complete displayable state of the TUI.
 *
 * All fields are `pub` so the renderer can read them directly.
 * Mutations still go through helper methods to keep invariants
 * in one place (e.g. scroll reset on room change).
 */
#[derive(Debug, Default)]
pub struct TuiState {
    /// Current connection status.
    pub status: ConnectionStatus,
    /// Address of the connected server, if any.
    pub server_addr: Option<SocketAddr>,
    /// User display name received after handshake,
    pub username: Option<String>,
    /// Name of the currently active room.
    pub current_room: Option<String>,
    /// Room names from the most recent `ListRooms` response.
    pub rooms: Vec<String>,
    /// Per-room message history.
    pub messages: HashMap<String, Vec<ChatMessage>>,
    /// Text currently in the input bar.
    pub input: String,
    /// Byte offset of the cursor within [`Self::input`]
    pub cursor_pos: usize,
    /// Last notification (error or status), shown in the status bar.
    pub notification: Option<String>,
    /// Per-room scroll offset: number of messages scrolled up from the bottom
    pub scroll: HashMap<String, usize>,
}

impl TuiState {
    /**
     * Returns the message history for the currently active room.
     *
     * # Returns
     * Slice of messages, or an empty slice if no room is active.
     */
    #[must_use]
    pub fn current_messages(&self) -> &[ChatMessage] {
        self.current_room
            .as_deref()
            .and_then(|r| self.messages.get(r))
            .map_or(&[], Vec::as_slice)
    }

    /**
     * Overwrites status.
     *
     * # Returns
     * Self.
     */
    pub const fn status(&mut self, status: ConnectionStatus) -> &mut Self {
        self.status = status;
        self
    }

    /**
     * Overwrites `server_addr`.
     *
     * # Returns
     * Self.
     */
    pub const fn server_addr(&mut self, addr: Option<SocketAddr>) -> &mut Self {
        self.server_addr = addr;
        self
    }

    /**
     * Overwrites username.
     *
     * # Returns
     * Self.
     */
    pub fn username(&mut self, username: Option<String>) -> &mut Self {
        self.username = username;
        self
    }

    /**
     * Overwrites `current_room`.
     *
     * # Returns
     * Self.
     */
    pub fn current_room(&mut self, room: Option<String>) -> &mut Self {
        self.current_room = room;
        self
    }

    /**
     * Overwrites notification.
     *
     * # Returns
     * Self.
     */
    pub fn notification(&mut self, notification: Option<String>) -> &mut Self {
        self.notification = notification;
        self
    }

    /**
     * Appends char to state input buffer and updates cursor.
     */
    pub fn append_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos = self.cursor_pos.saturating_add(c.len_utf8());
    }

    /**
     * Returns the scroll offset for the currently active room.
     *
     * # Returns
     * Scroll offset in messages from the bottom, or `0` if no room is active.
     */
    #[must_use]
    pub fn current_scroll(&self) -> usize {
        *self
            .current_room
            .as_deref()
            .and_then(|r| self.scroll.get(r))
            .unwrap_or(&0)
    }

    /**
     * Appends a message to `room`'s history.
     *
     * # Arguments
     * - `room`: target room name,
     * - `msg`: message to append.
     */
    pub fn push_message(&mut self, room: &str, msg: ChatMessage) {
        self.messages.entry(room.to_owned()).or_default().push(msg);
    }

    /**
     * Switches to `room` and replaces its history with `history`.
     *
     * Scroll is reset to the bottom on every join so the user always sees
     * the most recent messages first.
     *
     * # Arguments
     * - `room`: name of the room to join,
     * - `history`: message history delivered by the server.
     */
    pub fn join_room(&mut self, room: String, history: Vec<ChatMessage>) {
        let _ = self.messages.insert(room.clone(), history);
        let _ = self.scroll.insert(room.clone(), 0);
        self.current_room = Some(room);
    }

    /**
     * Scrolls the current room up by one message.
     */
    pub fn scroll_up(&mut self) {
        let Some(room) = self.current_room.clone() else {
            return;
        };

        let total = self.current_messages().len();
        let offset = self.scroll.entry(room).or_insert(0);
        *offset = offset.saturating_add(1).min(total);
    }

    /**
     * Scrolls the current room down by one message.
     */
    pub fn scroll_down(&mut self) {
        let Some(room) = self.current_room.clone() else {
            return;
        };

        let offset = self.scroll.entry(room).or_insert(0);
        *offset = offset.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::state::{ChatMessage, ConnectionStatus, TuiState};

    #[test]
    fn state_overrides() {
        let mut state = TuiState::default();
        assert_ne!(state.status, ConnectionStatus::Connected);
        assert!(state.server_addr.is_none());
        assert!(state.username.is_none());
        assert!(state.current_room.is_none());
        assert!(state.notification.is_none());

        state
            .status(ConnectionStatus::Connected)
            .server_addr(Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234)))
            .username(Some("bob".into()))
            .current_room(Some("general".into()))
            .notification(Some("warning".into()));

        assert_eq!(state.status, ConnectionStatus::Connected);
        assert!(state.server_addr.is_some());
        assert!(state.username.is_some());
        assert!(state.current_room.is_some());
        assert!(state.notification.is_some());
    }

    #[test]
    fn state_join_room() {
        let mut state = TuiState::default();
        assert!(state.current_room.is_none());

        state.join_room(
            "general".into(),
            vec![
                ChatMessage::User {
                    from: "bob".into(),
                    text: "msg 1".into(),
                },
                ChatMessage::User {
                    from: "bob".into(),
                    text: "msg 2".into(),
                },
            ],
        );

        assert!(state.current_room.is_some());
        assert_eq!(state.current_room, Some("general".into()));
        assert_eq!(state.current_messages().len(), 2);
    }

    #[test]
    fn state_push_message() {
        let mut state = TuiState::default();
        assert_eq!(state.current_messages().len(), 0);
        state.push_message(
            "general",
            ChatMessage::User {
                from: "bob".into(),
                text: "hello".into(),
            },
        );
        assert_eq!(state.current_messages().len(), 0);

        state.join_room("general".into(), vec![]);
        assert_eq!(state.current_messages().len(), 0);
        state.push_message(
            "general",
            ChatMessage::User {
                from: "bob".into(),
                text: "hello".into(),
            },
        );
        assert_eq!(state.current_messages().len(), 1);
    }

    #[test]
    fn state_scroll() {
        let mut state = TuiState::default();
        state.scroll_up();
        assert_eq!(state.current_scroll(), 0);

        state.join_room("general".into(), vec![]);
        state.push_message(
            "general",
            ChatMessage::User {
                from: "bob".into(),
                text: "hello".into(),
            },
        );
        state.push_message(
            "general",
            ChatMessage::User {
                from: "bob".into(),
                text: "hello".into(),
            },
        );
        state.scroll_up();
        state.scroll_up();
        assert_eq!(state.current_scroll(), 2);
        state.scroll_down();
        assert_eq!(state.current_scroll(), 1);

        state.join_room("new".into(), vec![]);
        assert_eq!(state.current_scroll(), 0);
    }

    #[test]
    fn state_input() {
        let mut state = TuiState::default();
        assert_eq!(state.cursor_pos, 0);
        assert_eq!(state.input, "");

        let chars = ['h', 'e', 'l', 'l', 'o'];

        for c in chars {
            state.append_char(c);
        }

        assert_eq!(state.cursor_pos, chars.len());
        assert_eq!(state.input, "hello");
    }
}
