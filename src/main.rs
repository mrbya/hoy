use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short = 'p', long = "port", default_value_t = 7777)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), args.port);

    hoy_net::server::run_server(addr).await?;

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
