---
id: TASK-18
title: Add unit test join_room_includes_message_history
status: To Do
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
dependencies:
  - TASK-16
  - TASK-17
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`handle_join_room` now loads recent message history from the store and includes it in the `RoomJoined` packet. There is currently no unit test that exercises this path. Add a unit test `join_room_includes_message_history` in `crates/net/src/server/core.rs` that verifies:

1. A client sends messages to a room (via `handle_send_message`)
2. A second client (or the same client after re-joining) joins the room
3. The `RoomJoined` packet contains the previously sent messages in its `messages` field
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New test `join_room_includes_message_history` exists in the server unit tests
- [ ] #2 Test persists at least one message via `handle_send_message` before the join
- [ ] #3 Test calls `handle_join_room` and asserts the resulting `RoomJoined.messages` contains the expected `MessageRecord` entries (correct `from` and `text` fields)
- [ ] #4 Test follows existing test harness patterns (uses `ServerHarness` or equivalent helpers)
- [ ] #5 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the existing server unit tests and `ServerHarness` setup in `crates/net/src/server/core.rs`
2. Add a new `#[tokio::test]` function `join_room_includes_message_history`
3. Set up harness with at least one registered client in the default room
4. Call `handle_send_message` one or more times to persist messages into the store
5. Call `handle_join_room` for a client joining the same room
6. Receive the `RoomJoined` packet and assert `messages` contains the expected `MessageRecord`s (matching `from` and `text`)
7. Run `cargo nextest run --all-features --workspace join_room_includes_message_history` to verify
8. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
