---
id: TASK-10
title: Add client core state transition unit tests
status: Done
assignee:
  - '@codex'
created_date: '2026-03-11 16:51'
updated_date: '2026-03-11 17:30'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests for client core command/internal event handling and state transitions in hoy-net.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tests cover connect flow: emits Connecting, sends Hello, transitions to AwaitingWelcome.
- [x] #2 Tests cover Welcome handling: transitions to Connected and emits Connected with username/room.
- [x] #3 Tests cover error/closed internal events: emits Error then Disconnected and resets state.
- [x] #4 Tests cover SendMessage/Ping while disconnected emitting client-visible errors.
- [x] #5 No clippy warnings are introduced; avoid allow-lints by fixing patterns.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect client core implementation and existing public APIs for test seams (spawn_client, handle_command/internal_event, ClientState transitions).
2. Build a test harness that runs client_loop via spawn_client and uses a local TCP listener to observe Hello/packets without sleeps.
3. Add tests for connect flow: Connecting event, Hello packet received by server side, and AwaitingWelcome state via internal event observation.
4. Add tests for Welcome handling: inject InternalEvent::PacketReceived(Welcome) and assert Connected event and state.
5. Add tests for ConnectionError/ConnectionClosed internal events and for SendMessage/Ping while disconnected.
6. Run just check; adjust to keep clippy clean.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Added client core tests in `crates/net/src/client/core.rs` for connect flow, welcome handling, error/closed events, and disconnected send/ping.
- Used loopback listener and async_ok timeouts; no sleeps.
- just check passes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added unit tests for client core state transitions in `crates/net/src/client/core.rs`.

Coverage:
- Connect flow emits Connecting, sends Hello over TCP, and leaves state AwaitingWelcome.
- Welcome packet transitions to Connected and emits Connected with username/room.
- ConnectionError and ConnectionClosed emit expected client events and reset state.
- Disconnected SendMessage/Ping emit client-visible errors.

Checks:
- just check

- Connect flow unit test now uses an in-memory session spawner (`handle_command_with_spawner`) and duplex stream to avoid TCP binds.
<!-- SECTION:FINAL_SUMMARY:END -->
