---
id: TASK-16
title: Fix hello_sends_welcome_and_broadcasts_join test
status: To Do
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The unit test `hello_sends_welcome_and_broadcasts_join` in `crates/net/src/server/core.rs` is currently failing because `handle_hello` was updated to internally call `handle_join_room` after the handshake completes.

This means the joiner now receives an additional `RoomJoined` packet (from `handle_join_room`) after the `Welcome` packet, and the observer (other room member) receives a second `SystemMessage` (the join broadcast from `handle_join_room`) in addition to the first one sent by `handle_hello`.

The test currently calls `h.assert_no_packet(joiner)` and `h.assert_no_packet(observer)` after asserting `Welcome` and the first `SystemMessage`, which now fails because extra packets arrive.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Test passes: consume the `RoomJoined` packet that the joiner now receives after `Welcome`
- [ ] #2 Test passes: consume the second `SystemMessage` (`"<user> joined #<room>"`) that the observer now receives from the internal `handle_join_room` call
- [ ] #3 Assert the content of the `RoomJoined` packet (room name correct, `messages` is empty since no history exists)
- [ ] #4 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the current test `hello_sends_welcome_and_broadcasts_join` in `crates/net/src/server/core.rs`
2. After the existing `Welcome` assert, add an assertion that consumes the `RoomJoined` packet from the joiner channel and checks `room` and `messages == []`
3. After the existing `SystemMessage` assert on the observer, add an assertion that consumes the second `SystemMessage` (from `handle_join_room`) — verify its text matches `"<user> joined #<room>"`
4. Remove or adjust the `assert_no_packet` calls that are now wrong, or move them after all expected packets have been consumed
5. Run `cargo nextest run --all-features --workspace hello_sends_welcome_and_broadcasts_join` to confirm the fix
6. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
