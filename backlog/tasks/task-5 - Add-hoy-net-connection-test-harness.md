---
id: TASK-5
title: Add hoy-net connection test harness
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:37'
updated_date: '2026-03-09 23:40'
labels: []
dependencies: []
references:
  - crates/net/src/connection.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a focused test module in crates/net/src/connection.rs to exercise read/write plumbing without real sockets. Use in-memory channels or duplex streams to drive the connection loop.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Connection unit tests live in crates/net/src/connection.rs under a #[cfg(test)] mod tests
- [x] #2 Helpers provide in-memory read/write streams and server command channel capture
- [x] #3 Test module compiles under tokio::test without touching network sockets
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect crates/net/src/connection.rs for boundaries to test without sockets.
2. Add #[cfg(test)] mod tests with a TestHarness that creates a tokio::io::duplex stream and captures server commands via mpsc.
3. Provide helpers to spawn handle_connection with a client_id and return handles to drive read/write ends.
4. Add a smoke tokio::test to ensure the harness compiles without network use.
5. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added generic handle_connection_io for IO-based tests and a ConnectionHarness using tokio::io::duplex.
Added smoke test that asserts Connected/Disconnected commands and closes writer task by dropping tx.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a connection module test harness using in-memory duplex streams and a generic IO handler to enable socket-free unit tests.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
