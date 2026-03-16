---
id: TASK-19
title: Add integration test join_room_returns_message_history
status: To Do
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
dependencies:
  - TASK-16
  - TASK-17
  - TASK-18
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add a wire-level integration test `join_room_returns_message_history` in `tests/integration_tests.rs` that verifies the full end-to-end message history flow:

1. Alice connects, completes the handshake, and sends one or more chat messages
2. Bob connects later and completes his handshake (which internally triggers a room join)
3. Bob's `RoomJoined` packet (received during handshake) contains Alice's messages in its `messages` field

This complements the unit test (TASK-18) by exercising the TCP stack and JSON serialisation path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New integration test `join_room_returns_message_history` exists in `tests/integration_tests.rs`
- [ ] #2 Test spins up a real server (using the existing `spawn_server` or equivalent helper)
- [ ] #3 Alice connects, sends `Hello`, receives `Welcome` + `RoomJoined`, then sends one or more `SendMessage` packets
- [ ] #4 Bob connects, sends `Hello`, and receives `Welcome` followed by a `RoomJoined` whose `messages` field contains Alice's messages
- [ ] #5 Test asserts message count and content (`from` == alice's username, `text` matches what was sent)
- [ ] #6 Test is deterministic (no timing sleeps; uses the existing timeout helpers)
- [ ] #7 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read `tests/integration_tests.rs` to understand existing helpers (`spawn_server`, `connect_client`, `send_packet`, `recv_packet_timeout`, etc.)
2. Add a new `#[tokio::test]` function `join_room_returns_message_history`
3. Spawn the server with `spawn_server()`
4. Connect Alice: TCP connect → send `Hello { username: "alice" }` → assert `Welcome` → assert `RoomJoined { messages: [] }`
5. Have Alice send one or more `SendMessage` packets and drain any echo/broadcast packets from her channel
6. Connect Bob: TCP connect → send `Hello { username: "bob" }` → assert `Welcome` → assert `RoomJoined` whose `messages` field is non-empty and contains Alice's message(s)
7. Run `cargo nextest run --all-features --workspace join_room_returns_message_history` to verify
8. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
