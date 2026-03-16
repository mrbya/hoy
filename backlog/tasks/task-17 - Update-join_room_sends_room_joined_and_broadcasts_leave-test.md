---
id: TASK-17
title: Update join_room_sends_room_joined_and_broadcasts_leave to assert messages field
status: To Do
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The unit test `join_room_sends_room_joined_and_broadcasts_leave` in `crates/net/src/server/core.rs` currently uses `..` to ignore the `messages` field when destructuring the `RoomJoined` packet. Now that `handle_join_room` loads real message history from the store, the test should explicitly assert the `messages` field rather than silently ignoring it.

In this test no messages are persisted before the room join, so `messages` should be an empty `Vec`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Replace `..` wildcard in the `RoomJoined` destructure with an explicit `messages` binding
- [ ] #2 Assert that `messages` is empty (`assert!(messages.is_empty())` or `assert_eq!(messages, vec![])`)
- [ ] #3 Test still passes after the change
- [ ] #4 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the test `join_room_sends_room_joined_and_broadcasts_leave` in `crates/net/src/server/core.rs`
2. Find the `ServerPacket::RoomJoined { room, .. }` destructure pattern
3. Replace `..` with `messages` and add `assert!(messages.is_empty())`
4. Run `cargo nextest run --all-features --workspace join_room_sends_room_joined_and_broadcasts_leave` to confirm
5. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
