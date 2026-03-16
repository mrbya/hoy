# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-03-16

### Added

#### `hoy-core` — Domain Types

- `dbstore` feature flag: enables the SQLite-backed store and the `cli` module (pulls in `sqlx`).
- `dbstore` module: `DbStore` — a `SqlitePool`-backed `ServerStore` that persists room definitions and message history across server restarts.
  - `DbStore::new(path)` — opens (or creates) the database at the given path, or resolves the platform data directory when `None` is passed (`~/.local/share/hoy/hoy.db` on Linux).
  - `DbStore::close()` — explicitly closes the underlying connection pool.
  - `DbStore::fetch_room_id(room)` — looks up the internal row ID for a room name.
  - `ServerStore` impl: `ensure_room` uses `INSERT OR IGNORE`; `load_recent_messages` fetches via `ORDER BY id DESC LIMIT ?` and reverses to return oldest-first.
  - Schema migrations (`migrations/`) applied automatically on construction: `0001_create_rooms.sql`, `0002_create_messages.sql`.
- `cli` module *(requires `dbstore` feature)*: `Hoy` — CLI argument parsing and application bootstrap extracted from the binary.
  - `Hoy::default()` — parses `std::env::args()` via `clap`.
  - `Hoy::resolve_address()` — resolves bind/connect address from `--address` / `--port`.
  - `Hoy::construct_store()` — opens the `DbStore` from `--db` or the platform default.
  - `Hoy::incognito_store()` — returns a fresh `InMemoryStore` (no persistence).
  - `Hoy::run_server()` — returns `true` when `-s/--server` was passed.
  - `Hoy::resolve_username()` — returns the `--username` value or `HoyError::NoUsername`.
- `ServerStore::storage_slug(&self) -> String` — new synchronous method on the trait; implementations return a human-readable label used in the server start-up banner.
- `StoreError::NoDataDirectory` — emitted by `DbStore::new(None)` when the platform data directory cannot be resolved.
- `StoreError::Io(std::io::Error)` — emitted when creating the storage directory fails.
- `HoyError` enum in the `error` module: `NoUsername` variant emitted by `Hoy::resolve_username()` when `--username` was not supplied.

#### `hoy-net` — Networking

- Server start-up banner printed to stdout on `run_server`: ASCII art logo, crate version, bind address, and storage slug.

#### Binary (`hoy`)

- `src/lib.rs` added: `run_hoy(hoy: Hoy, store: impl ServerStore)` — orchestrates server or client mode; enables integration testing of the full binary logic without spawning a subprocess.
- `src/main.rs` slimmed to three lines: parse CLI via `Hoy::default()`, open store, delegate to `run_hoy`.

#### Testing

- `DbStore` unit tests (behind `dbstore` feature + `tempfile` dev-dependency):
  - `ensure_room_is_idempotent` — room created once despite two calls.
  - `append_requires_room_to_exist` — `RoomNotFound` error when room is absent.
  - `load_recent_respects_limit` — correct tail slice returned.
  - `load_rooms_sorted` — rooms returned in alphabetical order.
  - `storage_persistence` — data survives closing and reopening the database at the same path.

### Changed

#### `hoy-core` — Domain Types

- `ServerStore` trait methods are now async, expressed as RPIT (`fn method() -> impl Future<Output = …> + Send`) for `Send`-safe use across tokio tasks.
- `InMemoryStore` impl methods updated to `async fn`; all existing unit tests converted to `#[tokio::test]`.
- CLI argument parsing (`clap`) and address/store resolution moved from `src/main.rs` into `hoy_core::cli::Hoy`; `clap` added as a regular dependency of `hoy-core`.

#### Binary (`hoy`)

- `src/main.rs` now delegates entirely to `hoy_core::cli::Hoy` and `run_hoy`; no longer contains argument parsing or store construction logic.

## [0.1.1] - 2026-03-16

### Added

