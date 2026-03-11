/// Unique server-local identifier assigned to a connected client.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(u64);

impl ClientId {
    /// Constructs client id from a raw numeric value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric client id value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
