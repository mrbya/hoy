use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::Parser;
use hoy_core::memory::InMemoryStore;
use hoy_core::store::{RoomName, ServerStore, StoredMessage};
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

    let mut store = InMemoryStore::new();
    let room1 = RoomName::new("test")?;
    let room2 = RoomName::new(RoomName::GENERAL).expect("hardcoded name is valid");
    let from = String::from("bob");
    let messages: Vec<StoredMessage> = vec![
        StoredMessage {
            room: room1.clone(),
            from: from.clone(),
            text: String::from("akafuka1"),
        },
        StoredMessage {
            room: room2.clone(),
            from: from.clone(),
            text: String::from("akafuka2"),
        },
        StoredMessage {
            room: room1.clone(),
            from: from.clone(),
            text: String::from("akafuka3"),
        },
    ];

    store.ensure_room(&room1)?;
    store.ensure_room(&room2)?;
    for message in messages {
        store.append_message(message)?;
    }

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
