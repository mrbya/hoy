---
id: TASK-26
title: Cover test_client module paths
status: Done
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
  - coverage
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`crates/net/src/client/test_client.rs` sits at 60 % line coverage (82 missed lines).
The two existing tests cover `/quit`, `/exit`, `/ping`, `/disconnect`, plain messages,
and most of `format_client_event`. The following paths are uncovered:

- `handle_input_line` with an **empty line** → returns `Ok(FrontendAction::Continue)` without calling any handle method
- `handle_input_line` with `/list` → calls `handle.list_rooms()`
- `handle_input_line` with `/room valid-room` → calls `handle.join_room(…)`
- `handle_input_line` with `/room Invalid Room!` (invalid name) → prints error event, returns `Continue`
- `format_client_event` for `ClientEvent::RoomJoined { messages: [one_msg] }` (non-empty messages path)
- `format_client_event` for `ClientEvent::RoomList { rooms }`

The `run_test_client` function and `spawn_input_thread` read from real stdin/socket and
cannot be easily unit-tested; they should be left as integration-tested rather than
force-testing them here.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `handle_input_line_empty_is_continue` — `handle_input_line(&handle, "".into())` returns `Ok(FrontendAction::Continue)` and emits no events
- [ ] #2 `handle_input_line_list_sends_list_rooms` — `/list` returns `Continue`; because the client is not connected, the core emits an `Error` event (same pattern as the `/ping` test)
- [ ] #3 `handle_input_line_room_valid_sends_join_room` — `/room general` returns `Continue`; core emits `Error` (client not connected)
- [ ] #4 `handle_input_line_room_invalid_name_prints_error` — `/room Bad Name!` returns `Ok(Continue)` and emits no command to the core (the error is printed locally, not forwarded)
- [ ] #5 `format_client_event_room_joined_with_messages` — `format_client_event(&ClientEvent::RoomJoined { room: "general".into(), messages: vec![MessageRecord { from: "alice".into(), text: "hi".into() }] })` contains both the join line and the message line
- [ ] #6 `format_client_event_room_list` — `format_client_event(&ClientEvent::RoomList { rooms: vec!["general".into()] })` returns the expected rooms string
- [ ] #7 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the existing test module in `test_client.rs` — note `spawn_handle`, `recv_event_timeout`, and the existing tests
2. Add tests #1–#6 to the same `#[cfg(test)]` module
3. For #1: call `handle_input_line` with `""` and assert `Continue`; use `recv_event_timeout` to confirm no event was emitted
4. For #2–#3: reuse the `spawn_handle` helper; the client is `Disconnected` so list/room commands produce `Error` events that can be asserted
5. For #4: call `handle_input_line` with `"/room Bad Name!"` and assert `Continue`; use `recv_event_timeout` to confirm no core event was emitted (the error is a local print)
6. For #5–#6: call `format_client_event` directly (it's pure) and assert the output string
7. Run `cargo nextest run --all-features --workspace test_client` to verify
8. Run `just coverage` and verify `test_client.rs` line coverage improves to ≥ 80 %
9. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
