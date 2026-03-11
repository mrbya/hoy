use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::thread;

use tokio::sync::mpsc;

use crate::client::core::{ClientEventStream, ClientHandle, spawn_client};
use crate::client::event::ClientEvent;
use crate::error::NetError;

/// Stdio frontend input channel receiver type.
type InputRx = mpsc::UnboundedReceiver<String>;
/// Stdio frontend input channel sender type.
type InputTx = mpsc::UnboundedSender<String>;

/**
 * Run the temporary test client.
 *
 * This call:
 * - spawns the reusable client core,
 * - connects to a target server,
 * - reads user input from stdin on a blocking thread,
 * - translates input lines into client commands,
 * - prints client events to stdout.
 *
 * # Errors
 * Returns `NetError` if sending commands to the client core fails.
 */
pub async fn run_test_client(server_addr: SocketAddr, username: String) -> Result<(), NetError> {
    let (handle, mut event_stream): (ClientHandle, ClientEventStream) = spawn_client();
    let (line_tx, mut line_rx): (InputTx, InputRx) = mpsc::unbounded_channel::<String>();

    let input_thread: thread::JoinHandle<()> = spawn_input_thread(line_tx);

    handle.connect(server_addr, username).await?;

    let mut should_join_input_thread = false;

    loop {
        tokio::select! {
            maybe_line = line_rx.recv() => {
                let Some(line) = maybe_line else {
                    break;
                };

                match handle_input_line(&handle, line).await? {
                    FrontendAction::Continue => {}
                    FrontendAction::Shutdown => {
                        should_join_input_thread = true;
                        break;
                    }
                }
            }

            maybe_event = event_stream.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };

                print_client_event(&event);
            }
        }
    }

    let shutdown_result = handle.shutdown().await;
    if let Err(error) = shutdown_result {
        eprintln!("Client shutdown error: {error}");
    }

    if should_join_input_thread {
        let input_join_result = input_thread.join();
        if let Err(join_error) = input_join_result {
            eprintln!("Input thread error: {join_error:?}");
        }
    }

    Ok(())
}

/// Frontend control action returned after handling one input line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendAction {
    /// Continue frontend loop execution.
    Continue,

    /// Shutdown frontend loop.
    Shutdown,
}

/// Spawns blocking stdin reader thread.
fn spawn_input_thread(line_tx: InputTx) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdin = io::stdin();

        loop {
            let mut line = String::new();

            let read_result = stdin.lock().read_line(&mut line);
            let bytes_read: usize = match read_result {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprint!("Stdin read error: {e}");
                    break;
                }
            };

            if bytes_read == 0 {
                break;
            }

            let trimmed: String = line.trim_end().to_owned();
            let should_exit = trimmed == "/quit" || trimmed == "/exit";

            if line_tx.send(trimmed).is_err() {
                break;
            }

            if should_exit {
                break;
            }
        }
    })
}

/**
 * Handles a frontend input line.
 *
 * # Errors
 *
 * Returns `NetError` if an underlying client core command fails.
 */
async fn handle_input_line(
    handle: &ClientHandle,
    line: String,
) -> Result<FrontendAction, NetError> {
    if line.is_empty() {
        return Ok(FrontendAction::Continue);
    }

    if line == "/quit" || line == "/exit" {
        return Ok(FrontendAction::Shutdown);
    }

    if line == "/ping" {
        handle.ping().await?;
        return Ok(FrontendAction::Continue);
    }

    if line == "/disconnect" {
        handle.disconnect().await?;
        return Ok(FrontendAction::Continue);
    }

    handle.send_message(line).await?;
    Ok(FrontendAction::Continue)
}

/// Print a client event.
fn print_client_event(event: &ClientEvent) {
    match *event {
        ClientEvent::Connecting {
            ref server_addr,
            ref username,
        } => {
            println!("Connecting to {server_addr} as {username}...");
        }

        ClientEvent::Connected {
            ref server_addr,
            ref username,
            ref room,
        } => {
            println!("Connected to {server_addr} as {username} in {room}.");
        }

        ClientEvent::Disconnected => {
            println!("Disconnected.");
        }

        ClientEvent::MessageReceived {
            ref from,
            ref room,
            ref text,
        } => {
            println!("[{room}] {from}: {text}");
        }

        ClientEvent::SystemMessage { ref text } => {
            println!("* {text}");
        }

        ClientEvent::Error { ref message } => {
            println!("Error: {message}");
        }

        ClientEvent::Pong => {
            println!("Pong!");
        }
    }
}
