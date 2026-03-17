//! App loop: wires the client core to terminal events and the renderer.

use std::io::Stdout;
use std::net::SocketAddr;

use crossterm::event::{Event, EventStream};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use futures::StreamExt;
use hoy_core::store::RoomName;
use hoy_net::client::core::{ClientEventStream, ClientHandle, spawn_client};
use hoy_net::client::event::ClientEvent;
use hoy_protocol::packet::MessageRecord;
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;

use crate::error::TuiError;
use crate::event::{AppEvent, map_key_event};
use crate::state::{ChatMessage, ConnectionStatus, TuiState};
use crate::ui;

/// TUI terminal backend alias.
type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

// ── Entry point ───────────────────────────────────────────────────────────────

/**
 * Runs the full TUI client: spawns the client core, connects, and enters the
 * interactive terminal loop.
 *
 * Restores the terminal to its original state before returning, even on error.
 *
 * # Arguments
 * - `server_addr`: address of the hoy server to connect to,
 * - `username`: display name for this session.
 *
 * # Returns
 * `Ok(())` after a clean exit.
 *
 * # Errors
 * Returns [`TuiError`] if the terminal cannot be initialised or the network
 * layer fails.
 */
pub async fn run_tui(server_addr: SocketAddr, username: String) -> Result<(), TuiError> {
    let (client, mut events) = spawn_client();
    client.connect(server_addr, username).await?;

    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &client, &mut events).await;
    restore_terminal(&mut terminal);

    result
}

// ── Terminal setup / teardown ─────────────────────────────────────────────────

/**
 * Enters raw mode and switches to the alternate screen buffer.
 *
 * # Returns
 * Initialised [`Terminal`] on success.
 *
 * # Errors
 * Returns [`TuiError::Io`] if raw mode or the alternate screen cannot be
 * enabled.
 */
fn setup_terminal() -> Result<TuiTerminal, TuiError> {
    crossterm::terminal::enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    if let Err(e) = crossterm::execute!(stdout, EnterAlternateScreen) {
        if let Err(e2) = crossterm::terminal::disable_raw_mode() {
            eprintln!("Failed to disable raw mode during setup error recovery: {e2}");
        }
        return Err(TuiError::Io(e));
    }

    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(TuiError::Io)
}

/**
 * Leaves the alternate screen, disables raw mode, and shows the cursor.
 *
 * Logs but does not propagate individual cleanup errors so that the original
 * application result is always returned to the caller.
 *
 * # Arguments
 * - `terminal`: mutable terminal handle to restore.
 */
fn restore_terminal(terminal: &mut TuiTerminal) {
    if let Err(e) = crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen) {
        eprintln!("Failed to leave alternative scree: {e}");
    }

    if let Err(e) = crossterm::terminal::disable_raw_mode() {
        eprintln!("Failed to disable raw mode: {e}");
    }

    if let Err(e) = terminal.show_cursor() {
        eprintln!("Failed to restore cursor: {e}");
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

/**
 * Core event loop: draws the UI then reacts to terminal and client events.
 *
 * # Arguments
 * - `terminal`: mutable terminal handle,
 * - `client`: handle for sending commands to the client core,
 * - `events`: event stream from the client core.
 *
 * # Returns
 * `Ok(())` on clean exit.
 *
 * # Errors
 * Returns [`TuiError`] on render or network failure.
 */
async fn run_loop(
    terminal: &mut TuiTerminal,
    client: &ClientHandle,
    events: &mut ClientEventStream,
) -> Result<(), TuiError> {
    let mut state = TuiState::default();
    let mut term_events = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, &state))?;

        tokio::select! {
            maybe_term = term_events.next() => {
                let Some(Ok(event)) = maybe_term else { break; };
                if handle_terminal_event(event, &mut state, client).await? {
                    break;
                }
            }

            maybe_client = events.recv() => {
                let Some(event) = maybe_client else { break; };
                handle_client_event(event, &mut state);
            }
        }
    }

    if let Err(e) = client.shutdown().await {
        eprintln!("Client shutdown error: {e}");
    }

    Ok(())
}

// ── Terminal event handling ───────────────────────────────────────────────────

/**
 * Dispatches a raw crossterm terminal event.
 *
 * # Returns
 * `Ok(true)` when the app should quit, `Ok(false)` to continue.
 *
 * # Errors
 * Returns [`TuiError`] if a resulting client command fails.
 */
async fn handle_terminal_event(
    event: Event,
    state: &mut TuiState,
    client: &ClientHandle,
) -> Result<bool, TuiError> {
    match event {
        Event::Key(key) => {
            let Some(app_event) = map_key_event(key) else {
                return Ok(false);
            };
            handle_app_event(app_event, state, client).await
        }

        _ => Ok(false),
    }
}

/**
 * Handles a mapped [`AppEvent`].
 *
 * # Returns
 * `Ok(true)` when the app should quit, `Ok(false)` to continue.
 *
 * # Errors
 * Returns [`TuiError`] if a resulting client command fails.
 */
