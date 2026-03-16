use hoy::run_hoy;
use hoy_core::cli::Hoy;

/**
 * Entry point for the hoy-incognito binary.
 *
 * Parses CLI arguments and runs either the server or the test client.
 *
 * Runs with a temporary in-memory storage instead of persistent db storage.
 *
 * # Errors
 * Returns a boxed error if the server or client encounters a fatal failure.
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hoy = Hoy::default();
    let store = Hoy::incognito_store();

    run_hoy(hoy, store).await?;

    Ok(())
}
