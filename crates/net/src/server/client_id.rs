use std::fmt::Display;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique server-local identifier assigned to a connected client.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(u64);

/// Populated client id tracker.
static NEXT: AtomicU64 = AtomicU64::new(1);

impl ClientId {
    /// Constructs client id.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric client id value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ClientId;

    #[test]
    fn client_id_get_returns_inner() {
        let id = ClientId::new();
        assert!(id.get() > 0);
    }

    #[test]
    fn client_id_display() {
        let id = ClientId::new();
        assert_eq!(format!("{id}"), format!("client#{}", id.get()));
    }

    #[test]
    fn client_id_sequential() {
        let a = ClientId::new();
        let b = ClientId::new();
        assert_eq!(b.get(), a.get() + 1);
    }
}
