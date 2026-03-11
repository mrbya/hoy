use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;
use hoy_net::client::test_client::run_test_client;
use hoy_net::server::core::run_server;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(short = 'p', long = "port", default_value_t = 7777)]
    port: u16,

    #[arg(short = 's', long = "server", default_value_t = false)]
    server: bool,

    #[arg(short = 'u', long = "username")]
    username: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), args.port);

    if args.server {
        run_server(addr).await?;
    } else {
        let Some(username) = args.username else {
            eprintln!("No client username provided.");
            return Ok(());
        };
        run_test_client(addr, username.clone()).await?;
    }

    Ok(())
}
