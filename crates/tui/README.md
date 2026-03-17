# hoy-tui
[![crates.io](https://img.shields.io/crates/v/hoy-tui.svg)](https://crates.io/crates/hoy-tui)
[![docs.rs](https://img.shields.io/docsrs/hoy-tui)](https://docs.rs/hoy-tui)

Terminal UI for the hoy chat client. Drives `hoy-net`'s client core and renders an interactive full-screen interface using [ratatui](https://ratatui.rs) and [crossterm](https://docs.rs/crossterm).

## Index

<!-- toc -->

- [Lib modules](#lib-modules)
- [`app` module](#app-module)
  * [`run_tui`](#run_tui)
- [`event` module](#event-module)
  * [`AppEvent`](#appevent)
  * [`map_key_event`](#map_key_event)
- [`state` module](#state-module)
  * [`TuiState`](#tuistate)
  * [`ChatMessage`](#chatmessage)
  * [`ConnectionStatus`](#connectionstatus)
- [`ui` module](#ui-module)
  * [`draw`](#draw)
- [`error` module](#error-module)
  * [`TuiError`](#tuierror)

<!-- tocstop -->

## Lib modules

- `app`: `run_tui` — entry point; wires client core, terminal events, and renderer
- `event`: `AppEvent`, `map_key_event` — key mapping layer
- `state`: `TuiState`, `ChatMessage`, `ConnectionStatus` — display state
- `ui`: `draw` — stateless ratatui renderer
- `error`: `TuiError`

---

## `app` module

Owns the main event loop. Spawns the `hoy-net` client, initialises the terminal, and drives the `tokio::select!` loop that multiplexes crossterm key events with client network events.

### `run_tui`

The single public entry point for the TUI client.

```rust
use hoy_tui::app::run_tui;

run_tui("127.0.0.1:7777".parse()?, "alice".to_owned()).await?;
```

```rust
pub async fn run_tui(server_addr: SocketAddr, username: String) -> Result<(), TuiError>
```

Enters alternate-screen raw mode, runs the interactive loop, and **always** restores the terminal before returning — even on error.

**Loop behaviour:**

Each iteration draws one frame then waits on whichever fires first:

| Source | What happens |
|---|---|
| Terminal key event | Mapped to `AppEvent` via `map_key_event`; dispatched to `handle_app_event` |
| `ClientEvent` from network | Translated into a `TuiState` mutation by `handle_client_event` |
| Either channel closes | Loop exits; client is shut down |

**Slash commands** entered in the input bar are dispatched by `dispatch_input`:

| Command | Effect |
|---|---|
| `/room <name>` | Join or switch to the named room |
| `/list` | Request the server's room list |
| `/ping` | Send a ping; server replies with a pong |
| `/quit` or `/exit` | Disconnect and exit |
| anything else | Send as a chat message |

An invalid `/room` argument (empty or failing `RoomName` validation) shows a notification instead of sending to the server.

---

## `event` module

Provides a thin abstraction over raw crossterm key events so that `app.rs` works with semantic actions rather than key codes.

### `AppEvent`

High-level actions derived from terminal key presses. Derives `Debug`, `Clone`, `PartialEq`, `Eq`.

| Variant | Description |
|---|---|
| `Quit` | Exit the application |
| `Submit` | Confirm the input bar contents |
| `InsertChar(char)` | Insert a character at the cursor position |
| `DeleteCharBack` | Delete the character before the cursor (`Backspace`) |
| `DeleteCharForward` | Delete the character after the cursor (`Delete`) |
| `MoveCursorLeft` | Move cursor one character to the left |
| `MoveCursorRight` | Move cursor one character to the right |
| `MoveCursorStart` | Jump cursor to the beginning of input |
| `MoveCursorEnd` | Jump cursor to the end of input |
| `ScrollUp` | Scroll the message view up |
| `ScrollDown` | Scroll the message view down |
| `Resize` | Terminal resize occurred; triggers a redraw |

### `map_key_event`

```rust
pub const fn map_key_event(key: KeyEvent) -> Option<AppEvent>
```

Maps a raw crossterm `KeyEvent` to an `AppEvent`. Returns `None` for unrecognised key combinations.

**Key bindings:**

| Key | `AppEvent` |
|---|---|
| `Ctrl-C` | `Quit` |
| `Enter` | `Submit` |
| `Backspace` | `DeleteCharBack` |
| `Delete` | `DeleteCharForward` |
| `←` | `MoveCursorLeft` |
| `→` | `MoveCursorRight` |
| `Home` / `Ctrl-A` | `MoveCursorStart` |
| `End` / `Ctrl-E` | `MoveCursorEnd` |
| `↑` | `ScrollUp` |
| `↓` | `ScrollDown` |
| any printable char | `InsertChar(c)` |

---

## `state` module

Contains the complete displayable state of the TUI. `TuiState` is the single source of truth for the renderer — it is owned by the app loop and mutated exclusively through `handle_client_event` and `handle_app_event`.

### `TuiState`

```rust
#[derive(Debug, Default)]
pub struct TuiState { /* ... */ }
```

All fields are `pub` for direct read access by the renderer.

| Field | Type | Description |
|---|---|---|
| `status` | `ConnectionStatus` | Current connection lifecycle state |
| `server_addr` | `Option<SocketAddr>` | Address of the connected server, if any |
| `username` | `Option<String>` | Display name received after handshake |
| `current_room` | `Option<String>` | Name of the currently active room |
| `rooms` | `Vec<String>` | Room names from the most recent `ListRooms` response |
| `messages` | `HashMap<String, Vec<ChatMessage>>` | Per-room message history |
| `input` | `String` | Text currently in the input bar |
| `cursor_pos` | `usize` | Byte offset of the cursor within `input` |
| `notification` | `Option<String>` | Last error or status notification (shown in status bar) |
| `scroll` | `HashMap<String, usize>` | Per-room scroll offset — messages scrolled up from the bottom |

**Methods:**

| Method | Returns | Description |
|---|---|---|
| `status(s)` | `&mut Self` | Overwrite connection status |
| `server_addr(addr)` | `&mut Self` | Overwrite server address |
| `username(u)` | `&mut Self` | Overwrite username |
| `current_room(r)` | `&mut Self` | Overwrite current room |
| `notification(n)` | `&mut Self` | Overwrite notification |
| `append_char(c)` | `()` | Insert `c` at `cursor_pos` and advance cursor |
| `push_message(room, msg)` | `()` | Append a `ChatMessage` to a room's history |
| `join_room(room, history)` | `()` | Switch to `room`, replace its history, reset scroll to bottom |
| `scroll_up()` | `()` | Increment scroll offset for the current room |
| `scroll_down()` | `()` | Decrement scroll offset for the current room |
| `current_messages()` | `&[ChatMessage]` | Message history for the current room, or `&[]` |
| `current_scroll()` | `usize` | Scroll offset for the current room, or `0` |

### `ChatMessage`

A single entry in a room's message history. Derives `Debug`, `Clone`.

| Variant | Fields | Description |
|---|---|---|
| `User { from, text }` | `from: String`, `text: String` | A message sent by a user |
| `System { text }` | `text: String` | A server-generated notification (e.g. join/leave) |

### `ConnectionStatus`

Connection lifecycle state shown in the status bar. Derives `Debug`, `Clone`, `PartialEq`, `Eq`, `Default`.

| Variant | Description |
|---|---|
| `Disconnected` *(default)* | No active connection |
| `Connecting` | TCP connected; waiting for the server handshake |
| `Connected` | Handshake complete; session active |

---

## `ui` module

A stateless ratatui renderer. `draw` is the only public symbol; it is a pure function that reads `TuiState` and writes to the provided `Frame`.

### `draw`

```rust
pub fn draw(frame: &mut Frame<'_>, state: &TuiState)
```

Renders the full TUI in a single frame. The layout is split vertically into three rows:

```
┌─────────────────────────────────────────────┐
│ hoy  |  alice @ #general  |  connected      │  ← status bar (1 line)
├──────────────┬──────────────────────────────┤
│ Rooms        │ #general                     │
│ > general    │ alice: hello!                │  ← main area (fills remaining)
│   dev        │ * bob joined the room        │
│              │ bob: hey                     │
├──────────────┴──────────────────────────────┤
│ Message                                     │
│ > |                                         │  ← input bar (3 lines)
└─────────────────────────────────────────────┘
```

| Panel | Description |
|---|---|
| **Status bar** | Format: `hoy  │  <username> @ #<room>  │  <status>  [notification]`. Status colour: green (connected), yellow (connecting), red (disconnected). Notification shown in yellow when present. |
| **Room list** | 22-column panel on the left. Active room prefixed with `> ` and bolded. Falls back to showing only the current room if no `/list` response has arrived yet. |
| **Message view** | Fills remaining width. Shows as many messages as fit the height, respecting per-room scroll offset. User messages formatted as `<bold>from:</bold> text`; system messages as `* text` in dark grey. |
| **Input bar** | 3-line bordered widget. Displays `> <input>` with a hardware cursor positioned at `cursor_pos`, correctly handling multi-byte Unicode characters. |

---

## `error` module

### `TuiError`

| Variant | When |
|---|---|
| `Io(std::io::Error)` | Terminal setup/teardown I/O failure, or a ratatui render error |
| `Net(NetError)` | Networking error propagated from `hoy-net` (e.g. channel closed, send failed) |