async fn handle_app_event(
    event: AppEvent,
    state: &mut TuiState,
    client: &ClientHandle,
) -> Result<bool, TuiError> {
    match event {
        AppEvent::Quit => return Ok(true),

        AppEvent::Submit => {
            let input = state.input.trim().to_owned();
            if !input.is_empty() {
                dispatch_input(&input, state, client).await?;
                state.input.clear();
                state.cursor_pos = 0;
            }
        }

        AppEvent::InsertChar(c) => {
            state.append_char(c);
        }

        AppEvent::DeleteCharBack => {
            if state.cursor_pos > 0 {
                let before = state.input.get(..state.cursor_pos).unwrap_or_default();
                if let Some((char_start, _)) = before.char_indices().next_back() {
                    state.input.remove(char_start);
                    state.cursor_pos = char_start;
                }
            }
        }

        AppEvent::DeleteCharForward => {
            let tail = state.input.get(state.cursor_pos..).unwrap_or_default();
            if let Some(c) = tail.chars().next() {
                state.input.remove(state.cursor_pos);
                let _ = c; // disregard len info
            }
        }

        AppEvent::MoveCursorLeft => {
            let before = state.input.get(..state.cursor_pos).unwrap_or_default();
            if let Some((i, _)) = before.char_indices().next_back() {
                state.cursor_pos = i;
            }
        }

        AppEvent::MoveCursorRight => {
            let tail = state.input.get(state.cursor_pos..).unwrap_or_default();
            if let Some(c) = tail.chars().next() {
                state.cursor_pos = state.cursor_pos.saturating_add(c.len_utf8());
            }
        }

        AppEvent::MoveCursorStart => {
            state.cursor_pos = 0;
        }

        AppEvent::MoveCursorEnd => {
            state.cursor_pos = state.input.len();
        }

        AppEvent::ScrollUp => state.scroll_up(),
        AppEvent::ScrollDown => state.scroll_down(),
        AppEvent::Resize => {}
    }

    Ok(false)
}

/**
 * Interprets a submitted input line as either a `/command` or a chat message.
 *
 * | Input | Effect |
 * |---|---|
 * | `/room <name>` | [`ClientHandle::join_room`] |
 * | `/list` | [`ClientHandle::list_rooms`] |
 * | `/ping` | [`ClientHandle::ping`] |
 * | anything else | [`ClientHandle::send_message`] |
 *
 * # Arguments
 * - `input`: trimmed, non-empty input string,
 * - `state`: mutable TUI state for local error notifications,
 * - `client`: client core handle.
 *
 * # Errors
 * Returns [`TuiError`] if the client channel is closed.
 */
async fn dispatch_input(
    input: &str,
    state: &mut TuiState,
    client: &ClientHandle,
) -> Result<(), TuiError> {
    if let Some(room) = input.strip_prefix("/room ") {
        let room = room.trim();
        if room.is_empty() || RoomName::new(room).is_err() {
            state.notification = Some(String::from(
                "Invalid or empty room name; usage: /room <name>; allowed chars: [a-zA-Z0-9-_]",
            ));
        } else {
            client.join_room(room.to_owned()).await?;
        }
    } else if input == "/list" {
        client.list_rooms().await?;
    } else if input == "/ping" {
        client.ping().await?;
    } else if input == "/quit" || input == "/exit" {
        client.shutdown().await?;
    } else if input.starts_with('/') {
        state.notification = Some(format!("Unknown command: {input}"));
    } else {
        client.send_message(input.to_owned()).await?;
    }

    Ok(())
}

// ── Client event handling ─────────────────────────────────────────────────────

/**
 * Translates an incoming [`ClientEvent`] into a [`TuiState`] mutation.
 *
 * # Arguments
 * - `event`: event from the client core,
 * - `state`: mutable TUI state to update.
 */
fn handle_client_event(event: ClientEvent, state: &mut TuiState) {
    match event {
        ClientEvent::Connecting {
            server_addr,
            username,
        } => {
            state
                .status(ConnectionStatus::Connecting)
                .server_addr(Some(server_addr))
                .username(Some(username))
                .notification(Some(String::from("Connecting...")));
        }

        ClientEvent::Connected {
            server_addr,
            username,
            ..
        } => {
            state
                .status(ConnectionStatus::Connected)
                .server_addr(Some(server_addr))
                .username(Some(username));
            // Current room is set when RoomJoined arrives.
        }

        ClientEvent::Disconnected => {
            state
                .status(ConnectionStatus::Disconnected)
                .current_room(None)
                .notification(Some(String::from("Disconnected")));
        }

        ClientEvent::MessageReceived { from, room, text } => {
            state.push_message(&room, ChatMessage::User { from, text });
        }

        ClientEvent::SystemMessage { text } => {
            if let Some(room) = state.current_room.clone() {
                state.push_message(&room, ChatMessage::System { text });
            } else {
                state.notification = Some(text);
            }
        }

        ClientEvent::RoomJoined { room, messages } => {
            let history: Vec<ChatMessage> = messages.into_iter().map(history_message).collect();
            state.join_room(room, history);
            state.notification = None;
        }

        ClientEvent::RoomList { rooms } => {
            state.rooms = rooms;
        }

        ClientEvent::Error { message } => {
            state.notification = Some(message);
        }

        ClientEvent::Pong => {
            state.notification = Some(String::from("Pong!"));
        }
    }
}

/// Converts a protocol [`MessageRecord`] to a display [`ChatMessage`].
fn history_message(record: MessageRecord) -> ChatMessage {
    ChatMessage::User {
        from: record.from,
        text: record.text,
    }
}
