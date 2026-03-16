use std::fmt::Write as _;
use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::thread;

use hoy_core::store::RoomName;
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

/**
 * Spawns a blocking stdin reader thread.
 *
 * Reads lines from stdin until EOF, `/quit`, or `/exit` is entered,
 * forwarding each trimmed line into the provided channel.
 *
 * # Arguments
 * - `line_tx`: unbounded sender for forwarding input lines.
 *
 * # Returns
 * Join handle for the spawned blocking thread.
 */
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
 * # Arguments
 * - `handle`: client handle for sending commands,
 * - `line`: trimmed input line from stdin.
 *
 * # Returns
 * `FrontendAction` indicating whether the frontend loop should continue
 * or shut down.
 *
 * # Errors
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

    if line == "/list" {
        handle.list_rooms().await?;
        return Ok(FrontendAction::Continue);
    }

    if line.starts_with("/room") {
        let room = line.clone().split_off("/room".len()).trim().to_owned();
        let Ok(name) = RoomName::new(&room) else {
            print_client_event(&ClientEvent::Error {
                message: format!("invalid room name: '{room}'"),
            });
            return Ok(FrontendAction::Continue);
        };

        handle.join_room(name.to_string()).await?;
        return Ok(FrontendAction::Continue);
    }

    handle.send_message(line).await?;
    Ok(FrontendAction::Continue)
}

/**
 * Prints a formatted client event to stdout.
 *
 * # Arguments
 * - `event`: client event to display.
 */
fn print_client_event(event: &ClientEvent) {
    println!("{}", format_client_event(event));
}

/**
 * Formats a client event into a user-visible output string.
 *
 * # Arguments
 * - `event`: client event to format.
 *
 * # Returns
 * Formatted display string for the event.
 */
fn format_client_event(event: &ClientEvent) -> String {
    match *event {
        ClientEvent::Connecting {
            ref server_addr,
            ref username,
        } => format!("Connecting to {server_addr} as {username}..."),

        ClientEvent::Connected {
            ref server_addr,
            ref username,
            ref room,
        } => format!("Connected to {server_addr} as {username} in {room}."),

        ClientEvent::Disconnected => String::from("Disconnected."),

        ClientEvent::MessageReceived {
            ref from,
            ref room,
            ref text,
        } => format!("[{room}] {from}: {text}"),

        ClientEvent::SystemMessage { ref text } => format!("* {text}"),

        ClientEvent::Error { ref message } => format!("Error: {message}"),

        ClientEvent::RoomJoined {
            ref room,
            ref messages,
        } => {
            let mut lines = format!("* joined #{room}");
            if !messages.is_empty() {
                for message in messages {
                    write!(lines, "\n[{room}] {}: {}", message.from, message.text)
                        .expect("writing to a String is infallible");
                }
            }
            lines
        }

        ClientEvent::RoomList { ref rooms } => format!("* available rooms: {rooms:?}"),

        ClientEvent::Pong => String::from("Pong!"),
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use hoy_test::async_ok;
    use tokio::time::{Duration, timeout};

    use crate::client::core::{ClientEventStream, ClientHandle, spawn_client};
    use crate::client::event::ClientEvent;
    use crate::client::test_client::{FrontendAction, format_client_event, handle_input_line};

    async fn recv_event_timeout(
        event_stream: &mut ClientEventStream,
        millis: u64,
    ) -> Option<ClientEvent> {
        let duration = Duration::from_millis(millis);
        timeout(duration, event_stream.recv()).await.unwrap_or(None)
    }

    fn spawn_handle() -> (ClientHandle, ClientEventStream) {
        spawn_client()
    }

    #[tokio::test]
    async fn handle_input_line_quit_and_exit() -> Result<(), ()> {
        let (handle, mut events) = spawn_handle();

        let action_quit =
            async_ok!(200, handle_input_line(&handle, String::from("/quit"))).map_err(|_err| ())?;
        assert_eq!(action_quit, FrontendAction::Shutdown);
        assert!(recv_event_timeout(&mut events, 50).await.is_none());

        let action_exit =
            async_ok!(200, handle_input_line(&handle, String::from("/exit"))).map_err(|_err| ())?;
        assert_eq!(action_exit, FrontendAction::Shutdown);
        assert!(recv_event_timeout(&mut events, 50).await.is_none());

        async_ok!(200, handle.shutdown()).map_err(|_err| ())?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_input_line_ping_disconnect_and_message_flow() -> Result<(), ()> {
        let (handle, mut events) = spawn_handle();

        let action_ping =
            async_ok!(200, handle_input_line(&handle, String::from("/ping"))).map_err(|_err| ())?;
        assert_eq!(action_ping, FrontendAction::Continue);
        let ping_event = recv_event_timeout(&mut events, 200).await.ok_or(())?;
        match ping_event {
            ClientEvent::Error { message } => {
                assert_eq!(message, "Client is not connected");
            }
            _ => return Err(()),
        }

        let action_disconnect =
            async_ok!(200, handle_input_line(&handle, String::from("/disconnect")))
                .map_err(|_err| ())?;
        assert_eq!(action_disconnect, FrontendAction::Continue);
        assert!(recv_event_timeout(&mut events, 50).await.is_none());

        let action_message =
            async_ok!(200, handle_input_line(&handle, String::from("hello"))).map_err(|_err| ())?;
        assert_eq!(action_message, FrontendAction::Continue);
        let message_event = recv_event_timeout(&mut events, 200).await.ok_or(())?;
        match message_event {
            ClientEvent::Error { message } => {
                assert_eq!(message, "Client is not connected");
            }
            _ => return Err(()),
        }

        async_ok!(200, handle.shutdown()).map_err(|_err| ())?;
        Ok(())
    }

    #[test]
    fn format_client_event_outputs_expected_strings() {
        let server_addr = SocketAddr::from(([127, 0, 0, 1], 4242));

        assert_eq!(
            format_client_event(&ClientEvent::Connecting {
                server_addr,
                username: String::from("bruce_lee"),
            }),
            "Connecting to 127.0.0.1:4242 as bruce_lee..."
        );

        assert_eq!(
            format_client_event(&ClientEvent::Connected {
                server_addr,
                username: String::from("bruce_lee"),
                room: String::from("#general"),
            }),
            "Connected to 127.0.0.1:4242 as bruce_lee in #general."
        );

        assert_eq!(
            format_client_event(&ClientEvent::Disconnected),
            "Disconnected."
        );

        assert_eq!(
            format_client_event(&ClientEvent::MessageReceived {
                from: String::from("bruce_lee"),
                room: String::from("#general"),
                text: String::from("hi"),
            }),
            "[#general] bruce_lee: hi"
        );

        assert_eq!(
            format_client_event(&ClientEvent::SystemMessage {
                text: String::from("welcome"),
            }),
            "* welcome"
        );

        assert_eq!(
            format_client_event(&ClientEvent::Error {
                message: String::from("oops"),
            }),
            "Error: oops"
        );

        assert_eq!(format_client_event(&ClientEvent::Pong), "Pong!");
    }
}
