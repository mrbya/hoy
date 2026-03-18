//! TUI error types.

use thiserror::Error;

/// TUI error variants.
#[derive(Debug, Error)]
pub enum TuiError {
    /// Terminal I/O error.
    #[error("Terminal error: {0}")]
    Io(#[from] std::io::Error),

    /// Networking error emmited from client core.
    #[error("Network error: {0}")]
    Net(#[from] hoy_net::error::NetError),
}
