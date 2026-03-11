use std::net::SocketAddr;

use hoy_protocol::packet::ClientPacket;
use tokio::sync::mpsc;

use crate::client::session::SessionHandle;

/**
 * Internal client state.
 *
 * This state machine is owned exclusively by the client core event loop
 * and tracks the lifecycle of the active client session.
 */
#[allow(dead_code)]
#[derive(Debug, Default)]
pub(crate) enum ClientState {
    /// Client disconnected - no active session.
    #[default]
    Disconnected,

    /**
     * An active session exsists and the client has sent `Hello`, but
     * has not recevied a `Welcome` acknowledgement from the server.
     */
    AwaitingWelcome {
        /// Connected server address.
        server_addr: SocketAddr,
        /// Requested username.
        username: String,
        /// Active network session handle.
        session: SessionHandle,
    },

    /// Client is connected (`Hello` -> `Welcome` handshake complete.)
    Connected {
        /// Connected server address.
        server_addr: SocketAddr,
        /// Requested username.
        username: String,
        /// Currently active room.
        room: String,
        /// Active network session handle.
        session: SessionHandle,
    },
}

#[allow(dead_code)]
impl ClientState {
    /// Returns true if the client is currently disconnected.
    #[must_use]
    pub const fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected)
    }

    /// Returns true if the client is currently awaiting welcome.
    #[must_use]
    pub const fn is_awaiting_welcome(&self) -> bool {
        matches!(self, Self::AwaitingWelcome { .. })
    }

    /// Returns true if the client is currently connected.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// Returns connected server address, if any.
    #[must_use]
    pub const fn server_addr(&self) -> Option<SocketAddr> {
        match self {
            &Self::Disconnected => None,
            &Self::AwaitingWelcome { server_addr, .. } | &Self::Connected { server_addr, .. } => {
                Some(server_addr)
            }
        }
    }

    /// Returns current session username, if any.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        match self {
            &Self::Disconnected => None,
            &Self::AwaitingWelcome { ref username, .. } | &Self::Connected { ref username, .. } => {
                Some(username.as_str())
            }
        }
    }

    /// Returns the currently joined room, if any.
    #[must_use]
    pub fn room(&self) -> Option<&str> {
        match *self {
            Self::Connected { ref room, .. } => Some(room.as_str()),
            Self::AwaitingWelcome { .. } | Self::Disconnected => None,
        }
    }

    /// Returns the outgoing packet channel of the active session, if any.
    #[must_use]
    pub const fn packet_tx(&self) -> Option<&mpsc::Sender<ClientPacket>> {
        match self {
            &Self::Disconnected => None,
            &Self::AwaitingWelcome { ref session, .. } | &Self::Connected { ref session, .. } => {
                Some(session.packet_tx())
            }
        }
    }

    /// Returns a mutable reference to the active session handle, if any.
    #[must_use]
    pub const fn session_mut(&mut self) -> Option<&mut SessionHandle> {
        match *self {
            Self::Disconnected => None,
            Self::AwaitingWelcome {
                ref mut session, ..
            }
            | Self::Connected {
                ref mut session, ..
            } => Some(session),
        }
    }

    /// Consume the current client state and return the owned session handle, if any.
    #[must_use]
    pub fn take_session(self) -> Option<SessionHandle> {
        match self {
            Self::Disconnected => None,
            Self::AwaitingWelcome { session, .. } | Self::Connected { session, .. } => {
                Some(session)
            }
        }
    }
}