#### `hoy-protocol` — Wire Protocol
- `ClientPacket::JoinRoom { room: String }` — request to join or create a room.
- `ClientPacket::ListRooms` — request a sorted list of all known rooms.
- `ServerPacket::RoomJoined { room: String, messages: Vec<MessageRecord> }` — confirms a room change and delivers up to 50 recent messages, oldest first.
- `ServerPacket::RoomList { rooms: Vec<String> }` — alphabetically sorted room list in response to `ListRooms`.
- `MessageRecord { from: String, text: String }` — message entry embedded in `RoomJoined`.

#### `hoy-core` — Domain Types
- `store` module: `RoomName` (validated room name: lowercase ASCII, digits, `-`, `_`; 1–64 chars), `RoomRecord`, `StoredMessage`, and the `ServerStore` trait (`ensure_room`, `load_rooms`, `append_message`, `load_recent_messages`).
- `memory` module: `InMemoryStore` — `HashMap`-backed in-memory `ServerStore` implementation used as the default runtime store.
- `error` module: `StoreError` with `InvalidRoomName`, `RoomNotFound`, and `Internal` variants.

#### `hoy-net` — Networking

**Client**
- `ClientCommand::JoinRoom { room }` and `ClientCommand::ListRooms` — new commands accepted by the client core.
- `ClientEvent::RoomJoined { room, messages }` and `ClientEvent::RoomList { rooms }` — new events emitted to the frontend.
- `ClientHandle::join_room(room)` and `ClientHandle::list_rooms()` — new methods on the public client handle.
- Test client (`run_test_client`) handles `/room <name>` and `/list` input commands.

**Server**
- `server/state.rs`: `ServerState` — owns all live runtime state (identified clients, room membership); all mutations go through typed methods (`add_client`, `remove_client`, `move_client_to_room`, `ensure_room`, `room_names`, `room_members`, `username_of`, `sender_of`, `current_room_of`).
- `server/handlers.rs`: `handle_hello`, `handle_send_message`, `handle_join_room`, `handle_list_rooms` extracted from the core loop, plus `send_packet`, `send_error`, and `broadcast_to_room` helpers.
- Multiple rooms: clients can join arbitrary rooms (validated via `RoomName`); new rooms are persisted and hydrated into runtime state on join.
- Message persistence: `handle_send_message` saves every message to the `ServerStore` (non-fatal on storage error).
- Message history delivery: `handle_join_room` loads up to 50 recent messages from the store and includes them in `RoomJoined`; also sent after `Welcome` on initial connect.
- `PendingClients` type alias for the pre-hello client map.
- `StateError` enum: `UsernameTaken`, `RoomNotFound`, `ClientNotFound`, `StoreError`.
- `NetError` expanded: `CommandChannelClosed`, `ClientChannelClosed`, `ClientTaskJoin(JoinError)`, `InternalServerError(StateError)`.

#### Testing
- Unit tests for `RoomName` validation: empty name, over-length, invalid characters (uppercase, space, `!`), valid name, `Display` formatting.
- Unit tests for `ClientId`: `get()` returns non-zero `u64`, `Display` formats as `client#N`, sequential IDs differ by 1.
- Server handler unit tests covering `handle_hello`, `handle_send_message`, `handle_join_room`, and `handle_list_rooms` including all error paths (username collision, duplicate hello, store failures, invalid room name, missing message history).
- Server connection IO unit tests: command channel closed on connect and on packet receipt, read error propagation, writer error propagation.
- Client session IO unit tests: reader task EOF, read failure, and corrupt frame; writer task write failure; `SessionHandle` clean shutdown.
- Client core state machine unit tests: connect in wrong state, session spawn error, disconnect and connection-closed/error no-ops while disconnected, shutdown from connected state, welcome and packets in wrong state, `JoinRoom`/`ListRooms`/`SendMessage`/`Ping` while disconnected all emit errors.
- `test_client` unit tests: empty input line, `/list` and `/room <name>` commands, invalid room name local error (no core event emitted), `RoomJoined` with messages formatting, `RoomList` formatting.
- Integration test: `join_room_returns_message_history` — verifies that sent messages are delivered in `RoomJoined` when a second client joins the same room.

