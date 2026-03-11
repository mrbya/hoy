# Changelog

All notable changes to this project will be documented in this file.

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
