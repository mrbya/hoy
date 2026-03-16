---
id: TASK-25
title: Cover client core state machine error paths
status: Done
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
  - coverage
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`crates/net/src/client/core.rs` sits at 73 % line coverage (136 missed lines) — the largest
coverage gap in the project. The file exposes `handle_command_with_spawner` and
`handle_internal_event` as private async functions testable via a custom session-spawner
stub, following the same pattern used by existing tests in the `#[cfg(test)]` module.

Key uncovered paths:

**`handle_command_with_spawner`**:
- `Connect` while state is already `AwaitingWelcome` or `Connected` (first guard: `!is_disconnected()`)
- `Connect` where the session spawner returns an error → emits `Error` event, loop continues
- `Connect` where `session.send(Hello)` fails (channel closed immediately) → emits `Error` event
- `emit_event` returns `false` (event channel closed) during `Connect` → loop terminates
- `Disconnect` while already `Disconnected` → no `Disconnected` event emitted, loop continues
- `Shutdown` while `Connected` → emits `Disconnected`, returns `false`
- `SendMessage` / `Ping` / `JoinRoom` / `ListRooms` while not connected → emits `Error`

**`handle_internal_event`**:
- `ConnectionClosed` while already `Disconnected` → returns `true` (no-op)
- `ConnectionError` while already `Disconnected` → returns `true` (no-op)
- `ConnectionError` while `Connected` → shuts down state, emits `Error`, emits `Disconnected`

**`handle_server_packet`**:
- `Welcome` received while in `Disconnected` or `Connected` state (not `AwaitingWelcome`) → emits `Error`
- Packets received while `Disconnected` (e.g. `ChatMessage`, `SystemMessage`, `Error`, `Pong`, `RoomJoined`, `RoomList`)

**`emit_event` returning `false`** (event_tx closed):
- Any call path where the event channel is closed mid-loop causes the loop to terminate
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `connect_while_awaiting_welcome_emits_error` — `Connect` while state is `AwaitingWelcome` emits `Error` event
- [ ] #2 `connect_session_spawn_error_emits_error` — session spawner returns `Err`; `Connect` command emits `Error` event, loop continues
- [ ] #3 `disconnect_while_disconnected_is_no_op` — `Disconnect` in `Disconnected` state emits no event and returns `true`
- [ ] #4 `shutdown_while_connected_emits_disconnected` — `Shutdown` while `Connected` emits `Disconnected` and returns `false`
- [ ] #5 `send_message_while_not_connected_emits_error` — `SendMessage` in `Disconnected` state emits `Error`
- [ ] #6 `connection_closed_while_disconnected_is_no_op` — `InternalEvent::ConnectionClosed` in `Disconnected` state returns `true` without emitting anything
- [ ] #7 `connection_error_while_disconnected_is_no_op` — `InternalEvent::ConnectionError` in `Disconnected` state returns `true` without emitting anything
- [ ] #8 `connection_error_while_connected_emits_error_then_disconnected` — `ConnectionError` while `Connected` emits `Error` then `Disconnected`
- [ ] #9 `welcome_in_wrong_state_emits_error` — `ServerPacket::Welcome` received while in `Disconnected` state emits `Error`
- [ ] #10 `packets_while_disconnected_are_ignored` — `ChatMessage`, `SystemMessage`, `Error`, `Pong`, `RoomJoined`, `RoomList` while `Disconnected` each return `true` without emitting events
- [ ] #11 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the existing `#[cfg(test)]` module in `core.rs` to understand the current test helpers and how `handle_command_with_spawner` / `handle_internal_event` are called directly
2. Create a `failing_spawner` that returns `Err(NetError::…)` for use in #2
3. Create a `no_op_spawner` that returns an in-memory `SessionHandle` (using duplex) for tests that need a spawner but don't exercise the network
4. Write tests #1–#10 inside the existing test module
   - Tests calling `handle_command_with_spawner` directly can avoid the full event loop
   - Tests that need the closed-event-channel path can drop `event_rx` before the call
5. Run `cargo nextest run --all-features --workspace core` to verify each new test
6. Run `just coverage` and verify `core.rs` line coverage improves to ≥ 90 %
7. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
