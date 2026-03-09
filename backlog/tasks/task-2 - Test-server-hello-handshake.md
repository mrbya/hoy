---
id: TASK-2
title: Test server hello handshake
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:23'
updated_date: '2026-03-09 23:27'
labels: []
dependencies: []
references:
  - crates/net/src/server.rs
  - crates/net/src/command.rs
  - crates/protocol/src/packet.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests for hello handling in hoy-net server loop. Cover duplicate hello, username collisions, and successful welcome + join broadcast.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 If a client sends Hello twice, the server responds with Error to that client
- [x] #2 If a username is already in use, the server responds with Error to the requester
- [x] #3 On successful Hello, the server sends Welcome to the client and broadcasts a SystemMessage join
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reuse server.rs TestHarness helpers for ServerState and packet collection.
2. Add tests for duplicate Hello (same client) and username collision (different client) using handle_hello or handle_server_command.
3. Add test for successful Hello sending Welcome to client and SystemMessage broadcast to all clients.
4. Run cargo test -p hoy-net and fix any issues.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added hello handshake tests using TestHarness: duplicate hello rejection, username collision rejection, and welcome + join broadcast verification.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Expanded server loop unit tests to cover hello handshake behavior, including duplicate hello and username collision errors plus welcome/join broadcast on success.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
