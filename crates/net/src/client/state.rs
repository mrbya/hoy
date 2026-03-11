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

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use hoy_protocol::packet::ClientPacket;
    use hoy_test::{assert_matches, async_ok};
    use tokio::sync::mpsc;

    use crate::client::session::SessionHandle;
    use crate::client::state::ClientState;
    use crate::error::NetError;

    fn dummy_session() -> SessionHandle {
        let (tx, _rx) = mpsc::channel::<ClientPacket>(1);

        let rt = tokio::spawn(async { Ok::<(), NetError>(()) });
        let wt = tokio::spawn(async { Ok::<(), NetError>(()) });

        SessionHandle::new(tx, rt, wt)
    }

    fn session_with_rx() -> (SessionHandle, mpsc::Receiver<ClientPacket>) {
        let (tx, rx) = mpsc::channel::<ClientPacket>(1);

        let rt = tokio::spawn(async { Ok::<(), NetError>(()) });
        let wt = tokio::spawn(async { Ok::<(), NetError>(()) });

        (SessionHandle::new(tx, rt, wt), rx)
    }

    fn server_addr() -> SocketAddr {
        SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }

    fn username() -> String {
        String::from("bruce_lee")
    }

    fn room() -> String {
        String::from("#general")
    }

    const fn disconnected() -> ClientState {
        ClientState::Disconnected
    }

    fn awaiting_welcome() -> ClientState {
        let server_addr = server_addr();
        let username = username();
        let session = dummy_session();

        ClientState::AwaitingWelcome {
            server_addr,
            username,
            session,
        }
    }

    fn connected() -> ClientState {
        let server_addr = server_addr();
        let username = username();
        let room = room();
        let session = dummy_session();

        ClientState::Connected {
            server_addr,
            username,
            room,
            session,
        }
    }

    fn variants() -> (ClientState, ClientState, ClientState) {
        (disconnected(), awaiting_welcome(), connected())
    }

    #[test]
    fn state_defaults_to_disconnected() {
        let state = ClientState::default();
        assert_matches!(state, ClientState::Disconnected);
    }

    #[tokio::test]
    async fn state_is_connected_and_disconnected() {
        let (disconnected, awaiting_welcome, connected) = variants();

        assert!(disconnected.is_disconnected());
        assert!(!disconnected.is_awaiting_welcome());
        assert!(!disconnected.is_connected());

        assert!(!awaiting_welcome.is_disconnected());
        assert!(awaiting_welcome.is_awaiting_welcome());
        assert!(!awaiting_welcome.is_connected());

        assert!(!connected.is_disconnected());
        assert!(!connected.is_awaiting_welcome());
        assert!(connected.is_connected());
    }

    #[tokio::test]
    async fn state_value_extraction() {
        let (mut disconnected, mut awaiting_welcome, mut connected) = variants();

        assert_eq!(disconnected.server_addr(), None);
        assert_eq!(awaiting_welcome.server_addr(), Some(server_addr()));
        assert_eq!(connected.server_addr(), Some(server_addr()));

        assert_eq!(disconnected.username(), None);
        assert_eq!(awaiting_welcome.username(), Some(username()).as_deref());
        assert_eq!(connected.username(), Some(username()).as_deref());

        assert_eq!(disconnected.room(), None);
        assert_eq!(awaiting_welcome.room(), None);
        assert_eq!(connected.room(), Some(room()).as_deref());

        assert!(disconnected.session_mut().is_none());
        assert!(awaiting_welcome.session_mut().is_some());
        assert!(connected.session_mut().is_some());

        assert!(disconnected.take_session().is_none());
        assert!(awaiting_welcome.take_session().is_some());
        assert!(connected.take_session().is_some());
    }

    #[tokio::test]
    async fn packet_tx_sends_when_session_present() -> Result<(), ()> {
        let (session, mut rx) = session_with_rx();
        let state = ClientState::AwaitingWelcome {
            server_addr: server_addr(),
            username: username(),
            session,
        };

        let sender = state.packet_tx().ok_or(())?;
        async_ok!(200, sender.send(ClientPacket::Ping)).map_err(|_err| ())?;

        let received = async_ok!(200, rx.recv()).ok_or(())?;
        assert_eq!(received, ClientPacket::Ping);

        assert!(ClientState::Disconnected.packet_tx().is_none());
        Ok(())
    }
}
