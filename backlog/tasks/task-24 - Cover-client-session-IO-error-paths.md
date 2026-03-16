---
id: TASK-24
title: Cover client session IO error paths
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
`crates/net/src/client/session.rs` sits at 71 % line coverage (71 missed lines).
The file already exposes `spawn_reader_task_io` and `spawn_writer_task_io` with
generic `AsyncRead`/`AsyncWrite` bounds, and the existing test module uses
`tokio::io::duplex` via `IoHarness`.

Uncovered paths:

**Reader task** (`spawn_reader_task_io`):
- Socket read returns an `io::Error` → emits `ConnectionError`, returns `Err(NetError::Io(…))`
- `frame_buffer.append` returns an error (e.g. malformed length prefix that exceeds buffer) → emits `ConnectionError`, returns `Err(NetError::Protocol(…))`
- `frame_buffer.try_decode` returns an error (invalid JSON payload) → emits `ConnectionError`, returns `Err(NetError::Protocol(…))`

**Writer task** (`spawn_writer_task_io`):
- `writer.write_all` returns an `io::Error` → emits `ConnectionError`, returns `Err(NetError::Io(…))`

**`SessionHandle::shutdown`**:
- Normal shutdown (drop `packet_tx`, writer task completes cleanly, reader task is aborted)
- Writer task returns an error → `shutdown` propagates the error

The existing `IoHarness` helper in the test module can be extended to support all these.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `reader_task_emits_error_on_read_failure` — reader backed by a stub that returns an `io::Error`; task emits `InternalEvent::ConnectionError` and returns `Err(NetError::Io(…))`
- [ ] #2 `reader_task_emits_error_on_corrupt_frame` — server writes bytes that make `try_decode` return a `ProtocolError`; task emits `ConnectionError` and returns `Err(NetError::Protocol(…))`
- [ ] #3 `writer_task_emits_error_on_write_failure` — writer backed by a stub that fails on `write_all`; task emits `ConnectionError` and returns `Err(NetError::Io(…))`
- [ ] #4 `session_handle_shutdown_clean` — construct a `SessionHandle` from in-memory tasks; call `shutdown()`, assert it returns `Ok(())`
- [ ] #5 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the existing test module in `session.rs` (particularly `IoHarness`) to understand current helpers
2. Implement a `FailReader` that always returns `io::Error` for `AsyncRead::poll_read` (can be a unit struct)
3. Implement a `FailWriter` that always returns `io::Error` for `AsyncWrite::poll_write` (can be a unit struct)
4. Add tests #1–#4 to the existing `#[cfg(test)]` module in `session.rs`
5. For #2: send raw bytes that form a valid 4-byte length prefix but whose JSON payload is malformed; use `tokio::io::duplex` with direct `write_all`
6. For #4: spawn reader/writer tasks with a real duplex, construct `SessionHandle::new(packet_tx, reader_task, writer_task)`, call `shutdown()`, assert `Ok(())`
7. Run `cargo nextest run --all-features --workspace session` to verify
8. Run `just coverage` and verify `session.rs` line coverage improves to ≥ 85 %
9. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
