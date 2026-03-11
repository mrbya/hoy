---
id: TASK-3
title: Test server message and ping handling
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:23'
updated_date: '2026-03-09 23:34'
labels: []
dependencies: []
references:
  - crates/net/src/server.rs
  - crates/protocol/src/packet.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests that validate message flow and ping responses in the server loop.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 SendMessage before Hello returns Error to the sender
- [x] #2 After Hello, SendMessage broadcasts ChatMesage to all connected clients
- [x] #3 Ping results in Pong to the requesting client
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extend server.rs tests with TestHarness to cover message/ping flows.
2. Add SendMessage-before-Hello test to assert Error to sender.
3. Add SendMessage-after-Hello test to assert ChatMessage broadcast to all clients.
4. Add Ping test via handle_server_command or direct send_to_client to assert Pong to requester.
5. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added server loop tests for SendMessage-before-Hello error, SendMessage broadcast to all clients, and Ping->Pong response.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added server loop unit tests for message and ping handling, including error-on-send-before-hello, broadcast of chat messages, and pong responses to ping.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
