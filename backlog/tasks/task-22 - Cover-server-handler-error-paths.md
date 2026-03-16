---
id: TASK-22
title: Cover server handler error paths
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
`crates/net/src/server/handlers.rs` sits at 86 % line coverage with 27 missed lines.
The uncovered branches are all error / guard paths inside the five handler functions:

- `handle_hello`: client already identified (first guard — `username_of` is `Some`)
- `handle_hello`: client not in the pending map (second guard — `pending.remove` returns `None`)
- `handle_hello`: username already taken (`StateError::UsernameTaken`)
- `handle_join_room`: `RoomName::new` fails (invalid room name string)
- `handle_join_room`: `store.ensure_room` returns an error
- `handle_join_room`: `store.load_recent_messages` returns an error
- `handle_send_message`: client has not called hello yet
- `handle_list_rooms`: client has not called hello yet
- `broadcast_to_room`: called for a room that has no members in `state`

All these paths are reachable from the existing `TestHarness` in `server/core.rs`.
A few tests may need a custom `ServerStore` stub that returns errors on demand.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `handle_hello_already_initialized_sends_error` — client already in state; calling `handle_hello` again sends an `Error` packet and returns early
- [ ] #2 `handle_hello_missing_pending_entry_is_no_op` — `client_id` not in `pending`; `handle_hello` returns without sending any packet
- [ ] #3 `handle_hello_username_taken_sends_error` — second client tries the same username; server sends `Error` packet with "already in use" message
- [ ] #4 `handle_join_room_invalid_name_sends_error` — `handle_join_room` called with an invalid room string (e.g. `"Bad Room!"`) sends an `Error` packet
- [ ] #5 `handle_join_room_store_ensure_fails_sends_error` — `store.ensure_room` returns an error; handler sends `Error` and returns early
- [ ] #6 `handle_join_room_store_load_fails_sends_error` — `store.load_recent_messages` returns an error; handler sends `Error` and returns early
- [ ] #7 `handle_send_message_unauthenticated_sends_error` — `handle_send_message` called for a client not yet in state sends `Error`
- [ ] #8 `handle_list_rooms_unauthenticated_sends_error` — `handle_list_rooms` called for unauthenticated client sends `Error`
- [ ] #9 `broadcast_to_room_unknown_room_is_no_op` — `broadcast_to_room` with a room name not tracked in state completes without panicking and sends nothing
- [ ] #10 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read `crates/net/src/server/core.rs` — understand `TestHarness`, `add_identified_client`, `add_pending_client`
2. Create a minimal `StoreStub` that returns `Err(StoreError::…)` from `ensure_room` or `load_recent_messages` when configured to do so (can be a simple struct with flags)
3. Write tests #1–#9 in the `#[cfg(test)]` module of `crates/net/src/server/core.rs` (to keep them close to the existing harness)
   - Tests #5 and #6 require the `StoreStub`
4. Run `cargo nextest run --all-features --workspace <test_name>` for each new test to verify
5. Run `just coverage` and verify `handlers.rs` line coverage improves to ≥ 95 %
6. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
