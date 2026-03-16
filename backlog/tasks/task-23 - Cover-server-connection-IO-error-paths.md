---
id: TASK-23
title: Cover server connection IO error paths
status: Done
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
  - coverage
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`crates/net/src/server/connection.rs` sits at 63 % line coverage with 33 missed lines.
The file already has an IO-generic entry point `handle_connection_io<R, W>` that accepts
any `AsyncRead + AsyncWrite`, making it straightforward to test with `tokio::io::duplex`.

Uncovered paths:

- `handle_connection_io`: server command channel (`server_tx`) is closed before
  the initial `ServerCommand::Connected` send — should return `NetError::CommandChannelClosed`
- `handle_connection_io`: server command channel is closed while forwarding a decoded
  `ServerCommand::Packet` — should return `NetError::CommandChannelClosed`
- `handle_connection_io`: socket read returns an I/O error — should propagate the error
- Writer task: `writer.write_all` fails — writer task returns `NetError::Io`
- `spawn_accept_loop` error branch: `listener.accept()` fails and the loop breaks
  (this branch is harder to reach without a real socket and may be left for integration coverage)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `connection_io_command_channel_closed_on_connect` — drop `server_tx` before calling `handle_connection_io`; function returns `Err(NetError::CommandChannelClosed)`
- [ ] #2 `connection_io_command_channel_closed_on_packet` — close `server_tx` after `Connected` is received but before a decoded packet is sent; function returns `Err(NetError::CommandChannelClosed)`
- [ ] #3 `connection_io_read_error_propagates` — provide a reader that returns an `io::Error` on first read; function returns `Err(NetError::Io(…))`
- [ ] #4 `connection_io_writer_error_propagates` — provide a writer that fails on `write_all`; the writer task returns `Err(NetError::Io(…))` (verify via `server_rx` or by checking that the connection task terminates)
- [ ] #5 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read `crates/net/src/server/connection.rs` in full to understand `handle_connection_io` signature and test hooks
2. Add a `#[cfg(test)] mod tests { … }` block inside `connection.rs`
3. Use `tokio::io::duplex` for in-memory streams; use custom readers/writers that return errors on demand for error-path tests
4. For #1: create `server_tx`/`server_rx` pair, drop `server_tx`, call `handle_connection_io` with a live duplex
5. For #2: spawn `handle_connection_io`, receive `Connected` from the channel, drop `server_tx`, write a valid encoded packet into the duplex writer, await the task result
6. For #3: implement a minimal `AsyncRead` stub that always returns `io::Error`
7. For #4: implement a minimal `AsyncWrite` stub that fails on `write_all`; send a packet through the writer task and observe the error
8. Run `cargo nextest run --all-features --workspace connection` to verify
9. Run `just coverage` and verify `connection.rs` line coverage improves to ≥ 85 %
10. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