#### Docs
- Comprehensive READMEs written for `hoy-core`, `hoy-protocol`, and `hoy-net` covering all public types, traits, packet shapes, state machine diagrams, and protocol behaviour.

## [0.1.0] - 2026-03-11 - 1st release

### Added

#### Binary (`hoy`)
- Dual-mode CLI: server mode (`-s/--server`) and client mode (default).
- Argument parsing via `clap`: username (`-u`), port (`-p`, default 7777), server address (`-a`, default 127.0.0.1).

#### `hoy-protocol` — Wire Protocol
- Length-prefixed framing: 4-byte big-endian u32 length header followed by a JSON payload.
- `ClientPacket` variants: `Hello`, `SendMessage`, `Ping`.
- `ServerPacket` variants: `Welcome`, `ChatMessage`, `SystemMessage`, `Error`, `Pong`.
- `encode_frame` / `decode_frame` for single-shot encoding and decoding.
- `try_decode_frame` for streaming decoding of multi-frame byte buffers.
- `FrameBuffer`: incremental buffer for streaming TCP reads, with overflow protection, `append`, `try_decode`, `discard_prefix`, and `clear`.
- `ProtocolError` type covering malformed headers, truncated frames, oversized payloads, capacity overflow, and JSON errors.

#### `hoy-net` — Networking
- **Client state machine**: `ClientState` enum with `Disconnected → AwaitingWelcome → Connected` transitions and typed field accessors per state.
- **`ClientHandle`**: frontend API for `connect`, `disconnect`, `send_message`, `ping`, and `shutdown`.
- **`ClientEventStream`**: async event stream delivering `Connecting`, `Connected`, `Disconnected`, `MessageReceived`, `SystemMessage`, `Error`, and `Pong` events to callers.
- **`SessionHandle`**: manages per-session tokio reader/writer tasks and the outgoing packet channel.
- **Client core loop**: `tokio::select!`-based event loop translating protocol packets to `ClientEvent`s and commands to outgoing packets.
- **Server** (`run_server`): `TcpListener`-based async server with per-connection handler tasks and a central event loop.
- Server-side duplicate username detection with `Error` packet rejection.
- Broadcast on join/leave: system messages sent to all connected clients.
- `broadcast` (all clients) and `send_to_client` (unicast) helpers.
- `NetError` type covering I/O, protocol, and channel failures.
- Temporary test client (`run_test_client`) with stdin command loop for manual integration testing.

#### `hoy-tui` — Terminal UI
- Skeleton module structure (`app`, `event`, `ui`) using `ratatui`, ready for implementation.

#### `hoy-core` — Domain Types
- Placeholder modules for `User` and `Room` domain types.

#### `hoy-test` — Test Utilities
- `async_ok!` macro: runs an async future with a configurable timeout (default 200 ms).
- `assert_err!` macro: pattern-matching assertion on `Err` variants.
- `assert_matches!` macro: general pattern-matching assertion.

#### Testing
- Integration test suite (`tests/integration_tests.rs`) with a `TestClient` helper supporting predicate-based packet waiting and configurable timeouts.
- Integration scenarios: connect/welcome handshake, multi-client broadcast, ping/pong keepalive.
- Unit tests for codec roundtrips, frame buffer streaming, and client state machine transitions.

#### Tooling & CI
- `justfile` task runner: `run`, `test`, `check`, `fmt`, `doc`, `coverage`, `benchmark`, `pre-commit`, `ci`, `thorough-check`.
- CI pipeline configuration (`.gitlab-ci.yml`).
- Pre-commit hook wiring via `just init`.
- Rustdoc style guide (`docs/rustdoc_style.md`) with doxygen-style `/** */` comment conventions.
- Strict Clippy configuration (pedantic + nursery deny set).
