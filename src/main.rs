use std::net::{Ipv4Addr, SocketAddr};

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use hoy_net::client::run_temp_client;

#[derive(Debug, Parser)]
struct Cli {
    #[arg(short = 'p', long = "port", default_value_t = 7777)]
    port: u16,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    #[command(alias = "c")]
    Client(ClientArgs),
}

#[derive(Args, Debug, Default, Clone)]
pub struct ClientArgs {
    #[arg(short = 'u', long = "username")]
    pub username: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), args.port);

    if let Some(command) = args.command {
        match command {
            Command::Client(ref c) => {
                run_temp_client(addr, c.username.clone().unwrap_or("bruce_lee".to_owned())).await?
            }
        }
    } else {
        hoy_net::server::run_server(addr).await?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(dead_code, unused)]
mod tests {
    use crate::main;

    #[test]
    fn test_main() {
        println!("Hoy!");
    }
}
