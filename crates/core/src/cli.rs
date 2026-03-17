use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

use crate::dbstore::DbStore;
use crate::error::{HoyError, StoreError};
use crate::memory::InMemoryStore;

/// Default server address port.
const DEFAULT_PORT: u16 = 7777;

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
    #[arg(short = 'p', long = "port", default_value_t = DEFAULT_PORT)]
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

    /// Run server with a temporary storage instead of a persistent db storage
    #[arg(short = 'i', long = "incognito", default_value_t = false)]
    incognito: bool,

    /// Run client with a simple stdout UI instead of TUI.
    #[arg(short = 'n', long = "no-tui", default_value_t = false)]
    notui: bool,
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
     * Constructs hoy CLI and parses + validates provided CLI args.
     *
     * # Returns
     * `Ok(Hoy)` on success.
     *
     * # Errors
     * Returns [`HoyError`] if arg validation fails.
     */
    pub fn new() -> Result<Self, HoyError> {
        let hoy = Self::default();
        hoy.validate_args()?;
        Ok(hoy)
    }

    /**
     * Validates provided args.
     *
     * Prints warning for invalid arg combinations (e.g. providing username when running client).
     *
     * # Returns
     * `Ok(())` if arg rules are not violated.
     *
     * # Errors
     * Returns [`HoyError::NoUsername`] if running client with no username provided.
     */
    pub fn validate_args(&self) -> Result<(), HoyError> {
        let args = &self.args;
        if args.server {
            if args.username.is_some() {
                eprintln!(
                    "`-u/--username` provided even though running server. Ignoring argument."
                );
            }

            if args.notui {
                eprintln!("`-n/--no-tui` provided even though running server. Ignoring argument.");
            }
        } else {
            if args.username.is_none() {
                return Err(HoyError::NoUsername);
            }

            if args.db.is_some() {
                eprintln!("`-d/--db` provided even though running client. Ignoring argument.");
            }

            if args.incognito {
                eprintln!(
                    "`-i/--incognito` provided even though running client. Ignoring argument."
                );
            }
        }

        Ok(())
    }

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
     * Returns true if `-i/--incognito` flag was provided.
     */
    #[must_use]
    pub const fn run_incognito(&self) -> bool {
        self.args.incognito
    }

    /**
     * Returns true if `-n/--no-tu` flag was provided.
     */
    #[must_use]
    pub const fn run_tui(&self) -> bool {
        !self.args.notui
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

#[cfg(test)]
mod tests {

    use std::fs;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::path::PathBuf;

    use clap::Parser;
    use hoy_test::assert_matches;

    use crate::cli::{Cli, DEFAULT_PORT, Hoy};
    use crate::store::ServerStore;

    const PORT: u16 = 1234;
    const PORT_STR: &str = "1234";
    const ADDR: Ipv4Addr = Ipv4Addr::UNSPECIFIED;
    const ADDR_STR: &str = "0.0.0.0";
    const USERNAME: &str = "bruce_lee";
    const DB: &str = "/tmp/a.db";

    const ARGS: [&str; 12] = [
        "hoy", "-s", "-p", PORT_STR, "-a", ADDR_STR, "-u", USERNAME, "-d", DB, "-i", "-n",
    ];

    fn hoy() -> (Hoy, Hoy) {
        (
            Hoy {
                args: Cli::try_parse_from(ARGS).expect("Parsing failed unexpectedly"),
            },
            Hoy {
                args: Cli::try_parse_from(["hoy"])
                    .expect("Parsing with no args failed unexpectedly"),
            },
        )
    }

    fn addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(ADDR), PORT)
    }

    #[test]
    fn parse_defaults() {
        let args = Cli::try_parse_from(["hoy"]).expect("Parsing failed unexpectedly");
        assert!(!args.server);
        assert_eq!(args.port, DEFAULT_PORT);
        assert!(args.ipv4.is_none());
        assert!(args.username.is_none());
        assert!(args.db.is_none());
        assert!(!args.incognito);
        assert!(!args.notui);
    }

    #[test]
    fn parse_flags_and_options() {
        let args = Cli::try_parse_from(ARGS).expect("Parsing failed unexpectedly");
        assert!(args.server);
        assert_eq!(args.port, PORT);
        assert_matches!(args.ipv4, Some(ADDR));
        assert_eq!(args.username.expect("Failed to parse username"), USERNAME);
        assert_eq!(
            args.db
                .expect("Failed to parse db")
                .to_str()
                .expect("Failed to convert db path"),
            DB
        );
        assert!(args.incognito);
        assert!(args.notui);
    }

    #[test]
    fn hoy_validate_args() {
        let (hoy, default) = hoy();
        assert_matches!(hoy.validate_args(), Ok(()));
        assert!(default.validate_args().is_err());
    }

    #[test]
    fn hoy_resolve_address() {
        let (hoy, _default) = hoy();
        let address = hoy.resolve_address();
        assert_eq!(address, addr());
    }

    #[tokio::test]
    async fn hoy_construct_db_store() {
        let (hoy, _default) = hoy();
        let store = hoy
            .construct_store()
            .await
            .expect("Failed to construct db store");
        assert_eq!(
            store
                .load_rooms()
                .await
                .expect("Failed to load rooms")
                .len(),
            0
        );

        assert!(PathBuf::from(DB).is_file());
        let _res = fs::remove_file(DB);
    }

    #[tokio::test]
    async fn hoy_construct_incognito_store() {
        let store = Hoy::incognito_store();
        assert_eq!(
            store
                .load_rooms()
                .await
                .expect("Failed to load rooms")
                .len(),
            0
        );
    }

    #[test]
    fn hoy_run_server() {
        let (hoy, default) = hoy();
        assert!(hoy.run_server());
        assert!(!default.run_server());
    }

    #[test]
    fn hoy_resolve_username() {
        let (hoy, default) = hoy();
        assert_eq!(
            hoy.resolve_username().expect("Failed to resolve username"),
            USERNAME
        );
        assert!(default.resolve_username().is_err_and(|_| true));
    }
}
