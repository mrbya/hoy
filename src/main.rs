use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;
use hoy_core::dbstore::DbStore;
use hoy_net::client::test_client::run_test_client;
use hoy_net::server::core::run_server;

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

/**
 * Entry point for the hoy binary.
 *
 * Parses CLI arguments and runs either the server or the test client.
 *
 * # Errors
 * Returns a boxed error if the server or client encounters a fatal failure.
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let address: SocketAddr;

    if let Some(addr) = args.ipv4 {
        address = SocketAddr::new(IpAddr::V4(addr), args.port);
    } else {
        address = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), args.port);
    }

    let store = DbStore::new(None).await?;

    if args.server {
        run_server(address, store).await?;
    } else {
        let Some(username) = args.username else {
            eprintln!("No client username provided.");
            return Ok(());
        };
        run_test_client(address, username.clone()).await?;
    }

    Ok(())
}
