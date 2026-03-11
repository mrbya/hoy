---
id: TASK-9
title: Add client session IO unit tests
status: Done
assignee:
  - '@codex'
created_date: '2026-03-11 16:51'
updated_date: '2026-03-11 17:29'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests for the client session reader/writer tasks in hoy-net to mirror server connection coverage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Reader task emits PacketReceived when a valid frame arrives from the server.
- [x] #2 Reader task emits ConnectionClosed when the TCP stream is closed.
- [x] #3 Writer task encodes and writes packets that the server side can decode.
- [x] #4 Tests use local loopback or in-memory sockets and are deterministic (no sleeps).
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review client session code to identify test seams (reader/writer tasks, internal events, packet encoding/decoding).
2. Create an in-memory IO harness (tokio duplex or loopback) similar to server connection tests for deterministic IO.
3. Add reader task tests for PacketReceived on valid frame and ConnectionClosed on EOF.
4. Add writer task test that sends a ClientPacket and decodes it on the server side.
5. Run just check and adjust tests to satisfy clippy and determinism requirements.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Added client session IO tests in `crates/net/src/client/session.rs` using loopback sockets.
- Verified reader emits PacketReceived and ConnectionClosed; writer encodes and writes frames.
- Kept tests deterministic with async_ok timeouts and no sleeps.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added unit tests for client session IO in `crates/net/src/client/session.rs` using loopback sockets.

Tests:
- Reader task emits PacketReceived for valid frames and ConnectionClosed on EOF.
- Writer task encodes ClientPacket and server-side decode verifies payload.

Checks:
- just check

- Switched client session IO tests to use in-memory `tokio::io::duplex` and generic reader/writer helpers to avoid network binding constraints.
<!-- SECTION:FINAL_SUMMARY:END -->
