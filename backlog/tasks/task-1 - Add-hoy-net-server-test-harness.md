---
id: TASK-1
title: Add hoy-net server test harness
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:22'
updated_date: '2026-03-09 23:25'
labels: []
dependencies: []
references:
  - crates/net/src/server.rs
  - crates/net/src/command.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a focused test module in hoy-net to exercise server loop helpers without opening sockets. Provide helper builders for ServerState, client channels, and packet collection.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Server unit tests live in crates/net/src/server.rs under a #[cfg(test)] mod tests
- [x] #2 Helpers build ServerState with in-memory mpsc channels and allow collecting ServerPacket outputs
- [x] #3 Test module compiles under tokio::test without touching network sockets
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect crates/net/src/server.rs helpers and visibility constraints for tests in-module.
2. Design test-only helpers: build ServerState with N clients and collect ServerPacket outputs via mpsc receivers.
3. Implement #[cfg(test)] mod tests in crates/net/src/server.rs with helpers and a smoke tokio::test that exercises helpers without sockets.
4. Run just check or cargo test -p hoy-net to confirm compilation.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added a TestHarness in crates/net/src/server.rs for in-memory ServerState + mpsc channels and updated unit tests to use it.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a server test harness in hoy-net and refactored server loop unit tests to use it without sockets.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
