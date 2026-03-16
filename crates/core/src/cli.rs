use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

use crate::dbstore::DbStore;
use crate::error::{HoyError, StoreError};
use crate::memory::InMemoryStore;

/// Hoy command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "hoy",
    about = "A TUI real time messaging app.",
    version,
    propagate_version = true,
    after_help = r#"Example:
  # Start server
  hoy -s

  # Connect with a client in another shell
  hoy -u bruce_lee

For more info, see https://gitlab.com/byacrates/hoy
"#
)]
struct Cli {
    /// Run in server mode instead of client mode
    #[arg(short = 's', long = "server", default_value_t = false)]
    server: bool,

    /// Port to bind the server or connect the client to
    #[arg(short = 'p', long = "port", default_value_t = 7777)]
    port: u16,

    /// Server address to connect to. [default: localhost]
    #[arg(short = 'a', long = "address")]
    ipv4: Option<Ipv4Addr>,

    /// Client username (required in client mode)
    #[arg(short = 'u', long = "username")]
    username: Option<String>,

    /// Path to storage db file [default: ~/.local/share/hoy/hoy.db]
    #[arg(short = 'd', long = "db", value_name = "FILE")]
    db: Option<PathBuf>,
}

/// Hoy executable core.
#[derive(Debug)]
pub struct Hoy {
    /// Args consumed by Hoy Cli.
    args: Cli,
}

impl Default for Hoy {
    fn default() -> Self {
        Self { args: Cli::parse() }
    }
}

impl Hoy {
    /**
     * Resolves server address to run or for a client to connect to.
     */
    #[must_use]
    pub fn resolve_address(&self) -> SocketAddr {
        self.args.ipv4.map_or_else(
            || SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.args.port),
            |addr| SocketAddr::new(IpAddr::V4(addr), self.args.port),
        )
    }

    /**
     * Constructs and connects hoy data storage for Hoy server.
     *
     * # Returns
     * Constructed [`DbStore`] storage.
     *
     * # Errors
     * Returns [`StoreError`] of storage construction/connection fails.
     */
    pub async fn construct_store(&self) -> Result<DbStore, StoreError> {
        let store = DbStore::new(self.args.db.clone()).await?;
        Ok(store)
    }

    /**
     * Returns a temporary in-memory storage for Hoy server.
     *
     * # Returns
     * Constructed [`InMemoryStore`] storage.
     */
    #[must_use]
    pub fn incognito_store() -> InMemoryStore {
        InMemoryStore::new()
    }

    /**
     * Returns true if `-s/--server` flag was provided.
     */
    #[must_use]
    pub const fn run_server(&self) -> bool {
        self.args.server
    }

    /**
     * Resolves client username.
     *
     * # Returns
     * `Ok(String)` if resolved succesfully.
     *
     * # Errors
     * Returns [`HoyError`] if no username provided to cli.
     */
    pub fn resolve_username(&self) -> Result<String, HoyError> {
        let Some(ref username) = self.args.username else {
            return Err(HoyError::NoUsername);
        };

        Ok(username.clone())
    }
}
