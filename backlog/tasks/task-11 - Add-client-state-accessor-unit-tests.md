---
id: TASK-11
title: Add client state accessor unit tests
status: Done
assignee:
  - '@codex'
created_date: '2026-03-11 16:51'
updated_date: '2026-03-11 17:16'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests for ClientState accessors and helpers in hoy-net.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tests validate is_disconnected/is_awaiting_welcome/is_connected across all variants.
- [x] #2 Tests validate server_addr/username/room accessors for each variant.
- [x] #3 Tests validate packet_tx/session_mut/take_session behavior with a real session handle.
- [x] #4 Tests are deterministic and do not rely on timing sleeps.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect `crates/net/src/client/state.rs` to list accessors and expected behavior per variant.
2. Build a minimal SessionHandle for tests using in-memory channels and no real IO.
3. Add tests for each variant covering is_* flags and server_addr/username/room outputs.
4. Add tests for packet_tx/session_mut/take_session ensuring Some/None behavior and ownership transfer.
5. Run just check and adjust for clippy compliance and determinism.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Added ClientState accessor tests in `crates/net/src/client/state.rs`, including packet_tx send/recv using a real SessionHandle.
- Removed unused test-module allow-lints and kept clippy clean.
- just check passes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added ClientState accessor tests in `crates/net/src/client/state.rs`.

Coverage:
- is_* flags across Disconnected/AwaitingWelcome/Connected.
- server_addr/username/room accessors for each variant.
- packet_tx/session_mut/take_session behavior with real SessionHandle and channel send/recv.

Checks:
- just check
<!-- SECTION:FINAL_SUMMARY:END -->
