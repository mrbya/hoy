use hoy_core::cli::Hoy;
use hoy_net::client::test_client::run_test_client;
use hoy_net::server::core::run_server;
use hoy_tui::app::run_tui;

/**
 * Runs hoy with provided parsed arguments.
 *
 * Runs either the server or the client with the configured storage and UI.
 *
 * # Returns
 * `Ok(())` on successful hoy exit.
 *
 * # Errors
 * Returns a boxed error if the server or client encounters a fatal failure.
 */
pub async fn run_hoy(hoy: Hoy) -> Result<(), Box<dyn std::error::Error>> {
    let address = hoy.resolve_address();

    if hoy.run_server() {
        if hoy.run_incognito() {
            run_server(address, Hoy::incognito_store()).await?;
        } else {
            run_server(address, hoy.construct_store().await?).await?;
        }
    } else {
        let username = hoy.resolve_username()?;
        if hoy.run_tui() {
            run_tui(address, username).await?;
        } else {
            run_test_client(address, username).await?;
        }
    }

    Ok(())
}

/**
 * Entry point for the hoy binary.
 *
 * Parses CLI arguments and runs either the server or the client
 * with the configured storage and UI.
 *
 * # Errors
 * Returns a boxed error if hoy encounters a fatal failure.
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hoy = Hoy::new()?;
    run_hoy(hoy).await?;

    Ok(())
}
